; 081_float_print — issue #551 item 2: a Float prints as KERNEL.md Part I
; specifies it, the shortest decimal that reads back to the same double
; (Rust's f64::to_string), never scientific, with ".0" appended when the
; text has no fraction, and the sign taken from the sign bit (-0.0 stays
; "-0.0"). Infinities and NaN print as "inf", "-inf", "NaN". Expected
; output generated with rustc's to_string.
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
    db `(PRINT -0.0)`, 10  ; -0.0
    db "(NEWLINE)", 10
    db `(PRINT (LIST 0.0 (PRINC-TO-STRING -0.0) (F- 0.0 0.0) (F* -1.0 0.0)))`, 10  ; (0.0 -0.0 0.0 -0.0)
    db "(NEWLINE)", 10
    db `(PRINT (LIST 3.14159265358979 0.9999999 -0.9999999 2.5 -1.25))`, 10  ; (3.14159265358979 0.9999999 -0.9999999 2.5 -1.25)
    db "(NEWLINE)", 10
    db `(PRINT (LIST 0.0000015 0.0000005 0.1234565 123456789012.5))`, 10  ; (0.0000015 0.0000005 0.1234565 123456789012.5)
    db "(NEWLINE)", 10
    db `(PRINT (LIST 1e400 -1e400 (F- 1e400 1e400)))`, 10  ; (inf -inf NaN)
    db "(NEWLINE)", 10
    db `(PRINT (LIST 1e18 1e19 1e30 1e23))`, 10  ; (1000000000000000000.0 10000000000000000000.0 1000000000000000000000000000000.0 100000000000000000000000.0)
    db "(NEWLINE)", 10
    db `(PRINT (LIST 0.1 1.4142135623730951 4.35 1.0E-7 5e-324))`, 10  ; (0.1 1.4142135623730951 4.35 0.0000001 <5e-324: 323 zeros then 5>)
    db "(NEWLINE)", 10
    db `(PRINT (LIST 1.0E22 9223372036854775808.0 100.0 -7.0))`, 10  ; (10000000000000000000000.0 9223372036854776000.0 100.0 -7.0)
    db "(NEWLINE)", 10
    db `(PRINT (LIST (PRINC-TO-STRING -1e400) (PRINC-TO-STRING (F- 1e400 1e400)) (PRINC-TO-STRING -0.0) (PRINC-TO-STRING 5e-324) (PRINC-TO-STRING 1e23)))`, 10  ; capture path
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
