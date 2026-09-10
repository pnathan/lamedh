; 010_list_builtins — CAR/CDR/CONS/EQ/ATOM/NULL reachable from compiled
; Lamedh source for the first time (previously only the reader and
; compiler themselves could call the underlying host routines).

%include "src/tags.inc"

extern reader_init
extern read_form
extern compile_thunk
extern print_fixnum
extern print_newline

section .rodata
e1: db "(CAR (CONS 1 2))"
e1_len: equ $ - e1
e2: db "(CDR (CONS 1 2))"
e2_len: equ $ - e2
e3: db "(CAR (CDR (CONS 1 (CONS 2 3))))"
e3_len: equ $ - e3
e4: db "(EQ 1 1)"
e4_len: equ $ - e4
e5: db "(EQ 1 2)"
e5_len: equ $ - e5
e6: db "(ATOM 1)"
e6_len: equ $ - e6
e7: db "(ATOM (CONS 1 2))"
e7_len: equ $ - e7
e8: db "(NULL (CDR (CONS 1 (QUOTE ()))))"
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

; prints 1 if not NIL, else 0
run_and_print_bool:
    call reader_init
    call read_form
    mov rdi, rax
    call compile_thunk
    push rax
    call qword [rsp]
    add rsp, 8
    xor rdi, rdi
    cmp rax, IMM_NIL
    setne dil
    TO_FIXNUM rdi
    call print_fixnum
    call print_newline
    ret

global lamedh_main
lamedh_main:
    mov rdi, e1
    mov rsi, e1_len
    call run_and_print_fixnum    ; 1

    mov rdi, e2
    mov rsi, e2_len
    call run_and_print_fixnum      ; 2

    mov rdi, e3
    mov rsi, e3_len
    call run_and_print_fixnum        ; 2

    mov rdi, e4
    mov rsi, e4_len
    call run_and_print_bool            ; 1

    mov rdi, e5
    mov rsi, e5_len
    call run_and_print_bool              ; 0

    mov rdi, e6
    mov rsi, e6_len
    call run_and_print_bool                ; 1

    mov rdi, e7
    mov rsi, e7_len
    call run_and_print_bool                  ; 0

    mov rdi, e8
    mov rsi, e8_len
    call run_and_print_bool                    ; 1

    xor rax, rax
    ret
