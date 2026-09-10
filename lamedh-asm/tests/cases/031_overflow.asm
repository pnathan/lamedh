; 031_overflow — KERNEL.md Part V / Part XII axis 1's OVERFLOW signal.
; Fixnum +/- on this kernel run directly on the tagged (shifted-left-by-2)
; representation, so the hardware overflow flag on the emitted add/sub is
; already exactly Part V's "wraps and sets the flag" model applied at this
; representation's own dynamic range — see overflow.asm. FLAG-SET-P and
; CLEAR-FLAG/CLEAR-ALL-FLAGS are the three names Part V's own prose gives.

%include "src/tags.inc"

extern reader_init
extern read_form
extern compile_thunk
extern print_newline
extern print_fixnum

section .rodata
; ordinary arithmetic never sets the flag
e1: db "(IF (FLAG-SET-P (QUOTE OVERFLOW)) 111 222)"      ; 222
e1_len: equ $ - e1
e2: db "(+ 2 3)"                                          ; 5 (no overflow)
e2_len: equ $ - e2
e3: db "(IF (FLAG-SET-P (QUOTE OVERFLOW)) 111 222)"      ; 222 (still clear)
e3_len: equ $ - e3

; a fixnum here is 62 bits of payload (tags.inc); the largest positive
; fixnum is 2^61 - 1. Adding 1 to it wraps into the negative range and
; must set OVERFLOW, exactly like the reference's own i64 wraparound.
d1: db "(DEFINE MAXFIX 2305843009213693951)"                 ; 2^61 - 1
d1_len: equ $ - d1
e4: db "(+ MAXFIX 1)"                                         ; wraps negative
e4_len: equ $ - e4
e5: db "(IF (FLAG-SET-P (QUOTE OVERFLOW)) 111 222)"          ; 111 (now set)
e5_len: equ $ - e5

; CLEAR-FLAG turns it back off
d2: db "(CLEAR-FLAG (QUOTE OVERFLOW))"
d2_len: equ $ - d2
e6: db "(IF (FLAG-SET-P (QUOTE OVERFLOW)) 111 222)"          ; 222 (cleared)
e6_len: equ $ - e6

; CLEAR-ALL-FLAGS also clears it
e7: db "(+ MAXFIX 1)"                                         ; sets it again
e7_len: equ $ - e7
d3: db "(CLEAR-ALL-FLAGS)"
d3_len: equ $ - d3
e8: db "(IF (FLAG-SET-P (QUOTE OVERFLOW)) 111 222)"          ; 222

e8_len: equ $ - e8

; subtraction wraps and sets the flag the same way
d4: db "(DEFINE MINFIX -2305843009213693952)"                ; -(2^61)
d4_len: equ $ - d4
e9: db "(- MINFIX 1)"                                         ; wraps positive
e9_len: equ $ - e9
e10: db "(IF (FLAG-SET-P (QUOTE OVERFLOW)) 111 222)"          ; 111
e10_len: equ $ - e10

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
    call run_and_print_fixnum        ; 222

    mov rdi, e2
    mov rsi, e2_len
    call run_thunk_discard
    mov rdi, e3
    mov rsi, e3_len
    call run_and_print_fixnum        ; 222

    mov rdi, d1
    mov rsi, d1_len
    call run_thunk_discard
    mov rdi, e4
    mov rsi, e4_len
    call run_thunk_discard
    mov rdi, e5
    mov rsi, e5_len
    call run_and_print_fixnum        ; 111

    mov rdi, d2
    mov rsi, d2_len
    call run_thunk_discard
    mov rdi, e6
    mov rsi, e6_len
    call run_and_print_fixnum        ; 222

    mov rdi, e7
    mov rsi, e7_len
    call run_thunk_discard
    mov rdi, d3
    mov rsi, d3_len
    call run_thunk_discard
    mov rdi, e8
    mov rsi, e8_len
    call run_and_print_fixnum        ; 222

    mov rdi, d4
    mov rsi, d4_len
    call run_thunk_discard
    mov rdi, e9
    mov rsi, e9_len
    call run_thunk_discard
    mov rdi, e10
    mov rsi, e10_len
    call run_and_print_fixnum        ; 111

    xor rax, rax
    ret
