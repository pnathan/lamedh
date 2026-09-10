; 029_block_while — BLOCK/RETURN-FROM (derived from the same CATCH/
; THROW machinery HANDLER-CASE uses, tag = the block's own unevaluated
; name) and WHILE (a straightforward backward-branch loop), per
; KERNEL.md Part VI/VII.

%include "src/tags.inc"

extern reader_init
extern read_form
extern compile_thunk
extern print_fixnum
extern print_newline

section .rodata
; normal completion (no RETURN-FROM): last form's value.
e1: db "(BLOCK B 1 2 3)"                              ; 3

; RETURN-FROM exits early with a value, skipping the rest of the body.
e2: db "(BLOCK B (RETURN-FROM B 42) (PRINT 999))"       ; 42, no 999 printed
e2_len: equ $ - e2
e1_len: equ $ - e1

; RETURN-FROM with no value defaults to NIL.
e3: db "(IF (NULL (BLOCK B (RETURN-FROM B))) 111 222)"    ; 111
e3_len: equ $ - e3

; dynamic, not lexical: a function called from inside the BLOCK can
; RETURN-FROM it.
d1: db "(DEFINE ESCAPE (LAMBDA () (RETURN-FROM OUTER 77)))"
d1_len: equ $ - d1
e4: db "(BLOCK OUTER (ESCAPE) 999)"                         ; 77
e4_len: equ $ - e4

; nested BLOCKs: RETURN-FROM targets the NAMED block, not just the
; innermost one.
e5: db "(BLOCK A (BLOCK B (RETURN-FROM A 5) 999) 888)"        ; 5
e5_len: equ $ - e5

; WHILE: side effects each pass, always returns NIL.
d2: db "(DEFINE N 0)"
d2_len: equ $ - d2
d3: db "(DEFINE SUM 0)"
d3_len: equ $ - d3
d4: db "(WHILE (< N 5) (SETQ SUM (+ SUM N)) (SETQ N (+ N 1)))"
d4_len: equ $ - d4
e6: db "SUM"                                                    ; 0+1+2+3+4=10
e6_len: equ $ - e6
e7: db "(IF (NULL (WHILE (EQ 1 2) 999)) 111 222)"                 ; 111 (WHILE is NIL)
e7_len: equ $ - e7

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

run_and_print_fixnum:
    call reader_init
    call read_form
    mov rdi, rax
    call compile_thunk
    push rax
    call qword [rsp]
    add rsp, 8
    mov rdi, rax
    call print_fixnum
    call print_newline
    ret

global lamedh_main
lamedh_main:
    mov rdi, e1
    mov rsi, e1_len
    call run_and_print_fixnum        ; 3
    mov rdi, e2
    mov rsi, e2_len
    call run_and_print_fixnum        ; 42 (no 999 printed before it)
    mov rdi, e3
    mov rsi, e3_len
    call run_and_print_fixnum        ; 111
    mov rdi, d1
    mov rsi, d1_len
    call run_thunk_discard
    mov rdi, e4
    mov rsi, e4_len
    call run_and_print_fixnum        ; 77
    mov rdi, e5
    mov rsi, e5_len
    call run_and_print_fixnum        ; 5

    mov rdi, d2
    mov rsi, d2_len
    call run_thunk_discard
    mov rdi, d3
    mov rsi, d3_len
    call run_thunk_discard
    mov rdi, d4
    mov rsi, d4_len
    call run_thunk_discard
    mov rdi, e6
    mov rsi, e6_len
    call run_and_print_fixnum        ; 10
    mov rdi, e7
    mov rsi, e7_len
    call run_and_print_fixnum        ; 111

    xor rax, rax
    ret
