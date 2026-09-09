; 048_char_literals — the Char value type (KERNEL.md Part II/IV): the
; `'x'` reader production, tried *before* quote sugar (a `'` followed
; by exactly one character or one \n \t \r \\ \' \0 escape, then a
; closing `'`); MAKE-CHAR/CHAR-CODE/CODE-CHAR; and that a Char and a
; numerically-equal fixnum are never EQ (Char and Number are distinct
; types, Part IV) even though CHAR-CODE deliberately bridges them.
;
; Disambiguation is the interesting part: `'a'` is the character `a`,
; but `'a` followed by a delimiter (no closing `'` right there) is
; ordinary quote sugar, `(QUOTE A)` — e2/e3 below are the spec's own
; two contrasting examples, `'(1)` reading as `(QUOTE (1))` (since `(`
; is followed by `1`, not `'`) while `'('` is the character `(`.

%include "src/tags.inc"

extern reader_init
extern read_form
extern compile_thunk
extern print_newline

section .rodata
; the char-literal production itself: a plain one-byte form, and every
; recognized escape, round-tripping through PRINT exactly.
e1: db "(PRINT 'a')"                                          ; 'a'
e1_len: equ $ - e1
e2: db "(PRINT 'a )"                                            ; A  (quote
e2_len: equ $ - e2                                               ; sugar: 'a followed by a delimiter)
e3: db "(PRINT '(1))"                                              ; (1) (quote
e3_len: equ $ - e3                                                  ; sugar: '( followed by '1', not a closing ')
e4: db "(PRINT '('))"                                                ; '(' (char
e4_len: equ $ - e4                                                    ; literal: '( followed by a closing ')
e5: db "(PRINT '\n')"                                                    ; '\n'
e5_len: equ $ - e5
e6: db "(PRINT '\\')"                                                     ; '\\'
e6_len: equ $ - e6
e7: db "(PRINT '\'')"                                                      ; '\''
e7_len: equ $ - e7
; an unrecognized escape decodes to that character alone, backslash
; dropped (the opposite convention from string escapes) — still a
; Char, so it still prints quoted, as 'q', not a bare q.
e8: db "(PRINT '\q')"                                                       ; 'q'
e8_len: equ $ - e8

; MAKE-CHAR/CHAR-CODE/CODE-CHAR.
e9:  db "(PRINT (CHAR-CODE 'a'))"                                             ; 97
e9_len:  equ $ - e9
e10: db "(PRINT (MAKE-CHAR 97))"                                                ; 'a'
e10_len: equ $ - e10
e11: db "(PRINT (CODE-CHAR 97))"                                                  ; a (a one-
e11_len: equ $ - e11                                                              ; character
                                                                                    ; string,
                                                                                    ; not a Char)

; Char and Number are distinct types (Part IV): EQ never conflates
; them, even at the same numeric value CHAR-CODE itself would produce.
e12: db "(PRINT (EQ 'a' 'a'))"                                                       ; T
e12_len: equ $ - e12
e13: db "(PRINT (EQ 'a' 97))"                                                          ; ()
e13_len: equ $ - e13
e14: db "(PRINT (EQ 'a' 'b'))"                                                           ; ()
e14_len: equ $ - e14

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

global lamedh_main
lamedh_main:
    mov rdi, e1
    mov rsi, e1_len
    call run_thunk_discard
    call print_newline               ; 'a'

    mov rdi, e2
    mov rsi, e2_len
    call run_thunk_discard
    call print_newline               ; A

    mov rdi, e3
    mov rsi, e3_len
    call run_thunk_discard
    call print_newline               ; (1)

    mov rdi, e4
    mov rsi, e4_len
    call run_thunk_discard
    call print_newline               ; '('

    mov rdi, e5
    mov rsi, e5_len
    call run_thunk_discard
    call print_newline               ; '\n'

    mov rdi, e6
    mov rsi, e6_len
    call run_thunk_discard
    call print_newline               ; '\\'

    mov rdi, e7
    mov rsi, e7_len
    call run_thunk_discard
    call print_newline               ; '\''

    mov rdi, e8
    mov rsi, e8_len
    call run_thunk_discard
    call print_newline               ; 'q'

    mov rdi, e9
    mov rsi, e9_len
    call run_thunk_discard
    call print_newline               ; 97

    mov rdi, e10
    mov rsi, e10_len
    call run_thunk_discard
    call print_newline               ; 'a'

    mov rdi, e11
    mov rsi, e11_len
    call run_thunk_discard
    call print_newline               ; a

    mov rdi, e12
    mov rsi, e12_len
    call run_thunk_discard
    call print_newline               ; T

    mov rdi, e13
    mov rsi, e13_len
    call run_thunk_discard
    call print_newline               ; ()

    mov rdi, e14
    mov rsi, e14_len
    call run_thunk_discard
    call print_newline               ; ()

    xor rax, rax
    ret
