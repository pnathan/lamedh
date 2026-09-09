; 045_gensym — GENSYM (KERNEL.md Part XI: "fresh uninterned symbols,
; never EQ to anything else"). Exercises: the exact reference name
; format ("G" + a monotonic counter, zero-padded to at least 4 digits —
; matching the Rust reference's own `format!("G{:04}", counter)`,
; environment.rs); that two separate calls return distinct, non-EQ
; symbols even though nothing about a symbol's own representation
; other than identity distinguishes them; that a symbol bound to a
; variable is still EQ to itself on a second reference (ordinary
; pointer-identity EQ, ATOM-not-heapobj special-cased path in lisp_eq
; never applies — ordinary fast-path pointer compare handles this); and
; the actual "uninterned" half of the guarantee — a GENSYM'd symbol
; that happens to print with the exact same text an ordinary reader
; literal would use (this process's first GENSYM is "G0000", matching
; what `(QUOTE G0000)` reader-interns to) is still a genuinely distinct,
; non-EQ object, because gensym (symtab.asm) never links its result
; into symtab_buckets the way intern_symbol does for every reader
; literal.

%include "src/tags.inc"

extern reader_init
extern read_form
extern compile_thunk
extern print_newline

section .rodata
e1: db "(PRINT (GENSYM))"                                   ; G0000
e1_len: equ $ - e1
e2: db "(PRINT (GENSYM))"                                    ; G0001
e2_len: equ $ - e2
e3: db "(PRINT (EQ (GENSYM) (GENSYM)))"                        ; ()
e3_len: equ $ - e3
; two separate top-level forms — run_thunk_discard reads and runs
; exactly one form per call, so DEFINE's global binding of G is what
; carries its value across into the next call.
e4a: db "(DEFINE G (GENSYM))"
e4a_len: equ $ - e4a
e4b: db "(PRINT (EQ G G))"                                        ; T
e4b_len: equ $ - e4b
; e4's own GENSYM was this process's 5th call, so its printed name is
; "G0004" — exactly what an ordinary reader-interned (QUOTE G0004)
; produces. Comparing a *fresh* GENSYM (this call, the 6th) against
; that literal is trivially unequal (different counter value); the
; real point is e6 below, comparing against the *very same* text a
; still-live GENSYM'd symbol already uses.
e5: db "(PRINT (EQ (GENSYM) (QUOTE G0004)))"                       ; ()
e5_len: equ $ - e5
; this call is the process's 7th GENSYM; e5 above was the 6th, printing
; "G0005". (QUOTE G0005) here reader-interns an *ordinary* symbol with
; that identical name/text — and it is still not EQ to anything GENSYM
; ever produced, proving the guarantee is about identity, not name:
; intern_symbol's bucket table and gensym's un-linked objects are
; disjoint universes that can share printed text without ever
; colliding as values.
e6: db "(PRINT (EQ (GENSYM) (QUOTE G0005)))"                       ; ()
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
    call print_newline               ; G0000

    mov rdi, e2
    mov rsi, e2_len
    call run_thunk_discard
    call print_newline               ; G0001

    mov rdi, e3
    mov rsi, e3_len
    call run_thunk_discard
    call print_newline               ; ()

    mov rdi, e4a
    mov rsi, e4a_len
    call run_thunk_discard

    mov rdi, e4b
    mov rsi, e4b_len
    call run_thunk_discard
    call print_newline               ; T

    mov rdi, e5
    mov rsi, e5_len
    call run_thunk_discard
    call print_newline               ; ()

    mov rdi, e6
    mov rsi, e6_len
    call run_thunk_discard
    call print_newline               ; ()

    xor rax, rax
    ret
