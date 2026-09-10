; 036_string_ops — STRING-REF, STRING-APPEND, SUBSTRING: the small
; extra string surface FORMAT (and any real text processing) needs
; beyond STRING-LENGTH/PRINT — see README Roadmap. STRING-REF returns
; a byte's numeric value as a fixnum (this kernel has no Char type
; yet, v0); STRING-APPEND and SUBSTRING return fresh strings, since
; strings are immutable here like every other value.

%include "src/tags.inc"

extern reader_init
extern read_form
extern compile_thunk
extern print_fixnum
extern print_newline

section .rodata
d1: db '(DEFINE S "abcde")'
d1_len: equ $ - d1
e1: db "(STRING-REF S 0)"                     ; 97 ('a')
e1_len: equ $ - e1
e2: db "(STRING-REF S 4)"                     ; 101 ('e')
e2_len: equ $ - e2

e3: db '(PRINT (STRING-APPEND "foo" "bar"))'   ; foobar
e3_len: equ $ - e3
d2: db '(DEFINE T2 (STRING-APPEND "ab" ""))'   ; append with an empty string
d2_len: equ $ - d2
e4: db "(STRING-LENGTH T2)"                     ; 2
e4_len: equ $ - e4
e5: db "(PRINT T2)"                               ; ab
e5_len: equ $ - e5

e6: db '(PRINT (SUBSTRING S 1 3))'              ; bc
e6_len: equ $ - e6
e7: db '(PRINT (SUBSTRING S 0 0))'                ; (empty string, no output)
e7_len: equ $ - e7
e8: db "(STRING-LENGTH (SUBSTRING S 0 5))"         ; 5 (the whole string)
e8_len: equ $ - e8

; STRING-APPEND is non-destructive: S itself is unchanged afterward.
e9: db "(STRING-LENGTH S)"                          ; 5
e9_len: equ $ - e9

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
    call run_and_print_fixnum        ; 97
    mov rdi, e2
    mov rsi, e2_len
    call run_and_print_fixnum        ; 101

    mov rdi, e3
    mov rsi, e3_len
    call run_thunk_discard
    call print_newline               ; foobar

    mov rdi, d2
    mov rsi, d2_len
    call run_thunk_discard
    mov rdi, e4
    mov rsi, e4_len
    call run_and_print_fixnum        ; 2
    mov rdi, e5
    mov rsi, e5_len
    call run_thunk_discard
    call print_newline               ; ab

    mov rdi, e6
    mov rsi, e6_len
    call run_thunk_discard
    call print_newline               ; bc
    mov rdi, e7
    mov rsi, e7_len
    call run_thunk_discard
    call print_newline               ; (empty)
    mov rdi, e8
    mov rsi, e8_len
    call run_and_print_fixnum        ; 5

    mov rdi, e9
    mov rsi, e9_len
    call run_and_print_fixnum        ; 5

    xor rax, rax
    ret
