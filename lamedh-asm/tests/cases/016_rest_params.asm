; 016_rest_params — &REST parameters. Restriction (see README): only
; supported when the fixed-parameter count is >= 3, so every rest
; argument is always stack-resident (never register-spilled) — a
; deliberately narrow v1 scope, not an accident. Exercises: zero rest
; args (MORE = NIL), several rest args with order preserved (not
; reversed by the cons-while-walking-backward construction), and a
; &REST param sitting after a *stack-passed* fixed param (nfixed=4).

%include "src/tags.inc"

extern reader_init
extern read_form
extern compile_thunk
extern print_fixnum
extern print_newline

section .rodata
; SUM-LIST: recursive helper, sums a proper list via CAR/CDR/NULL/IF.
d1: db "(DEFINE SUM-LIST (LAMBDA (L) (IF (NULL L) 0 (+ (CAR L) (SUM-LIST (CDR L))))))"
d1_len: equ $ - d1

; F: 3 fixed (all register-spilled) + REST.
d2: db "(DEFINE F (LAMBDA (A B C &REST MORE) (SUM-LIST MORE)))"
d2_len: equ $ - d2
e1: db "(F 1 2 3)"                       ; MORE = NIL -> 0
e1_len: equ $ - e1
e2: db "(F 1 2 3 4 5 6)"                 ; MORE = (4 5 6) -> 15
e2_len: equ $ - e2

; G: same shape, returns (CAR MORE) directly to confirm order (first
; rest arg first, not last-consed-first).
d3: db "(DEFINE G (LAMBDA (A B C &REST MORE) (CAR MORE)))"
d3_len: equ $ - d3
e3: db "(G 1 2 3 4 5 6)"                 ; CAR MORE = 4
e3_len: equ $ - e3

; H: 4 fixed params (D is stack-passed, not register-spilled) + REST.
d4: db "(DEFINE H (LAMBDA (A B C D &REST MORE) (+ D (CAR MORE))))"
d4_len: equ $ - d4
e4: db "(H 1 2 3 4 5 6)"                 ; D=4, CAR MORE=5 -> 9
e4_len: equ $ - e4

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

    mov rdi, e1
    mov rsi, e1_len
    call run_and_print_fixnum        ; 0

    mov rdi, e2
    mov rsi, e2_len
    call run_and_print_fixnum        ; 15

    mov rdi, d3
    mov rsi, d3_len
    call run_thunk_discard

    mov rdi, e3
    mov rsi, e3_len
    call run_and_print_fixnum        ; 4

    mov rdi, d4
    mov rsi, d4_len
    call run_thunk_discard

    mov rdi, e4
    mov rsi, e4_len
    call run_and_print_fixnum        ; 9

    xor rax, rax
    ret
