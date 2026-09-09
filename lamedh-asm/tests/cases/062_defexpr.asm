; 062_defexpr — DEFEXPR (Lisp 1.5's FEXPR, KERNEL.md/the reference's
; own SpecialForm::Defexpr): like DEFMACRO, receives its call's raw,
; unevaluated argument list — but unlike a macro, there is no separate
; expansion step recompiled in the call's place: the FEXPR's own body
; runs directly and its return value is the call's result. This host
; has no separate FEXPR representation: DEFEXPR is sugar over $VAU,
; building an Operative with a single named parameter (bound to the
; raw argument list) plus a second, auto-appended, never-referenced
; GENSYM parameter (matching $VAU/DEFVAU's own fixed 2-parameter
; shape) — needing no change to the operative-call check or
; emit_check_callable.
%include "src/tags.inc"

extern reader_init
extern read_form
extern compile_thunk
extern print_newline

section .rodata
e1: db "(DEFEXPR MY-FEXPR (X) (PRINT X) (NEWLINE) (QUOTE DONE))"
e1_len: equ $ - e1

e2: db "(PRINT (MY-FEXPR 1 (+ 2 3) UNBOUND-NAME))"
e2_len: equ $ - e2                                    ; (1 (+ 2 3) UNBOUND-NAME) then DONE

; the body's own explicit 1-argument EVAL, on a piece of the raw list —
; the reference's own README-documented Lisp-1.5 SELECT idiom
; (lib/09-lisp15.lisp), evaluating in this kernel's one global
; environment (see compile_defexpr's own comment for the scope this
; narrows relative to the reference's own caller-environment EVAL).
e3: db "(DEFINE GLOBAL-X 5)"
e3_len: equ $ - e3

e4: db "(DEFEXPR MY-SELECT (X) (IF (EQ (EVAL (CAR X)) 5) (QUOTE FIVE) (QUOTE OTHER)))"
e4_len: equ $ - e4

e5: db "(PRINT (MY-SELECT GLOBAL-X))"
e5_len: equ $ - e5                                    ; FIVE

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

global lamedh_main
lamedh_main:
    mov rdi, e1
    mov rsi, e1_len
    call run_thunk_discard

    mov rdi, e2
    mov rsi, e2_len
    call run_thunk_discard
    call print_newline               ; DONE

    mov rdi, e3
    mov rsi, e3_len
    call run_thunk_discard

    mov rdi, e4
    mov rsi, e4_len
    call run_thunk_discard

    mov rdi, e5
    mov rsi, e5_len
    call run_thunk_discard
    call print_newline               ; FIVE

    xor rax, rax
    ret
