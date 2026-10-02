; 081_float_print — issue #551 item 2 (the part fixed here): float
; printing keeps the sign of -0.0 (a compare against 0.0 dropped it:
; "0.000000") and rounds its six decimals to nearest, ties to even,
; carrying into the integer part, instead of truncating (3.14159265...
; printed "3.141592"). Infinities and NaN print as the reference prints
; them. Still this kernel's fixed six-decimal format: shortest
; round-trip printing, as the reference does it, is a separate issue.
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
    db `(PRINT -0.0)`, 10  ; -0.000000
    db "(NEWLINE)", 10
    db `(PRINT (LIST 0.0 (PRINC-TO-STRING -0.0) (F- 0.0 0.0) (F* -1.0 0.0)))`, 10  ; (0.000000 -0.000000 0.000000 -0.000000)
    db "(NEWLINE)", 10
    db `(PRINT (LIST 3.14159265358979 0.9999999 -0.9999999 2.5 -1.25))`, 10  ; (3.141593 1.000000 -1.000000 2.500000 -1.250000)
    db "(NEWLINE)", 10
    db `(PRINT (LIST 0.0000015 0.0000005 0.1234565 123456789012.5))`, 10  ; (0.000002 0.000000 0.123456 123456789012.500000)
    db "(NEWLINE)", 10
    db `(PRINT (LIST 1e400 -1e400 (F- 1e400 1e400)))`, 10  ; (inf -inf NaN)
    db "(NEWLINE)", 10
    db `(PRINT (LIST 1e18 1e19))`, 10  ; (1000000000000000000.000000 10000000000000000000.000000)
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
