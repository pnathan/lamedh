; 017_strings — the string value type: reader support for "..." literals
; (HDR_STRING heapobj, self-evaluating exactly like a fixnum literal —
; no compiler change was needed for that part), PRINT dispatching on the
; runtime tag (string bytes vs. a fixnum's decimal value), and
; STRING-LENGTH.

%include "src/tags.inc"

extern reader_init
extern read_form
extern compile_thunk
extern print_newline

section .rodata
; PRINT of a string writes its raw bytes, no quoting, and (like the
; fixnum case) returns the string itself.
e1: db '(PRINT "hello")'
e1_len: equ $ - e1

; Escapes: \n and \" both need to survive the reader intact.
e2: db '(PRINT "a', 92, 'nb', 92, '"c")'
e2_len: equ $ - e2

; STRING-LENGTH
d1: db '(DEFINE S "abcde")'
d1_len: equ $ - d1
e3: db "(STRING-LENGTH S)"
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

extern print_fixnum
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
    call reader_init
    call read_form
    mov rdi, rax
    call compile_thunk
    push rax
    call qword [rsp]
    add rsp, 8
    call print_newline

    mov rdi, e2
    mov rsi, e2_len
    call reader_init
    call read_form
    mov rdi, rax
    call compile_thunk
    push rax
    call qword [rsp]
    add rsp, 8
    call print_newline

    mov rdi, d1
    mov rsi, d1_len
    call run_thunk_discard

    mov rdi, e3
    mov rsi, e3_len
    call run_and_print_fixnum        ; 5

    xor rax, rax
    ret
