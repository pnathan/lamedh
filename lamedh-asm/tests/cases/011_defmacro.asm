; 011_defmacro — the reflective primitive: a macro transformer is an
; ordinary compiled closure, invoked directly from host code at compile
; time with unevaluated argument forms, producing a new form that is
; recompiled in its place. UNLESS derives IF exactly the way it would in
; any real Lisp — no new special form, no kernel change.

%include "src/tags.inc"

extern reader_init
extern read_form
extern compile_thunk
extern print_fixnum
extern print_newline

section .rodata
; (UNLESS test body) => (IF test NIL body)
d1: db "(DEFMACRO UNLESS (TEST BODY) (CONS (QUOTE IF) (CONS TEST (CONS (QUOTE ()) (CONS BODY (QUOTE ()))))))"
d1_len: equ $ - d1
e1: db "(UNLESS (EQ 1 2) 99)"
e1_len: equ $ - e1
e2: db "(UNLESS (EQ 1 1) 99)"
e2_len: equ $ - e2

; (SQUARE-OF x) => (* x x), exercising a macro whose parameter is reused
; twice in the expansion
d2: db "(DEFMACRO SQUARE-OF (X) (CONS (QUOTE *) (CONS X (CONS X (QUOTE ())))))"
d2_len: equ $ - d2
e3: db "(SQUARE-OF 7)"
e3_len: equ $ - e3

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

; prints 1 if not NIL, else 0
run_and_print_bool:
    call reader_init
    call read_form
    mov rdi, rax
    call compile_thunk
    push rax
    call qword [rsp]
    add rsp, 8
    xor rdi, rdi
    cmp rax, IMM_NIL
    setne dil
    TO_FIXNUM rdi
    call print_fixnum
    call print_newline
    ret

global lamedh_main
lamedh_main:
    mov rdi, d1
    mov rsi, d1_len
    call run_thunk_discard

    mov rdi, e1
    mov rsi, e1_len
    call run_and_print_fixnum        ; 1 != 2, so UNLESS runs body -> 99

    mov rdi, e2
    mov rsi, e2_len
    call run_and_print_bool            ; 1 == 2 is false... wait EQ(1,1) is
                                        ; true, so UNLESS suppresses body -> NIL

    mov rdi, d2
    mov rsi, d2_len
    call run_thunk_discard

    mov rdi, e3
    mov rsi, e3_len
    call run_and_print_fixnum            ; 7*7 = 49

    xor rax, rax
    ret
