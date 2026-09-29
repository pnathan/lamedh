; 070_prelude_basics — the portability basics lib/prelude.lisp gained
; for #552: CONSP, LENGTH (lists, UTF-8 strings in characters, arrays,
; hash tables), ABS, MEMBER, DOLIST, MAKE-ARRAY, AREF, ARRAY-LENGTH,
; LIST->ARRAY, ARRAY->LIST, STRING=, SYMBOL-NAME, SETF/PUSH/INCF over
; symbol/GETHASH/FETCH/AREF/ELT places, FLET, and LABELS (mutual and
; self recursion through the array box, parameter shadowing of a
; sibling name, a closure over an outer variable, &REST).
;
; Every expected value was checked against the Rust reference
; (target/release/lamedh) except LABELS, SYMBOL-NAME and ARRAY-LENGTH,
; which the reference does not define (Common Lisp semantics), and the
; float printing, which is this kernel's own %f format.
;
; The last group is the reason every definition is a WHILE loop: a
; 200 000-element list through LENGTH/MEMBER/LIST->ARRAY/ARRAY->LIST/
; DOLIST, and 1 000 001 mutually tail-recursive LABELS calls, none of
; which may consume native stack per element.
;
; Unlike the kernel-only cases, this one loads lib/prelude.lisp first
; (incbin, as file_runner.asm does) and runs both buffers through the
; same read/compile/run loop as file_runner.asm's run_buffer.
%include "src/tags.inc"

extern reader_init
extern read_form
extern compile_thunk
extern rc_pin_depth
extern compile_nesting_depth
extern current_lambda_depth

section .rodata
prelude_start:
    incbin "lib/prelude.lisp"
prelude_end:

