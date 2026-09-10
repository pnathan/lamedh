; 003_symtab_intern — interning the same name twice yields the same
; pointer (EQ), interning a different name yields a different pointer,
; and the value cell starts out unbound.

%include "src/tags.inc"

extern intern_symbol
extern print_fixnum
extern print_newline

section .rodata
name_foo: db "FOO"
name_foo2: db "FOO"
name_bar: db "BAR"

section .text
global lamedh_main
lamedh_main:
    push r12
    push r13

    mov rdi, name_foo
    mov rsi, 3
    call intern_symbol
    mov r12, rax                  ; FOO (first interning)

    mov rdi, name_foo2
    mov rsi, 3
    call intern_symbol
    mov r13, rax                  ; FOO (second interning, different buffer)

    xor rdi, rdi
    cmp r12, r13
    sete dil
    TO_FIXNUM rdi
    call print_fixnum              ; expect 1 (EQ)
    call print_newline

    mov rdi, name_bar
    mov rsi, 3
    call intern_symbol             ; BAR

    xor rdi, rdi
    cmp r12, rax
    sete dil
    TO_FIXNUM rdi
    call print_fixnum               ; expect 0 (not EQ)
    call print_newline

    ; value cell (at [ptr+16], stripping the heapobj tag) starts unbound
    mov rax, r12
    UNTAG_PTR rax
    mov rdi, [rax+16]
    cmp rdi, IMM_UNBOUND
    sete dil
    movzx rdi, dil
    TO_FIXNUM rdi
    call print_fixnum                ; expect 1
    call print_newline

    xor rax, rax
    pop r13
    pop r12
    ret
