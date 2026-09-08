; 022_hashtable_array — a *real* hash table, expected O(1) per
; operation, replacing the O(n) persistent alist of
; tests/cases/020_hashtable.asm. Still pure Lamedh library code, not a
; kernel primitive: it's built entirely on MAKE-ARRAY/ARRAY-REF/
; ARRAY-SET/HASH-CODE/MOD (021_arrays.asm) plus the same CONS/CAR/CDR/
; EQ/NULLP/IF this kernel already exposed for exactly this purpose —
; only the bucket array and the mutation primitive to write into it are
; new kernel surface; the hashing/bucketing/chaining *policy* is all
; DEFINE'd Lamedh, same as before.
;
; Design: a fixed-size bucket array (no resizing — a real hash table
; would grow it; this is the honestly-scoped v0, same spirit as every
; other v0 limit in this project); each bucket holds a short alist
; chain of (key . val) pairs for whatever hashed to it. HT-SET!
; *mutates* the table in place (ARRAY-SET on the bucket slot) — unlike
; the old alist version, this is no longer a persistent structure,
; because a real RPLACD-free hash table needs somewhere to mutate, and
; the bucket array is it.

%include "src/tags.inc"

extern reader_init
extern read_form
extern compile_thunk
extern print_fixnum
extern print_newline

section .rodata
d1: db "(DEFINE HT-NBUCKETS 61)"
d1_len: equ $ - d1

d2: db "(DEFINE HT-MAKE (LAMBDA () (MAKE-ARRAY HT-NBUCKETS)))"
d2_len: equ $ - d2

; walks one bucket's chain looking for KEY -> the (key . val) pair, or NIL
d3: db "(DEFINE HT-BUCKET-ASSOC (LAMBDA (BUCKET KEY) (IF (NULLP BUCKET) (QUOTE ()) (IF (EQ (CAR (CAR BUCKET)) KEY) (CAR BUCKET) (HT-BUCKET-ASSOC (CDR BUCKET) KEY)))))"
d3_len: equ $ - d3

; a copy of BUCKET with any existing binding for KEY dropped, so
; HT-SET! doesn't leak a duplicate on every update to the same key
d4: db "(DEFINE HT-BUCKET-REMOVE (LAMBDA (BUCKET KEY) (IF (NULLP BUCKET) (QUOTE ()) (IF (EQ (CAR (CAR BUCKET)) KEY) (HT-BUCKET-REMOVE (CDR BUCKET) KEY) (CONS (CAR BUCKET) (HT-BUCKET-REMOVE (CDR BUCKET) KEY))))))"
d4_len: equ $ - d4

d5: db "(DEFINE HT-INDEX (LAMBDA (KEY) (MOD (HASH-CODE KEY) HT-NBUCKETS)))"
d5_len: equ $ - d5

; HT-SET!(ht key val) -> val, mutating HT in place
d6: db "(DEFINE HT-SET! (LAMBDA (HT KEY VAL) (ARRAY-SET HT (HT-INDEX KEY) (CONS (CONS KEY VAL) (HT-BUCKET-REMOVE (ARRAY-REF HT (HT-INDEX KEY)) KEY)))))"
d6_len: equ $ - d6

d7: db "(DEFINE HT-GET (LAMBDA (HT KEY) (IF (NULLP (HT-BUCKET-ASSOC (ARRAY-REF HT (HT-INDEX KEY)) KEY)) (QUOTE ()) (CDR (HT-BUCKET-ASSOC (ARRAY-REF HT (HT-INDEX KEY)) KEY)))))"
d7_len: equ $ - d7

d8: db "(DEFINE HT-HAS-KEY (LAMBDA (HT KEY) (IF (NULLP (HT-BUCKET-ASSOC (ARRAY-REF HT (HT-INDEX KEY)) KEY)) (QUOTE ()) (QUOTE T))))"
d8_len: equ $ - d8

; --- exercise it ---
d9: db "(DEFINE HT (HT-MAKE))"
d9_len: equ $ - d9
d10: db "(HT-SET! HT (QUOTE A) 1)"
d10_len: equ $ - d10
d11: db "(HT-SET! HT (QUOTE B) 2)"
d11_len: equ $ - d11
d12: db "(HT-SET! HT (QUOTE A) 99)"           ; overwrite: mutates in place now
d12_len: equ $ - d12

e1: db "(HT-GET HT (QUOTE A))"                ; 99
e1_len: equ $ - e1
e2: db "(HT-GET HT (QUOTE B))"                ; 2
e2_len: equ $ - e2
e3: db "(IF (NULLP (HT-GET HT (QUOTE C))) 111 222)"    ; 111 (missing key)
e3_len: equ $ - e3
e4: db "(IF (HT-HAS-KEY HT (QUOTE A)) 1 0)"             ; 1
e4_len: equ $ - e4
e5: db "(IF (HT-HAS-KEY HT (QUOTE C)) 1 0)"             ; 0
e5_len: equ $ - e5

; insert enough keys to force at least one bucket collision (61
; buckets, 80 distinct fixnum keys sharing the table with the symbol
; keys above) and confirm several still read back correctly through
; their chain — the actual test of the hashing + chaining logic, not
; just the empty-table happy path. There's no PROGN yet (single-
; expression lambda bodies, see README), so "set, then recurse" is one
; IF exploiting HT-SET!'s own return value: it's always a fixnum
; (never EQ to NIL), so the true branch always runs.
d13: db "(DEFINE HT-FILL-LOOP (LAMBDA (HT N) (IF (EQ N 0) 0 (IF (HT-SET! HT N (* N N)) (HT-FILL-LOOP HT (- N 1)) 0))))"
d13_len: equ $ - d13
d14: db "(HT-FILL-LOOP HT 80)"
d14_len: equ $ - d14

e6: db "(HT-GET HT 80)"                        ; 6400
e6_len: equ $ - e6
e7: db "(HT-GET HT 1)"                            ; 1
e7_len: equ $ - e7
e8: db "(HT-GET HT 45)"                             ; 2025
e8_len: equ $ - e8
; the symbol keys inserted before the fill loop must still read back
; correctly — no accidental cross-talk between fixnum and symbol keys
; sharing the same bucket array (HASH-CODE + EQ, not just the bucket
; index, disambiguate them).
e9: db "(HT-GET HT (QUOTE A))"                        ; 99
e9_len: equ $ - e9

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

    mov rdi, d9
    mov rsi, d9_len
    call run_thunk_discard
    mov rdi, d10
    mov rsi, d10_len
    call run_thunk_discard
    mov rdi, d11
    mov rsi, d11_len
    call run_thunk_discard
    mov rdi, d12
    mov rsi, d12_len
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

    mov rdi, d13
    mov rsi, d13_len
    call run_thunk_discard
    mov rdi, d14
    mov rsi, d14_len
    call run_thunk_discard

    mov rdi, e6
    mov rsi, e6_len
    call run_and_print_fixnum        ; 6400
    mov rdi, e7
    mov rsi, e7_len
    call run_and_print_fixnum        ; 1
    mov rdi, e8
    mov rsi, e8_len
    call run_and_print_fixnum        ; 2025
    mov rdi, e9
    mov rsi, e9_len
    call run_and_print_fixnum        ; 99

    xor rax, rax
    ret
