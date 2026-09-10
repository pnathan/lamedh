; 030_print_opaque — KERNEL.md Part III requires an opaque, non-readable
; tag for value types with no literal syntax: "<lambda>" for closures,
; "<array:N>" for arrays. Before this, PRINT of either fell through to
; print_fixnum, which reinterprets a tagged heapobj pointer's raw bits
; as a signed fixnum and prints meaningless garbage instead of a tag.

%include "src/tags.inc"

extern reader_init
extern read_form
extern compile_thunk
extern print_newline

section .rodata
e1: db "(PRINT (ARRAY 3))"                    ; <array:3>
e1_len: equ $ - e1
e2: db "(PRINT (ARRAY 0))"                    ; <array:0>
e2_len: equ $ - e2
e3: db "(PRINT (LAMBDA (X) X))"               ; <lambda>
e3_len: equ $ - e3

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

    xor rax, rax
    ret
