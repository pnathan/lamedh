; 070_numeric_contagion — #543: compiled + - * < = on anything but two
; fixnums. compile_binop used to emit raw add/sub/imul/cmp on the tagged
; words whatever they held, so (+ 1 2.0) added a heap pointer's bits,
; (< 3 2.0) compared an address, and (+ 1 'a) returned garbage instead
; of an error. Now the inline fixnum path is guarded on both tags and
; everything else goes to generic_binop (floats.asm): KERNEL.md Part V
; float contagion, f64 comparisons, and a HANDLER-CASE-catchable error
; for a non-number.
;
; The NIL/T rows pin two latent bugs in emit_coerce_char_in_rax (the
; Char coercion ahead of the same ops) that became reachable once a
; non-number was no longer silently computed with: a stale rax after
; patch_rel32 looped the range check forever, and its restore stub read
; an rcx that emit_cmp_rax_imm64 had already overwritten (NIL became 0).

%include "src/tags.inc"

extern reader_init
extern read_form
extern compile_thunk
extern print_newline

section .rodata
; int op float gives a float (Part V)
e1: db "(PRINT (+ 1 2.0))"
e1_len: equ $ - e1
e2: db "(PRINT (+ 1.5 2))"
e2_len: equ $ - e2
e3: db "(PRINT (- 5 0.5))"
e3_len: equ $ - e3
e4: db "(PRINT (* 2 1.5))"
e4_len: equ $ - e4
; float op float
e5: db "(PRINT (+ 1.5 2.25))"
e5_len: equ $ - e5
; mixed comparisons compare as f64
e6: db "(PRINT (< 3 2.0))"
e6_len: equ $ - e6
e7: db "(PRINT (< 2.0 3))"
e7_len: equ $ - e7
e8: db "(PRINT (= 1 1.0))"
e8_len: equ $ - e8
e9: db "(PRINT (= 1 1.5))"
e9_len: equ $ - e9
; Char coercion composes with contagion
e10: db "(PRINT (+ 'a' 1.0))"
e10_len: equ $ - e10
; = is IEEE ==: NaN is not = NaN
e11: db "(PRINT (= (F/ 0.0 0.0) (F/ 0.0 0.0)))"
e11_len: equ $ - e11
; an unordered < is false
e12: db "(PRINT (< (F/ 0.0 0.0) 1))"
e12_len: equ $ - e12
; neither operand a literal; slow stub flushed after a LAMBDA's ret
e13: db "(PRINT ((LAMBDA (X Y) (+ X Y)) 1 2.5))"
e13_len: equ $ - e13
; non-numbers signal a catchable error, culprit as ERROR-DATA
e14: db "(PRINT (HANDLER-CASE (+ 1 (QUOTE A)) (ERROR (E) (ERROR-DATA E))))"
e14_len: equ $ - e14
e15: db "(PRINT (HANDLER-CASE (* 2 ",34,"abc",34,") (ERROR (E) (ERROR-DATA E))))"
e15_len: equ $ - e15
e16: db "(PRINT (HANDLER-CASE ((LAMBDA (X Y) (< X Y)) 1 (QUOTE B)) (ERROR (E) (ERROR-DATA E))))"
e16_len: equ $ - e16
; a non-Char immediate operand: used to spin forever
e17: db "(PRINT (HANDLER-CASE (< 1 NIL) (ERROR (E) (QUOTE CAUGHT))))"
e17_len: equ $ - e17
e18: db "(PRINT (HANDLER-CASE (+ 1 NIL) (ERROR (E) (QUOTE CAUGHT))))"
e18_len: equ $ - e18
e19: db "(PRINT (HANDLER-CASE (= T 1) (ERROR (E) (QUOTE CAUGHT))))"
e19_len: equ $ - e19
; the fixnum fast path is unchanged
e20: db "(PRINT (+ 2 3))"
e20_len: equ $ - e20
e21: db "(PRINT (* -3 4))"
e21_len: equ $ - e21
e22: db "(PRINT (< 1 2))"
e22_len: equ $ - e22
e23: db "(PRINT ((LAMBDA (X) (- X 1)) 43))"
e23_len: equ $ - e23

align 8
exprs:
    dq e1, e1_len
    dq e2, e2_len
    dq e3, e3_len
    dq e4, e4_len
    dq e5, e5_len
    dq e6, e6_len
    dq e7, e7_len
    dq e8, e8_len
    dq e9, e9_len
    dq e10, e10_len
    dq e11, e11_len
    dq e12, e12_len
    dq e13, e13_len
    dq e14, e14_len
    dq e15, e15_len
    dq e16, e16_len
    dq e17, e17_len
    dq e18, e18_len
    dq e19, e19_len
    dq e20, e20_len
    dq e21, e21_len
    dq e22, e22_len
    dq e23, e23_len
exprs_end:

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
    ; the cursor lives on the stack: compiled code preserves no register
    ; (README "Calling convention"), rbx included.
    lea rax, [rel exprs]
    push rax
.loop:
    mov rax, [rsp]
    lea rcx, [rel exprs_end]
    cmp rax, rcx
    jae .done
    mov rdi, [rax]
    mov rsi, [rax+8]
    call run_thunk_discard
    call print_newline
    add qword [rsp], 16
    jmp .loop
.done:
    add rsp, 8
    xor rax, rax
    ret
