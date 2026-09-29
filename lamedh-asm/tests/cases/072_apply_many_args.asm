; 072_apply_many_args — APPLY (and macro expansion, which shares
; invoke_macro) with more than 32 arguments. invoke_macro used to copy
; the argument list into a fixed 32-slot host-stack scratch array and
; stop collecting at slot 32 without complaint, so
; (APPLY L (MKL 0 40)) silently returned only (0 ... 31) — issue #545.
; It now counts the list first and sizes the scratch array to fit, so
; every element is passed (registers for the first 3, host stack for
; the rest, exactly as before). No prelude here (standalone case), so
; MKL/LEN/SUM-LIST stand in for IOTA/LENGTH/REDUCE.

%include "src/tags.inc"

extern reader_init
extern read_form
extern compile_thunk
extern print_newline

section .rodata
f0: db "(DEFINE MKL (LAMBDA (I N) (IF (EQ I N) (QUOTE ()) (CONS I (MKL (+ I 1) N)))))"
f0_len: equ $ - f0
f1: db "(DEFINE LEN (LAMBDA (L) (IF (NULL L) 0 (+ 1 (LEN (CDR L))))))"
f1_len: equ $ - f1
f2: db "(DEFINE SUM-LIST (LAMBDA (L) (IF (NULL L) 0 (+ (CAR L) (SUM-LIST (CDR L))))))"
f2_len: equ $ - f2
f3: db "(DEFINE L (LAMBDA (&REST XS) XS))"
f3_len: equ $ - f3
f4: db "(PRINT (APPLY L (MKL 0 40)))"
f4_len: equ $ - f4   ; the issue's repro: all 40, not (0 ... 31)
f5: db "(PRINT (LEN (APPLY L (MKL 0 40))))"
f5_len: equ $ - f5   ; 40
f6: db "(PRINT (LEN (APPLY L (MKL 0 32))))"
f6_len: equ $ - f6   ; 32 — the old cap exactly
f7: db "(PRINT (LEN (APPLY L (MKL 0 33))))"
f7_len: equ $ - f7   ; 33 — one past it, odd count (scratch array rounds up)
f8: db "(PRINT (LEN (APPLY L (MKL 0 1000))))"
f8_len: equ $ - f8   ; 1000
f9: db "(PRINT (SUM-LIST (APPLY L (MKL 0 1000))))"
f9_len: equ $ - f9   ; 0+...+999 = 499500 — every value, in order
f10: db "(PRINT (APPLY L (QUOTE ())))"
f10_len: equ $ - f10   ; () — zero arguments still fine
f11: db "(DEFINE P35 (LAMBDA (A0 A1 A2 A3 A4 A5 A6 A7 A8 A9 A10 A11 A12 A13 A14 A15 A16 A17 A18 A19 A20 A21 A22 A23 A24 A25 A26 A27 A28 A29 A30 A31 A32 A33 A34) (CONS A0 (CONS A32 (CONS A33 (CONS A34 (QUOTE ())))))))"
f11_len: equ $ - f11
f12: db "(PRINT (APPLY P35 (MKL 0 35)))"
f12_len: equ $ - f12   ; (0 32 33 34) — fixed params past index 31 land in the right stack slots

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
    mov rdi, f0
    mov rsi, f0_len
    call run_thunk_discard
    mov rdi, f1
    mov rsi, f1_len
    call run_thunk_discard
    mov rdi, f2
    mov rsi, f2_len
    call run_thunk_discard
    mov rdi, f3
    mov rsi, f3_len
    call run_thunk_discard
    mov rdi, f4
    mov rsi, f4_len
    call run_thunk_discard
    call print_newline
    mov rdi, f5
    mov rsi, f5_len
    call run_thunk_discard
    call print_newline
    mov rdi, f6
    mov rsi, f6_len
    call run_thunk_discard
    call print_newline
    mov rdi, f7
    mov rsi, f7_len
    call run_thunk_discard
    call print_newline
    mov rdi, f8
    mov rsi, f8_len
    call run_thunk_discard
    call print_newline
    mov rdi, f9
    mov rsi, f9_len
    call run_thunk_discard
    call print_newline
    mov rdi, f10
    mov rsi, f10_len
    call run_thunk_discard
    call print_newline
    mov rdi, f11
    mov rsi, f11_len
    call run_thunk_discard
    mov rdi, f12
    mov rsi, f12_len
    call run_thunk_discard
    call print_newline

    xor rax, rax
    ret
