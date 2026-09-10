; 057_defdynamic — DEFDYNAMIC / dynamic-extent LET rebinding (KERNEL.md
; Part VI): (defdynamic name init) declares a global as dynamic; a
; later (let ((name new-val)) body) rebinds the *global* slot for the
; dynamic extent of body — visible to any separately-compiled function
; called during that extent, not just code lexically inside the LET —
; and restores the old value on every exit path, including a THROW
; passing through. compile_let rewrites such a LET into
; (LET ((tmp name) (val new-val)) (UNWIND-PROTECT (PROGN (SETQ name val)
; body...) (SETQ name tmp))), reusing UNWIND-PROTECT's own marker-frame
; restore-on-every-exit machinery rather than any new codegen.
%include "src/tags.inc"

extern reader_init
extern read_form
extern compile_thunk
extern print_newline

section .rodata
; SHOW reads *X* as a plain global reference from a *different*,
; separately compiled closure than the one that rebinds it — the case
; ordinary lexical shadowing gets wrong, since SHOW's own body was
; compiled with no knowledge of any enclosing LET.
setup: db "(PROGN (DEFDYNAMIC *X* 1) (DEFINE SHOW (LAMBDA () (PRINT *X*))))"
setup_len: equ $ - setup

e1: db "(SHOW)"
e1_len: equ $ - e1                          ; 1 (before any rebinding)

e2: db "(LET ((*X* 42)) (SHOW))"
e2_len: equ $ - e2                            ; 42 (rebound, seen across the call)

e3: db "(SHOW)"
e3_len: equ $ - e3                              ; 1 (restored after the LET)

e4: db "(PRINT (LET ((*X* 99)) *X*))"
e4_len: equ $ - e4                                ; 99 (LET's own body sees it too)

e5: db "(SHOW)"
e5_len: equ $ - e5                                  ; 1 (restored again)

; restoration must fire even when a THROW passes straight through the
; dynamic extent, never reaching the LET's own normal exit.
e6: db "(CATCH (QUOTE TAG) (LET ((*X* 777)) (SHOW) (THROW (QUOTE TAG) 0)))"
e6_len: equ $ - e6                                    ; 777

e7: db "(SHOW)"
e7_len: equ $ - e7                                      ; 1 (still restored)

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
    mov rdi, setup
    mov rsi, setup_len
    call run_thunk_discard

    mov rdi, e1
    mov rsi, e1_len
    call run_thunk_discard
    call print_newline               ; 1

    mov rdi, e2
    mov rsi, e2_len
    call run_thunk_discard
    call print_newline               ; 42

    mov rdi, e3
    mov rsi, e3_len
    call run_thunk_discard
    call print_newline               ; 1

    mov rdi, e4
    mov rsi, e4_len
    call run_thunk_discard
    call print_newline               ; 99

    mov rdi, e5
    mov rsi, e5_len
    call run_thunk_discard
    call print_newline               ; 1

    mov rdi, e6
    mov rsi, e6_len
    call run_thunk_discard
    call print_newline               ; 777

    mov rdi, e7
    mov rsi, e7_len
    call run_thunk_discard
    call print_newline               ; 1

    xor rax, rax
    ret
