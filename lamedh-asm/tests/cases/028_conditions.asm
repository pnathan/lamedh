; 028_conditions — the condition system (KERNEL.md Part VIII): a
; two-field condition value (ERROR-P/ERROR-MESSAGE/ERROR-DATA),
; (ERROR) signaling with 0/1/2 arguments, and HANDLER-CASE catching
; unconditionally. Built entirely on the existing CATCH/THROW
; machinery via one shared internal tag (Part XII axis 3's own license
; to derive a special form this way) — no new signaling primitive.

%include "src/tags.inc"

extern reader_init
extern read_form
extern compile_thunk
extern print_fixnum
extern print_newline

section .rodata
; HANDLER-CASE around a form that doesn't error: normal value passes through.
e1: db "(HANDLER-CASE 42 (ERROR (E) 999))"
e1_len: equ $ - e1

; (ERROR) with no args, caught, message inspected.
e2: db "(HANDLER-CASE (ERROR) (ERROR (E) (ERROR-MESSAGE E)))"
e2_len: equ $ - e2

; (ERROR message data) with both, data inspected.
e3: db '(HANDLER-CASE (ERROR "boom" 7) (ERROR (E) (ERROR-DATA E)))'
e3_len: equ $ - e3

; (ERROR c) with c already a condition: re-signaled unchanged (message
; and data both survive) to the NEXT enclosing HANDLER-CASE, since the
; inner one's own catch frame is already popped by the time its
; handler body runs.
e4: db '(HANDLER-CASE (HANDLER-CASE (ERROR "first" 5) (ERROR (E) (ERROR E))) (ERROR (E2) (ERROR-DATA E2)))'
e4_len: equ $ - e4

; nesting: an ERROR inside the *protected form* of an outer
; HANDLER-CASE, from inside a nested LAMBDA call, is caught by the
; nearest enclosing HANDLER-CASE, not the outer one.
d1: db "(DEFINE BOOM (LAMBDA () (ERROR)))"
d1_len: equ $ - d1
e5: db "(HANDLER-CASE (HANDLER-CASE (BOOM) (ERROR (E) 111)) (ERROR (E) 222))"
e5_len: equ $ - e5    ; 111, caught by the INNER handler-case

; ERROR-P
e6: db '(HANDLER-CASE (ERROR "x") (ERROR (E) (IF (ERROR-P E) 1 0)))'
e6_len: equ $ - e6

; ERRORSET: success wraps in a list, CAR gives the value back.
e7: db "(CAR (ERRORSET (+ 1 2)))"
e7_len: equ $ - e7    ; 3
; ERRORSET: failure -> NIL
e8: db "(IF (NULL (ERRORSET (ERROR))) 111 222)"
e8_len: equ $ - e8

section .text

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

extern print_value
run_and_print_value:
    call reader_init
    call read_form
    mov rdi, rax
    call compile_thunk
    push rax
    call qword [rsp]
    add rsp, 8
    mov rdi, rax
    call print_value
    call print_newline
    ret

global lamedh_main
lamedh_main:
    mov rdi, e1
    mov rsi, e1_len
    call run_and_print_fixnum          ; 42

    mov rdi, e2
    mov rsi, e2_len
    call run_and_print_value           ; "Error"

    mov rdi, e3
    mov rsi, e3_len
    call run_and_print_fixnum          ; 7

    mov rdi, e4
    mov rsi, e4_len
    call run_and_print_fixnum          ; 5

    mov rdi, d1
    mov rsi, d1_len
    call reader_init
    call read_form
    mov rdi, rax
    call compile_thunk
    push rax
    call qword [rsp]
    add rsp, 8

    mov rdi, e5
    mov rsi, e5_len
    call run_and_print_fixnum          ; 111

    mov rdi, e6
    mov rsi, e6_len
    call run_and_print_fixnum          ; 1

    mov rdi, e7
    mov rsi, e7_len
    call run_and_print_fixnum          ; 3

    mov rdi, e8
    mov rsi, e8_len
    call run_and_print_fixnum          ; 111

    xor rax, rax
    ret
