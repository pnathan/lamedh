; 082_number_exponent — issue #551 item 3: a number token with an e/E
; exponent reads as one float, as the reference's parse_float does.
; "1.5e2" used to read as 1.5 followed by the symbol E2. The significand's
; digits scale by one exact power of ten (Clinger's fast path), so these
; are the doubles the reference reads — EQUAL to the plain literals.
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
    db `(PRINT (LIST 1.5e2 1e5 1.5E2 1.5e+2 -2E-3 1.5e-3))`, 10  ; (150.000000 100000.000000 150.000000 150.000000 -0.002000 0.001500)
    db "(NEWLINE)", 10
    db `(PRINT (LIST (EQUAL 1.5e2 150.0) (EQUAL 1.5e-3 0.0015) (EQUAL 1e5 100000.0) (EQUAL 25e-1 2.5) (EQUAL 1.5e2 150.00001)))`, 10  ; (T T T T ())
    db "(NEWLINE)", 10
    db `(PRINT (LIST (QUOTE (1e2 . 3)) ($LENGTH (QUOTE (1.5e2 X)))))`, 10  ; ((100.000000 . 3) 2)
    db "(NEWLINE)", 10
    db `(PRINT (LIST 12345678901234567890e-10 -0e5 1e400))`, 10  ; (1234567890.123457 -0.000000 inf)
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
