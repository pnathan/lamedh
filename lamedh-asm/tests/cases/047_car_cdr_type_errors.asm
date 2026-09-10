; 047_car_cdr_type_errors — CAR/CDR on a value that is neither a cons
; nor NIL is now a genuine, catchable native failure (fail_wrong_type,
; native_errors.asm; native_throw there is the same catch-stack
; search/restore/jump compile_throw's own generated code performs,
; factored into one callable host routine) instead of silently
; segfaulting on an out-of-bounds dereference. KERNEL.md Part IV/XI:
; `(car nil)` = `(cdr nil)` = `NIL` (not an error — e1/e2 below), and
; every other non-cons argument is a native failure a host must signal
; *a* condition for (Part VIII) — `(errorset '(car 5))` is exactly the
; case the README previously named as demonstrating this gap.
;
; v0 scope: the message text is a fixed string per caller ("CAR:
; expected a cons or NIL"), not the Rust reference's own interpolated
; "CAR: expected a list, got 5" — ERROR-DATA still exposes the actual
; culprit value, just not folded into the message text (see
; native_errors.asm's own comment on this exact divergence).

%include "src/tags.inc"

extern reader_init
extern read_form
extern compile_thunk
extern print_newline

section .rodata
; (car nil)/(cdr nil) are not errors — still NIL.
e1: db "(PRINT (CAR (QUOTE ())))"                                    ; ()
e1_len: equ $ - e1
e2: db "(PRINT (CDR (QUOTE ())))"                                    ; ()
e2_len: equ $ - e2

; ERRORSET now genuinely catches a wrong-type CAR/CDR instead of
; segfaulting or trapping — this is the exact case the README's own
; roadmap named as the concrete demonstration of the gap.
e3: db "(PRINT (ERRORSET (QUOTE (CAR 5))))"                            ; ()
e3_len: equ $ - e3
e4: db "(PRINT (ERRORSET (QUOTE (CDR (QUOTE HELLO)))))"                 ; ()
e4_len: equ $ - e4

; HANDLER-CASE catches it too, and ERROR-MESSAGE/ERROR-DATA both work
; on the resulting condition — the culprit value survives as DATA even
; though it isn't folded into the message text (v0 scope, see above).
e5: db "(PRINT (HANDLER-CASE (CAR 5) (E (X) (ERROR-DATA X))))"           ; 5
e5_len: equ $ - e5
e6: db '(PRINT (HANDLER-CASE (CDR "hi") (E (X) (ERROR-DATA X))))'         ; hi
e6_len: equ $ - e6

; a normal cons argument is entirely unaffected by the new check.
e7: db "(PRINT (CAR (CONS 1 2)))"                                          ; 1
e7_len: equ $ - e7
e8: db "(PRINT (CDR (CONS 1 2)))"                                           ; 2
e8_len: equ $ - e8

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
    call print_newline               ; ()

    mov rdi, e2
    mov rsi, e2_len
    call run_thunk_discard
    call print_newline               ; ()

    mov rdi, e3
    mov rsi, e3_len
    call run_thunk_discard
    call print_newline               ; ()

    mov rdi, e4
    mov rsi, e4_len
    call run_thunk_discard
    call print_newline               ; ()

    mov rdi, e5
    mov rsi, e5_len
    call run_thunk_discard
    call print_newline               ; 5

    mov rdi, e6
    mov rsi, e6_len
    call run_thunk_discard
    call print_newline               ; hi

    mov rdi, e7
    mov rsi, e7_len
    call run_thunk_discard
    call print_newline               ; 1

    mov rdi, e8
    mov rsi, e8_len
    call run_thunk_discard
    call print_newline               ; 2

    xor rax, rax
    ret
