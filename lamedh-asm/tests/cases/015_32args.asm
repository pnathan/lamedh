; 015_32args — validates the documented "up to 32" ceiling for real,
; not just by extrapolation from the 5-arg test: 3 register-passed
; params plus 29 stack-passed ones, summed via a named (inline-cached)
; call. Sum of 1..32 = 528.

%include "src/tags.inc"

extern reader_init
extern read_form
extern compile_thunk
extern print_fixnum
extern print_newline

section .rodata
d1: db "(DEFINE SUM32 (LAMBDA (P0 P1 P2 P3 P4 P5 P6 P7 P8 P9 P10 P11 P12 P13 P14 P15 P16 P17 P18 P19 P20 P21 P22 P23 P24 P25 P26 P27 P28 P29 P30 P31) (+ (+ (+ (+ (+ (+ (+ (+ (+ (+ (+ (+ (+ (+ (+ (+ (+ (+ (+ (+ (+ (+ (+ (+ (+ (+ (+ (+ (+ (+ (+ P0 P1) P2) P3) P4) P5) P6) P7) P8) P9) P10) P11) P12) P13) P14) P15) P16) P17) P18) P19) P20) P21) P22) P23) P24) P25) P26) P27) P28) P29) P30) P31)))"
d1_len: equ $ - d1
e1: db "(SUM32 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20 21 22 23 24 25 26 27 28 29 30 31 32)"
e1_len: equ $ - e1

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

global lamedh_main
lamedh_main:
    mov rdi, d1
    mov rsi, d1_len
    call run_thunk_discard

    mov rdi, e1
    mov rsi, e1_len
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

    xor rax, rax
    ret
