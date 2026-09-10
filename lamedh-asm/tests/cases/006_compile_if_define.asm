; 006_compile_if_define — DEFINE stores into a symbol's global value cell
; at a fixed, compile-time-known address; a later reference to that name
; loads from the same fixed address. IF is a compile-time-backpatched
; branch. Each form here is compiled and run independently, exactly as
; a batch/REPL driver would do it one top-level form at a time.

%include "src/tags.inc"

extern reader_init
extern read_form
extern compile_thunk
extern print_fixnum
extern print_newline

section .rodata
d1: db "(DEFINE X 10)"
d1_len: equ $ - d1
e1: db "(IF (< X 5) 111 222)"
e1_len: equ $ - e1
e2: db "(+ X 5)"
e2_len: equ $ - e2
e3: db "(IF (= X 10) 1 0)"
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
    call run_and_print_fixnum          ; DEFINE returns the value: 10

    mov rdi, e1
    mov rsi, e1_len
    call run_and_print_fixnum           ; X=10, not <5, so 222

    mov rdi, e2
    mov rsi, e2_len
    call run_and_print_fixnum            ; 10+5=15

    mov rdi, e3
    mov rsi, e3_len
    call run_and_print_fixnum             ; X==10 -> 1

    xor rax, rax
    ret
