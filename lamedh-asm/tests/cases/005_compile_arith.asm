; 005_compile_arith — reads real Lisp source text, compiles each form to
; native code, executes the compiled native function, and checks the
; result — the full read -> compile -> run pipeline for the first time.

%include "src/tags.inc"

extern reader_init
extern read_form
extern compile_thunk
extern print_fixnum
extern print_newline

section .rodata
e1: db "(+ 1 2)"
e1_len: equ $ - e1
e2: db "(* 3 4)"
e2_len: equ $ - e2
e3: db "(- 10 4)"
e3_len: equ $ - e3
e4: db "(< 1 2)"
e4_len: equ $ - e4
e5: db "(= 5 5)"
e5_len: equ $ - e5
e6: db "(< 9 2)"
e6_len: equ $ - e6

section .text

; run_and_print_fixnum(rdi=buf, rsi=len)
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

; run_and_print_bool(rdi=buf, rsi=len) — prints 1 if the result is not NIL
run_and_print_bool:
    push rbx
    call reader_init
    call read_form
    mov rdi, rax
    call compile_thunk
    mov rbx, rax
    call rbx
    xor rdi, rdi
    cmp rax, IMM_NIL
    setne dil
    TO_FIXNUM rdi
    call print_fixnum
    call print_newline
    pop rbx
    ret

global lamedh_main
lamedh_main:
    mov rdi, e1
    mov rsi, e1_len
    call run_and_print_fixnum        ; 3

    mov rdi, e2
    mov rsi, e2_len
    call run_and_print_fixnum        ; 12

    mov rdi, e3
    mov rsi, e3_len
    call run_and_print_fixnum        ; 6

    mov rdi, e4
    mov rsi, e4_len
    call run_and_print_bool          ; 1

    mov rdi, e5
    mov rsi, e5_len
    call run_and_print_bool          ; 1

    mov rdi, e6
    mov rsi, e6_len
    call run_and_print_bool          ; 0

    xor rax, rax
    ret
