; 044_eq_value_equality — EQ on strings and floats is value equality
; (KERNEL.md Part IV), not pointer equality: two freshly computed,
; unshared values holding the same content must be EQ. compile_eq used
; to emit a bare tagged-value compare, which is correct for fixnums/
; characters/symbols/every immediate (the tagged bits *are* the value)
; but silently wrong for two separately heap-allocated strings or
; floats with identical content — exactly the gap examples/fizzbuzz/
; main.lisp's own self-check exercised (EQUAL, over EQ at the leaves,
; comparing two independently-built lists of strings). Also covers the
; spec's own explicit float carve-outs: (eq 0.0 -0.0) is T (IEEE ==
; says they're equal) and (eq nan nan) is T (the explicit carve-out
; beyond plain IEEE ==, where NaN == NaN is normally false).

%include "src/tags.inc"

extern reader_init
extern read_form
extern compile_thunk
extern print_newline
extern print_fixnum

section .rodata
; two independently-read string literals with the same bytes
e1: db '(IF (EQ "hi" "hi") 111 222)'                   ; 111
e1_len: equ $ - e1

; a literal vs. one built at runtime
e2: db '(IF (EQ "ab" (STRING-APPEND "a" "b")) 111 222)'  ; 111
e2_len: equ $ - e2

; different content is still correctly unequal
e3: db '(IF (EQ "ab" "ac") 111 222)'                       ; 222
e3_len: equ $ - e3
e4: db '(IF (EQ "ab" "abc") 111 222)'                        ; 222 (prefix,
e4_len: equ $ - e4                                           ; different length)

; two independently-read float literals with the same value
e5: db "(IF (EQ 3.5 3.5) 111 222)"                             ; 111
e5_len: equ $ - e5
; 0.0 and -0.0: IEEE == says equal
e6: db "(IF (EQ 0.0 -0.0) 111 222)"                              ; 111
e6_len: equ $ - e6
; NaN carve-out: (/ 0.0 0.0) is NaN; NaN is EQ to NaN here
e7: db "(IF (EQ (F/ 0.0 0.0) (F/ 0.0 0.0)) 111 222)"                ; 111
e7_len: equ $ - e7
; distinct float values remain unequal
e8: db "(IF (EQ 3.5 4.5) 111 222)"                                    ; 222
e8_len: equ $ - e8

; a string and a float (or a string and a fixnum) are never EQ,
; regardless of content — different types never are (KERNEL.md Part IV).
e9: db "(IF (EQ 3.5 3) 111 222)"                                        ; 222
e9_len: equ $ - e9

section .text

run_thunk_print_result:
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
    call run_thunk_print_result        ; 111
    mov rdi, e2
    mov rsi, e2_len
    call run_thunk_print_result        ; 111
    mov rdi, e3
    mov rsi, e3_len
    call run_thunk_print_result        ; 222
    mov rdi, e4
    mov rsi, e4_len
    call run_thunk_print_result        ; 222
    mov rdi, e5
    mov rsi, e5_len
    call run_thunk_print_result        ; 111
    mov rdi, e6
    mov rsi, e6_len
    call run_thunk_print_result        ; 111
    mov rdi, e7
    mov rsi, e7_len
    call run_thunk_print_result        ; 111
    mov rdi, e8
    mov rsi, e8_len
    call run_thunk_print_result        ; 222
    mov rdi, e9
    mov rsi, e9_len
    call run_thunk_print_result        ; 222

    xor rax, rax
    ret