prog_start:
    db '(PRINT (LIST (LENGTH NIL) (LENGTH (LIST 1 2 3)) (LENGTH "héllo") (LENGTH (MAKE-ARRAY 4)) (LENGTH (MAKE-HASH-TABLE))))', 10
    db '(NEWLINE)', 10
    db '(DEFINE H (MAKE-HASH-TABLE))', 10
    db '(SETHASH H (QUOTE A) 1)', 10
    db '(SETHASH H (QUOTE B) 2)', 10
    db '(PRINT (LENGTH H))', 10
    db '(NEWLINE)', 10
    db '(PRINT (HANDLER-CASE (LENGTH (CONS 1 2)) (ERROR (E) (LIST (ERROR-MESSAGE E) (ERROR-DATA E)))))', 10
    db '(NEWLINE)', 10
    db '(PRINT (LIST (ABS -5) (ABS 5) (ABS 0) (ABS -1.5) (ABS 2.5)))', 10
    db '(NEWLINE)', 10
    db '(PRINT (EQ (F/ 1.0 (ABS -0.0)) (F/ 1.0 -0.0)))', 10
    db '(NEWLINE)', 10
    db '(PRINT (HANDLER-CASE (ABS "x") (ERROR (E) (ERROR-MESSAGE E))))', 10
    db '(NEWLINE)', 10
    db '(PRINT (LIST (MEMBER 2 (LIST 1 2 3)) (MEMBER (LIST 1) (LIST (LIST 1) 2)) (MEMBER 9 (LIST 1 2)) (MEMBER 1 NIL)))', 10
    db '(NEWLINE)', 10
    db '(PRINT (LIST (CONSP NIL) (CONSP (CONS 1 2)) (CONSP "s") (CONSP (MAKE-ARRAY 1)) (CONSP 3) (CONSP (QUOTE A))))', 10
    db '(NEWLINE)', 10
    db '(PRINT (LET ((ACC NIL)) (DOLIST (X (LIST 1 2 3) ACC) (SETQ ACC (CONS X ACC)))))', 10
    db '(NEWLINE)', 10
    db '(PRINT (LIST (DOLIST (X (LIST 1 2)) X) (DOLIST (X (LIST 1 2) X) X) (DOLIST (X NIL 7) X)))', 10
    db '(NEWLINE)', 10
    db '(PRINT (LET ((ACC NIL)) (DOLIST (X (LIST 1 2)) (DOLIST (Y (LIST 3 4)) (SETQ ACC (CONS (LIST X Y) ACC)))) ACC))', 10
    db '(NEWLINE)', 10
    db '(PRINT (LET ((N 0)) (DOLIST (X (LIST 1 2 3)) (SETQ X 10) (SETQ N (+ N 1))) N))', 10
    db '(NEWLINE)', 10
    db '(PRINT (LET ((A (MAKE-ARRAY 3))) (STORE A 1 (QUOTE B)) (LIST (AREF A 0) (AREF A 1) (ARRAY-LENGTH A) (ARRAY-LENGTH (MAKE-ARRAY 0)))))', 10
    db '(NEWLINE)', 10
    db '(PRINT (LIST (ARRAY->LIST (LIST->ARRAY (LIST 1 2 3))) (ARRAY->LIST (MAKE-ARRAY 0)) (ARRAYP (LIST->ARRAY NIL))))', 10
    db '(NEWLINE)', 10
    db '(PRINT (LIST (STRING= "ab" "ab") (STRING= "ab" "ac") (STRING= "" "") (STRING= (QUOTE AB) "AB")))', 10
    db '(NEWLINE)', 10
    db '(PRINT (LIST (SYMBOL-NAME (QUOTE FOO)) (SYMBOL-NAME NIL) (SYMBOL-NAME T) (STRINGP (SYMBOL-NAME (QUOTE FOO)))))', 10
    db '(NEWLINE)', 10
    db '(PRINT (HANDLER-CASE (SYMBOL-NAME 5) (ERROR (E) (ERROR-MESSAGE E))))', 10
    db '(NEWLINE)', 10
    db '(PRINT (LET* ((X 1) (A (INCF X)) (B (INCF X 5)) (C (INCF X 1 2))) (LIST A B C X)))', 10
    db '(NEWLINE)', 10
    db '(PRINT (LET* ((X NIL) (A (PUSH 1 X)) (B (PUSH 2 X))) (LIST A B X)))', 10
    db '(NEWLINE)', 10
    db '(PRINT (LET ((A (MAKE-ARRAY 2))) (SETF (AREF A 0) 5) (INCF (AREF A 0)) (PUSH 9 (AREF A 1)) (ARRAY->LIST A)))', 10
    db '(NEWLINE)', 10
    db '(PRINT (LET* ((HT (MAKE-HASH-TABLE)) (A (SETF (GETHASH HT 1) 2)) (B (INCF (GETHASH HT 1)))) (LIST A B (GETHASH HT 1))))', 10
    db '(NEWLINE)', 10
    db '(PRINT (LET ((X 1) (Y 2) (V (MAKE-ARRAY 2))) (SETF X 5 Y (+ X 1) (FETCH V 0) X (ELT V 1) Y) (LIST X Y (ARRAY->LIST V) (SETF))))', 10
    db '(NEWLINE)', 10
    db '(PRINT (FLET ((F (X) (* X 2)) (G (X Y) (+ X Y))) (LIST (F 4) (G 1 2))))', 10
    db '(NEWLINE)', 10
    db '(PRINT (LET ((F 10)) (FLET ((F (X) (+ X 1)) (G () F)) (LIST (F 1) (G)))))', 10
    db '(NEWLINE)', 10
    db '(PRINT (LABELS ((EV (N) (IF (= N 0) T (OD (- N 1)))) (OD (N) (IF (= N 0) (QUOTE ()) (EV (- N 1))))) (LIST (EV 10) (EV 7))))', 10
    db '(NEWLINE)', 10
    db '(PRINT (LABELS ((FACT (N) (IF (= N 0) 1 (* N (FACT (- N 1)))))) (FACT 10)))', 10
    db '(NEWLINE)', 10
    db '(PRINT (LABELS ((F (EV) (LIST EV)) (EV () 1)) (F 5)))', 10
    db '(NEWLINE)', 10
    db '(PRINT (LET ((K 3)) (LABELS ((ADDK (X) (+ X K))) (MAPCAR (FUNCTION ADDK) (LIST 1 2)))))', 10
    db '(NEWLINE)', 10
    db '(PRINT (LIST (LABELS () 7) (LABELS ((F (&REST XS) XS)) (F 1 2 3))))', 10
    db '(NEWLINE)', 10
    db '(DEFINE BIG (IOTA 200000 1))', 10
    db '(PRINT (LIST (LENGTH BIG) (MEMBER 200000 BIG) (LENGTH (ARRAY->LIST (LIST->ARRAY BIG)))))', 10
    db '(NEWLINE)', 10
    db '(PRINT (LET ((N 0)) (DOLIST (X BIG) (SETQ N (+ N X))) N))', 10
    db '(NEWLINE)', 10
    db '(PRINT (LABELS ((EV (N) (IF (= N 0) T (OD (- N 1)))) (OD (N) (IF (= N 0) (QUOTE ()) (EV (- N 1))))) (OD 1000001)))', 10
prog_end:

section .text

run_buffer:
    call reader_init
.loop:
    call read_form
    cmp rax, IMM_EOF
    je .done
    mov rdi, rax
    mov qword [compile_nesting_depth], 0
    mov qword [rc_pin_depth], 0
    mov qword [current_lambda_depth], 0
    call compile_thunk
    push rax
    call qword [rsp]
    add rsp, 8
    jmp .loop
.done:
    ret

global lamedh_main
lamedh_main:
    sub rsp, 8
    mov rdi, prelude_start
    mov rsi, prelude_end - prelude_start
    call run_buffer
    mov rdi, prog_start
    mov rsi, prog_end - prog_start
    call run_buffer
    add rsp, 8
    xor eax, eax
    ret
