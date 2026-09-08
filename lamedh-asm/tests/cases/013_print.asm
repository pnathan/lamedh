; 013_print — the first program whose output comes from *compiled
; Lamedh code itself* calling PRINT, not from the hand-written test
; driver calling print_fixnum directly. Every earlier test printed via
; the driver; this is the first proof compiled code can produce output
; at all.

%include "src/tags.inc"

extern reader_init
extern read_form
extern compile_thunk

section .rodata
; PRINT returns its argument, so this whole expression's own value is
; 42 too — the driver checks that independently of what got printed.
e1: db "(PRINT 42)"
e1_len: equ $ - e1

; PRINT + NEWLINE composed in one function; DEFMACRO's UNLESS-style
; derived forms would build straight on this.
e2: db "((LAMBDA (X) (PRINT (* X X))) 7)"
e2_len: equ $ - e2

section .text
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
    ; rax should be 42 (PRINT's own return value); use it as the exit
    ; code so the harness checks both the printed text and the result.
    push rax

    mov rdi, e2
    mov rsi, e2_len
    call reader_init
    call read_form
    mov rdi, rax
    call compile_thunk
    push rax
    call qword [rsp]
    add rsp, 8
    mov rdi, rax
    pop rax
    ; Both PRINT results are *tagged* fixnums (value<<2), not raw ints.
    cmp rax, 42 << 2
    jne .fail
    cmp rdi, 49 << 2
    jne .fail
    xor rax, rax
    ret
.fail:
    mov rax, 1
    ret
