; 063_records_and_macro_free_vars — two independent kernel-level fixes
; found while chasing lib/25-variants.lisp's DEFVARIANT through the
; reference standard library. Deliberately uses only kernel-primitive
; special forms (CONS/CAR/CDR/QUOTE/DEFUN/DEFMACRO/LAMBDA/DEFINE/PRINT/
; RECORD-NEW/RECORD-BRAND/RECORD-FIELDS/EQ) — no LIST, no FUNCALL, no
; MAPCAR: this file, like every other tests/cases/*.asm, runs with no
; prelude.lisp loaded, only the bare compiled kernel.
;
; 1. RECORD-NEW/RECORD-BRAND/RECORD-FIELDS (tags.inc's HDR_RECORD): a
;    fixed-size, brand-tagged tuple backing lib/20-condensation.lisp's
;    DEFRECORD and lib/25-variants.lisp's DEFVARIANT. RECORD-NEW takes
;    a compile-time-constant-length variadic field list (compile_
;    record_new, compiler.asm) fully unrolled at compile time — no
;    runtime loop needed since the field count is a Lisp-source
;    constant at every real call site (a generated constructor).
;
; 2. scan_free_vars (compiler.asm) is now macro-aware: a nested LAMBDA
;    whose body is a macro CALL (not a literal reference) to a free
;    variable — e.g. a backquote template built by the macro's own
;    transformer, referencing a variable that never appears in the
;    call's own raw syntax — used to silently fail to capture that
;    variable into the closure, so the reference resolved to garbage/
;    NIL once the expansion was actually compiled. scan_free_vars now
;    expands any macro call it encounters (the same invoke_macro this
;    kernel's compile_form dispatch already uses) and scans the
;    expansion instead of the raw call, exactly mirroring how
;    compile_form itself treats a macro call site.
%include "src/tags.inc"

extern reader_init
extern read_form
extern compile_thunk
extern print_newline

section .rodata
; --- part 1: RECORD-NEW/RECORD-BRAND/RECORD-FIELDS ---
e1: db "(DEFINE R (RECORD-NEW (QUOTE POINT) 3 4))"
e1_len: equ $ - e1

e2: db "(PRINT (RECORD-BRAND R))"
e2_len: equ $ - e2                              ; POINT

e3: db "(PRINT (RECORD-FIELDS R))"
e3_len: equ $ - e3                              ; (3 4)

; --- part 2: scan_free_vars must expand a macro call to discover a
; free variable that only appears in the macro's own expansion, never
; in the raw call-site syntax at all. WRAP-WITH-OUTER's transformer
; builds the AST `(CONS OUTER x)` via raw CONS (no LIST primitive
; available with no prelude loaded, X below is the macro's own
; parameter, bound to the raw, unevaluated operand symbol `X` from the
; call site) — OUTER never appears in `(WRAP-WITH-OUTER X)`'s own raw
; call-site syntax, only inside the expansion this transformer builds.
e4: db "(DEFMACRO WRAP-WITH-OUTER (X) (CONS (QUOTE CONS) (CONS (QUOTE OUTER) (CONS X (QUOTE ())))))"
e4_len: equ $ - e4

; DEFINE+LAMBDA, not DEFUN — DEFUN is a prelude.lisp macro
; ($defun-auto-compile), unavailable with no prelude loaded, unlike
; DEFINE/LAMBDA/DEFMACRO/QUOTE/CONS/CAR/CDR/PRINT/RECORD-*, which are
; all genuine compiler.asm special forms/hostcalls.
e5: db "(DEFINE MAKE-ADDER (LAMBDA (OUTER) (LAMBDA (X) (WRAP-WITH-OUTER X))))"
e5_len: equ $ - e5

e6: db "(DEFINE ADDER10 (MAKE-ADDER 10))"
e6_len: equ $ - e6

e7: db "(PRINT (ADDER10 5))"
e7_len: equ $ - e7                              ; (10 . 5)

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

    mov rdi, e2
    mov rsi, e2_len
    call run_thunk_discard
    call print_newline               ; POINT

    mov rdi, e3
    mov rsi, e3_len
    call run_thunk_discard
    call print_newline               ; (3 4)

    mov rdi, e4
    mov rsi, e4_len
    call run_thunk_discard

    mov rdi, e5
    mov rsi, e5_len
    call run_thunk_discard

    mov rdi, e6
    mov rsi, e6_len
    call run_thunk_discard

    mov rdi, e7
    mov rsi, e7_len
    call run_thunk_discard
    call print_newline               ; (10 . 5)

    xor rax, rax
    ret
