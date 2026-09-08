; 012_catch_throw — the non-local-exit primitive: THROW walks the catch
; stack for an EQ tag match and unwinds rbp/rsp to that frame's saved
; state before jumping to its resume point — an ordinary longjmp, real
; stack-frame surgery, not a Rust-style Result unwind.

%include "src/tags.inc"

extern reader_init
extern read_form
extern compile_thunk
extern print_fixnum
extern print_newline

section .rodata
; no throw at all: CATCH just returns its body's value
e1: db "(CATCH (QUOTE TAG) 42)"
e1_len: equ $ - e1

; a throw caught by the immediately enclosing CATCH
e2: db "(CATCH (QUOTE TAG) (+ 1 (THROW (QUOTE TAG) 99)))"
e2_len: equ $ - e2

; a throw from inside a function call, unwinding through it — proves
; this is real stack surgery, not just a compile-time-local branch
d1: db "(DEFINE ESCAPE (LAMBDA (X) (THROW (QUOTE OUTER) (* X X))))"
d1_len: equ $ - d1
e3: db "(CATCH (QUOTE OUTER) (+ 1000 (ESCAPE 7)))"
e3_len: equ $ - e3

; nested CATCH: the inner one has the wrong tag, so THROW skips it and
; is caught by the outer one instead
e4: db "(CATCH (QUOTE A) (+ 1 (CATCH (QUOTE B) (THROW (QUOTE A) 7))))"
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
    mov rdi, e1
    mov rsi, e1_len
    call run_and_print_fixnum        ; 42

    mov rdi, e2
    mov rsi, e2_len
    call run_and_print_fixnum          ; 99 — the (+ 1 ...) never runs

    mov rdi, d1
    mov rsi, d1_len
    call run_thunk_discard

    mov rdi, e3
    mov rsi, e3_len
    call run_and_print_fixnum            ; 49 — the (+ 1000 ...) never runs

    mov rdi, e4
    mov rsi, e4_len
    call run_and_print_fixnum              ; 7 — caught by A, not B

    xor rax, rax
    ret
