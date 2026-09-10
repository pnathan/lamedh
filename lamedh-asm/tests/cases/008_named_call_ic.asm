; 008_named_call_ic — a DEFINE'd global function, called by name from two
; different call sites. Each call site's inline cache resolves and
; self-patches independently on its first hit; calling the SAME site
; twice proves the patched direct call still produces the right answer
; the second time (i.e. patching didn't corrupt anything).

%include "src/tags.inc"

extern reader_init
extern read_form
extern compile_thunk
extern print_fixnum
extern print_newline

section .rodata
d1: db "(DEFINE SQUARE (LAMBDA (X) (* X X)))"
d1_len: equ $ - d1
e1: db "(SQUARE 7)"
e1_len: equ $ - e1
e2: db "(SQUARE 8)"
e2_len: equ $ - e2
e3: db "(+ (SQUARE 3) (SQUARE 4))"
e3_len: equ $ - e3

section .text

run_and_print_fixnum:
    push rbx
    call reader_init
    call read_form
    mov rdi, rax
    call compile_thunk
    mov rbx, rax
    call rbx
    mov rdi, rax
    call print_fixnum
    call print_newline
    pop rbx
    ret

global lamedh_main
lamedh_main:
    mov rdi, d1
    mov rsi, d1_len
    call reader_init
    call read_form
    mov rdi, rax
    call compile_thunk
    mov rbx, rax
    call rbx

    ; Compiled Lisp functions follow no callee-saved convention at all —
    ; every register is fair game as scratch (that is the whole point:
    ; no register discipline forced on us by a C ABI). A caller that
    ; needs a value to survive a call into compiled code must keep it on
    ; the stack, never in a register — exactly as the compiler itself
    ; already does for every Lisp-level intermediate value (compile_binop
    ; pushes, it never trusts a register across a nested call). Holding
    ; the thunk pointer on the stack here, not in rbx, respects that.
    mov rdi, e1
    mov rsi, e1_len
    call reader_init
    call read_form
    mov rdi, rax
    call compile_thunk
    push rax                             ; [thunk_ptr]
    call qword [rsp]                     ; first hit: resolves + self-patches
    mov rdi, rax
    call print_fixnum
    call print_newline                    ; 49

    call qword [rsp]                     ; same call site, now patched direct
    mov rdi, rax
    call print_fixnum
    call print_newline                     ; 49 again
    add rsp, 8

    mov rdi, e2
    mov rsi, e2_len
    call run_and_print_fixnum         ; 64

    mov rdi, e3
    mov rsi, e3_len
    call run_and_print_fixnum          ; 9+16=25, two call sites in one thunk

    xor rax, rax
    ret
