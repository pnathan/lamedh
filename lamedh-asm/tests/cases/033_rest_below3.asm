; 033_rest_below3 — &REST parameters with nfixed<3 (0, 1, and 2 fixed
; params), lifting the compiler's former nfixed>=3 restriction. The
; calling convention (compile_call_args/compile_call) always places
; global argument indices 0/1/2 in rsi/rdx/rcx regardless of a given
; callee's own fixed/REST split, so with nfixed<3 some REST elements
; are register-resident (spilled into local slots by compile_lambda's
; prologue) rather than stack-resident — the case 016_rest_params.asm's
; nfixed>=3 examples never exercise. Covers, for each of nfixed=0,1,2:
; no rest args at all, rest args that are entirely register-resident,
; and rest args split across both register- and stack-resident
; positions — the exact three regimes compile_lambda's register-fold
; step and stack loop must agree on.

%include "src/tags.inc"

extern reader_init
extern read_form
extern compile_thunk
extern print_fixnum
extern print_newline

section .rodata
d1: db "(DEFINE SUM-LIST (LAMBDA (L) (IF (NULL L) 0 (+ (CAR L) (SUM-LIST (CDR L))))))"
d1_len: equ $ - d1

; nfixed=0: every argument, if any, is a REST element.
d2: db "(DEFINE F0 (LAMBDA (&REST ALL) (SUM-LIST ALL)))"
d2_len: equ $ - d2
e1: db "(F0)"                              ; ALL=NIL -> 0
e1_len: equ $ - e1
e2: db "(F0 1)"                            ; ALL=(1), register-only -> 1
e2_len: equ $ - e2
e3: db "(F0 1 2)"                          ; ALL=(1 2), register-only -> 3
e3_len: equ $ - e3
e4: db "(F0 1 2 3)"                        ; ALL=(1 2 3), register-only -> 6
e4_len: equ $ - e4
e5: db "(F0 1 2 3 4 5)"                    ; ALL=(1 2 3 4 5), register+stack -> 15
e5_len: equ $ - e5

; nfixed=1: A is fixed (register), MORE starts at global index 1.
d3: db "(DEFINE F1 (LAMBDA (A &REST MORE) (SUM-LIST MORE)))"
d3_len: equ $ - d3
e6: db "(F1 10)"                           ; MORE=NIL -> 0
e6_len: equ $ - e6
e7: db "(F1 10 1 2)"                       ; MORE=(1 2), register-only -> 3
e7_len: equ $ - e7
e8: db "(F1 10 1 2 3 4)"                   ; MORE=(1 2 3 4), register+stack -> 10
e8_len: equ $ - e8

; nfixed=2: A B are fixed, MORE starts at global index 2 (register C).
d4: db "(DEFINE F2 (LAMBDA (A B &REST MORE) (SUM-LIST MORE)))"
d4_len: equ $ - d4
e9: db "(F2 10 20)"                        ; MORE=NIL -> 0
e9_len: equ $ - e9
e10: db "(F2 10 20 1)"                     ; MORE=(1), register-only -> 1
e10_len: equ $ - e10
e11: db "(F2 10 20 1 2 3)"                 ; MORE=(1 2 3), register+stack -> 6
e11_len: equ $ - e11

; order check: first rest arg first, not reversed, across the
; register/stack boundary this case actually crosses.
d5: db "(DEFINE G2 (LAMBDA (A B &REST MORE) (CAR MORE)))"
d5_len: equ $ - d5
e12: db "(G2 10 20 1 2 3)"                 ; CAR MORE = 1
e12_len: equ $ - e12
d6: db "(DEFINE H2 (LAMBDA (A B &REST MORE) (CAR (CDR (CDR MORE)))))"
d6_len: equ $ - d6
e13: db "(H2 10 20 1 2 3)"                 ; third of MORE = 3
e13_len: equ $ - e13

; fixed params are still read correctly alongside a register-resident
; rest element in the very next slot.
d7: db "(DEFINE F2SUM (LAMBDA (A B &REST MORE) (+ A (+ B (SUM-LIST MORE)))))"
d7_len: equ $ - d7
e14: db "(F2SUM 10 20 1 2 3)"              ; 10+20+1+2+3 = 36
e14_len: equ $ - e14

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
    call run_and_print_fixnum        ; 1
    mov rdi, e3
    mov rsi, e3_len
    call run_and_print_fixnum        ; 3
    mov rdi, e4
    mov rsi, e4_len
    call run_and_print_fixnum        ; 6
    mov rdi, e5
    mov rsi, e5_len
    call run_and_print_fixnum        ; 15

    mov rdi, d3
    mov rsi, d3_len
    call run_thunk_discard
    mov rdi, e6
    mov rsi, e6_len
    call run_and_print_fixnum        ; 0
    mov rdi, e7
    mov rsi, e7_len
    call run_and_print_fixnum        ; 3
    mov rdi, e8
    mov rsi, e8_len
    call run_and_print_fixnum        ; 10

    mov rdi, d4
    mov rsi, d4_len
    call run_thunk_discard
    mov rdi, e9
    mov rsi, e9_len
    call run_and_print_fixnum        ; 0
    mov rdi, e10
    mov rsi, e10_len
    call run_and_print_fixnum        ; 1
    mov rdi, e11
    mov rsi, e11_len
    call run_and_print_fixnum        ; 6

    mov rdi, d5
    mov rsi, d5_len
    call run_thunk_discard
    mov rdi, e12
    mov rsi, e12_len
    call run_and_print_fixnum        ; 1

    mov rdi, d6
    mov rsi, d6_len
    call run_thunk_discard
    mov rdi, e13
    mov rsi, e13_len
    call run_and_print_fixnum        ; 3

    mov rdi, d7
    mov rsi, d7_len
    call run_thunk_discard
    mov rdi, e14
    mov rsi, e14_len
    call run_and_print_fixnum        ; 36

    xor rax, rax
    ret
