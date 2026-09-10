; 027_setq — SETQ per KERNEL.md Part VI: assigns each var left to
; right, writing to its first lexically bound slot (LET/LET*/LAMBDA
; param/free) if there is one, else its global value cell (no dynamic
; variables exist in this kernel yet, so that half of the spec's
; resolution rule doesn't apply). Leaves the last value assigned.

%include "src/tags.inc"

extern reader_init
extern read_form
extern compile_thunk
extern print_fixnum
extern print_newline

section .rodata
; global SETQ: no prior DEFINE, still writes the symbol's global cell.
e1: db "(SETQ G 100)"
e1_len: equ $ - e1
e2: db "G"
e2_len: equ $ - e2

; lexical SETQ inside a LET, visible after the SETQ within the same
; LET, and the mutation is what a later read sees.
e3: db "(LET ((X 1)) (SETQ X (+ X 1)) X)"          ; 2
e3_len: equ $ - e3

; SETQ of a LAMBDA param, visible to a later expression in the body.
e4: db "((LAMBDA (N) (SETQ N (* N 10)) N) 3)"          ; 30
e4_len: equ $ - e4

; a SETQ'd LET binding stays mutated across a nested LET (frame_lookup
; walks the whole current_scope chain, not just the innermost level).
e5: db "(LET ((X 1)) (LET ((Y 2)) (SETQ X 99)) X)"        ; 99
e5_len: equ $ - e5

; multiple var/val pairs, left to right, later ones see earlier writes.
e6: db "(LET ((X 1) (Y 2)) (SETQ X 10 Y X) Y)"               ; 10
e6_len: equ $ - e6

; SETQ's own value is the last val assigned.
e7: db "(LET ((X 0)) (SETQ X 7))"                              ; 7
e7_len: equ $ - e7

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
    mov rdi, e2
    mov rsi, e2_len
    call run_and_print_fixnum        ; 100

    mov rdi, e3
    mov rsi, e3_len
    call run_and_print_fixnum        ; 2
    mov rdi, e4
    mov rsi, e4_len
    call run_and_print_fixnum        ; 30
    mov rdi, e5
    mov rsi, e5_len
    call run_and_print_fixnum        ; 99
    mov rdi, e6
    mov rsi, e6_len
    call run_and_print_fixnum        ; 10
    mov rdi, e7
    mov rsi, e7_len
    call run_and_print_fixnum        ; 7

    xor rax, rax
    ret
