; 037_eval_read_princ — EVAL, READ-FROM-STRING, and PRINC-TO-STRING
; (KERNEL.md Part XI's reflection primitives). EVAL is compile_thunk
; plus one indirect call, exposed to *compiled* Lamedh code for the
; first time; READ-FROM-STRING saves and restores the reader's own
; global buf/pos/end around its own use (reader.asm), since it can be
; called from code the top-level file-runner loop is itself mid-way
; through reading — the critical case this test exercises directly:
; (EVAL (READ-FROM-STRING ...)) must not disturb this test harness's
; own reader_init/read_form loop over its *own* literal buffers, each
; a completely separate reader_init call from the one
; read_from_string_tagged uses internally and restores afterward.

%include "src/tags.inc"

extern reader_init
extern read_form
extern compile_thunk
extern print_fixnum
extern print_newline

section .rodata
e1: db "(EVAL (QUOTE (+ 1 2)))"                       ; 3
e1_len: equ $ - e1
e2: db '(EVAL (READ-FROM-STRING "(+ 3 4)"))'            ; 7
e2_len: equ $ - e2

; PRINC-TO-STRING: renders a value the same way PRINT would, into a
; fresh string, for a symbol, a fixnum, and a string (unquoted, its
; "aesthetic" convention).
e4: db "(PRINT (PRINC-TO-STRING (QUOTE FOO)))"            ; FOO
e4_len: equ $ - e4
e5: db '(PRINT (PRINC-TO-STRING 42))'                       ; 42
e5_len: equ $ - e5
e6: db '(PRINT (PRINC-TO-STRING "hi"))'                       ; hi
e6_len: equ $ - e6

; the critical nesting case: EVAL+READ-FROM-STRING runs while this
; test's own harness is between two of its own separate reader_init
; calls (one per literal below) — proving read_from_string_tagged's
; own save/restore doesn't leak into the *next* literal read here.
d1: db '(EVAL (READ-FROM-STRING "(DEFINE Y 99)"))'
d1_len: equ $ - d1
e3: db "Y"                                                ; 99 — reads
e3_len: equ $ - e3                                        ; correctly
                                                           ; from *this*
                                                           ; test's own
                                                           ; next literal,
                                                           ; not leftover
                                                           ; reader state

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
    call run_and_print_fixnum        ; 3

    mov rdi, e2
    mov rsi, e2_len
    call run_and_print_fixnum        ; 7

    mov rdi, e4
    mov rsi, e4_len
    call run_thunk_discard
    call print_newline                ; FOO
    mov rdi, e5
    mov rsi, e5_len
    call run_thunk_discard
    call print_newline                ; 42
    mov rdi, e6
    mov rsi, e6_len
    call run_thunk_discard
    call print_newline                ; hi

    mov rdi, d1
    mov rsi, d1_len
    call run_thunk_discard
    mov rdi, e3
    mov rsi, e3_len
    call run_and_print_fixnum        ; 99

    xor rax, rax
    ret
