; 049_bitwise — LOGAND/LOGIOR/LOGXOR/LOGNOT/ASH (bitwise.asm), the
; reference's own integer bitwise operations, adapted to this kernel's
; 62-bit fixnum. LOGAND/LOGIOR/LOGXOR reuse the tagged-word trick
; tags.inc's own header describes for ADD/SUB (a fixnum's tag bits are
; already 00, so a plain AND/OR/XOR of the tagged words is already
; correctly tagged); LOGNOT and ASH need an untag/retag since a shift
; or a full bit-flip must operate on the value, not the tag-prefixed
; representation. v0 scope: LOGAND/LOGIOR/LOGXOR are fixed 2-operand,
; not the reference's own variadic fold (see bitwise.asm's own
; comment).

%include "src/tags.inc"

extern reader_init
extern read_form
extern compile_thunk
extern print_fixnum
extern print_newline

section .rodata
e1: db "(LOGAND 12 10)"                    ; 8   (1100 & 1010)
e1_len: equ $ - e1
e2: db "(LOGIOR 12 10)"                      ; 14  (1100 | 1010)
e2_len: equ $ - e2
e3: db "(LOGXOR 12 10)"                        ; 6   (1100 ^ 1010)
e3_len: equ $ - e3
e4: db "(LOGNOT 0)"                              ; -1
e4_len: equ $ - e4
e5: db "(LOGNOT 5)"                                ; -6
e5_len: equ $ - e5
e6: db "(ASH 1 4)"                                   ; 16 (left shift)
e6_len: equ $ - e6
e7: db "(ASH 16 -4)"                                   ; 1  (right shift)
e7_len: equ $ - e7
; arithmetic (sign-preserving) right shift: -1 stays -1 no matter how
; far right it shifts, same as the reference's own i64 >> semantics.
e8: db "(ASH -1 -1)"                                     ; -1
e8_len: equ $ - e8
; shift by 0 is a no-op, either direction.
e9: db "(ASH 7 0)"                                         ; 7
e9_len: equ $ - e9

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
    call run_and_print_fixnum        ; 8
    mov rdi, e2
    mov rsi, e2_len
    call run_and_print_fixnum        ; 14
    mov rdi, e3
    mov rsi, e3_len
    call run_and_print_fixnum        ; 6
    mov rdi, e4
    mov rsi, e4_len
    call run_and_print_fixnum        ; -1
    mov rdi, e5
    mov rsi, e5_len
    call run_and_print_fixnum        ; -6
    mov rdi, e6
    mov rsi, e6_len
    call run_and_print_fixnum        ; 16
    mov rdi, e7
    mov rsi, e7_len
    call run_and_print_fixnum        ; 1
    mov rdi, e8
    mov rsi, e8_len
    call run_and_print_fixnum        ; -1
    mov rdi, e9
    mov rsi, e9_len
    call run_and_print_fixnum        ; 7

    xor rax, rax
    ret
