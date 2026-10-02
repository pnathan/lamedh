; 070_apply_not_callable — APPLY on a non-function is the same
; catchable "not a function" condition 034/035 pin for an ordinary call
; site, not a SIGSEGV (#544). APPLY compiles to a hostcall into
; invoke_macro, which used to untag whatever value it was handed and
; `call [raw+8]` straight through it: a fixnum, NIL or a string jumped
; to garbage and died as "fatal: SIGSEGV - most likely the native stack
; is exhausted", which no HANDLER-CASE could catch. invoke_macro now
; checks the tag and header first (HDR_CLOSURE or HDR_OPERATIVE), and
; resolves a symbol to its function binding — its global value cell,
; this host being a Lisp-1 — so (APPLY 'CAR ...) calls CAR.
;
; e1..e3 are caught (ERROR-MESSAGE / ERROR-DATA of the real condition);
; e4 is the symbol case (a DEFINEd function: in this bare kernel,
; without lib/prelude.lisp, CAR is a compiler primitive with no value
; binding — (APPLY 'CAR ...) itself is a file_runner_errors case); e5 is uncaught: one line on stderr and exit 1
; (report_unhandled_throw, native_errors.asm; the .exitcode file).
; FUNCALL is lib/prelude.lisp sugar over APPLY, so it is covered by
; tests/run.sh's file_runner_errors cases, which load the prelude.

%include "src/tags.inc"

extern reader_init
extern read_form
extern compile_thunk
extern print_newline

section .rodata
e1: db "(PRINT (HANDLER-CASE (APPLY 5 (QUOTE ())) (E (X) (CONS (ERROR-MESSAGE X) (ERROR-DATA X)))))"
e1_len: equ $ - e1
e2: db "(PRINT (HANDLER-CASE (APPLY (QUOTE ()) (QUOTE ())) (E (X) (QUOTE CAUGHT-NIL))))"
e2_len: equ $ - e2
e3: db "(PRINT (HANDLER-CASE (APPLY ", 34, "s", 34, " (QUOTE ())) (E (X) (QUOTE CAUGHT-STRING))))"
e3_len: equ $ - e3
d1: db "(DEFINE FIRST1 (LAMBDA (X) (CAR X)))"
d1_len: equ $ - d1
e4: db "(PRINT (APPLY (QUOTE FIRST1) (QUOTE ((1)))))"
e4_len: equ $ - e4
e5: db "(APPLY 5 (QUOTE ()))"
e5_len: equ $ - e5

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
    call run_thunk_discard        ; (not a function . 5)
    call print_newline

    mov rdi, e2
    mov rsi, e2_len
    call run_thunk_discard        ; CAUGHT-NIL
    call print_newline

    mov rdi, e3
    mov rsi, e3_len
    call run_thunk_discard        ; CAUGHT-STRING
    call print_newline

    mov rdi, d1
    mov rsi, d1_len
    call run_thunk_discard

    mov rdi, e4
    mov rsi, e4_len
    call run_thunk_discard        ; 1
    call print_newline

    mov rdi, e5
    mov rsi, e5_len
    call run_thunk_discard        ; never returns: uncaught, exit 1

    ; unreachable
    mov rax, 99
    ret
