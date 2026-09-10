; 026_let — LET (parallel binding) and LET* (sequential binding), per
; KERNEL.md Part VI. LET shares its own slots with whatever the
; enclosing LAMBDA already reserved (current_frame_depth), so nesting
; LET inside LET inside a LAMBDA body, and calling a function from
; inside one, must not corrupt anything above it.

%include "src/tags.inc"

extern reader_init
extern read_form
extern compile_thunk
extern print_fixnum
extern print_newline

section .rodata
; basic LET: multiple bindings, multi-form body (implicit PROGN)
e1: db "(LET ((X 1) (Y 2)) (PRINT X) (+ X Y))"
e1_len: equ $ - e1

; LET is parallel: Y's init sees the OUTER X (10), not the new X (1)
d1: db "(DEFINE X 10)"
d1_len: equ $ - d1
e2: db "(LET ((X 1) (Y X)) Y)"           ; 10, not 1
e2_len: equ $ - e2

; LET* is sequential: Y's init sees the LET*-local X
e3: db "(LET* ((X 1) (Y X)) Y)"           ; 1
e3_len: equ $ - e3

; nested LET inside a LAMBDA body, calling another function from
; inside it — must not corrupt the lambda's own params/frees or the
; called function's own frame.
d2: db "(DEFINE DOUBLE (LAMBDA (N) (* N 2)))"
d2_len: equ $ - d2
e4: db "((LAMBDA (A B) (LET ((C (DOUBLE A)) (D (DOUBLE B))) (LET ((E (+ C D))) (+ E 1)))) 3 4)"
e4_len: equ $ - e4      ; (DOUBLE 3)=6, (DOUBLE 4)=8, E=14, result=15

; a closure captured inside a LET still works after the LET returns
d3: db "(DEFINE MAKE-ADDER (LAMBDA (N) (LET ((M (* N 10))) (LAMBDA (X) (+ X M)))))"
d3_len: equ $ - d3
d4: db "(DEFINE ADD5 (MAKE-ADDER 5))"           ; M = 50
d4_len: equ $ - d4
e5: db "(ADD5 1)"                                 ; 51
e5_len: equ $ - e5

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
    mov rdi, e1
    mov rsi, e1_len
    call run_and_print_fixnum        ; (PRINT 1) then 3

    mov rdi, d1
    mov rsi, d1_len
    call run_thunk_discard
    mov rdi, e2
    mov rsi, e2_len
    call run_and_print_fixnum        ; 10
    mov rdi, e3
    mov rsi, e3_len
    call run_and_print_fixnum        ; 1

    mov rdi, d2
    mov rsi, d2_len
    call run_thunk_discard
    mov rdi, e4
    mov rsi, e4_len
    call run_and_print_fixnum        ; 15

    mov rdi, d3
    mov rsi, d3_len
    call run_thunk_discard
    mov rdi, d4
    mov rsi, d4_len
    call run_thunk_discard
    mov rdi, e5
    mov rsi, e5_len
    call run_and_print_fixnum        ; 51

    xor rax, rax
    ret
