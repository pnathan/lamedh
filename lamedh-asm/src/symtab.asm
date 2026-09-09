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
;   [40] name bytes, padded to a multiple of 8
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

section .text

; bytes_equal(rdi=ptr1, rsi=ptr2, rdx=len) -> rax = 1 if equal else 0
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

; fnv1a_hash(rdi=ptr, rsi=len) -> rax = 64-bit hash
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
    lea rdi, [rbx+40]
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
    ; allocate: 40 header bytes + name, padded to 8
    mov rdi, r13
    add rdi, 7
    and rdi, ~7
    add rdi, 40
    call data_alloc                ; rax = raw new symbol address
    mov rbx, rax

    mov qword [rbx], HDR_SYMBOL
    mov [rbx+8], r13
    mov qword [rbx+16], IMM_UNBOUND
    mov qword [rbx+24], IMM_NIL     ; not a macro until DEFMACRO says otherwise
    mov rdi, [r14]
    mov [rbx+32], rdi              ; next = old bucket head
    mov [r14], rbx                 ; bucket head = new symbol

    ; copy name bytes
    lea rdi, [rbx+40]
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

section .rodata
t_name: db "T"
