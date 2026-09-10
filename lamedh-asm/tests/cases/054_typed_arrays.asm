; 054_typed_arrays — typed arrays (KERNEL.md Part IV/XI): `(typed-array
; n elem-type)` — n as for ARRAY, elem-type exactly the symbol INT64
; or FLOAT64 — with per-slot type-checked storage narrower than
; arithmetic coercion (a Char is never coerced to its code point here,
; unlike Part V's own +/-/*/< contagion) and slots zero-initialized.
; FETCH/STORE/ARRAY-LENGTH* are one polymorphic primitive over both
; plain and typed arrays (arrays.asm), not new names — "reading always
; yields the declared type" (Number for INT64, Float for FLOAT64), so
; every FETCH re-tags/re-boxes fresh from the raw stored word.
;
; v0 scope: no bounds checking (the same divergence plain ARRAY
; already has, not a new one — see README).

%include "src/tags.inc"

extern reader_init
extern read_form
extern compile_thunk
extern print_newline

section .rodata
d1: db "(DEFINE A (TYPED-ARRAY 5 (QUOTE INT64)))"
d1_len: equ $ - d1
e1: db "(PRINT A)"                                                     ; <typed-array:int64:5>
e1_len: equ $ - e1
d2: db "(STORE A 0 42)"
d2_len: equ $ - d2
d3: db "(STORE A 1 -7)"
d3_len: equ $ - d3
e2: db "(PRINT (FETCH A 0))"                                             ; 42
e2_len: equ $ - e2
e3: db "(PRINT (FETCH A 1))"                                               ; -7
e3_len: equ $ - e3
e4: db "(PRINT (FETCH A 2))"                                                 ; 0 (zero-init)
e4_len: equ $ - e4
e5: db "(PRINT (ARRAY-LENGTH* A))"                                             ; 5
e5_len: equ $ - e5

d4: db "(DEFINE B (TYPED-ARRAY 3 (QUOTE FLOAT64)))"
d4_len: equ $ - d4
e6: db "(PRINT B)"                                                               ; <typed-array:float64:3>
e6_len: equ $ - e6
d5: db "(STORE B 0 3.5)"
d5_len: equ $ - d5
d6: db "(STORE B 1 7)"                                                             ; fixnum -> f64
d6_len: equ $ - d6
e7: db "(PRINT (FETCH B 0))"                                                         ; 3.500000
e7_len: equ $ - e7
e8: db "(PRINT (FETCH B 1))"                                                           ; 7.000000
e8_len: equ $ - e8

; type-check errors, all genuinely catchable.
e9:  db "(PRINT (HANDLER-CASE (STORE A 0 3.5) (E (X) (QUOTE CAUGHT))))"                  ; CAUGHT
e9_len:  equ $ - e9
e10: db "(PRINT (HANDLER-CASE (STORE B 0 (QUOTE HI)) (E (X) (QUOTE CAUGHT))))"             ; CAUGHT
e10_len: equ $ - e10
e11: db "(PRINT (HANDLER-CASE (TYPED-ARRAY 3 (QUOTE BOGUS)) (E (X) (QUOTE CAUGHT))))"        ; CAUGHT
e11_len: equ $ - e11

; a plain ARRAY is unaffected — still prints its own tag, not typed.
e12: db "(PRINT (ARRAY 3))"                                                                    ; <array:3>
e12_len: equ $ - e12

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
    call run_thunk_discard
    call print_newline               ; <typed-array:int64:5>

    mov rdi, d2
    mov rsi, d2_len
    call run_thunk_discard
    mov rdi, d3
    mov rsi, d3_len
    call run_thunk_discard

    mov rdi, e2
    mov rsi, e2_len
    call run_thunk_discard
    call print_newline               ; 42

    mov rdi, e3
    mov rsi, e3_len
    call run_thunk_discard
    call print_newline               ; -7

    mov rdi, e4
    mov rsi, e4_len
    call run_thunk_discard
    call print_newline               ; 0

    mov rdi, e5
    mov rsi, e5_len
    call run_thunk_discard
    call print_newline               ; 5

    mov rdi, d4
    mov rsi, d4_len
    call run_thunk_discard
    mov rdi, e6
    mov rsi, e6_len
    call run_thunk_discard
    call print_newline               ; <typed-array:float64:3>

    mov rdi, d5
    mov rsi, d5_len
    call run_thunk_discard
    mov rdi, d6
    mov rsi, d6_len
    call run_thunk_discard

    mov rdi, e7
    mov rsi, e7_len
    call run_thunk_discard
    call print_newline               ; 3.500000

    mov rdi, e8
    mov rsi, e8_len
    call run_thunk_discard
    call print_newline               ; 7.000000

    mov rdi, e9
    mov rsi, e9_len
    call run_thunk_discard
    call print_newline               ; CAUGHT

    mov rdi, e10
    mov rsi, e10_len
    call run_thunk_discard
    call print_newline               ; CAUGHT

    mov rdi, e11
    mov rsi, e11_len
    call run_thunk_discard
    call print_newline               ; CAUGHT

    mov rdi, e12
    mov rsi, e12_len
    call run_thunk_discard
    call print_newline               ; <array:3>

    xor rax, rax
    ret
