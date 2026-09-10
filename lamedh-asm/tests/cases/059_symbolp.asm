; 059_symbolp — SYMBOLP: a genuine Rust-level builtin in the reference
; (environment.rs), missing here until lib/06-require.lisp's own
; `$require-canonical-name` (`(cond ((symbolp x) x) ...)`) surfaced the
; gap. NIL is a distinct immediate, not a symbol (this reader's own
; "NIL" special case, matching the reference's reader.rs), so SYMBOLP
; on NIL is correctly NIL, same as any other non-symbol value.
%include "src/tags.inc"

extern reader_init
extern read_form
extern compile_thunk
extern print_newline

section .rodata
e1: db "(PRINT (SYMBOLP (QUOTE FOO)))"
e1_len: equ $ - e1                            ; T

e2: db "(PRINT (SYMBOLP T))"
e2_len: equ $ - e2                              ; T

e3: db "(PRINT (SYMBOLP NIL))"
e3_len: equ $ - e3                                ; ()

e4: db "(PRINT (SYMBOLP 5))"
e4_len: equ $ - e4                                  ; ()

e5: db '(PRINT (SYMBOLP "hi"))'
e5_len: equ $ - e5                                    ; ()

e6: db "(PRINT (SYMBOLP (CONS 1 2)))"
e6_len: equ $ - e6                                      ; ()

section .text

run_thunk_discard:
    call reader_init
    call read_form
    mov rdi, rax
    call compile_thunk
    push rax
    call qword [rsp]
    add rsp, 8
    ret

global lamedh_main
lamedh_main:
    mov rdi, e1
    mov rsi, e1_len
    call run_thunk_discard
    call print_newline               ; T

    mov rdi, e2
    mov rsi, e2_len
    call run_thunk_discard
    call print_newline               ; T

    mov rdi, e3
    mov rsi, e3_len
    call run_thunk_discard
    call print_newline               ; ()

    mov rdi, e4
    mov rsi, e4_len
    call run_thunk_discard
    call print_newline               ; ()

    mov rdi, e5
    mov rsi, e5_len
    call run_thunk_discard
    call print_newline               ; ()

    mov rdi, e6
    mov rsi, e6_len
    call run_thunk_discard
    call print_newline               ; ()

    xor rax, rax
    ret
