; 052_unwind_protect — UNWIND-PROTECT (KERNEL.md Part VII):
; "body-form is evaluated; then every cleanup form is evaluated
; unconditionally — after a normal return, an error, or any non-local
; exit passing through — and the body's outcome is then delivered."
;
; Unlike BLOCK/HANDLER-CASE (each just another CATCH/THROW derivation
; reacting only to a throw targeting *them*), this needs to react to
; *any* throw merely passing through — which is why compile_throw and
; emit_throw_baked (ERROR's own signaling path) were both refactored
; to funnel every THROW through one shared host routine, native_throw
; (native_errors.asm): that is the one place a passing throw's search
; can notice this form's own catch-stack "marker" frame and fire its
; cleanup right there, in LIFO order for nested UNWIND-PROTECTs,
; before continuing to search for the real target.
;
; v0 scope: "an error raised by a cleanup form is discarded" is not
; yet true here (see native_throw's own comment) — not exercised by
; this test.

%include "src/tags.inc"

extern reader_init
extern read_form
extern compile_thunk
extern print_newline

section .rodata
; normal completion: body runs, then cleanup, in that order.
e1: db "(UNWIND-PROTECT (PRINT (QUOTE BODY)) (PRINT (QUOTE CLEANUP)))"
e1_len: equ $ - e1

; the body's own value is captured *before* cleanup runs, even when
; cleanup mutates the same variable the body just read.
d1: db "(DEFINE X 10)"
d1_len: equ $ - d1
e2: db "(PRINT (UNWIND-PROTECT X (SETQ X (+ X 1))))"                ; 10
e2_len: equ $ - e2
e3: db "(PRINT X)"                                                    ; 11
e3_len: equ $ - e3

; a THROW passing straight through an UNWIND-PROTECT (never caught
; inside it) still fires the cleanup, then the enclosing CATCH
; delivers the thrown value unaffected.
e4: db "(PRINT (CATCH (QUOTE TAG) (UNWIND-PROTECT (THROW (QUOTE TAG) 7) (PRINT (QUOTE CLEANUP2)))))"  ; CLEANUP2 then 7
e4_len: equ $ - e4

; nested UNWIND-PROTECTs unwind in LIFO order: inner cleanup fires
; before outer, both before the CATCH that finally stops the throw.
e5: db "(CATCH (QUOTE OUTER) (UNWIND-PROTECT (UNWIND-PROTECT (THROW (QUOTE OUTER) 1) (PRINT (QUOTE INNER))) (PRINT (QUOTE OUTER))))"
e5_len: equ $ - e5

; a native failure (CAR on a non-cons) passing through also fires the
; cleanup, and HANDLER-CASE still catches it afterward.
e6: db "(PRINT (HANDLER-CASE (UNWIND-PROTECT (CAR 5) (PRINT (QUOTE CLEANUP3))) (E (X) (QUOTE CAUGHT))))"  ; CLEANUP3 then CAUGHT
e6_len: equ $ - e6

; zero cleanup forms is legal — an empty cleanup body.
e7: db "(PRINT (UNWIND-PROTECT 42))"                                ; 42
e7_len: equ $ - e7

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
    call print_newline               ; BODYCLEANUP -> printed with no
                                      ; newline between, then this one

    mov rdi, d1
    mov rsi, d1_len
    call run_thunk_discard

    mov rdi, e2
    mov rsi, e2_len
    call run_thunk_discard
    call print_newline               ; 10

    mov rdi, e3
    mov rsi, e3_len
    call run_thunk_discard
    call print_newline               ; 11

    mov rdi, e4
    mov rsi, e4_len
    call run_thunk_discard
    call print_newline               ; CLEANUP27

    mov rdi, e5
    mov rsi, e5_len
    call run_thunk_discard
    call print_newline               ; INNEROUTER

    mov rdi, e6
    mov rsi, e6_len
    call run_thunk_discard
    call print_newline               ; CLEANUP3CAUGHT

    mov rdi, e7
    mov rsi, e7_len
    call run_thunk_discard
    call print_newline               ; 42

    xor rax, rax
    ret
