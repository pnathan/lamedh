; 041_call_stack_cleanup — a real, previously-latent bug: calling a
; function with more than 3 arguments left its stack-passed extra
; arguments (compile_call_args, positioned for the callee's own
; build_param_frame-addressed params) sitting on the target stack
; after the call returned. A callee's own `leave`/`ret` only unwinds
; what it pushed after its own `push rbp`, never the caller-pushed
; extra args sitting below the return address — cleaning those up is
; the *caller's* job, and neither of compile_call's two paths did it.
; Invisible as long as a >3-arg call's result was used immediately
; (the extra bytes just sat harmlessly below whatever came next), but
; corrupting the moment the *enclosing* expression had already pushed
; something of its own onto the stack for safekeeping around the call
; — compile_binop's own lhs, compile_binary_hostcall's arg1, and so on
; — since the callee's return landed with those bytes still occupying
; the slot the caller expected to pop its own saved value back from.
; `(CONS 'X (F a b c d))` for any 4+-arg F silently returned garbage
; instead of X as its car; lib/prelude.lisp's own FORMAT macro (whose
; expansion is exactly a CONS of a literal onto a call chain) is what
; surfaced this for real.

%include "src/tags.inc"

extern reader_init
extern read_form
extern compile_thunk
extern print_fixnum
extern print_newline

section .rodata
d1: db "(DEFINE FOUR (LAMBDA (A B C D) D))"
d1_len: equ $ - d1

; The exact failure shape: a >3-arg call as the second argument to
; CONS, whose first argument must survive the call untouched.
e1: db "(CAR (CONS 99 (CONS (FOUR 1 2 3 4) (QUOTE ()))))"        ; 99
e1_len: equ $ - e1

; A deeper case: two >3-arg calls nested inside binary ops, each with
; its own operand that must survive the other call.
d2: db "(DEFINE FIVE (LAMBDA (A B C D E) E))"
d2_len: equ $ - d2
e2: db "(+ (FOUR 1 2 3 100) (FIVE 1 2 3 4 200))"         ; 300
e2_len: equ $ - e2

; A repeated call from the same site, to confirm the fix doesn't
; corrupt the *next* call's own stack-passed arguments either (the
; cleanup amount is baked per call site, not shared global state).
e3: db "(+ (FOUR 1 2 3 10) (FOUR 1 2 3 20))"               ; 30
e3_len: equ $ - e3

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
    call run_and_print_fixnum        ; 99

    mov rdi, d2
    mov rsi, d2_len
    call run_thunk_discard
    mov rdi, e2
    mov rsi, e2_len
    call run_and_print_fixnum        ; 300

    mov rdi, e3
    mov rsi, e3_len
    call run_and_print_fixnum        ; 30

    xor rax, rax
    ret
