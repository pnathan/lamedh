; symtab.asm — symbol interning: same name always yields the same heap
; object, so EQ on symbols is pointer equality and callers may cache a
; resolved symbol address across calls instead of re-hashing a string.
;
; Symbol object layout (heapobj, tag 10), all fields 8 bytes:
;   [0]  header       = HDR_SYMBOL
;   [8]  name_len
;   [16] value        (tagged LispVal; IMM_UNBOUND until DEFINEd)
;   [24] macro         (tagged closure if this name is a macro; IMM_NIL
;                        otherwise — checked at compile time only, never
;                        emitted as a runtime load, since macro expansion
;                        is a host-time computation, not a target one)
;   [32] next         (raw ptr to next symbol in this bucket, or 0)
;   [40] plist        (tagged LispVal; an alist of (indicator . value)
;                        pairs built with ordinary CONS, IMM_NIL until
;                        PUTP first extends it — KERNEL.md Part XI's
;                        "symbol property lists (GETP/PUTP)". A mutable
;                        slot on the symbol itself, the same way [16]
;                        already is for DEFINE/SETQ — not a new kind of
;                        mutation this kernel didn't already have.)
;   [48] name bytes, padded to a multiple of 8
;
; A fixed 512-bucket direct-chained hash table (FNV-1a), sized generously
; for the fixed test/bench corpus this v0 targets — see README roadmap
; for growth-on-load-factor.

%include "src/tags.inc"

extern data_alloc

section .bss
align 8
global symtab_buckets
symtab_buckets: resq 512
gensym_counter: resq 1

section .text

; bytes_equal(rdi=ptr1, rsi=ptr2, rdx=len) -> rax = 1 if equal else 0
global bytes_equal
bytes_equal:
    xor rax, rax
.loop:
    test rdx, rdx
    jz .eq
    mov cl, [rdi]
    cmp cl, [rsi]
    jne .ne
    inc rdi
    inc rsi
    dec rdx
    jmp .loop
.eq:
    mov rax, 1
    ret
.ne:
    xor rax, rax
    ret

; fnv1a_hash(rdi=ptr, rsi=len) -> rax = 64-bit hash. Exported so
; arrays.asm's hash_code_tagged can hash a string's bytes with it too
; (HASH-CODE must agree with EQ — lisp_eq, strings.asm, is now content
; equality for strings, so two equal-content strings must hash equal).
global fnv1a_hash
fnv1a_hash:
    mov rax, 0xcbf29ce484222325     ; FNV offset basis
    mov r8, 0x100000001b3            ; FNV prime
.loop:
    test rsi, rsi
    jz .done
    movzx rcx, byte [rdi]
    xor rax, rcx
    imul rax, r8
    inc rdi
    dec rsi
    jmp .loop
.done:
    ret

; intern_symbol(rdi=name ptr, rsi=len) -> rax = tagged symbol pointer
global intern_symbol
intern_symbol:
    push rbx
    push r12
    push r13
    push r14
    mov r12, rdi                 ; name ptr
    mov r13, rsi                 ; len

    mov rdi, r12
    mov rsi, r13
    call fnv1a_hash
    and rax, 511
    lea r14, [symtab_buckets + rax*8]   ; r14 = &bucket slot

    mov rbx, [r14]                ; rbx = bucket head (raw ptr or 0)
.scan:
    test rbx, rbx
    jz .not_found
    cmp qword [rbx+8], r13         ; compare name_len
    jne .next
    lea rdi, [rbx+48]
    mov rsi, r12
    mov rdx, r13
    call bytes_equal
    test rax, rax
    jz .next
    ; found: return tagged pointer
    mov rax, rbx
    or rax, TAG_HEAPOBJ
    jmp .out
.next:
    mov rbx, [rbx+32]
    jmp .scan

.not_found:
    ; allocate: 48 header bytes + name, padded to 8
    mov rdi, r13
    add rdi, 7
    and rdi, ~7
    add rdi, 48
    call data_alloc                ; rax = raw new symbol address
    mov rbx, rax

    mov qword [rbx], HDR_SYMBOL
    mov [rbx+8], r13
    mov qword [rbx+16], IMM_UNBOUND
    mov qword [rbx+24], IMM_NIL     ; not a macro until DEFMACRO says otherwise
    mov rdi, [r14]
    mov [rbx+32], rdi              ; next = old bucket head
    mov [r14], rbx                 ; bucket head = new symbol
    mov qword [rbx+40], IMM_NIL     ; empty plist until PUTP first extends it

    ; copy name bytes
    lea rdi, [rbx+48]
    mov rsi, r12
    mov rcx, r13
    rep movsb

    mov rax, rbx
    or rax, TAG_HEAPOBJ

.out:
    pop r14
    pop r13
    pop r12
    pop rbx
    ret

; bootstrap_globals() — binds the symbol T to itself in the global
; environment (KERNEL.md Part IV: "T is an ordinary interned symbol,
; bound to itself in the global environment"). Must run once, before
; any compiled code can reference T as a variable: a symbol's value
; cell is IMM_UNBOUND until something writes it, and nothing else in
; this kernel ever writes T's — evaluating the bare symbol `T` (as
; opposed to the *separate* IMM_TRUE immediate value EQ/comparisons
; return, which every prior test exercised instead) fell through to
; print_fixnum on whatever garbage its unbound cell held, since nothing
; had ever referenced it as a variable before lib/prelude.lisp's own
; NOT/WHEN/UNLESS used it as one directly. `NIL` needs no such
; bootstrapping: it is a distinct immediate value, not a symbol at all,
; per Part IV, and every other symbol is correctly unbound until
; DEFINEd, matching the spec's own default.
global bootstrap_globals
bootstrap_globals:
    mov rdi, t_name
    mov rsi, 1
    call intern_symbol
    mov rbx, rax
    UNTAG_PTR rbx
    mov [rbx+16], rax
    ret

; gensym() -> rax = tagged symbol, a fresh HDR_SYMBOL object with the
; exact same layout intern_symbol builds, except it is never linked
; into symtab_buckets — so no name, however constructed, can ever
; INTERN or otherwise look it up again, matching KERNEL.md Part XI's
; "fresh uninterned symbol, never EQ to anything else" requirement:
; EQ on symbols is pointer identity (lisp_eq, strings.asm, falls
; through to false for any non-string/float heapobj pair, symbols
; included), so an object no lookup path can ever return again is
; automatically never EQ to anything but itself. Name format matches
; the Rust reference exactly (environment.rs: `format!("G{:04}",
; counter)`) — "G" followed by the counter, zero-padded to at least 4
; digits, growing wider past 9999 rather than wrapping or truncating.
global gensym
gensym:
    push rbx
    push r12
    push r13
    push r14
    sub rsp, 32

    mov rax, [gensym_counter]
    mov r12, rax
    inc rax
    mov [gensym_counter], rax

    ; render r12 as decimal into [rsp..rsp+31], back to front, then
    ; left-pad with '0' to a minimum width of 4 digits.
    mov rax, r12
    mov r9, 10
    lea r13, [rsp+31]
    mov byte [r13], 0
    xor r14, r14
.divloop:
    xor rdx, rdx
    div r9
    add dl, '0'
    dec r13
    mov [r13], dl
    inc r14
    test rax, rax
    jnz .divloop
.padloop:
    cmp r14, 4
    jae .paddone
    dec r13
    mov byte [r13], '0'
    inc r14
    jmp .padloop
.paddone:
    ; r13 = start of the digit run, r14 = its length; total name is
    ; "G" (1 byte) followed by those r14 digit bytes.
    lea rbx, [r14+1]              ; name_len = 1 + digit count
    mov rdi, rbx
    add rdi, 7
    and rdi, ~7
    add rdi, 48
    call data_alloc                ; rax = raw new symbol address;
                                    ; data_alloc preserves r13/r14 (the
                                    ; digit run), the original counter
                                    ; value in r12 is already dead here
    mov r12, rax

    mov qword [r12], HDR_SYMBOL
    mov [r12+8], rbx
    mov qword [r12+16], IMM_UNBOUND
    mov qword [r12+24], IMM_NIL
    mov qword [r12+32], 0           ; not linked into any bucket chain
    mov qword [r12+40], IMM_NIL     ; empty plist

    mov byte [r12+48], 'G'
    lea rdi, [r12+49]
    mov rsi, r13
    mov rcx, r14
    rep movsb

    mov rax, r12
    or rax, TAG_HEAPOBJ

    add rsp, 32
    pop r14
    pop r13
    pop r12
    pop rbx
    ret

; symbolp_tagged(rdi=tagged value) -> rax = IMM_TRUE/IMM_NIL. The
; Lisp-visible SYMBOLP predicate (compiler.asm) — a genuine Rust-level
; builtin in the reference (environment.rs), missing here until now;
; lib/06-require.lisp's own `$require-canonical-name`
; (`(cond ((symbolp x) x) ...)`) is what surfaced the gap. NIL is a
; distinct immediate, not a symbol (this reader's own "NIL" special
; case, matching the reference), so SYMBOLP on NIL is correctly NIL,
; same as any other non-symbol value.
global symbolp_tagged
symbolp_tagged:
    mov rax, rdi
    and rax, TAG_MASK
    cmp rax, TAG_HEAPOBJ
    jne .no
    mov rax, rdi
    UNTAG_PTR rax
    cmp qword [rax], HDR_SYMBOL
    jne .no
    mov rax, IMM_TRUE
    ret
.no:
    mov rax, IMM_NIL
    ret

; symbol_plist(rdi=tagged symbol) -> rax = its plist slot's current
; value (IMM_NIL until some PUTP has run). The SYMBOL-PLIST kernel
; primitive's host half (compiler.asm) — GETP/PUTP themselves are
; ordinary prelude library code over this plus SET-SYMBOL-PLIST!/CONS/
; CAR/CDR/EQ, per KERNEL.md Part XII axis 3's usual license, matching
; every other derived form in lib/prelude.lisp.
global symbol_plist
symbol_plist:
    mov rax, rdi
    UNTAG_PTR rax
    mov rax, [rax+40]
    ret

; set_symbol_plist(rdi=tagged symbol, rsi=new plist value) -> rax = rsi.
; The SET-SYMBOL-PLIST! kernel primitive's host half — an ordinary
; mutable-slot write, the same way DEFINE/SETQ already write a
; symbol's separate value slot ([16]); PUTP (lib/prelude.lisp) is the
; only caller, always storing a freshly CONSed (indicator . value)
; pair onto the front of the existing plist, never mutating a cons
; cell itself (this kernel's cons cells stay immutable — Part XII,
; axis 2).
global set_symbol_plist
set_symbol_plist:
    mov rax, rdi
    UNTAG_PTR rax
    mov [rax+40], rsi
    mov rax, rsi
    ret

; boundp_tagged(rdi=tagged symbol) -> rax = IMM_TRUE/IMM_NIL. Reads the
; symbol's own value slot ([16], the same one DEFINE/SETQ/an ordinary
; global variable reference already use) and compares it against
; IMM_UNBOUND, its initial value from intern_symbol until something
; DEFINEs it — needed by lib/00-core.lisp's own DEFUN macro
; (`(if (boundp '$cg-pending) ...)`), which must not treat an
; as-yet-undefined optional bookkeeping global as an error.
global boundp_tagged
boundp_tagged:
    mov rax, rdi
    UNTAG_PTR rax
    mov rax, [rax+16]
    cmp rax, IMM_UNBOUND
    je .no
    mov rax, IMM_TRUE
    ret
.no:
    mov rax, IMM_NIL
    ret

section .rodata
t_name: db "T"
