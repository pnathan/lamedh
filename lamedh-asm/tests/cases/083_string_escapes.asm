; 083_string_escapes — issue #551 item 4: string escapes are the
; reference's (reader.rs parse_string). \n \t \r \0 \" and a doubled
; backslash decode; any other backslash-prefixed character keeps its
; backslash, where it used to be dropped ("\a" read as "a").
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
    db `(PRINT (LIST (STRING-LENGTH "\\a") (STRING-LENGTH "\\q\\z") (STRING-REF "\\a" 0) (STRING-REF "\\a" 1)))`, 10  ; (2 4 92 97)
    db "(NEWLINE)", 10
    db `(PRINT (PRIN1-TO-STRING "\\a"))`, 10  ; "\\a"
    db "(NEWLINE)", 10
    db `(PRINT (LIST (STRING-REF "\\r" 0) (STRING-LENGTH "\\0") (STRING-REF "\\0" 0) (STRING-REF "\\n" 0) (STRING-REF "\\t" 0)))`, 10  ; (13 1 0 10 9)
    db "(NEWLINE)", 10
    db `(PRINT (LIST (STRING-REF "\\"" 0) (STRING-REF "\\\\" 0) (STRING-LENGTH "\\\\")))`, 10  ; (34 92 1)
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
