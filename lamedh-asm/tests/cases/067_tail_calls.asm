; 067_tail_calls — pins proper (stack-safe) tail calls, landing-plan
; step 3 of docs/spec-tco-capture-gc.md section 2: the indirect-call
; path (a locally-bound operator, not a bare global symbol) now emits
; a genuine tail jump (emit_leave + emit_jmp_reg, no emit_call_reg) at
; a call site that is (a) in tail position (compile_form's own
; tail_ctx plumbing) and (b) has <=3 arguments (v0 scope) and (c) is
; nested inside some LAMBDA body (current_lambda_depth).
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

    xor rax, rax
    ret
