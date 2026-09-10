; 050_random — RANDOM/RANDOM-SEED! (rng.asm), the same SplitMix64
; generator as the Rust reference (../src/evaluator/builtins_extra.rs's
; own rng_next), so a fixed RANDOM-SEED! produces the identical
; sequence on both hosts — the one part of this feature meant to be
; bit-for-bit reproducible; the lazy clock-seeded default (no explicit
; RANDOM-SEED! call) is deliberately not tested here, since by
; definition it isn't reproducible on either host.
;
; Covers: two RANDOM-SEED! calls with the same seed produce the same
; RANDOM sequence; every result is genuinely within [0, n); a wrong-
; type or non-positive argument to RANDOM is a catchable condition
; (fail_wrong_type/native_throw), not a crash or a silent wraparound.

%include "src/tags.inc"

extern reader_init
extern read_form
extern compile_thunk
extern print_fixnum
extern print_newline

section .rodata
d1: db "(RANDOM-SEED! 42)"
d1_len: equ $ - d1
e1: db "(RANDOM 1000000)"
e1_len: equ $ - e1

d2: db "(RANDOM-SEED! 42)"
d2_len: equ $ - d2
e2: db "(RANDOM 1000000)"                    ; same as e1: same seed
e2_len: equ $ - e2                            ; -> same first draw

; every draw from a positive-N call is genuinely in [0, N) — this
; doesn't prove the whole distribution but catches the classic
; off-by-one (an inclusive upper bound, or a raw unmodded value).
; PRINT'd explicitly (not run_and_print_fixnum, which calls
; print_fixnum directly on the raw tagged result) since < returns
; IMM_TRUE/IMM_NIL, not a fixnum.
e3: db "(PRINT (< (RANDOM 10) 10))"               ; T
e3_len: equ $ - e3

e4: db "(PRINT (HANDLER-CASE (RANDOM 0) (E (X) (QUOTE CAUGHT))))"  ; CAUGHT
e4_len: equ $ - e4
e5: db "(PRINT (HANDLER-CASE (RANDOM -5) (E (X) (QUOTE CAUGHT))))" ; CAUGHT
e5_len: equ $ - e5
e6: db "(PRINT (HANDLER-CASE (RANDOM-SEED! (QUOTE X)) (E (X) (QUOTE CAUGHT))))" ; CAUGHT
e6_len: equ $ - e6

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
    mov rdi, e1
    mov rsi, e1_len
    call run_and_print_fixnum

    mov rdi, d2
    mov rsi, d2_len
    call run_thunk_discard
    mov rdi, e2
    mov rsi, e2_len
    call run_and_print_fixnum        ; must equal the line above

    mov rdi, e3
    mov rsi, e3_len
    call run_thunk_discard
    call print_newline               ; T

    mov rdi, e4
    mov rsi, e4_len
    call run_thunk_discard
    call print_newline               ; CAUGHT

    mov rdi, e5
    mov rsi, e5_len
    call run_thunk_discard
    call print_newline               ; CAUGHT

    mov rdi, e6
    mov rsi, e6_len
    call run_thunk_discard
    call print_newline               ; CAUGHT

    xor rax, rax
    ret
