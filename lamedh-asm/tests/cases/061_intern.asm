; 061_intern — INTERN and STRING-LENGTH*: two genuine Rust-level
; builtins (environment.rs) surfaced by lib/27-modules.lisp's own
; $MODULE-QUALIFY (`(intern (concat ...))`) and STRING-INDEX-OF
; (`(string-length* s)`, lib/14-strings.lisp) respectively.
;
; INTERN on a String uppercases into a local scratch buffer first
; (matching the reference's own `s.to_uppercase()`) then interns —
; `(intern "foo")` and `(intern "FOO")` must both yield the same
; symbol as a bareword `FOO` read from source, and `(eq ...)` must see
; that. INTERN on a Symbol returns it unchanged (already interned,
; under its own existing name).
;
; STRING-LENGTH* is the reference's own actual name for this
; primitive (environment.rs registers only "STRING-LENGTH*", never a
; bare "STRING-LENGTH") — this kernel answers to both spellings now,
; the same host routine either way.
%include "src/tags.inc"

extern reader_init
extern read_form
extern compile_thunk
extern print_newline

section .rodata
e1: db '(PRINT (EQ (INTERN "foo") (QUOTE FOO)))'
e1_len: equ $ - e1                                    ; T

e2: db '(PRINT (EQ (INTERN "FOO") (QUOTE FOO)))'
e2_len: equ $ - e2                                      ; T

e3: db "(PRINT (EQ (INTERN (QUOTE FOO)) (QUOTE FOO)))"
e3_len: equ $ - e3                                        ; T

e4: db '(PRINT (STRING-LENGTH* "hello"))'
e4_len: equ $ - e4                                          ; 5

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
    call print_newline               ; T

    mov rdi, e2
    mov rsi, e2_len
    call run_thunk_discard
    call print_newline               ; T

    mov rdi, e3
    mov rsi, e3_len
    call run_thunk_discard
    call print_newline               ; T

    mov rdi, e4
    mov rsi, e4_len
    call run_thunk_discard
    call print_newline               ; 5

    xor rax, rax
    ret
