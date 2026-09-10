; 024_print_readable — PRINT now handles every value this kernel has,
; PRIN1-style, per KERNEL.md Part III: NIL as "()", a symbol as its
; name, a cons as a recursively printed list (proper and dotted), not
; just fixnums/strings/floats. Previously PRINT of anything but those
; three fell through to print_fixnum and misread the tag.

%include "src/tags.inc"

extern reader_init
extern read_form
extern compile_thunk
extern print_newline

section .rodata
e1: db "(PRINT (QUOTE ()))"                 ; ()
e1_len: equ $ - e1
e2: db "(PRINT (QUOTE FOO))"                ; FOO
e2_len: equ $ - e2
e3: db "(PRINT (EQ 1 1))"                     ; T  (IMM_TRUE)
e3_len: equ $ - e3
e4: db "(PRINT (QUOTE (1 2 3)))"                ; (1 2 3)
e4_len: equ $ - e4
e5: db "(PRINT (CONS 1 2))"                       ; (1 . 2)
e5_len: equ $ - e5
e6: db "(PRINT (QUOTE (A (B C) D)))"                ; (A (B C) D) -- nested
e6_len: equ $ - e6

section .text

global lamedh_main
lamedh_main:
%macro RUN_PRINT_NL 2
    mov rdi, %1
    mov rsi, %2
    call reader_init
    call read_form
    mov rdi, rax
    call compile_thunk
    push rax
    call qword [rsp]
    add rsp, 8
    call print_newline
%endmacro

    RUN_PRINT_NL e1, e1_len
    RUN_PRINT_NL e2, e2_len
    RUN_PRINT_NL e3, e3_len
    RUN_PRINT_NL e4, e4_len
    RUN_PRINT_NL e5, e5_len
    RUN_PRINT_NL e6, e6_len

    xor rax, rax
    ret
