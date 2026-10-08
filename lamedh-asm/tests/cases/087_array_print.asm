; 087_array_print — arrays print their contents (KERNEL.md Part III,
; issues #527 / #594 / #607): "#(e1 ... en)", "#()" when empty, elements
; abridged after 100 with the unreadable marker "#<...N more>", a
; back-reference to an array already being printed as "#<circular-array>",
; and typed arrays as the non-readable "#<typed-array:TYPE e1 ... en>".
; Also the "#(...)" array literal reader production (elements read, not
; evaluated; a dotted tail or an unterminated literal is a catchable
; read error). Expected text was checked against the Rust reference binary;
; the only deliberate difference is float elements, which this host prints
; in its own fixed-decimal float format (see 054_typed_arrays).

; tests/cases/*.asm binaries load no prelude, so this one incbins
; lib/prelude.lisp (LIST->ARRAY, IOTA, LIST) and runs it, then the program
; below, as 086_iota does.
%include "src/tags.inc"

extern reader_init
extern read_form
extern compile_thunk
extern print_newline

section .rodata
prelude_start:
    incbin "lib/prelude.lisp"
prelude_end:
prelude_len: equ prelude_end - prelude_start

prog_start:
    db `(PRINT (LIST->ARRAY (LIST 1 2 3)))`, 10
    db "(NEWLINE)", 10
    db `(PRINT (ARRAY 0))`, 10
    db "(NEWLINE)", 10
    db `(PRINT (LIST->ARRAY (LIST "a" (QUOTE B) (QUOTE (1 2)) (LIST->ARRAY (LIST 4)))))`, 10
    db "(NEWLINE)", 10
    db `(PRINT (PRIN1-TO-STRING (LIST->ARRAY (LIST "a"))))`, 10
    db "(NEWLINE)", 10
    db `(PRINT (PRINC-TO-STRING (LIST->ARRAY (LIST "a"))))`, 10
    db "(NEWLINE)", 10
    db `(PRINT (QUOTE #(1 (A B) "s")))`, 10
    db "(NEWLINE)", 10
    db `(PRINT (QUOTE #()))`, 10
    db "(NEWLINE)", 10
    db `(PRINT (ARRAYP (QUOTE #(1))))`, 10
    db "(NEWLINE)", 10
    db `(PRINT (ARRAY-LENGTH* (QUOTE #(A B C D))))`, 10
    db "(NEWLINE)", 10
    db `(PRINT (FETCH (QUOTE #((+ 1 2))) 0))`, 10
    db "(NEWLINE)", 10
    db `(PRINT (ARRAYP (FETCH (QUOTE #(#(1))) 0)))`, 10
    db "(NEWLINE)", 10
    db `(PRINT (READ-FROM-STRING "#(1 #(2) (3))"))`, 10
    db "(NEWLINE)", 10
    db `(PRINT (READ-FROM-STRING (PRIN1-TO-STRING (LIST->ARRAY (LIST 1 "x" (LIST 2 3) (LIST->ARRAY (LIST 4)))))))`, 10
    db "(NEWLINE)", 10
    db `(PRINT (LIST (ARRAY 2) "q"))`, 10
    db "(NEWLINE)", 10
    db `(PRINT (HANDLER-CASE (READ-FROM-STRING "#(1 . 2)") (E (X) (QUOTE CAUGHT))))`, 10
    db "(NEWLINE)", 10
    db `(PRINT (HANDLER-CASE (READ-FROM-STRING "#(1 2") (E (X) (QUOTE CAUGHT))))`, 10
    db "(NEWLINE)", 10
    db `(PRINT (HANDLER-CASE (READ-FROM-STRING "# (1 2)") (E (X) (QUOTE CAUGHT))))`, 10
    db "(NEWLINE)", 10
    db `(PRINT (LIST->ARRAY (IOTA 150)))`, 10
    db "(NEWLINE)", 10
    db `(PRINT (LIST->ARRAY (IOTA 100)))`, 10
    db "(NEWLINE)", 10
    db `(PRINT (HANDLER-CASE (READ-FROM-STRING (PRIN1-TO-STRING (LIST->ARRAY (IOTA 150)))) (E (X) (QUOTE CAUGHT))))`, 10
    db "(NEWLINE)", 10
    db `(DEFINE C (ARRAY 2))`, 10
    db `(STORE C 0 C)`, 10
    db `(PRINT C)`, 10
    db "(NEWLINE)", 10
    db `(PRINT C)`, 10
    db "(NEWLINE)", 10
    db `(DEFINE D (ARRAY 2))`, 10
    db `(STORE D 1 (LIST 1 D))`, 10
    db `(PRINT D)`, 10
    db "(NEWLINE)", 10
    db `(DEFINE S (QUOTE #(1)))`, 10
    db `(PRINT (LIST->ARRAY (LIST S S)))`, 10
    db "(NEWLINE)", 10
    db `(DEFINE T1 (TYPED-ARRAY 3 (QUOTE INT64)))`, 10
    db `(STORE T1 1 7)`, 10
    db `(PRINT T1)`, 10
    db "(NEWLINE)", 10
    db `(PRINT (TYPED-ARRAY 0 (QUOTE INT64)))`, 10
    db "(NEWLINE)", 10
    db `(PRINT (TYPED-ARRAY 0 (QUOTE FLOAT64)))`, 10
    db "(NEWLINE)", 10
    db `(PRINT (TYPED-ARRAY 101 (QUOTE INT64)))`, 10
    db "(NEWLINE)", 10
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
