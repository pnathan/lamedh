; 068_capture_shadowing — the over-capture half of
; docs/spec-tco-capture-gc.md section 1 (defect D2), which
; 066_transitive_capture cannot see.
;
; 066 pins that multi-level capture produces the right *values*. It
; cannot pin what a closure captured, because an over-captured slot is
; by construction never read: build_param_frame's entries precede the
; free frame in new_scope and a LET frame is prepended, so the innermost
; binding always wins frame_lookup and a needlessly captured outer name
; simply sits there, costing 8 bytes plus a load/store per closure
; creation. Every assertion below is therefore on CLOSURE-NFREE — the
; closure's own HDR_CLOSURE [24] field — not on a printed value.
;
; What analyze_lambda_captures (compiler.asm) changed: the free-variable
; scan is no longer shadowing-blind. A name re-bound inside a nested
; lambda — as its parameter, by a LET/LET*, by a PROG, by a
; HANDLER-CASE clause — is no longer captured into the enclosing
; closure. The LET-vs-LET* pair below is spec section 1.4's own named
; classic mistake in both directions, and is the one place where getting
; this wrong is a *correctness* bug rather than a wasted slot: LET's
; inits are evaluated in the OUTER scope (so an init naming X refers to
; the outer X and MUST be captured), LET*'s are evaluated one binding at
; a time (so it must NOT be).
;
; Kernel-only (no prelude: no DEFUN/LIST/FUNCALL), like every other
; standalone case here.
%include "src/tags.inc"

extern reader_init
extern read_form
extern compile_thunk
extern print_newline

section .rodata
; --- a nested lambda shadowing its enclosing lambda's parameter ---
; The inner closure needs nothing from outside itself: nfree = 0.
; Before this change the scan saw the inner body's X, resolved it in the
; outer frame, and captured it — nfree = 1.
e1: db "(DEFINE SH1 (LAMBDA (X) (LAMBDA (X) X)))"
e1_len: equ $ - e1
e2: db "(PRINT (CLOSURE-NFREE (SH1 1)))"
e2_len: equ $ - e2                              ; 0
e3: db "(PRINT ((SH1 1) 100))"
e3_len: equ $ - e3                              ; 100 — still the inner X

; --- the same shape unshadowed, as the control: this one really is a
; capture, and must stay one ---
e4: db "(DEFINE NS1 (LAMBDA (X) (LAMBDA (Y) (+ X Y))))"
e4_len: equ $ - e4
e5: db "(PRINT (CLOSURE-NFREE (NS1 1)))"
e5_len: equ $ - e5                              ; 1
e6: db "(PRINT ((NS1 1) 2))"
e6_len: equ $ - e6                              ; 3

; --- shadowing by a LET inside the nested lambda ---
e7: db "(DEFINE SH2 (LAMBDA (X) (LAMBDA (Y) (LET ((X 5)) (+ X Y)))))"
e7_len: equ $ - e7
e8: db "(PRINT (CLOSURE-NFREE (SH2 40)))"
e8_len: equ $ - e8                              ; 0
e9: db "(PRINT ((SH2 40) 2))"
e9_len: equ $ - e9                              ; 7

; --- risk #1, LET (parallel): Z's init names the OUTER X, so the inner
; closure must capture X even though this same LET rebinds it ---
e10: db "(DEFINE LP (LAMBDA (X) (LAMBDA (Y) (LET ((X 1) (Z X)) (+ Z Y)))))"
e10_len: equ $ - e10
e11: db "(PRINT (CLOSURE-NFREE (LP 40)))"
e11_len: equ $ - e11                            ; 1
e12: db "(PRINT ((LP 40) 2))"
e12_len: equ $ - e12                            ; 42 — 40 from the outer X

; --- risk #1, LET* (sequential): Z's init names the LET*'s own X, so
; nothing is captured and the answer is 1+2, not 40+2 ---
e13: db "(DEFINE LS (LAMBDA (X) (LAMBDA (Y) (LET* ((X 1) (Z X)) (+ Z Y)))))"
e13_len: equ $ - e13
e14: db "(PRINT (CLOSURE-NFREE (LS 40)))"
e14_len: equ $ - e14                            ; 0
e15: db "(PRINT ((LS 40) 2))"
e15_len: equ $ - e15                            ; 3

