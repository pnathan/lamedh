; 055_char_contagion — arithmetic/comparison contagion for Char
; operands (KERNEL.md Part V): "A Char operand is unconditionally
; coerced to its code point" in +/-/*/</=' own contagion rule.
; emit_coerce_char_in_rax (compiler.asm) runs right after compiling
; each operand of a compiled +/-/*/</=, replacing a tagged Char in
; place with its code point as a tagged fixnum; an ordinary fixnum (or
; anything else — a symbol, a cons) is left completely unchanged, the
; case this test's own e6/e7 exist specifically to guard: an earlier,
; buggy draft of the coercion check corrupted every ordinary (non-
; Char) operand by leaving a leftover scratch value in rax on the
; "not a Char" exit path instead of restoring the original — caught
; immediately by the full test suite (043_function_sharp_quote and
; others), not shipped.
;
; EQ deliberately does *not* contagion-coerce (KERNEL.md Part IV:
; "(eq 'a' 97) is NIL" — Char and Number are distinct types for EQ) —
; already covered by tests/cases/048_char_literals.asm, not repeated
; here.

%include "src/tags.inc"

extern reader_init
extern read_form
extern compile_thunk
extern print_newline

section .rodata
e1: db "(PRINT (+ 'a' 1))"                    ; 98
e1_len: equ $ - e1
e2: db "(PRINT (- 'b' 'a'))"                    ; 1
e2_len: equ $ - e2
e3: db "(PRINT (* 'a' 2))"                        ; 194
e3_len: equ $ - e3
e4: db "(PRINT (< 'a' 'b'))"                        ; T
e4_len: equ $ - e4
e5: db "(PRINT (= 'a' 97))"                           ; T (contagion — unlike EQ)
e5_len: equ $ - e5
; ordinary fixnum arithmetic is completely unaffected.
e6: db "(PRINT (+ 1 2))"                                ; 3
e6_len: equ $ - e6
e7: db "(PRINT (+ 100 (* 3 7)))"                           ; 121
e7_len: equ $ - e7

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
    mov rdi, e1
    mov rsi, e1_len
    call run_thunk_discard
    call print_newline               ; 98

    mov rdi, e2
    mov rsi, e2_len
    call run_thunk_discard
    call print_newline               ; 1

    mov rdi, e3
    mov rsi, e3_len
    call run_thunk_discard
    call print_newline               ; 194

    mov rdi, e4
    mov rsi, e4_len
    call run_thunk_discard
    call print_newline               ; T

    mov rdi, e5
    mov rsi, e5_len
    call run_thunk_discard
    call print_newline               ; T

    mov rdi, e6
    mov rsi, e6_len
    call run_thunk_discard
    call print_newline               ; 3

    mov rdi, e7
    mov rsi, e7_len
    call run_thunk_discard
    call print_newline               ; 121

    xor rax, rax
    ret
