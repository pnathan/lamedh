; 085_function_values — issue #551 item 6: #'+ and #'* (and #'-,
; #'<, #'=) are variadic function values, as the reference's builtins
; are — (FUNCALL #'+ 1 2 3) was an arity error, the prelude's value
; being a 2-parameter closure — and #'LOGAND/#'LOGIOR/#'LOGXOR, which
; had no value binding at all, are bound, variadic, with the reference's
; no-argument identities. Operator-position calls are unchanged (still
; the inline forms). Checked against the Rust reference.
;
; tests/cases/*.asm binaries load no prelude, so this one incbins
; lib/prelude.lisp and runs it, then the program below, through the
; same read/compile/run loop file_runner.asm's run_buffer uses.
%include "src/tags.inc"

extern reader_init
extern read_form
extern compile_thunk

section .rodata
prelude_start:
    incbin "lib/prelude.lisp"
prelude_end:
prelude_len: equ prelude_end - prelude_start

prog_start:
    db `(PRINT (LIST (FUNCALL #'+ 1 2 3) (FUNCALL #'+) (FUNCALL #'+ 5) (APPLY #'+ (LIST 1 2 3 4))))`, 10  ; (6 0 5 10)
    db "(NEWLINE)", 10
    db `(PRINT (LIST (FUNCALL #'* 1 2 3 4) (APPLY #'* ()) (REDUCE #'* (IOTA 5 1) 1)))`, 10  ; (24 1 120)
    db "(NEWLINE)", 10
    db `(PRINT (LIST (FUNCALL #'- 5) (FUNCALL #'- 10 1 2) (FUNCALL #'< 1 2 3) (FUNCALL #'< 1 3 2) (FUNCALL #'= 1 1 1) (FUNCALL #'= 1 1 2)))`, 10  ; (-5 7 T () T ())
    db "(NEWLINE)", 10
    db `(PRINT (LIST (FUNCALL #'LOGXOR 12 10) (FUNCALL #'LOGAND 12 10 8) (FUNCALL #'LOGIOR 1 2 4) (FUNCALL #'LOGIOR) (FUNCALL #'LOGAND) (FUNCALL #'LOGXOR)))`, 10  ; (6 8 7 0 -1 0)
    db "(NEWLINE)", 10
    db `(PRINT (LIST (REDUCE #'LOGIOR (LIST 1 2 4) 0) (MAPCAR (LAMBDA (X) (FUNCALL #'LOGAND X 6)) (LIST 3 5 7))))`, 10  ; (7 (2 4 6))
    db "(NEWLINE)", 10
    db `(PRINT (LIST (+ 1 2 3) (* 2 3) (LOGAND 12 10) (LOGXOR 1 2 4)))`, 10  ; (6 6 8 7)
prog_end:
prog_len: equ prog_end - prog_start

section .text

; run_buffer(rdi=buf, rsi=len): read, compile and run every top-level
; form in the buffer, as file_runner.asm's own run_buffer does.
run_buffer:
    call reader_init
.loop:
    call read_form
    cmp rax, IMM_EOF
    je .done
    mov rdi, rax
    call compile_thunk
    push rax
    call qword [rsp]
    add rsp, 8
    jmp .loop
.done:
    ret

global lamedh_main
lamedh_main:
    mov rdi, prelude_start
    mov rsi, prelude_len
    call run_buffer

    mov rdi, prog_start
    mov rsi, prog_len
    call run_buffer

    xor rax, rax
    ret
