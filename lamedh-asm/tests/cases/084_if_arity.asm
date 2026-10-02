; 084_if_arity — issue #551 item 5: IF takes exactly three operands
; (KERNEL.md Part VI). (IF NIL 1 2 3) used to evaluate to 2 and
; (IF NIL 1) to NIL; both are now an error ("if takes exactly three
; arguments", the reference's message, with the form as its data), at
; compile time — so here through EVAL, inside HANDLER-CASE.
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
    db `(PRINT (HANDLER-CASE (EVAL (QUOTE (IF NIL 1 2 3))) (ERROR (E) (LIST (ERROR-MESSAGE E) (ERROR-DATA E)))))`, 10  ; (if takes exactly three arguments (IF () 1 2 3))
    db "(NEWLINE)", 10
    db `(PRINT (HANDLER-CASE (EVAL (QUOTE (IF NIL 1))) (ERROR (E) (LIST (ERROR-MESSAGE E) (ERROR-DATA E)))))`, 10  ; (if takes exactly three arguments (IF () 1))
    db "(NEWLINE)", 10
    db `(PRINT (HANDLER-CASE (EVAL (QUOTE (IF))) (ERROR (E) (LIST (ERROR-MESSAGE E) (ERROR-DATA E)))))`, 10  ; (if takes exactly three arguments (IF))
    db "(NEWLINE)", 10
    db `(PRINT (HANDLER-CASE (EVAL (QUOTE (DEFUN IF2 (X) (IF X 1)))) (ERROR (E) (LIST (ERROR-MESSAGE E) (ERROR-DATA E)))))`, 10  ; (if takes exactly three arguments (IF X 1))
    db "(NEWLINE)", 10
    db `(PRINT (LIST (IF NIL 1 2) (IF T 1 2) (IF NIL 1 (IF T 3 4))))`, 10  ; (2 1 3)
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