; --- shadowing by a PROG's own variable list ---
e16: db "(DEFINE SH3 (LAMBDA (X) (LAMBDA (Y) (PROG (X) (SETQ X 5) (RETURN (+ X Y))))))"
e16_len: equ $ - e16
e17: db "(PRINT (CLOSURE-NFREE (SH3 40)))"
e17_len: equ $ - e17                            ; 0
e18: db "(PRINT ((SH3 40) 2))"
e18_len: equ $ - e18                            ; 7

; --- a three-level nest where the MIDDLE lambda shadows: the innermost
; lambda's A is the middle one's parameter, so neither the middle nor
; the outer closure has anything to capture from the outermost A ---
e19: db "(DEFINE SH4 (LAMBDA (A) (LAMBDA (A) (LAMBDA (B) (+ A B)))))"
e19_len: equ $ - e19
e20: db "(PRINT (CLOSURE-NFREE (SH4 1)))"
e20_len: equ $ - e20                            ; 0 — the middle closure
e21: db "(PRINT (CLOSURE-NFREE ((SH4 1) 10)))"
e21_len: equ $ - e21                            ; 1 — the inner one, over A
e22: db "(PRINT (((SH4 1) 10) 2))"
e22_len: equ $ - e22                            ; 12

; --- transitive capture is unchanged: a name referenced only three
; levels down is still captured at every intervening level ---
e23: db "(DEFINE TR (LAMBDA (A) (LAMBDA (B) (LAMBDA (C) (+ A (+ B C))))))"
e23_len: equ $ - e23
e24: db "(PRINT (CLOSURE-NFREE (TR 1)))"
e24_len: equ $ - e24                            ; 1 — A
e25: db "(PRINT (CLOSURE-NFREE ((TR 1) 2)))"
e25_len: equ $ - e25                            ; 2 — A and B
e26: db "(PRINT (((TR 1) 2) 3))"
e26_len: equ $ - e26                            ; 6

; --- a global mentioned in a nested lambda body is never captured
; (phase 2's "intersect with what actually resolves") ---
e27: db "(DEFINE GV 7)"
e27_len: equ $ - e27
e28: db "(DEFINE GL (LAMBDA (X) (LAMBDA (Y) (+ GV Y))))"
e28_len: equ $ - e28
e29: db "(PRINT (CLOSURE-NFREE (GL 1)))"
e29_len: equ $ - e29                            ; 0
e30: db "(PRINT ((GL 1) 2))"
e30_len: equ $ - e30                            ; 9

; --- CLOSURE-NFREE on a non-closure answers NIL, not garbage ---
e31: db "(PRINT (CLOSURE-NFREE 5))"
e31_len: equ $ - e31                            ; ()

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

%macro RUN 1
    mov rdi, %1
    mov rsi, %1 %+ _len
    call run_thunk_discard
%endmacro

%macro RUN_PRINTLN 1
    mov rdi, %1
    mov rsi, %1 %+ _len
    call run_thunk_discard
    call print_newline
%endmacro

global lamedh_main
lamedh_main:
    RUN e1
    RUN_PRINTLN e2
    RUN_PRINTLN e3
    RUN e4
    RUN_PRINTLN e5
    RUN_PRINTLN e6
    RUN e7
    RUN_PRINTLN e8
    RUN_PRINTLN e9
    RUN e10
    RUN_PRINTLN e11
    RUN_PRINTLN e12
    RUN e13
    RUN_PRINTLN e14
    RUN_PRINTLN e15
    RUN e16
    RUN_PRINTLN e17
    RUN_PRINTLN e18
    RUN e19
    RUN_PRINTLN e20
    RUN_PRINTLN e21
    RUN_PRINTLN e22
    RUN e23
    RUN_PRINTLN e24
    RUN_PRINTLN e25
    RUN_PRINTLN e26
    RUN e27
    RUN e28
    RUN_PRINTLN e29
    RUN_PRINTLN e30
    RUN_PRINTLN e31

    xor rax, rax
    ret
