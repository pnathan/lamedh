; 038_macro_rest_below3 — a DEFMACRO whose own parameter list has fewer
; than 3 fixed params plus &REST (nfixed<3, lifted in the &REST fix —
; see 033_rest_below3.asm), invoked as a macro. This exercises a path
; 033's own tests didn't: macro invocation goes through a *separate*
; host-side mechanism (raw_args_to_regs/invoke_closure_host,
; compiler.asm), distinct from compile_call's own compiled-code calling
; convention, and it had its own bug independent of the &REST fix
; itself — invoke_closure_host never set the incoming nargs (target
; rax) at all before calling the transformer, so a &REST-taking
; transformer's own register-fold logic (which decides whether a
; register slot holds real REST data by checking nargs at runtime) read
; whatever host-side garbage happened to be in rax, not the real
; argument count. lib/prelude.lisp's own DEFUN macro
; ((NAME PARAMS &REST BODY), nfixed=2) is exactly this shape and is
; what surfaced the bug in the first place.

%include "src/tags.inc"

extern reader_init
extern read_form
extern compile_thunk
extern print_fixnum
extern print_newline

section .rodata
; MYDEFUN: (NAME PARAMS &REST BODY), nfixed=2 (below the old
; nfixed>=3 &REST restriction), invoked as a macro with exactly 3
; operands (NAME, PARAMS, one BODY form) — within raw_args_to_regs's
; own separate 3-operand forwarding cap (see README Roadmap).
d1: db "(DEFMACRO MYDEFUN (NAME PARAMS &REST BODY) (CONS (QUOTE DEFINE) (CONS NAME (CONS (CONS (QUOTE LAMBDA) (CONS PARAMS BODY)) (QUOTE ())))))"
d1_len: equ $ - d1

d2: db "(MYDEFUN SQUARE (X) (* X X))"
d2_len: equ $ - d2
e1: db "(SQUARE 9)"                          ; 81
e1_len: equ $ - e1

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
    call run_and_print_fixnum        ; 81

    xor rax, rax
    ret
