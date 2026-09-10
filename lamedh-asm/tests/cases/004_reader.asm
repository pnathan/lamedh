; 004_reader — parses proper lists, negative and positive integers,
; case-normalized symbol interning, and 'quote sugar.

%include "src/tags.inc"

extern reader_init
extern read_form
extern car
extern cdr
extern intern_symbol
extern print_fixnum
extern print_newline

section .rodata
buf1: db "(1 2 -3)"
buf1_len: equ $ - buf1
buf2: db "foo"
buf2_len: equ $ - buf2
buf3: db "FOO"
buf3_len: equ $ - buf3
buf4: db "'x"
buf4_len: equ $ - buf4
sym_quote: db "QUOTE"

section .text
global lamedh_main
lamedh_main:
    push r12
    push r13
    push r14

    ; --- (1 2 -3) ---
    mov rdi, buf1
    mov rsi, buf1_len
    call reader_init
    call read_form
    mov r12, rax                   ; the list

    mov rdi, r12
    call car
    mov rdi, rax
    call print_fixnum               ; 1
    call print_newline

    mov rdi, r12
    call cdr
    mov r12, rax
    mov rdi, r12
    call car
    mov rdi, rax
    call print_fixnum                ; 2
    call print_newline

    mov rdi, r12
    call cdr
    mov r12, rax
    mov rdi, r12
    call car
    mov rdi, rax
    call print_fixnum                 ; -3
    call print_newline

    mov rdi, r12
    call cdr                          ; should be NIL
    xor rdi, rdi
    cmp rax, IMM_NIL
    sete dil
    TO_FIXNUM rdi
    call print_fixnum                  ; 1
    call print_newline

    ; --- case-normalized interning: "foo" and "FOO" intern the same ---
    mov rdi, buf2
    mov rsi, buf2_len
    call reader_init
    call read_form
    mov r13, rax

    mov rdi, buf3
    mov rsi, buf3_len
    call reader_init
    call read_form
    mov r14, rax

    xor rdi, rdi
    cmp r13, r14
    sete dil
    TO_FIXNUM rdi
    call print_fixnum                   ; 1
    call print_newline

    ; --- 'x reads as (QUOTE X) ---
    mov rdi, buf4
    mov rsi, buf4_len
    call reader_init
    call read_form
    mov r12, rax

    mov rdi, r12
    call car                            ; should be the QUOTE symbol
    mov r13, rax
    mov rdi, sym_quote
    mov rsi, 5
    call intern_symbol
    xor rdi, rdi
    cmp r13, rax
    sete dil
    TO_FIXNUM rdi
    call print_fixnum                    ; 1
    call print_newline

    xor rax, rax
    pop r14
    pop r13
    pop r12
    ret
