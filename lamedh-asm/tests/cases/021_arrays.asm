; 021_arrays — the array value type: MAKE-ARRAY/ARRAY-REF/ARRAY-SET/
; ARRAY-LENGTH, the first kernel primitives that mutate a heap value
; after creation, plus HASH-CODE and MOD — the small extra surface an
; array-backed hash table library needs beyond CONS/CAR/CDR/EQ.

%include "src/tags.inc"

extern reader_init
extern read_form
extern compile_thunk
extern print_fixnum
extern print_newline

section .rodata
d1: db "(DEFINE A (MAKE-ARRAY 5))"
d1_len: equ $ - d1
e1: db "(ARRAY-LENGTH A)"                    ; 5
e1_len: equ $ - e1
e2: db "(IF (NULLP (ARRAY-REF A 0)) 111 222)"  ; 111 (fresh slot is NIL)
e2_len: equ $ - e2
e3: db "(ARRAY-SET A 2 42)"                    ; 42 (ARRAY-SET returns its value)
e3_len: equ $ - e3
e4: db "(ARRAY-REF A 2)"                        ; 42 (mutation visible on read-back)
e4_len: equ $ - e4
e5: db "(ARRAY-REF A 0)"                          ; 0-tagged NIL untouched: still NIL, print as 0? no
e5_len: equ $ - e5
; overwrite twice; second write wins
e6: db "(ARRAY-SET A 2 99)"
e6_len: equ $ - e6
e7: db "(ARRAY-REF A 2)"                            ; 99
e7_len: equ $ - e7

; MOD
e8: db "(MOD 17 5)"                                   ; 2
e8_len: equ $ - e8
e9: db "(MOD 20 5)"                                     ; 0
e9_len: equ $ - e9

; HASH-CODE: same key -> same hash, always non-negative
d2: db "(DEFINE H1 (HASH-CODE (QUOTE FOO)))"
d2_len: equ $ - d2
d3: db "(DEFINE H2 (HASH-CODE (QUOTE FOO)))"
d3_len: equ $ - d3
e10: db "(IF (EQ H1 H2) 111 222)"                        ; 111 (stable per-symbol hash)
e10_len: equ $ - e10
e11: db "(IF (< H1 0) 111 222)"                            ; 222 (never negative)
e11_len: equ $ - e11

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
    call run_and_print_fixnum        ; 5
    mov rdi, e2
    mov rsi, e2_len
    call run_and_print_fixnum        ; 111
    mov rdi, e3
    mov rsi, e3_len
    call run_and_print_fixnum        ; 42
    mov rdi, e4
    mov rsi, e4_len
    call run_and_print_fixnum        ; 42
    mov rdi, e6
    mov rsi, e6_len
    call run_thunk_discard
    mov rdi, e7
    mov rsi, e7_len
    call run_and_print_fixnum        ; 99
    mov rdi, e8
    mov rsi, e8_len
    call run_and_print_fixnum        ; 2
    mov rdi, e9
    mov rsi, e9_len
    call run_and_print_fixnum        ; 0

    mov rdi, d2
    mov rsi, d2_len
    call run_thunk_discard
    mov rdi, d3
    mov rsi, d3_len
    call run_thunk_discard
    mov rdi, e10
    mov rsi, e10_len
    call run_and_print_fixnum        ; 111
    mov rdi, e11
    mov rsi, e11_len
    call run_and_print_fixnum        ; 222

    xor rax, rax
    ret
