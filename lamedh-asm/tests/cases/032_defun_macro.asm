; 032_defun_macro — DEFUN as pure library code over DEFMACRO/DEFINE/
; LAMBDA, no compiler change: the concrete first step toward running
; ../examples/*/main.lisp (README Roadmap), since every one of those
; programs uses DEFUN. Scoped to a single body form for now, exactly
; the shape KERNEL.md Part VI gives DEFEXPR/DEFMACRO themselves ("name
; params [docstring] body — the body is a single form"): a multi-form
; body still needs an explicit PROGN, since a variadic
; (NAME PARAMS . BODY) macro parameter list would need &REST with only
; 2 fixed params, which this compiler doesn't support yet (nfixed>=3 —
; see README).

%include "src/tags.inc"

extern reader_init
extern read_form
extern compile_thunk
extern print_fixnum
extern print_newline

section .rodata
; (DEFUN NAME PARAMS BODY) => (DEFINE NAME (LAMBDA PARAMS BODY))
d1: db "(DEFMACRO DEFUN (NAME PARAMS BODY) (CONS (QUOTE DEFINE) (CONS NAME (CONS (CONS (QUOTE LAMBDA) (CONS PARAMS (CONS BODY (QUOTE ())))) (QUOTE ())))))"
d1_len: equ $ - d1

d2: db "(DEFUN SQUARE (X) (* X X))"
d2_len: equ $ - d2
e1: db "(SQUARE 7)"                          ; 49
e1_len: equ $ - e1

; a recursive DEFUN'd function, multi-form body via an explicit PROGN
d3: db "(DEFUN FACT (N) (IF (< N 2) 1 (* N (FACT (- N 1)))))"
d3_len: equ $ - d3
e2: db "(FACT 5)"                              ; 120
e2_len: equ $ - e2

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
    mov rdi, d1
    mov rsi, d1_len
    call run_thunk_discard

    mov rdi, d2
    mov rsi, d2_len
    call run_thunk_discard
    mov rdi, e1
    mov rsi, e1_len
    call run_and_print_fixnum        ; 49

    mov rdi, d3
    mov rsi, d3_len
    call run_thunk_discard
    mov rdi, e2
    mov rsi, e2_len
    call run_and_print_fixnum        ; 120

    xor rax, rax
    ret
