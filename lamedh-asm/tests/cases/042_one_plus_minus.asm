; 042_one_plus_minus — the reader's "1+"/"1-" two-character literal
; symbol production (KERNEL.md Part II: tried before ordinary number
; parsing, no boundary guard — "1+x" reads as the symbol 1+ followed
; by X). examples/factorial/main.lisp uses (1+ i) directly. Also
; exercises the exact regression this fix's own first (buggy) draft
; introduced: the lookahead byte used to decide "1+/1- or plain
; number" must never leak into the fallback classification for an
; ordinary leading-'1' number (a bare "1", "15", "1)", ...), which an
; earlier version of this fix broke by clobbering al with the
; lookahead character instead of a separate register.

%include "src/tags.inc"

extern reader_init
extern read_form
extern compile_thunk
extern print_fixnum
extern print_newline

section .rodata
e1: db "(PRINT (QUOTE 1+))"
e1_len: equ $ - e1
e2: db "(PRINT (QUOTE 1-))"
e2_len: equ $ - e2

d1: db "(DEFINE 1+ (LAMBDA (N) (+ N 1)))"
d1_len: equ $ - d1
e3: db "(1+ 5)"                             ; 6
e3_len: equ $ - e3

d2: db "(DEFINE 1- (LAMBDA (N) (- N 1)))"
d2_len: equ $ - d2
e4: db "(1- 5)"                             ; 4
e4_len: equ $ - e4

; ordinary leading-'1' numbers, unaffected by the 1+/1- check above.
e5: db "1"                                    ; 1
e5_len: equ $ - e5
e6: db "15"                                     ; 15
e6_len: equ $ - e6
e7: db "(+ 1 1)"                                  ; 2 (a bare "1" at the
e7_len: equ $ - e7                                ; very end of a list,
                                                   ; immediately before ')')
e8: db "100"                                        ; 100
e8_len: equ $ - e8

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
    call run_thunk_discard
    call print_newline               ; 1+

    mov rdi, e2
    mov rsi, e2_len
    call run_thunk_discard
    call print_newline               ; 1-

    mov rdi, d1
    mov rsi, d1_len
    call run_thunk_discard
    mov rdi, e3
    mov rsi, e3_len
    call run_and_print_fixnum        ; 6

    mov rdi, d2
    mov rsi, d2_len
    call run_thunk_discard
    mov rdi, e4
    mov rsi, e4_len
    call run_and_print_fixnum        ; 4

    mov rdi, e5
    mov rsi, e5_len
    call run_and_print_fixnum        ; 1
    mov rdi, e6
    mov rsi, e6_len
    call run_and_print_fixnum        ; 15
    mov rdi, e7
    mov rsi, e7_len
    call run_and_print_fixnum        ; 2
    mov rdi, e8
    mov rsi, e8_len
    call run_and_print_fixnum        ; 100

    xor rax, rax
    ret
