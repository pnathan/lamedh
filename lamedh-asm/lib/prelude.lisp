; prelude.lisp — a bootstrap library, loaded by lamedhc (file_runner.asm)
; before any user program. Every form here compiles through DEFMACRO/
; DEFINE/LAMBDA/CONS/QUOTE — plain kernel primitives, no compiler
; change needed for any of it, proving out the project's own central
; claim: "prefer the Lisp layer; keep the kernel small" (see the repo
; root's AGENTS.md, and lamedh-asm's own README "kernel surface"
; section) holds even for the standard library's own bootstrap.
;
; DEFMACRO's parameter list must spell a rest parameter as "&REST sym"
; (never a raw dotted tail "(a . b)") — this kernel's split_rest_params
; only recognizes that one spelling (see compiler.asm; a genuinely
; dotted parameter list is not supported and misbehaves).
;
; A macro call site's operand count is no longer capped at 3
; (invoke_macro, compiler.asm, forwards every operand — see the
; README's "kernel surface" section), so DEFUN and WHEN/UNLESS below
; take any number of body forms directly, the same way LAMBDA's own
; body already does.

(DEFMACRO DEFUN (NAME PARAMS &REST BODY)
  (CONS (QUOTE DEFINE)
        (CONS NAME
              (CONS (CONS (QUOTE LAMBDA) (CONS PARAMS BODY))
                    (QUOTE ())))))

(DEFUN NOT (X) (IF X (QUOTE ()) T))

; WHEN/UNLESS — the one-armed IF forms KERNEL.md Part VI's own IF
; section names as the reason IF itself stays strictly two-armed: IF
; takes *exactly* three operands here (test/then/else), so each
; expansion must supply an explicit NIL else-branch, not omit it, and
; wraps its (possibly multiple) body forms in PROGN, matching COND's
; own "last body form is in tail position" convention.
(DEFMACRO WHEN (TEST &REST BODY)
  (CONS (QUOTE IF)
        (CONS TEST
              (CONS (CONS (QUOTE PROGN) BODY)
                    (CONS (QUOTE ()) (QUOTE ()))))))

(DEFMACRO UNLESS (TEST &REST BODY)
  (CONS (QUOTE IF)
        (CONS TEST
              (CONS (QUOTE ())
                    (CONS (CONS (QUOTE PROGN) BODY) (QUOTE ()))))))

; LIST — an ordinary function, not a macro: every argument is already
; evaluated before this runs, and a &REST parameter already collects
; exactly that evaluated surplus into a fresh proper list, so the rest
; parameter itself *is* the answer.
(DEFUN LIST (&REST ITEMS) ITEMS)

(DEFUN REVERSE-ONTO (L ACC)
  (IF (NULL L) ACC (REVERSE-ONTO (CDR L) (CONS (CAR L) ACC))))
(DEFUN REVERSE (L) (REVERSE-ONTO L (QUOTE ())))

; FORMAT — every example in ../examples/*/main.lisp uses this (README
; Roadmap). A DEFMACRO, not a function: its control string is a literal
; (self-evaluating) argument, so the macro transformer receives the
; actual string value directly as unevaluated syntax and can walk its
; bytes with STRING-REF at macro-expansion (compile) time, splitting it
; into literal runs (each becoming a PRINT of that substring) and `~a`
; directives (each becoming a PRINT of the corresponding, still-
; unevaluated, argument form) — the expansion is an ordinary PROGN of
; PRINT/NEWLINE calls, run once the expansion is itself compiled and
; executed, not built by any runtime variadic mechanism.
;
; v0 scope, honestly narrow rather than silently wrong: only `~a` and
; `~%` are recognized (every other directive's `~` and the following
; character are copied through literally — including `~~`, which is
; therefore not an escape for a literal tilde yet); the STREAM operand
; is accepted but ignored — `(format t ...)` and `(format nil ...)`
; currently behave identically, always writing to stdout, so
; `(format nil ...)`'s "return a string instead" behavior is not yet
; implemented (PRINC-TO-STRING/STRING-APPEND exist and could build one,
; a natural next step); and running out of ARGS before the control
; string's own `~a` count runs out prints `()` for the missing ones
; (CAR of NIL is NIL, not an error, so this degrades rather than
; crashing) instead of signaling anything.
; NOT (< a b) rather than (>= a b): this kernel's compile_binop only
; ever grew "+ - * < =" (see README "What's compiled") — there is no
; native >, >=, or <=.
(DEFUN FORMAT-FIND-TILDE (CTRL I)
  (IF (NOT (< I (STRING-LENGTH CTRL)))
      I
      (IF (= (STRING-REF CTRL I) 126)
          I
          (FORMAT-FIND-TILDE CTRL (+ I 1)))))

(DEFUN FORMAT-BUILD (CTRL IDX ARGS ACC)
  (IF (NOT (< IDX (STRING-LENGTH CTRL)))
      ACC
      (IF (NOT (= (STRING-REF CTRL IDX) 126))
          (LET ((NEXT (FORMAT-FIND-TILDE CTRL IDX)))
            (FORMAT-BUILD CTRL NEXT ARGS
              (CONS (LIST (QUOTE PRINT) (SUBSTRING CTRL IDX NEXT)) ACC)))
          (IF (NOT (< (+ IDX 1) (STRING-LENGTH CTRL)))
              ACC
              (IF (= (STRING-REF CTRL (+ IDX 1)) 97)
                  (FORMAT-BUILD CTRL (+ IDX 2) (CDR ARGS)
                    (CONS (LIST (QUOTE PRINT) (CAR ARGS)) ACC))
                  (IF (= (STRING-REF CTRL (+ IDX 1)) 37)
                      (FORMAT-BUILD CTRL (+ IDX 2) ARGS
                        (CONS (LIST (QUOTE NEWLINE)) ACC))
                      (FORMAT-BUILD CTRL (+ IDX 2) ARGS ACC)))))))

(DEFMACRO FORMAT (STREAM CTRL &REST ARGS)
  (CONS (QUOTE PROGN) (REVERSE (FORMAT-BUILD CTRL 0 ARGS (QUOTE ())))))

; 1+/1- — the reader's own "1+"/"1-" two-character literal symbol
; production (KERNEL.md Part II) makes these ordinary names, not
; special syntax; examples/factorial/main.lisp uses (1+ i) directly.
(DEFUN 1+ (N) (+ N 1))
(DEFUN 1- (N) (- N 1))

; +/-/*/</= as ordinary global closures, not just inline operators.
; `(+ a b)` in *operator* position always compiles to the fast inline
; path (compile_binop) regardless of these definitions — that dispatch
; is checked before an ordinary function call ever would be — but nothing
; before this bound the *bare symbol* +/-/*/</= to a callable value, so
; passing one as a higher-order argument (`(REDUCE #'* ...)`,
; examples/factorial/main.lisp's own usage) had nothing to resolve to.
; #'+ (FUNCTION, compiler.asm) then just reads this ordinary global
; value, the same as referencing +/-/*/</= as a bare variable would.
(DEFUN + (A B) (+ A B))
(DEFUN - (A B) (- A B))
(DEFUN * (A B) (* A B))
(DEFUN < (A B) (< A B))
(DEFUN = (A B) (= A B))

(DEFUN APPEND (A B) (IF (NULL A) B (CONS (CAR A) (APPEND (CDR A) B))))

; IOTA — (iota n start) is the n-element list (start start+1 ... start+n-1).
(DEFUN IOTA-ONTO (N START ACC)
  (IF (= N 0) (REVERSE ACC) (IOTA-ONTO (- N 1) (+ START 1) (CONS START ACC))))
(DEFUN IOTA (N START) (IOTA-ONTO N START (QUOTE ())))

; REDUCE — a left fold: (reduce fn (a b c) init) is (fn (fn (fn init a) b) c).
; FN is an ordinary value here (ordinarily #'some-global), not unevaluated
; syntax, so calling it is just an ordinary application through whatever
; variable holds it — ".indirect_path" (compile_call, compiler.asm)
; already supports an arbitrary expression in operator position; no
; separate FUNCALL primitive is needed.
(DEFUN REDUCE (FN L ACC)
  (IF (NULL L) ACC (REDUCE FN (CDR L) (FN ACC (CAR L)))))

; DOTIMES — (dotimes (var count) body...) runs body with var bound to
; 0, 1, ..., count-1 in turn, derived from LET/WHILE/SETQ (all kernel
; primitives — KERNEL.md Part XII axis 3 explicitly allows this).
; COUNT is evaluated once, into a fixed internal temporary name — a
; real but narrow v0 limitation (no GENSYM yet, see README): a nested
; DOTIMES using that exact name as its own loop variable would collide.
(DEFMACRO DOTIMES (SPEC &REST BODY)
  (LIST (QUOTE LET)
        (LIST (LIST (CAR SPEC) 0)
              (LIST (QUOTE DOTIMES-COUNT) (CAR (CDR SPEC))))
        (LIST (QUOTE WHILE)
              (LIST (QUOTE <) (CAR SPEC) (QUOTE DOTIMES-COUNT))
              (CONS (QUOTE PROGN)
                    (APPEND BODY
                            (LIST (LIST (QUOTE SETQ) (CAR SPEC)
                                        (LIST (QUOTE +) (CAR SPEC) 1))))))))
