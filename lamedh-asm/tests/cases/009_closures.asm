; 009_closures — the canonical closure-conversion test: a function that
; returns a lambda capturing its own parameter, with two distinct
; closures over two distinct captured values coexisting.

%include "src/tags.inc"

extern reader_init
extern read_form
extern compile_thunk
extern print_fixnum
extern print_newline

section .rodata
d1: db "(DEFINE MAKE-ADDER (LAMBDA (N) (LAMBDA (X) (+ X N))))"
d1_len: equ $ - d1
d2: db "(DEFINE ADD5 (MAKE-ADDER 5))"
d2_len: equ $ - d2
d3: db "(DEFINE ADD10 (MAKE-ADDER 10))"
d3_len: equ $ - d3
e1: db "(ADD5 1)"
e1_len: equ $ - e1
e2: db "(ADD10 1)"
e2_len: equ $ - e2
e3: db "(ADD5 100)"
e3_len: equ $ - e3

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

run_and_print_fixnum:
    call reader_init
    call read_form
    mov rdi, rax
    call compile_thunk
    push rax
    call qword [rsp]
    add rsp, 8
    mov rdi, rax
    call print_fixnum
    call print_newline
    ret

global lamedh_main
lamedh_main:
    mov rdi, d1
    mov rsi, d1_len
    call run_thunk_discard

    mov rdi, d2
    mov rsi, d2_len
    call run_thunk_discard

    mov rdi, d3
    mov rsi, d3_len
    call run_thunk_discard

    mov rdi, e1
    mov rsi, e1_len
    call run_and_print_fixnum        ; ADD5(1) = 6

    mov rdi, e2
    mov rsi, e2_len
    call run_and_print_fixnum          ; ADD10(1) = 11

    mov rdi, e3
    mov rsi, e3_len
    call run_and_print_fixnum           ; ADD5(100) = 105 — ADD5's captured N still 5

    xor rax, rax
    ret
