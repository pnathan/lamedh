; 025_progn_cond_and_or — PROGN, COND, AND, OR (KERNEL.md Part VI/VII),
; plus multi-form LAMBDA bodies now that PROGN exists to wrap them
; (previously single-expression only).

%include "src/tags.inc"

extern reader_init
extern read_form
extern compile_thunk
extern print_fixnum
extern print_newline

section .rodata
; PROGN: side effects in order, value is the last form's.
e1: db "(PROGN 1 2 3)"                             ; 3
e1_len: equ $ - e1
e2: db "(PROGN)"                                     ; NIL -> printed via IF below
e2_len: equ $ - e2

; multi-form LAMBDA body: three PRINTs, last one's value returned.
e3: db "((LAMBDA (X) (PRINT X) (PRINT (* X 2)) (+ X 1)) 10)"
e3_len: equ $ - e3

; COND: first truthy clause wins; no match -> NIL; a clause with no
; body returns the test's own value.
e4: db "(COND ((EQ 1 2) 111) ((EQ 1 1) 222) (T 333))"     ; 222
e4_len: equ $ - e4
e5: db "(IF (NULL (COND ((EQ 1 2) 111))) 444 555)"           ; 444 (no match)
e5_len: equ $ - e5
e6: db "(COND (42))"                                           ; 42 (test with no body)
e6_len: equ $ - e6

; AND/OR
e7: db "(IF (AND) 1 0)"                              ; 1  ((AND) is T)
e7_len: equ $ - e7
e8: db "(AND 1 2 3)"                                   ; 3  (last value)
e8_len: equ $ - e8
e9: db "(IF (NULL (AND 1 (QUOTE ()) 3)) 111 222)"        ; 111 (short-circuits on NIL)
e9_len: equ $ - e9
e10: db "(IF (NULL (OR)) 111 222)"                          ; 111  ((OR) is NIL)
e10_len: equ $ - e10
e11: db "(OR (QUOTE ()) 5 6)"                                 ; 5  (first truthy)
e11_len: equ $ - e11

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

global lamedh_main
lamedh_main:
    mov rdi, e1
    mov rsi, e1_len
    call run_and_print_fixnum            ; 3

    mov rdi, e2
    mov rsi, e2_len
    call reader_init
    call read_form
    mov rdi, rax
    call compile_thunk
    push rax
    call qword [rsp]
    add rsp, 8
    cmp rax, IMM_NIL
    jne .fail
    mov rsi, ok_buf
    mov rdx, 3
    call write_buf
    call print_newline

    mov rdi, e3
    mov rsi, e3_len
    call run_and_print_fixnum            ; (PRINT 10)(PRINT 20) then 11

    mov rdi, e4
    mov rsi, e4_len
    call run_and_print_fixnum            ; 222
    mov rdi, e5
    mov rsi, e5_len
    call run_and_print_fixnum            ; 444
    mov rdi, e6
    mov rsi, e6_len
    call run_and_print_fixnum            ; 42

    mov rdi, e7
    mov rsi, e7_len
    call run_and_print_fixnum            ; 1
    mov rdi, e8
    mov rsi, e8_len
    call run_and_print_fixnum            ; 3
    mov rdi, e9
    mov rsi, e9_len
    call run_and_print_fixnum            ; 111
    mov rdi, e10
    mov rsi, e10_len
    call run_and_print_fixnum            ; 111
    mov rdi, e11
    mov rsi, e11_len
    call run_and_print_fixnum            ; 5

    xor rax, rax
    ret
.fail:
    mov rax, 1
    ret

extern write_buf
section .rodata
ok_buf: db "NIL"
