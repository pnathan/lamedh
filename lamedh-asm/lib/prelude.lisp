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
; Every macro below takes at most 3 *syntactic operands at its own call
; site* — not to be confused with how many fixed parameters a LAMBDA or
; DEFUN-defined function may take, which is unrestricted (see the
; README's "Calling convention" section on &REST). A macro transformer
; is invoked from host code, at compile time, through
; raw_args_to_regs/invoke_closure_host (compiler.asm), which only ever
; forwards the *first three* unevaluated operand forms at a call site,
; silently dropping anything past the third — a separate, real
; limitation from the &REST/nfixed restriction that was lifted, and not
; yet fixed (see README Roadmap). DEFUN and WHEN/UNLESS are therefore
; deliberately single-body-form here: `(DEFUN NAME (PARAMS) BODY)` is
; 3 operands (NAME, PARAMS, BODY) and `(WHEN TEST BODY)` is 2 — a
; second body form at either call site would silently vanish rather
; than erroring. A caller wanting more than one body form must wrap it
; in an explicit PROGN: `(DEFUN NAME (PARAMS) (PROGN form1 form2))`.

(DEFMACRO DEFUN (NAME PARAMS BODY)
  (CONS (QUOTE DEFINE)
        (CONS NAME
              (CONS (CONS (QUOTE LAMBDA) (CONS PARAMS (CONS BODY (QUOTE ()))))
                    (QUOTE ())))))

(DEFUN NOT (X) (IF X (QUOTE ()) T))

; WHEN/UNLESS — the one-armed IF forms KERNEL.md Part VI's own IF
; section names as the reason IF itself stays strictly two-armed: IF
; takes *exactly* three operands here (test/then/else), so WHEN's
; expansion must supply an explicit NIL else-branch, not omit it. Each
; call site is 2 operands (TEST, BODY), well within the 3-operand
; macro-invocation cap explained above.
(DEFMACRO WHEN (TEST BODY)
  (CONS (QUOTE IF) (CONS TEST (CONS BODY (CONS (QUOTE ()) (QUOTE ()))))))

(DEFMACRO UNLESS (TEST BODY)
  (CONS (QUOTE IF) (CONS TEST (CONS (QUOTE ()) (CONS BODY (QUOTE ()))))))

; LIST — an ordinary function, not a macro, so it is not subject to
; the 3-operand cap above at all: every argument is already evaluated
; before this runs (through the general application path, which
; already supports any number of arguments — see "Calling convention"),
; and a &REST parameter already collects exactly that evaluated surplus
; into a fresh proper list, so the rest parameter itself *is* the
; answer.
(DEFUN LIST (&REST ITEMS) ITEMS)
