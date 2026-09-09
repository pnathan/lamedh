; 039_t_self_bound — the bare symbol T, evaluated as an ordinary
; variable reference, must be bound to itself (KERNEL.md Part IV: "T
; is an ordinary interned symbol, bound to itself in the global
; environment"). This is a *different* value from the separate
; IMM_TRUE immediate EQ/comparisons return (024_print_readable.asm's
; own "T (IMM_TRUE)" case) — bare T is the reader-interned HDR_SYMBOL
; heapobj named "T", which had no binding at all before
; bootstrap_globals (symtab.asm, called from boot.asm) existed: every
; prior test exercised only the IMM_TRUE path, so evaluating the bare
; symbol as a variable fell through print_value's dispatch to
; print_fixnum on its unbound cell's raw bits (IMM_UNBOUND, 15),
; printing the fixnum 3 instead of "T".

%include "src/tags.inc"

extern reader_init
extern read_form
extern compile_thunk
extern print_newline

section .rodata
e1: db "(PRINT T)"                          ; T
e1_len: equ $ - e1
; not a keyword and not unrebindable in *this* kernel yet (no such
; check exists), but still self-bound afterward for this test's
; purposes: exercising it as an ordinary LAMBDA parameter/return value.
e2: db "(PRINT ((LAMBDA (X) X) T))"           ; T
e2_len: equ $ - e2
e3: db "(PRINT (EQ T T))"                       ; T (same interned
e3_len: equ $ - e3                              ; symbol both times)

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
    call print_newline               ; T

    mov rdi, e2
    mov rsi, e2_len
    call run_thunk_discard
    call print_newline               ; T

    mov rdi, e3
    mov rsi, e3_len
    call run_thunk_discard
    call print_newline               ; T

    xor rax, rax
    ret
