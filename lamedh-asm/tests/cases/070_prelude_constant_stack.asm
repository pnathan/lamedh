; 070_prelude_constant_stack — issue #549: lib/prelude.lisp's list
; functions run in constant native stack, and MAKE-STRING in linear
; time.
;
; Before the fix, MAPCAR was `(CONS (FN (CAR L)) (MAPCAR FN (CDR L)))`:
; one native frame per element, so a 300 000-element list exhausted
; the 8 MiB stack (SIGSEGV, exit 139). APPEND, $LENGTH, FILTER,
; DELETE, EFFACE, $ARRAY->LIST, SORT's merge, EVLIS, REMPROP and
; PLIST had the same shape; each is exercised here at the same scale
; (or, for the ones whose inputs are never that long in practice, for
; its exact result on a small list, pinning order and sharing).
; MAKE-STRING appended one byte at a time, copying the whole
; accumulator at every step: 1000 x (MAKE-STRING 10000) did not finish
; in 60 s; it now builds by doubling.
;
; tests/cases/*.asm binaries load no prelude, so this one incbins
; lib/prelude.lisp and runs it, then the program below, through the
; same read/compile/run loop file_runner.asm's run_buffer uses.
%include "src/tags.inc"

extern reader_init
extern read_form
extern compile_thunk

section .rodata
prelude_start:
    incbin "lib/prelude.lisp"
prelude_end:
prelude_len: equ prelude_end - prelude_start

prog_start:
    db "(DEFINE L (MAPCAR 1+ (IOTA 300000 1)))", 10
    db "(PRINT (LIST ($LENGTH L) (CAR L) (CAR (LAST L))))", 10          ; (300000 2 300001)
    db "(NEWLINE)", 10
    db "(PRINT ($LENGTH (APPEND L L)))", 10                              ; 600000
    db "(NEWLINE)", 10
    db "(PRINT ($LENGTH (FILTER (LAMBDA (X) (EQ (REMAINDER X 2) 0)) L)))", 10 ; 150000
    db "(NEWLINE)", 10
    db "(PRINT (LIST ($LENGTH (DELETE 5 L)) ($LENGTH (EFFACE 5 L))))", 10 ; (299999 299999)
    db "(NEWLINE)", 10
    db "(PRINT ($LENGTH ($ARRAY->LIST ($LIST->ARRAY L))))", 10          ; 300000
    db "(NEWLINE)", 10
    db "(PRINT (CAR (SORT (REVERSE L) <)))", 10                          ; 2
    db "(NEWLINE)", 10
    db "(PRINT (LIST (MAPCAR 1+ (LIST 1 2 3)) (MAPCAR 1+ ())))", 10     ; ((2 3 4) ())
    db "(NEWLINE)", 10
    db "(DEFINE ORDER ())", 10
    db "(MAPCAR (LAMBDA (X) (SETQ ORDER (CONS X ORDER))) (LIST 1 2 3))", 10
    db "(PRINT ORDER)", 10                                               ; (3 2 1): FN runs left to right
    db "(NEWLINE)", 10
    db "(DEFINE TAIL (LIST 3 4))", 10
    db "(PRINT (LIST (APPEND (LIST 1 2) TAIL) (APPEND () TAIL) (EQ (CDR (APPEND (LIST 1) TAIL)) TAIL)))", 10 ; ((1 2 3 4) (3 4) T)
    db "(NEWLINE)", 10
    db "(PRINT (LIST (FILTER (LAMBDA (X) (< 1 X)) (LIST 1 2 3 1)) (DELETE 2 (LIST 1 2 3 2)) (EFFACE 2 (LIST 1 2 3 2))))", 10 ; ((2 3) (1 3) (1 3 2))
    db "(NEWLINE)", 10
    db "(PRINT (LIST (SORT (LIST 3 1 2 1) <) ($ARRAY->LIST ($LIST->ARRAY (LIST 1 2 3))) ($ARRAY->LIST (ARRAY 0)) (EVLIS (LIST 1 (QUOTE (+ 1 2))))))", 10 ; ((1 1 2 3) (1 2 3) () (1 3))
    db "(NEWLINE)", 10
    db "(PUTP (QUOTE PQ) (QUOTE A) 1)", 10
    db "(PUTP (QUOTE PQ) (QUOTE B) 2)", 10
    db "(PUTP (QUOTE PQ) (QUOTE C) 3)", 10
    db "(REMPROP (QUOTE PQ) (QUOTE B))", 10
    db "(PRINT (PLIST (QUOTE PQ)))", 10                                  ; (C 3 A 1)
    db "(NEWLINE)", 10
    db "(DOTIMES (I 1000) (MAKE-STRING 10000))", 10
    db "(PRINT (LIST (STRING-LENGTH (MAKE-STRING 10000)) (STRING-LENGTH (MAKE-STRING 7)) (STRING-REF (MAKE-STRING 7) 6) (STRING-LENGTH (MAKE-STRING 0)) (STRING-LENGTH (MAKE-STRING -3))))", 10 ; (10000 7 0 0 0)
prog_end:
prog_len: equ prog_end - prog_start

section .text

; run_buffer(rdi=buf, rsi=len): read, compile and run every top-level
; form in the buffer, as file_runner.asm's own run_buffer does.
run_buffer:
    call reader_init
.loop:
    call read_form
    cmp rax, IMM_EOF
    je .done
    mov rdi, rax
    call compile_thunk
    push rax
    call qword [rsp]
    add rsp, 8
    jmp .loop
.done:
    ret

global lamedh_main
lamedh_main:
    mov rdi, prelude_start
    mov rsi, prelude_len
    call run_buffer

    mov rdi, prog_start
    mov rsi, prog_len
    call run_buffer

    xor rax, rax
    ret
