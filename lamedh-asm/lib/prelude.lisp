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
