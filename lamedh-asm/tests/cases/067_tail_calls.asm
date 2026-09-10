; 067_tail_calls — pins proper (stack-safe) tail calls, landing-plan
; steps 3 and 4 of docs/spec-tco-capture-gc.md section 2: the
; indirect-call path (a locally-bound operator, not a bare global
; symbol) emits a genuine tail jump (emit_leave + emit_jmp_reg, no
; emit_call_reg), and the named-global path emits a `jmp`-based call
; site patched via a mode=1 baked-address inline-cache trampoline
; (emit_ic_trampoline) instead of an ordinary `call` — both only at a
; call site that is (a) in tail position (compile_form's own tail_ctx
; plumbing), (b) has <=3 arguments (v0 scope), and (c) is nested
; inside some LAMBDA body (current_lambda_depth).
;
; COUNTDOWN below takes itself as an explicit parameter (SELF) and
; calls (SELF SELF ...) from the IF's else branch — SELF is a LAMBDA
; parameter, so frame_lookup finds it locally bound and compile_call
; takes .indirect_path, exactly the path this step changes. Before
; this step, each recursive call grew the native call stack by one
; frame; at 1,000,000 iterations and this kernel's 512 MiB
; with_large_stack, that used to segfault well before completion (the
; spec's own measurement: 8 MiB * frame math puts the old crash point
; around 200k-300k iterations, and this project's real stack is larger
; but the same unbounded-growth bug still eventually crashes it at
; 1,000,000 — verified against this commit's own immediate parent). A
; genuine tail call instead reuses the same native frame every
; iteration, so this must now return cleanly.
;
; Deliberately uses only kernel-primitive special forms (DEFINE/LAMBDA/
; IF/PRINT), no DEFUN/prelude — standalone tests/cases/*.asm run with
; no prelude loaded at all.
%include "src/tags.inc"

extern reader_init
extern read_form
extern compile_thunk
extern print_newline

section .rodata
; COUNTDOWN SELF N ACC: counts N down to 0, accumulating 1 per step in
; ACC via SELF's own indirect tail self-call.
e1: db "(DEFINE COUNTDOWN (LAMBDA (SELF N ACC) (IF (= N 0) ACC (SELF SELF (- N 1) (+ ACC 1)))))"
e1_len: equ $ - e1
e2: db "(PRINT (COUNTDOWN COUNTDOWN 1000000 0))"
e2_len: equ $ - e2                              ; 1000000

; mutual indirect tail recursion: EVENP2/ODDP2 each take the OTHER as
; an explicit parameter and tail-call through it, so both call sites
; are indirect (a local parameter, not a global symbol).
e3: db "(DEFINE EVENP2 (LAMBDA (SELF OTHER N) (IF (= N 0) 1 (OTHER OTHER SELF (- N 1)))))"
e3_len: equ $ - e3
e4: db "(PRINT (EVENP2 EVENP2 EVENP2 1000000))"
e4_len: equ $ - e4                              ; 1 (even)

; --- named-global tail calls (step 4): a bare global symbol operator,
; the call site itself a `jmp` patched via a mode=1 baked-address
; trampoline (emit_ic_trampoline), since the usual return-address trick
; would misidentify the call site once a tail jmp is involved. ---

; COUNTDOWN2: self-recursive by GLOBAL NAME (not a SELF parameter), the
; same stack-safety stress test as COUNTDOWN above but through the
; named path this step actually changes.
e5: db "(DEFINE COUNTDOWN2 (LAMBDA (N ACC) (IF (= N 0) ACC (COUNTDOWN2 (- N 1) (+ ACC 1)))))"
e5_len: equ $ - e5
e6: db "(PRINT (COUNTDOWN2 1000000 0))"
e6_len: equ $ - e6                              ; 1000000

; --- trampoline-patch regression (docs/spec-tco-capture-gc.md section
; 2.6's own named worry): H's body is a single call site to F, reached
; via a `jmp` (H's own call to F is itself tail); F's body,
; conditionally, tail-calls G through a SEPARATE call site, resolved
; the first time H(1 ...) runs. The pre-mode-1 design's danger was
; exactly this: a tail-entered trampoline discovering its own patch
; site from the (wrong, unrelated) return address on the stack could
; corrupt H's OWN call-to-F site into jumping straight to G instead.
; Since mode=1 bakes each call site's field_addr at compile time, the
; two sites can never be confused — H(NIL ...), run AFTER H(1 ...) has
; already resolved and patched both trampolines, must still reach F's
; own (X+1) branch, not G's (X+1000) one.
e7: db "(DEFINE G (LAMBDA (X) (+ X 1000)))"
e7_len: equ $ - e7
e8: db "(DEFINE F (LAMBDA (FLAG X) (IF FLAG (G X) (+ X 1))))"
e8_len: equ $ - e8
e9: db "(DEFINE H (LAMBDA (FLAG X) (F FLAG X)))"
e9_len: equ $ - e9
e10: db "(PRINT (H 1 10))"
e10_len: equ $ - e10                            ; 1010 (via G)
; NOTE the flag value here is NIL, not 0: this kernel is Lisp 1.5, where
; the *only* false value is the NIL immediate and every other object —
; 0 emphatically included — is true (see README.md's own "only the
; literal NIL immediate is false"). An earlier draft of this case wrote
; (H 0 10) and expected 11, which is simply not what this language
; means: (IF 0 ...) takes the THEN branch, so F(0, 10) legitimately
; tail-calls G and yields 1010. Using NIL is what actually drives F down
; its else branch, which is the whole point of this regression.
e11: db "(PRINT (H NIL 10))"
e11_len: equ $ - e11                            ; 11 (F's own branch,
                                                 ; NOT corrupted to G)

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
    RUN e8
    RUN e9
    RUN_PRINTLN e10
    RUN_PRINTLN e11

    xor rax, rax
    ret
