; 035_not_callable_indirect — the same controlled failure as
; 034_not_callable.asm, exercising compile_call's *other* path: a
; lexically bound local (LET) holding a non-closure value, called as
; if it were one. emit_check_callable (compiler.asm) guards both
; compile_call paths identically; this is the ".indirect_path" one —
; a local variable or an immediately-invoked expression, as opposed to
; a named global reached through the self-patching inline cache.

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

    mov rdi, e1
    mov rsi, e1_len
    call run_thunk_discard        ; never returns: exit(1)

    ; unreachable
    mov rax, 99
    ret
