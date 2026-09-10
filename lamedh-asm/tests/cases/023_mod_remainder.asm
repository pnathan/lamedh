; 023_mod_remainder — MOD (Euclidean, always 0<=r<|b|) and REMAINDER
; (truncated, sign follows the dividend) are two distinct operators
; per KERNEL.md Part V, and they disagree exactly when the operands'
; signs differ. Exact worked examples straight from the spec text.

%include "src/tags.inc"

extern reader_init
extern read_form
extern compile_thunk
extern print_fixnum
extern print_newline

section .rodata
e1: db "(REMAINDER -7 2)"     ; -1
e1_len: equ $ - e1
e2: db "(REMAINDER 7 -2)"     ; 1
e2_len: equ $ - e2
e3: db "(MOD -7 2)"           ; 1
e3_len: equ $ - e3
e4: db "(MOD 7 -2)"           ; 1
e4_len: equ $ - e4
; the two agree when signs match
e5: db "(MOD 17 5)"           ; 2
e5_len: equ $ - e5
e6: db "(REMAINDER 17 5)"     ; 2
e6_len: equ $ - e6

section .text

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
    call run_and_print_fixnum
    mov rdi, e2
    mov rsi, e2_len
    call run_and_print_fixnum
    mov rdi, e3
    mov rsi, e3_len
    call run_and_print_fixnum
    mov rdi, e4
    mov rsi, e4_len
    call run_and_print_fixnum
    mov rdi, e5
    mov rsi, e5_len
    call run_and_print_fixnum
    mov rdi, e6
    mov rsi, e6_len
    call run_and_print_fixnum

    xor rax, rax
    ret
