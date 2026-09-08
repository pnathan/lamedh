; 020_hashtable — a hash table as *pure Lamedh library code*, not a new
; kernel primitive: an alist (list of (key . val) conses), built from
; nothing but CONS/CAR/CDR/EQ/NULLP/IF/DEFINE — every primitive this
; kernel already exposed for exactly this purpose (see README's "kernel
; surface" section and issue #452's kernel/library boundary). This is
; the deliberate demonstration that a hash table does *not* need to be
; a kernel primitive once list-processing exists in the language itself.
;
; Persistent, not mutated in place: HT-SET returns a *new* table (the
; old key . val pair consed on front, shadowing any earlier binding for
; the same key) rather than rewriting a cell — there is no RPLACD/
; SET-CDR! primitive in this kernel (captured/heap values aren't
; mutated anywhere in this project yet), so a functional table is what
; falls out of the primitives actually available, not a design
; preference for its own sake.

%include "src/tags.inc"

extern reader_init
extern read_form
extern compile_thunk
extern print_fixnum
extern print_newline

section .rodata
; The library itself — four small top-level DEFINEs.
d1: db "(DEFINE HT-EMPTY (QUOTE ()))"
d1_len: equ $ - d1

; HT-SET(ht, key, val) -> a new table with (key . val) shadowing
; anything already bound for key.
d2: db "(DEFINE HT-SET (LAMBDA (HT KEY VAL) (CONS (CONS KEY VAL) HT)))"
d2_len: equ $ - d2

; HT-ASSOC(ht, key) -> the first (key . val) pair for key, or NIL.
d3: db "(DEFINE HT-ASSOC (LAMBDA (HT KEY) (IF (NULLP HT) (QUOTE ()) (IF (EQ (CAR (CAR HT)) KEY) (CAR HT) (HT-ASSOC (CDR HT) KEY)))))"
d3_len: equ $ - d3

; HT-GET(ht, key) -> the bound value, or NIL if key is absent.
d4: db "(DEFINE HT-GET (LAMBDA (HT KEY) (IF (NULLP (HT-ASSOC HT KEY)) (QUOTE ()) (CDR (HT-ASSOC HT KEY)))))"
d4_len: equ $ - d4

; HT-HAS-KEY(ht, key) -> TRUE/NIL.
d5: db "(DEFINE HT-HAS-KEY (LAMBDA (HT KEY) (IF (NULLP (HT-ASSOC HT KEY)) (QUOTE ()) (QUOTE T))))"
d5_len: equ $ - d5

; Build a small table: 'A -> 1, 'B -> 2, then shadow 'A -> 99.
d6: db "(DEFINE T0 (HT-SET HT-EMPTY (QUOTE A) 1))"
d6_len: equ $ - d6
d7: db "(DEFINE T1 (HT-SET T0 (QUOTE B) 2))"
d7_len: equ $ - d7
d8: db "(DEFINE T2 (HT-SET T1 (QUOTE A) 99))"
d8_len: equ $ - d8

e1: db "(HT-GET T2 (QUOTE A))"           ; 99 (shadowed)
e1_len: equ $ - e1
e2: db "(HT-GET T2 (QUOTE B))"           ; 2
e2_len: equ $ - e2
e3: db "(IF (NULLP (HT-GET T2 (QUOTE C))) 111 222)"    ; 111 (missing key)
e3_len: equ $ - e3
e4: db "(IF (HT-HAS-KEY T2 (QUOTE A)) 1 0)"             ; 1
e4_len: equ $ - e4
e5: db "(IF (HT-HAS-KEY T2 (QUOTE C)) 1 0)"             ; 0
e5_len: equ $ - e5
; T0 (the earlier table, from before 'A was shadowed) is untouched —
; the whole point of a persistent structure: HT-SET never mutates.
e6: db "(HT-GET T0 (QUOTE A))"           ; 1, not 99
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
    mov rdi, d1
    mov rsi, d1_len
    call run_thunk_discard
    mov rdi, d2
    mov rsi, d2_len
    call run_thunk_discard
    mov rdi, d3
    mov rsi, d3_len
    call run_thunk_discard
    mov rdi, d4
    mov rsi, d4_len
    call run_thunk_discard
    mov rdi, d5
    mov rsi, d5_len
    call run_thunk_discard
    mov rdi, d6
    mov rsi, d6_len
    call run_thunk_discard
    mov rdi, d7
    mov rsi, d7_len
    call run_thunk_discard
    mov rdi, d8
    mov rsi, d8_len
    call run_thunk_discard

    mov rdi, e1
    mov rsi, e1_len
    call run_and_print_fixnum        ; 99
    mov rdi, e2
    mov rsi, e2_len
    call run_and_print_fixnum        ; 2
    mov rdi, e3
    mov rsi, e3_len
    call run_and_print_fixnum        ; 111
    mov rdi, e4
    mov rsi, e4_len
    call run_and_print_fixnum        ; 1
    mov rdi, e5
    mov rsi, e5_len
    call run_and_print_fixnum        ; 0
    mov rdi, e6
    mov rsi, e6_len
    call run_and_print_fixnum        ; 1

    xor rax, rax
    ret
