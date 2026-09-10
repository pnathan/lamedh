; 066_transitive_capture — pins the ACTUAL (already-correct) behavior of
; multi-level free-variable capture through nested LAMBDAs, and the
; measured redundant-macro-expansion cost this file's own README entry
; corrects (see docs/spec-tco-capture-gc.md section 1).
;
; The README previously claimed "single-level" capture — that a nested
; LAMBDA can only see its *immediately* enclosing lambda's own
; variables. That was never true of this compiler: scan_free_vars
; (compiler.asm) never stops at a nested LAMBDA, so an intermediate
; lambda transitively captures whatever any deeper lambda references,
; because it scans its own body (which contains the deeper lambda) as
; one subtree. Every probe below already returns the correct value on
; this commit, with no compiler change — this file exists to make that
; baseline explicit and prevent regression, and to pin the ACTUAL
; defect (D1: a macro at nesting depth D is expanded 2D+1 times at
; compile time, via CNT below) that later commits in the spec's
; landing plan fix.
;
; Deliberately uses only kernel-primitive special forms (DEFINE/LAMBDA/
; LET/QUOTE/CONS/CAR/CDR/IF/SETQ/DEFMACRO/PRINT/&REST) — no DEFUN/LIST/
; FUNCALL — since standalone tests/cases/*.asm run with no prelude
; loaded at all.
%include "src/tags.inc"

extern reader_init
extern read_form
extern compile_thunk
extern print_newline

section .rodata
; --- transitive capture through 3 levels ---
e1: db "(DEFINE F (LAMBDA (A) (LAMBDA (B) (LAMBDA (C) (+ A (+ B C))))))"
e1_len: equ $ - e1
e2: db "(PRINT (((F 1) 2) 3))"
e2_len: equ $ - e2                              ; 6

; --- transitive capture of a LET-bound name in the middle ---
e3: db "(DEFINE G (LAMBDA (A) (LET ((L 10)) (LAMBDA (B) (LAMBDA (C) (+ L (+ A (+ B C))))))))"
e3_len: equ $ - e3
e4: db "(PRINT (((G 1) 2) 3))"
e4_len: equ $ - e4                              ; 16

; --- shadowing: the innermost A must win, not the outermost ---
e5: db "(DEFINE H (LAMBDA (A) (LAMBDA (B) (LAMBDA (A) (+ A B)))))"
e5_len: equ $ - e5
e6: db "(PRINT (((H 1) 2) 100))"
e6_len: equ $ - e6                              ; 102

; --- four levels deep ---
e7: db "(DEFINE M (LAMBDA (A) (LAMBDA (B) (LAMBDA (C) (LAMBDA (D) (+ A (+ B (+ C D))))))))"
e7_len: equ $ - e7
e8: db "(PRINT ((((M 1) 2) 3) 4))"
e8_len: equ $ - e8                              ; 10

; --- stack-passed outer params (P3/P4 live beyond the 3-register slots) ---
e9: db "(DEFINE OUTER4 (LAMBDA (P0 P1 P2 P3 P4) (LAMBDA (B) (LAMBDA (C) (+ P4 (+ P3 (+ B C)))))))"
e9_len: equ $ - e9
e10: db "(PRINT (((OUTER4 0 0 0 10 20) 2) 3))"
e10_len: equ $ - e10                            ; 35

; --- &REST in the middle lambda ---
e11: db "(DEFINE OUTER3 (LAMBDA (A) (LAMBDA (&REST BS) (LAMBDA (C) (+ A (+ (CAR BS) C))))))"
e11_len: equ $ - e11
e12: db "(PRINT (((OUTER3 1) 2 9) 3))"
e12_len: equ $ - e12                            ; 6

; --- macro-produced inner lambda still captures transitively ---
e13: db "(DEFMACRO MK (X) (CONS (QUOTE LAMBDA) (CONS (CONS (QUOTE Q) (QUOTE ())) (CONS (CONS (QUOTE +) (CONS X (CONS (QUOTE Q) (QUOTE ())))) (QUOTE ())))))"
e13_len: equ $ - e13
e14: db "(DEFINE OUTER6 (LAMBDA (A) (LAMBDA (B) (MK (+ A B)))))"
e14_len: equ $ - e14
e15: db "(PRINT (((OUTER6 1) 2) 3))"
e15_len: equ $ - e15                            ; 6

; --- D1 (fixed): a macro at nesting depth D used to be expanded 2D+1
; times (3, 5, 7 for a 1-, 2-, and 3-deep lambda nest) because
; compile_lambda scanned each lambda's body twice and each scan
; independently re-ran the transformer. macroexpand_once/
; macroexpand_memo (compiler.asm) now memoize expansion by the call
; form's own address, so scan and compile share one invocation: CNT
; is 1 at every depth.
e16: db "(DEFINE CNT 0)"
e16_len: equ $ - e16
e17: db "(DEFMACRO CM (X) (SETQ CNT (+ CNT 1)) X)"
e17_len: equ $ - e17
e18: db "(DEFINE F1 (LAMBDA (A) (CM A)))"
e18_len: equ $ - e18
e19: db "(PRINT CNT)"
e19_len: equ $ - e19                            ; 1
e20: db "(SETQ CNT 0)"
e20_len: equ $ - e20
e21: db "(DEFINE F2 (LAMBDA (A) (LAMBDA (B) (CM B))))"
e21_len: equ $ - e21
e22: db "(PRINT CNT)"
e22_len: equ $ - e22                            ; 1
e23: db "(SETQ CNT 0)"
e23_len: equ $ - e23
e24: db "(DEFINE F3 (LAMBDA (A) (LAMBDA (B) (LAMBDA (C) (CM C)))))"
e24_len: equ $ - e24
e25: db "(PRINT CNT)"
e25_len: equ $ - e25                            ; 1

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
    RUN e3
    RUN_PRINTLN e4
    RUN e5
    RUN_PRINTLN e6
    RUN e7
    RUN_PRINTLN e8
    RUN e9
    RUN_PRINTLN e10
    RUN e11
    RUN_PRINTLN e12
    RUN e13
    RUN e14
    RUN_PRINTLN e15
    RUN e16
    RUN e17
    RUN e18
    RUN_PRINTLN e19
    RUN e20
    RUN e21
    RUN_PRINTLN e22
    RUN e23
    RUN e24
    RUN_PRINTLN e25

    xor rax, rax
    ret
