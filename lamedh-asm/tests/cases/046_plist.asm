; 046_plist — the new symbol-plist kernel primitives, SYMBOL-PLIST and
; SET-SYMBOL-PLIST! (symtab.asm's new plist slot on every symbol, at
; offset 40 — shifting name bytes from 40 to 48). GETP/PUTP themselves
; are ordinary prelude library code built over these two plus CONS/
; CAR/CDR/EQ (lib/prelude.lisp) — not kernel primitives, so they are
; only reachable once lib/prelude.lisp is loaded (exercised instead via
; tests/run.sh's file_runner_prelude, the same way FORMAT/DOTIMES/
; MAPCAR/EQUAL already are), not from a standalone tests/cases/*.asm
; binary like this one, which links against the kernel core alone.
;
; This test also stands in as a regression check that ordinary symbol
; interning/printing/EQ still work correctly after the plist layout
; change: e5/e6 below re-run 039_t_self_bound-style checks on a
; different symbol.

%include "src/tags.inc"

extern reader_init
extern read_form
extern compile_thunk
extern print_newline

section .rodata
; a fresh symbol's plist starts out NIL.
e1: db "(PRINT (SYMBOL-PLIST (QUOTE FOO)))"                        ; ()
e1_len: equ $ - e1
; SET-SYMBOL-PLIST! overwrites the slot outright and returns the new
; value.
e2: db "(PRINT (SET-SYMBOL-PLIST! (QUOTE FOO) (QUOTE ((A . 1)))))" ; ((A . 1))
e2_len: equ $ - e2
; ...and the overwrite is visible on a later read (top-level forms
; here each run in a separate reader_init/read_form/compile_thunk
; call, but the symbol's own heap slot persists across them).
e3: db "(PRINT (SYMBOL-PLIST (QUOTE FOO)))"                            ; ((A . 1))
e3_len: equ $ - e3
; an unrelated symbol's plist is untouched.
e4: db "(PRINT (SYMBOL-PLIST (QUOTE BAR)))"                             ; ()
e4_len: equ $ - e4
; ordinary symbol identity/printing/EQ still hold after the layout
; change (name bytes moved from offset 40 to 48).
e5: db "(PRINT (EQ (QUOTE FOO) (QUOTE FOO)))"                             ; T
e5_len: equ $ - e5
e6: db "(PRINT (QUOTE FOO))"                                               ; FOO
e6_len: equ $ - e6

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
    call print_newline               ; ()

    mov rdi, e2
    mov rsi, e2_len
    call run_thunk_discard
    call print_newline               ; ((A . 1))

    mov rdi, e3
    mov rsi, e3_len
    call run_thunk_discard
    call print_newline               ; ((A . 1))

    mov rdi, e4
    mov rsi, e4_len
    call run_thunk_discard
    call print_newline               ; ()

    mov rdi, e5
    mov rsi, e5_len
    call run_thunk_discard
    call print_newline               ; T

    mov rdi, e6
    mov rsi, e6_len
    call run_thunk_discard
    call print_newline               ; FOO

    xor rax, rax
    ret
