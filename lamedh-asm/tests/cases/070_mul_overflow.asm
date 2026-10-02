; 070_mul_overflow — `*` sets OVERFLOW (#546), the same way `+`/`-` do
; in 031_overflow. compile_binop untags the rhs before its `imul`, so the
; hardware OF is overflow of the represented fixnum product: "overflow"
; means the product left the 62-bit fixnum range [-2^61, 2^61 - 1], not
; the 64-bit machine range. EXPT (lib/prelude.lisp) is repeated `*`; it
; is not exercised here because test cases do not load the prelude.

%include "src/tags.inc"

extern reader_init
extern read_form
extern compile_thunk
extern print_newline
extern print_fixnum

section .rodata
flag: db "(IF (FLAG-SET-P (QUOTE OVERFLOW)) 111 222)"
flag_len: equ $ - flag
clr: db "(CLEAR-ALL-FLAGS)"
clr_len: equ $ - clr

; an ordinary product is exact and leaves the flag clear
m1: db "(* 6 -7)"                                     ; -42
m1_len: equ $ - m1                                     ; then 222
; the issue's repro: (2^61 - 1) * 2 wraps and must set the flag
m2: db "(* 2305843009213693951 2)"                    ; wrapped
m2_len: equ $ - m2                                     ; then 111
; a product that fits in 64 bits but not in a fixnum still overflows:
; 2^31 * 2^30 = 2^61, one past the largest fixnum
m3: db "(* 2147483648 1073741824)"
m3_len: equ $ - m3                                     ; 111
; exact boundaries do not: 2^30 * 2^30 = 2^60, and -(2^60) * 2 = -(2^61),
; the smallest fixnum (an earlier <<4-then-sar scheme would have
; misreported both, since the pre-correction product is 4x wider)
m4: db "(* 1073741824 1073741824)"                    ; 1152921504606846976
m4_len: equ $ - m4                                     ; then 222
m5: db "(* -1152921504606846976 2)"                   ; -2305843009213693952
m5_len: equ $ - m5                                     ; then 222

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

print_flag:
    mov rdi, flag
    mov rsi, flag_len
    jmp run_and_print_fixnum

clear_flags:
    mov rdi, clr
    mov rsi, clr_len
    jmp run_thunk_discard

global lamedh_main
lamedh_main:
    mov rdi, m1
    mov rsi, m1_len
    call run_and_print_fixnum        ; -42
    call print_flag                  ; 222

    mov rdi, m2
    mov rsi, m2_len
    call run_thunk_discard
    call print_flag                  ; 111

    call clear_flags
    mov rdi, m3
    mov rsi, m3_len
    call run_thunk_discard
    call print_flag                  ; 111

    call clear_flags
    mov rdi, m4
    mov rsi, m4_len
    call run_and_print_fixnum        ; 1152921504606846976
    call print_flag                  ; 222

    mov rdi, m5
    mov rsi, m5_len
    call run_and_print_fixnum        ; -2305843009213693952
    call print_flag                  ; 222

    xor rax, rax
    ret
