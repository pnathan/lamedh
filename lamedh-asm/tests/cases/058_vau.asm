; 058_vau — $VAU (KERNEL.md's vau combiner, John Shutt's vau-calculus):
; ($VAU (operands-param env-param) body...) builds an Operative — a
; LAMBDA-built closure retagged HDR_OPERATIVE — whose call sites do NOT
; evaluate their operands: operands-param is bound to the raw,
; unevaluated argument list, and env-param to a placeholder "caller's
; environment" value (this kernel has no first-class environments).
; EVAL's own 2-argument form (eval form e) already tolerates this for
; free, silently ignoring the second operand and evaluating in the one
; global environment this kernel has.
%include "src/tags.inc"

extern reader_init
extern read_form
extern compile_thunk
extern print_newline

section .rodata
; operands-param bound to the raw, unevaluated list; env-param bound to
; the sentinel symbol (not inspected here beyond printing it).
e1: db "(DEFINE MY-OP ($VAU (X E) (PRINT X) (NEWLINE) (PRINT E) (QUOTE DONE)))"
e1_len: equ $ - e1

e2: db "(PRINT (MY-OP 1 (+ 2 3) UNBOUND-NAME))"
e2_len: equ $ - e2                                 ; (1 (+ 2 3) UNBOUND-NAME) then the
                                                    ; sentinel symbol, then DONE

; (EVAL form e) — the second operand is silently ignored; EVAL always
; evaluates in the one global environment this kernel has.
e3: db "(DEFINE MY-IF ($VAU (X E) (IF (EVAL (CAR X) E) (EVAL (CAR (CDR X)) E) (EVAL (CAR (CDR (CDR X))) E))))"
e3_len: equ $ - e3

e4: db "(PRINT (MY-IF T (QUOTE YES) (QUOTE NO)))"
e4_len: equ $ - e4                                   ; YES

e5: db "(PRINT (MY-IF NIL (QUOTE YES) (QUOTE NO)))"
e5_len: equ $ - e5                                     ; NO

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
    call print_newline               ; DONE

    mov rdi, e3
    mov rsi, e3_len
    call run_thunk_discard

    mov rdi, e4
    mov rsi, e4_len
    call run_thunk_discard
    call print_newline               ; YES

    mov rdi, e5
    mov rsi, e5_len
    call run_thunk_discard
    call print_newline               ; NO

    xor rax, rax
    ret
