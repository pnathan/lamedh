; 034_not_callable — calling a non-callable value is a genuine,
; catchable native failure (fail_wrong_type/native_throw,
; native_errors.asm — the same real CATCH/HANDLER-CASE/ERRORSET-
; signaling machinery CAR/CDR's own wrong-type check already uses),
; not undefined behavior. KERNEL.md Part VIII lists this among the
; native-failure classes a host must signal for; before
; emit_check_callable existed, both compile_call paths blindly
; dereferenced whatever tagged value they were given as if it were a
; closure — an unbound global defaults to IMM_UNBOUND, whose tag bits
; mask to a near-NULL pointer, so calling one segfaulted rather than
; failing in any way a caller (or this test) could observe. This case
; is the named-global inline-cache path (a global that was never
; DEFINEd); 035_not_callable_indirect.asm is the other compile_call
; path (a lexically bound local holding a non-closure value).
;
; An earlier version of this check used a separate, cruder
; fail_not_callable() (a fixed message to stderr, then a hard exit(1))
; predating native_throw; emit_check_callable now goes through
; fail_wrong_type/native_throw instead, so a HANDLER-CASE/ERRORSET
; genuinely catches it (e2 below), and an *uncaught* one is reported —
; one line on stderr, `lamedhc: unhandled error: not a function: ...`,
; then exit(1) (report_unhandled_throw, native_errors.asm; tests/run.sh's
; `.exitcode` file) — the same way any other unmatched THROW is.

%include "src/tags.inc"

extern reader_init
extern read_form
extern compile_thunk
extern print_fixnum
extern print_newline

section .rodata
d1: db "(PRINT (QUOTE BEFORE))"
d1_len: equ $ - d1
; FOOBAR was never DEFINEd: the named-global inline-cache path.
e1: db "(FOOBAR 1 2 3)"
e1_len: equ $ - e1
; HANDLER-CASE catches it cleanly first, proving this is the real
; condition-signaling path, not merely a deterministic crash — the
; culprit value (FOOBAR's own unbound-global cell contents, IMM_NIL)
; survives as ERROR-DATA even though it isn't folded into the message
; text (v0 scope, matching CAR/CDR's own divergence from the
; reference's interpolated message).
e2: db "(PRINT (HANDLER-CASE (FOOBAR 1 2 3) (E (X) (QUOTE CAUGHT))))"
e2_len: equ $ - e2

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
    mov rdi, d1
    mov rsi, d1_len
    call run_thunk_discard        ; prints "BEFORE" — proves the
                                   ; process was running normally right
                                   ; up until the failing call below
    call print_newline

    mov rdi, e2
    mov rsi, e2_len
    call run_thunk_discard        ; CAUGHT
    call print_newline

    mov rdi, e1
    mov rsi, e1_len
    call run_thunk_discard        ; never returns: uncaught, traps (int3)

    ; unreachable
    mov rax, 99
    ret
