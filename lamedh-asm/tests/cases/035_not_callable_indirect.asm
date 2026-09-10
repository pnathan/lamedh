; 035_not_callable_indirect — the same controlled, catchable failure as
; 034_not_callable.asm, exercising compile_call's *other* path: a
; lexically bound local (LET) holding a non-closure value, called as
; if it were one. emit_check_callable (compiler.asm) guards both
; compile_call paths identically, and both now route through
; fail_wrong_type/native_throw (native_errors.asm) — the same real
; CATCH/HANDLER-CASE/ERRORSET-signaling machinery CAR/CDR's own
; wrong-type check already uses — instead of a separate, cruder hard
; exit(1). This is the ".indirect_path" one — a local variable or an
; immediately-invoked expression, as opposed to a named global reached
; through the self-patching inline cache.

%include "src/tags.inc"

extern reader_init
extern read_form
extern compile_thunk
extern print_fixnum
extern print_newline

section .rodata
d1: db "(PRINT (QUOTE BEFORE))"
d1_len: equ $ - d1
e1: db "(LET ((F 5)) (F 1))"
e1_len: equ $ - e1
; HANDLER-CASE catches this path's failure too — proving the
; ".indirect_path" call site's own check signals the identical real
; condition the named-global path does, not a separate mechanism.
e2: db "(PRINT (HANDLER-CASE (LET ((F 5)) (F 1)) (E (X) (QUOTE CAUGHT))))"
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

global lamedh_main
lamedh_main:
    mov rdi, d1
    mov rsi, d1_len
    call run_thunk_discard        ; prints "BEFORE"
    call print_newline

    mov rdi, e2
    mov rsi, e2_len
    call run_thunk_discard        ; CAUGHT
    call print_newline

    mov rdi, e1
    mov rsi, e1_len
    call run_thunk_discard        ; never returns: uncaught, reported on stderr and exit(1)

    ; unreachable
    mov rax, 99
    ret
