; 007_lambda_call — an immediately-invoked lambda literal, exercising
; LAMBDA compilation, the indirect-call path, and local parameter access
; via a compile-time-resolved stack slot.

%include "src/tags.inc"

extern reader_init
extern read_form
extern compile_thunk
extern print_fixnum
extern print_newline

section .rodata
e1: db "((LAMBDA (X) (+ X 1)) 5)"
e1_len: equ $ - e1
e2: db "((LAMBDA (A B) (* A B)) 6 7)"
e2_len: equ $ - e2

section .text

run_and_print_fixnum:
    push rbx
    call reader_init
    call read_form
    mov rdi, rax
    call compile_thunk
    mov rbx, rax
    call rbx
    mov rdi, rax
    call print_fixnum
    call print_newline
    pop rbx
    ret

global lamedh_main
lamedh_main:
    mov rdi, e1
    mov rsi, e1_len
    call run_and_print_fixnum        ; 6

    mov rdi, e2
    mov rsi, e2_len
    call run_and_print_fixnum          ; 42

    xor rax, rax
    ret
