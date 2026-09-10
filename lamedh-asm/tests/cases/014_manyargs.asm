; 014_manyargs — more than 3 arguments: the first 3 arrive in registers
; as before, the rest arrive already on the caller's stack. Exercises
; the indirect call path, the named (inline-cached) call path, and a
; closure whose free variable must be placed *after* accounting for how
; many of its own params actually consumed a register-spilled local
; slot (only min(nparams,3), not nparams).

%include "src/tags.inc"

extern reader_init
extern read_form
extern compile_thunk

section .rodata
; indirect path, 5 args: 1+2+3+4+5 = 15
e1: db "((LAMBDA (A B C D E) (+ A (+ B (+ C (+ D E))))) 1 2 3 4 5)"
e1_len: equ $ - e1

; named (inline-cached) path, 5 args
d1: db "(DEFINE SUM5 (LAMBDA (A B C D E) (+ A (+ B (+ C (+ D E))))))"
d1_len: equ $ - d1
e2: db "(SUM5 10 20 30 40 50)"
e2_len: equ $ - e2

; a closure with 4 params (so param D is stack-passed, no register slot)
; plus a captured free variable N — the free var's own local slot must
; land after only the *register-spilled* params (3), not after all 4.
d2: db "(DEFINE MAKE-ADDER4 (LAMBDA (N) (LAMBDA (A B C D) (+ N (+ A (+ B (+ C D)))))))"
d2_len: equ $ - d2
d3: db "(DEFINE ADDER4 (MAKE-ADDER4 1000))"
d3_len: equ $ - d3
e3: db "(ADDER4 1 2 3 4)"
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

; result printed via the driver, as usual — the point here is testing
; the calling convention, not PRINT.
extern print_fixnum
extern print_newline
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
    mov rdi, e1
    mov rsi, e1_len
    call run_and_print_fixnum        ; 15

    mov rdi, d1
    mov rsi, d1_len
    call run_thunk_discard

    mov rdi, e2
    mov rsi, e2_len
    call run_and_print_fixnum          ; 150

    mov rdi, d2
    mov rsi, d2_len
    call run_thunk_discard

    mov rdi, d3
    mov rsi, d3_len
    call run_thunk_discard

    mov rdi, e3
    mov rsi, e3_len
    call run_and_print_fixnum            ; 1000+1+2+3+4 = 1010

    xor rax, rax
    ret
