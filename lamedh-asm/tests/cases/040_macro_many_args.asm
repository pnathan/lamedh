; 040_macro_many_args — a macro call site with more than 3 syntactic
; operands, generalizing macro invocation the same way 033/038 already
; generalized &REST itself: invoke_macro (compiler.asm, replacing an
; earlier raw_args_to_regs+invoke_closure_host pair) forwards *every*
; operand form, not just the first 3 — operand index 3 onward is
; pushed onto the real host stack immediately before the call, in the
; same order compile_call_args' own target-code convention produces,
; so a transformer sees them at exactly the stack offsets
; build_param_frame already expects. This is what makes a multi-body-
; form DEFUN-style macro possible without the caller wrapping extra
; body forms in an explicit PROGN (previously required — see
; lib/prelude.lisp and the README).

%include "src/tags.inc"

extern reader_init
extern read_form
extern compile_thunk
extern print_fixnum
extern print_newline

section .rodata
; DEFUN2: (NAME PARAMS &REST BODY) — same shape as lib/prelude.lisp's
; own DEFUN, but invoked here with *four* operands: NAME, PARAMS, and
; two separate BODY forms, both of which must survive to the compiled
; LAMBDA's own implicitly-PROGN'd body.
d1: db "(DEFMACRO DEFUN2 (NAME PARAMS &REST BODY) (CONS (QUOTE DEFINE) (CONS NAME (CONS (CONS (QUOTE LAMBDA) (CONS PARAMS BODY)) (QUOTE ())))))"
d1_len: equ $ - d1

d2: db "(DEFUN2 F (X) (SETQ X (* X X)) (+ X 1))"
d2_len: equ $ - d2
e1: db "(F 5)"                                  ; (5*5)+1 = 26 — both
e1_len: equ $ - e1                              ; body forms ran, in
                                                 ; order, and the second
                                                 ; is the return value

; a six-operand macro call, to exercise more than one extra
; (stack-passed) operand at once, not just one.
d3: db "(DEFMACRO SIX (A B &REST REST6) (CONS (QUOTE LIST6HELPER) (CONS A (CONS B REST6))))"
d3_len: equ $ - d3
d4: db "(DEFINE LIST6HELPER (LAMBDA (A B C D E F2) (+ A (+ B (+ C (+ D (+ E F2)))))))"
d4_len: equ $ - d4
e2: db "(SIX 1 2 3 4 5 6)"                        ; 1+2+3+4+5+6 = 21
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
    call run_and_print_fixnum        ; 26

    mov rdi, d3
    mov rsi, d3_len
    call run_thunk_discard
    mov rdi, d4
    mov rsi, d4_len
    call run_thunk_discard
    mov rdi, e2
    mov rsi, e2_len
    call run_and_print_fixnum        ; 21

    xor rax, rax
    ret
