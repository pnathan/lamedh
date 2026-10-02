; 080_dotted_pairs — issue #551 item 1: the reader reads dotted pairs.
; Before, '.' inside a list read as the symbol |.|, so '(A . (B C)) was
; the three-element (A |.| (B C)) and EQUAL to (A B C) was NIL, silently.
; Now a '.' starting an element after at least one element is the dot,
; as in the reference (reader.rs parse_list_contents); (. A), (A .),
; (A . B C), (A . B . C) and input ending inside a list are READ errors
; (catchable conditions, where the reference reports a parse error), and
; a READ-FROM-STRING error leaves the enclosing reader where it was: the
; forms after it are still read from this buffer. Every successful read
; here was checked against the Rust reference (target/release/lamedh).
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
    db `(PRINT (QUOTE (A . B)))`, 10  ; (A . B)
    db "(NEWLINE)", 10
    db `(PRINT (QUOTE (A . (B C))))`, 10  ; (A B C)
    db "(NEWLINE)", 10
    db `(PRINT (EQUAL (QUOTE (A . (B C))) (QUOTE (A B C))))`, 10  ; T
    db "(NEWLINE)", 10
    db `(PRINT (LIST (QUOTE (A B . C)) (QUOTE (A .B)) (CDR (QUOTE (1 . 2))) (QUOTE ((A . B) . (C . D)))))`, 10  ; ((A B . C) (A . B) 2 ((A . B) C . D))
    db "(NEWLINE)", 10
    db `(PRINT (LIST (QUOTE (A . (QUOTE B))) (QUOTE (A . ())) (QUOTE (A .\n B))))`, 10  ; ((A QUOTE B) (A) (A . B))
    db "(NEWLINE)", 10
    db `(PRINT (READ-FROM-STRING "(X . Y)"))`, 10  ; (X . Y)
    db "(NEWLINE)", 10
    db `(PRINT (LIST (HANDLER-CASE (READ-FROM-STRING "(. A)") (ERROR (E) (ERROR-MESSAGE E))) (HANDLER-CASE (READ-FROM-STRING "(A .)") (ERROR (E) (ERROR-MESSAGE E))) (HANDLER-CASE (READ-FROM-STRING "(A . B C)") (ERROR (E) (ERROR-MESSAGE E))) (HANDLER-CASE (READ-FROM-STRING "(A . B . C)") (ERROR (E) (ERROR-MESSAGE E)))))`, 10  ; (READ: malformed dotted list READ: malformed dotted list READ: malformed dotted list READ: malformed dotted list)
    db "(NEWLINE)", 10
    db `(PRINT (HANDLER-CASE (READ-FROM-STRING "((A B") (ERROR (E) (ERROR-MESSAGE E))))`, 10  ; READ: end of input inside a list
    db "(NEWLINE)", 10
    db `(PRINT (QUOTE AFTER))`, 10  ; AFTER
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
