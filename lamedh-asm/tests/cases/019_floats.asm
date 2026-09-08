; 019_floats — the float value type: reader support for "3.14"-style
; literals (boxed HDR_FLOAT heapobj, self-evaluating like any other
; literal), and F+/F-/F*/F//F</FLOAT, all ordinary host-routine calls
; (compile_binary_hostcall/compile_unary_hostcall) — no XMM support was
; added to codegen.asm; see floats.asm's own note on why.

%include "src/tags.inc"

extern reader_init
extern read_form
extern compile_thunk
extern print_newline

section .rodata
; PRINT of a float literal, fixed 6-decimal-place formatting.
e1: db "(PRINT 3.5)"
e1_len: equ $ - e1

; F+/F-/F*/F/ on literals.
e2: db "(PRINT (F+ 1.5 2.25))"           ; 3.75
e2_len: equ $ - e2
e3: db "(PRINT (F- 5.0 1.5))"            ; 3.5
e3_len: equ $ - e3
e4: db "(PRINT (F* 2.5 4.0))"            ; 10.0
e4_len: equ $ - e4
e5: db "(PRINT (F/ 9.0 2.0))"            ; 4.5
e5_len: equ $ - e5

; F<
e6: db "(IF (F< 1.0 2.0) (PRINT 111) (PRINT 222))"
e6_len: equ $ - e6
e7: db "(IF (F< 2.0 1.0) (PRINT 111) (PRINT 222))"
e7_len: equ $ - e7

; FLOAT: fixnum -> float
e8: db "(PRINT (FLOAT 7))"               ; 7.0
e8_len: equ $ - e8

; a negative float literal
e9: db "(PRINT -2.5)"
e9_len: equ $ - e9

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
    RUN_PRINT_NL e7, e7_len
    RUN_PRINT_NL e8, e8_len
    RUN_PRINT_NL e9, e9_len

    xor rax, rax
    ret
