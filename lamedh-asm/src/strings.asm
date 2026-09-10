; strings.asm — the string value type: a length-prefixed byte buffer
; heapobj (HDR_STRING: [0]=header [8]=len [16..]=raw bytes). Immutable
; and self-evaluating exactly like a fixnum literal — the data heap
; never relocates, so a string literal's absolute address bakes as a
; code immediate the same way any other QUOTE'd/literal datum does (see
; compiler.asm's compile_form ".literal" path; no compiler change was
; needed to make string literals self-evaluating).
;
; No mutation, no unicode, no interning — a string is just bytes.

%include "src/tags.inc"

extern data_alloc
extern write_buf
extern float_eq_exact

section .text

; lisp_eq(rdi=tagged a, rsi=tagged b) -> rax = IMM_TRUE/IMM_NIL. The
; EQ builtin's actual implementation (compiler.asm's compile_eq calls
; here instead of emitting a bare tagged-value compare). KERNEL.md
; Part IV: "value equality" for fixnum/float/char/string means two
; freshly computed, unshared values holding the same value must be EQ
; — a raw pointer/tagged-value compare gets this right for free for
; fixnums, characters, symbols, and every immediate (their tagged
; representation already *is* their value), but is wrong for two
; separately heap-allocated strings or floats with identical content,
; which a plain compare would call unequal. EQ on two distinct cons
; cells stays pointer identity here, which is undefined behavior a
; portable program must not rely on either way (KERNEL.md Part IV,
; issue #454) — this kernel picks the pointer-identity answer, one of
; the two the spec allows.
global lisp_eq
lisp_eq:
    cmp rdi, rsi
    je .true_fast

    mov rax, rdi
    and rax, TAG_MASK
    cmp rax, TAG_HEAPOBJ
    jne .false_fast
    mov rax, rsi
    and rax, TAG_MASK
    cmp rax, TAG_HEAPOBJ
    jne .false_fast

    push rbx
    push r12
    mov rbx, rdi
    UNTAG_PTR rbx
    mov r12, rsi
    UNTAG_PTR r12
    mov rax, [rbx]
    cmp rax, [r12]
    jne .false_slow

    cmp rax, HDR_STRING
    je .cmp_string
    cmp rax, HDR_FLOAT
    je .cmp_float
    jmp .false_slow                     ; different heapobj kind, or a
                                         ; kind with no value-equality
                                         ; rule (symbol, closure, array,
                                         ; ... — identity only, and the
                                         ; fast path above already
                                         ; covers "same object")

.cmp_string:
    mov rax, [rbx+8]
    cmp rax, [r12+8]
    jne .false_slow
    mov rcx, rax
    lea rdi, [rbx+16]
    lea rsi, [r12+16]
    xor rdx, rdx
.strcmp_loop:
    cmp rdx, rcx
    jae .true_slow
    mov al, [rdi+rdx]
    cmp al, [rsi+rdx]
    jne .false_slow
    inc rdx
    jmp .strcmp_loop

.cmp_float:
    mov rdi, rbx
    or rdi, TAG_HEAPOBJ
    mov rsi, r12
    or rsi, TAG_HEAPOBJ
    call float_eq_exact
    test rax, rax
    jz .false_slow
    ; fall through: true

.true_slow:
    pop r12
    pop rbx
.true_fast:
    mov rax, IMM_TRUE
    ret

.false_slow:
    pop r12
    pop rbx
.false_fast:
    mov rax, IMM_NIL
    ret

; make_string(rdi=byte buf, rsi=len) -> rax = tagged HDR_STRING heapobj,
; copying (rsi) bytes out of (rdi) into the new object. Always writes
; one extra NUL byte right after the string's own data (not counted in
; its length) so a string's byte pointer is also safe to hand directly
; to a raw syscall expecting a C string (see fileio.asm's file_open) —
; without this, an "open"-style path string would need a copy into a
; scratch NUL-terminated buffer at every call site.
global make_string
make_string:
    push rbx
    push r12
    push r13
    mov rbx, rdi                  ; src buf
    mov r12, rsi                    ; len
    lea rdi, [r12+17]                 ; header + len fields + bytes + NUL
    call data_alloc
    mov r13, rax
    mov qword [r13], HDR_STRING
    mov [r13+8], r12
    xor rcx, rcx
.copy:
    cmp rcx, r12
    jae .done
    mov dl, [rbx+rcx]
    mov [r13+16+rcx], dl
    inc rcx
    jmp .copy
.done:
    mov byte [r13+16+r12], 0
    mov rax, r13
    or rax, TAG_HEAPOBJ
    pop r13
    pop r12
    pop rbx
    ret

; is_string(rdi=tagged value) -> rax=1/0
global is_string
is_string:
    mov rax, rdi
    and rax, TAG_MASK
    cmp rax, TAG_HEAPOBJ
    jne .no
    mov rax, rdi
    UNTAG_PTR rax
    cmp qword [rax], HDR_STRING
    jne .no
    mov rax, 1
    ret
.no:
    xor rax, rax
    ret

; stringp_tagged(rdi=tagged value) -> rax = IMM_TRUE/IMM_NIL. The
; Lisp-visible STRINGP predicate (compiler.asm) over is_string's own
; raw 0/1 — needed by lib/00-core.lisp's own DEFUN macro, which checks
; `(stringp (car body))` on every macro-expansion to peel off an
; optional leading docstring.
global stringp_tagged
stringp_tagged:
    call is_string
    test rax, rax
    jz .no
    mov rax, IMM_TRUE
    ret
.no:
    mov rax, IMM_NIL
    ret

; string_len(rdi=tagged string) -> rax = raw (untagged) byte length
global string_len
string_len:
    mov rax, rdi
    UNTAG_PTR rax
    mov rax, [rax+8]
    ret

; string_length_tagged(rdi=tagged string) -> rax = tagged fixnum length.
; The STRING-LENGTH builtin's host half (see compiler.asm).
global string_length_tagged
string_length_tagged:
    mov rax, rdi
    UNTAG_PTR rax
    mov rax, [rax+8]
    TO_FIXNUM rax
    ret

; string_bytes(rdi=tagged string) -> rax = address of the first byte
global string_bytes
string_bytes:
    mov rax, rdi
    UNTAG_PTR rax
    add rax, 16
    ret

; string_ref_tagged(rdi=tagged string, rsi=tagged fixnum index) -> rax
; = tagged fixnum, the byte value (0..255) at that index. No bounds
; check (v0 — same scope as ARRAY's FETCH/STORE) and no Char type to
; return instead (v0 — see README): a byte's numeric value is the
; closest honest answer this kernel can give until one exists.
global string_ref_tagged
string_ref_tagged:
    push rbx
    mov rbx, rdi
    call string_bytes
    mov rcx, rsi
    UNTAG_FIXNUM rcx
    movzx rax, byte [rax+rcx]
    TO_FIXNUM rax
    pop rbx
    ret

; string_append(rdi=tagged string, rsi=tagged string) -> rax = a fresh
; tagged HDR_STRING heapobj, the byte-for-byte concatenation of both —
; neither argument is modified (strings are immutable, like every
; other value in this kernel; see the header comment above).
global string_append
string_append:
    push rbx
    push r12
    push r13
    push r14
    push r15
    mov rbx, rdi                      ; string A (tagged)
    mov r12, rsi                        ; string B (tagged)

    call string_len                       ; rax = len(A)
    mov r13, rax
    mov rdi, r12
    call string_len                          ; rax = len(B)
    mov r14, rax

    mov rdi, r13
    add rdi, r14
    add rdi, 17                                ; header+len+bytes+NUL
    call data_alloc
    mov r15, rax                                 ; the new object (untagged)
    mov qword [r15], HDR_STRING
    lea rax, [r13+r14]
    mov [r15+8], rax                              ; total length

    mov rdi, rbx
    call string_bytes                               ; rax = A's bytes
    mov rsi, rax
    lea rdi, [r15+16]
    xor rcx, rcx
.copy_a:
    cmp rcx, r13
    jae .copy_a_done
    mov dl, [rsi+rcx]
    mov [rdi+rcx], dl
    inc rcx
    jmp .copy_a
.copy_a_done:

    mov rdi, r12
    call string_bytes                               ; rax = B's bytes
    mov rsi, rax
    lea rdi, [r15+16+r13]
    xor rcx, rcx
.copy_b:
    cmp rcx, r14
    jae .copy_b_done
    mov dl, [rsi+rcx]
    mov [rdi+rcx], dl
    inc rcx
    jmp .copy_b
.copy_b_done:

    lea rax, [r13+r14]
    mov byte [r15+16+rax], 0                          ; trailing NUL
    mov rax, r15
    or rax, TAG_HEAPOBJ
    pop r15
    pop r14
    pop r13
    pop r12
    pop rbx
    ret

; substring(rdi=tagged string, rsi=tagged fixnum start, rdx=tagged
; fixnum end) -> rax = a fresh tagged HDR_STRING heapobj holding bytes
; [start,end) of the source. No bounds check (v0 — same scope as
; ARRAY's FETCH/STORE): start>end or an out-of-range index copies
; adjacent heap memory rather than raising anything.
global substring
substring:
    push rbx
    push r12
    push r13
    mov rbx, rdi                       ; source string
    mov r12, rsi
    UNTAG_FIXNUM r12                     ; raw start
    mov r13, rdx
    UNTAG_FIXNUM r13                     ; raw end
    call string_bytes                      ; rax = source bytes
    add rax, r12                             ; rax = &bytes[start]
    mov rdi, rax
    mov rsi, r13
    sub rsi, r12                               ; len = end-start
    call make_string
    pop r13
    pop r12
    pop rbx
    ret

; print_string(rdi=tagged string) -> writes its raw bytes to stdout, no
; trailing newline (matches print_fixnum's own convention) — unless
; print_readably is set (prin1_to_string, print.asm), in which case the
; bytes are double-quoted with the reader's own escapes for `"`, `\`,
; newline and tab, so the result reads back as the same string: the
; reference's PRIN1 / PRIN1-TO-STRING contract (printer.rs).
section .bss
global print_readably
print_readably: resq 1
section .rodata
dquote_buf: db '"'
esc_dquote: db '\"'
esc_backslash: db '\\'
esc_newline: db '\n'
esc_tab: db '\t'
section .text
global print_string
print_string:
    push rbx
    cmp qword [print_readably], 0
    jne .readably
    mov rbx, rdi
    call string_len
    mov rdx, rax
    mov rdi, rbx
    call string_bytes
    mov rsi, rax
    call write_buf
    pop rbx
    ret
.readably:
    push r12
    push r13
    mov rbx, rdi
    call string_len
    mov r12, rax                        ; len
    mov rdi, rbx
    call string_bytes
    mov r13, rax                        ; bytes
    mov rsi, dquote_buf
    mov rdx, 1
    call write_buf
    xor ebx, ebx                        ; i
.rd_loop:
    cmp rbx, r12
    jae .rd_done
    movzx eax, byte [r13+rbx]
    cmp al, '"'
    je .rd_dq
    cmp al, '\'
    je .rd_bs
    cmp al, 10
    je .rd_nl
    cmp al, 9
    je .rd_tab
    lea rsi, [r13+rbx]
    mov rdx, 1
    call write_buf
    jmp .rd_next
.rd_dq:
    mov rsi, esc_dquote
    jmp .rd_esc
.rd_bs:
    mov rsi, esc_backslash
    jmp .rd_esc
.rd_nl:
    mov rsi, esc_newline
    jmp .rd_esc
.rd_tab:
    mov rsi, esc_tab
.rd_esc:
    mov rdx, 2
    call write_buf
.rd_next:
    inc rbx
    jmp .rd_loop
.rd_done:
    mov rsi, dquote_buf
    mov rdx, 1
    call write_buf
    pop r13
    pop r12
    pop rbx
    ret

; --- T/NIL type predicates for the compiler's FIXP/FLOATP/ARRAYP/CHARP
; keywords (compile_unary_hostcall), each a thin wrapper around the
; internal 1/0 check print_value already dispatches on. The reference
; has all four as builtins (environment.rs) and lib/32-base64.lisp's
; ENCODE, among others, calls FIXP at runtime.
global fixp_tagged
fixp_tagged:
    mov rax, rdi
    and rax, TAG_MASK
    cmp rax, TAG_FIXNUM
    je .yes
    mov rax, IMM_NIL
    ret
.yes:
    mov rax, IMM_TRUE
    ret
global floatp_tagged
floatp_tagged:
    call is_float
    jmp bool_to_lisp
global arrayp_tagged
arrayp_tagged:
    call is_array
    jmp bool_to_lisp
global charp_tagged
charp_tagged:
    call is_char_tagged
    jmp bool_to_lisp
bool_to_lisp:                           ; rax = 1/0 -> T/NIL
    test rax, rax
    jz .no
    mov rax, IMM_TRUE
    ret
.no:
    mov rax, IMM_NIL
    ret

; print_symbol(rdi=tagged symbol) -> writes its interned name bytes
; verbatim (already uppercased at intern time; see symtab.asm's
; layout — name_len at [addr+8], name bytes starting at [addr+48]).
print_symbol:
    mov rax, rdi
    UNTAG_PTR rax
    mov rdx, [rax+8]
    lea rsi, [rax+48]
    jmp write_buf

; print_list(rdi=tagged cons) -> "(a b c)", or "(a . b)" for an
; improper list — PRIN1-style, recursing through print_value so every
; element prints by these same rules (KERNEL.md Part III). No cycle
; detection: cons cells are immutable in this kernel (nothing can
; RPLACD one into a cycle), so none is reachable from Lamedh code.
extern car
extern cdr
extern is_cons
print_list:
    push rbx
    mov rbx, rdi
    mov rsi, lparen_buf
    mov rdx, 1
    call write_buf
.loop:
    mov rdi, rbx
    call car
    mov rdi, rax
    call print_value
    mov rdi, rbx
    call cdr
    mov rbx, rax
    cmp rbx, IMM_NIL
    je .done
    mov rdi, rbx
    call is_cons
    test rax, rax
    jz .dotted
    mov rsi, space_buf
    mov rdx, 1
    call write_buf
    jmp .loop
.dotted:
    mov rsi, dot_buf
    mov rdx, 3
    call write_buf
    mov rdi, rbx
    call print_value
.done:
    mov rsi, rparen_buf
    mov rdx, 1
    call write_buf
    pop rbx
    ret

; print_value(rdi=tagged value) -> writes the PRIN1-style readable
; representation of any value this kernel has (KERNEL.md Part III):
; NIL as "()", T as "T", a symbol as its name, a cons as a recursively
; printed list, a string's raw bytes, a float's fixed-decimal form, or
; a fixnum's decimal value — dispatching on the runtime tag (the
; argument's type isn't known until then). This is what PRINT actually
; calls; compile_print itself is unchanged — only the host address it
; bakes moved from print_fixnum to this dispatcher.
extern print_fixnum
extern is_float
extern float_print
extern is_array
extern is_typed_array
extern array_length_tagged
extern is_char_tagged
extern print_char
global print_value
print_value:
    push rbx
    mov rbx, rdi

    cmp rbx, IMM_NIL
    jne .not_nil
    mov rsi, nil_buf
    mov rdx, 2
    call write_buf
    jmp .out
.not_nil:
    cmp rbx, IMM_TRUE
    jne .not_true
    mov rsi, true_buf
    mov rdx, 1
    call write_buf
    jmp .out
.not_true:
    mov rdi, rbx
    call is_char_tagged
    test rax, rax
    jz .not_char
    mov rdi, rbx
    call print_char
    jmp .out
.not_char:
    mov rdi, rbx
    call is_cons
    test rax, rax
    jz .not_cons
    mov rdi, rbx
    call print_list
    jmp .out
.not_cons:
    mov rax, rbx
    and rax, TAG_MASK
    cmp rax, TAG_HEAPOBJ
    jne .not_symbol
    mov rax, rbx
    UNTAG_PTR rax
    cmp qword [rax], HDR_SYMBOL
    jne .not_symbol
    mov rdi, rbx
    call print_symbol
    jmp .out
.not_symbol:
    mov rdi, rbx
    call is_string
    test rax, rax
    jz .not_string
    mov rdi, rbx
    call print_string
    jmp .out
.not_string:
    mov rdi, rbx
    call is_float
    test rax, rax
    jz .not_float
    mov rdi, rbx
    call float_print
    jmp .out
.not_float:
    mov rdi, rbx
    call is_array
    test rax, rax
    jz .not_array
    mov rsi, array_tag_open
    mov rdx, array_tag_open_len
    call write_buf
    mov rdi, rbx
    call array_length_tagged
    mov rdi, rax
    call print_fixnum
    mov rsi, array_tag_close
    mov rdx, 1
    call write_buf
    jmp .out
.not_array:
    mov rdi, rbx
    call is_typed_array
    test rax, rax
    jz .not_typed_array
    mov rsi, typed_array_tag_open
    mov rdx, typed_array_tag_open_len
    call write_buf
    mov rax, rbx
    UNTAG_PTR rax
    cmp qword [rax+16], 0                 ; elem_type: 0=INT64
    jne .typed_array_float_tag
    mov rsi, int64_tag
    mov rdx, int64_tag_len
    call write_buf
    jmp .typed_array_tag_done
.typed_array_float_tag:
    mov rsi, float64_tag
    mov rdx, float64_tag_len
    call write_buf
.typed_array_tag_done:
    mov rsi, colon_buf
    mov rdx, 1
    call write_buf
    mov rdi, rbx
    call array_length_tagged
    mov rdi, rax
    call print_fixnum
    mov rsi, array_tag_close
    mov rdx, 1
    call write_buf
    jmp .out
.not_typed_array:
    mov rax, rbx
    and rax, TAG_MASK
    cmp rax, TAG_HEAPOBJ
    jne .not_closure
    mov rax, rbx
    UNTAG_PTR rax
    cmp qword [rax], HDR_CLOSURE
    jne .not_closure
    mov rsi, lambda_tag
    mov rdx, lambda_tag_len
    call write_buf
    jmp .out
.not_closure:
    mov rax, rbx
    and rax, TAG_MASK
    cmp rax, TAG_HEAPOBJ
    jne .not_record
    mov rax, rbx
    UNTAG_PTR rax
    cmp qword [rax], HDR_OPERATIVE
    jne .not_operative
    mov rsi, lambda_tag
    mov rdx, lambda_tag_len
    call write_buf
    jmp .out
.not_operative:
    mov rax, rbx
    UNTAG_PTR rax
    cmp qword [rax], HDR_RECORD
    jne .not_record
    push r12
    push r13
    mov rsi, record_tag_open
    mov rdx, record_tag_open_len
    call write_buf
    mov rax, rbx
    UNTAG_PTR rax
    mov rdi, [rax+8]                    ; brand symbol
    call print_value
    mov rax, rbx
    UNTAG_PTR rax
    mov r12, [rax+16]                     ; nfields
    xor r13, r13
.record_fields_loop:
    cmp r13, r12
    jae .record_fields_done
    mov rsi, space_buf
    mov rdx, 1
    call write_buf
    mov rax, rbx
    UNTAG_PTR rax
    mov rdi, [rax+24+r13*8]
    call print_value
    inc r13
    jmp .record_fields_loop
.record_fields_done:
    mov rsi, rparen_buf
    mov rdx, 1
    call write_buf
    pop r13
    pop r12
    jmp .out
.not_record:
    mov rdi, rbx
    call print_fixnum
.out:
    pop rbx
    ret

section .rodata
nil_buf:    db "()"
true_buf:   db "T"
lparen_buf: db "("
rparen_buf: db ")"
space_buf:  db " "
dot_buf:    db " . "
array_tag_open:     db "<array:"
array_tag_open_len: equ $ - array_tag_open
array_tag_close:    db ">"
typed_array_tag_open:     db "<typed-array:"
typed_array_tag_open_len: equ $ - typed_array_tag_open
int64_tag:     db "int64"
int64_tag_len: equ $ - int64_tag
float64_tag:     db "float64"
float64_tag_len: equ $ - float64_tag
colon_buf: db ":"
lambda_tag:     db "<lambda>"
lambda_tag_len: equ $ - lambda_tag
record_tag_open:     db "#S("
record_tag_open_len: equ $ - record_tag_open
