; 086_iota — issue #551 item 7: IOTA takes the reference's
; (n &optional start step) (lib/13-functional.lisp): (IOTA 5) was an
; arity error. N below 1 is the empty list (the old (= N 0) test never
; ended for a negative N), and a long list is built in constant stack.
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
    db `(PRINT (LIST (IOTA 5) (IOTA 0) (IOTA -2) (IOTA 5 1)))`, 10  ; ((0 1 2 3 4) () () (1 2 3 4 5))
    db "(NEWLINE)", 10
    db `(PRINT (LIST (IOTA 3 5 2) (IOTA 3 10 -1)))`, 10  ; ((5 7 9) (10 9 8))
    db "(NEWLINE)", 10
    db `(PRINT (LIST ($LENGTH (IOTA 300000)) (CAR (LAST (IOTA 300000)))))`, 10  ; (300000 299999)
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
