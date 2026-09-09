; 043_function_sharp_quote — (FUNCTION x) and its #'x reader sugar
; (KERNEL.md Part II/VI): compiling FUNCTION's operand directly, the
; same way any other expression position already would, gives the
; right answer for both spellings this kernel needs — a bare symbol
; compiles as the ordinary local-or-global variable read compile_form's
; own atom case already does (there is no separate function namespace
; here), and #'(LAMBDA ...) compiles as an ordinary LAMBDA. Lets a
; global closure be passed as a higher-order value, e.g. to REDUCE
; (lib/prelude.lisp) — examples/factorial/main.lisp's own
; (reduce #'* (iota n 1) 1) is exactly this.

%include "src/tags.inc"

extern reader_init
extern read_form
extern compile_thunk
extern print_fixnum
extern print_newline

section .rodata
d1: db "(DEFINE SQ (LAMBDA (X) (* X X)))"
d1_len: equ $ - d1
e1: db "((FUNCTION SQ) 7)"                  ; 49
e1_len: equ $ - e1
e2: db "(#'SQ 7)"                             ; 49 (reader sugar)
e2_len: equ $ - e2
e4: db "((FUNCTION (LAMBDA (X) (+ X 1))) 5)"      ; 6
e4_len: equ $ - e4

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

    mov rdi, e1
    mov rsi, e1_len
    call run_and_print_fixnum        ; 49
    mov rdi, e2
    mov rsi, e2_len
    call run_and_print_fixnum        ; 49
    mov rdi, e4
    mov rsi, e4_len
    call run_and_print_fixnum        ; 6

    xor rax, rax
    ret
