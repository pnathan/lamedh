; 056_stdlib_bootstrap_primitives — three small kernel primitives added
; specifically to make the Rust reference's own lib/00-core.lisp load
; unmodified through this compiler (the concrete next conformance
; target after examples/*/main.lisp: the entire reference standard
; library, minus the networking/TLS/regex/OS tiers this freestanding
; host has no I/O surface for — see README Roadmap):
;
;   STRINGP  — is the value a String? (00-core's DEFUN macro peels an
;              optional leading docstring off a function body with
;              `(stringp (car body))` on every expansion.)
;   BOUNDP   — has this symbol's global value ever been DEFINEd? (the
;              same macro guards its optional call-graph bookkeeping
;              globals with `(if (boundp '$cg-pending) ...)`.)
;   JIT-OPTIMIZE — a real special form in the reference (jit.rs),
;              taking its symbol operand unevaluated; this host has no
;              distinct "optimize this closure" step (every LAMBDA is
;              already compiled to native code the moment it's read),
;              so it is a documented no-op returning its own operand,
;              exactly like QUOTE.
%include "src/tags.inc"

extern reader_init
extern read_form
extern compile_thunk
extern print_newline

section .rodata
e1: db '(PRINT (STRINGP "hi"))'
e1_len: equ $ - e1                          ; T

e2: db "(PRINT (STRINGP 5))"
e2_len: equ $ - e2                            ; ()

e3: db "(PRINT (STRINGP (QUOTE FOO)))"
e3_len: equ $ - e3                              ; ()

e4: db "(PRINT (BOUNDP (QUOTE UNDEFINED-GLOBAL-XYZ)))"
e4_len: equ $ - e4                                ; ()

e5: db "(PROGN (DEFINE BOUND-GLOBAL-XYZ 42) (PRINT (BOUNDP (QUOTE BOUND-GLOBAL-XYZ))))"
e5_len: equ $ - e5                                  ; T

e6: db "(PRINT (JIT-OPTIMIZE SOME-UNDEFINED-NAME))"
e6_len: equ $ - e6                                    ; SOME-UNDEFINED-NAME

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
    call print_newline               ; T

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
    call print_newline               ; T

    mov rdi, e6
    mov rsi, e6_len
    call run_thunk_discard
    call print_newline               ; SOME-UNDEFINED-NAME

    xor rax, rax
    ret
