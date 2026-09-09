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
; COUNT is evaluated once, into a hygienic internal temporary: GENSYM
; (symtab.asm's gensym, exposed as the GENSYM builtin) runs here at
; macro-expansion time — the transformer body is ordinary compiled code
; invoked once per DOTIMES call site (invoke_macro, compiler.asm), so
; each expansion gets its own fresh, never-EQ-to-anything-else symbol
; spliced in as an unevaluated datum (COUNT-SYM below is a bare
; variable reference to that already-a-value symbol, not a QUOTEd
; literal name) — a nested DOTIMES can no longer collide with an outer
; one's own count variable the way a single fixed literal name would.
(DEFMACRO DOTIMES (SPEC &REST BODY)
  (LET ((COUNT-SYM (GENSYM)))
    (LIST (QUOTE LET)
          (LIST (LIST (CAR SPEC) 0)
                (LIST COUNT-SYM (CAR (CDR SPEC))))
          (LIST (QUOTE WHILE)
                (LIST (QUOTE <) (CAR SPEC) COUNT-SYM)
                (CONS (QUOTE PROGN)
                      (APPEND BODY
                              (LIST (LIST (QUOTE SETQ) (CAR SPEC)
                                          (LIST (QUOTE +) (CAR SPEC) 1)))))))))

; EQUAL — deep structural equality (KERNEL.md Part IV): EQ on either
; side being an atom, else the recursive conjunction of car and cdr.
; Library code in the reference too, over just EQ/ATOM/CAR/CDR, all
; kernel primitives here already.
(DEFUN EQUAL (A B)
  (IF (ATOM A)
      (EQ A B)
      (IF (ATOM B)
          (QUOTE ())
          (IF (EQUAL (CAR A) (CAR B)) (EQUAL (CDR A) (CDR B)) (QUOTE ())))))

; ASSOC — a genuine Rust-level builtin in the reference
; (evaluator/builtins_extra.rs), missing here until
; lib/27-modules.lisp's own DEFMODULE surfaced the gap
; (`(assoc ':export sections)`). Ordinary library code over EQUAL/
; ATOM/CAR/CDR, all already available — no new kernel primitive
; needed. `(NOT (ATOM (CAR ALIST)))` inlines what CONSP would check
; (not yet defined at this point in lamedh-asm's own small prelude;
; the reference's own 01-list.lisp defines CONSP identically as
; `(not (atom x))`) rather than depending on it. Matches the
; reference's own "malformed alist elements are skipped, not an
; error" graceful-degradation behavior; EQUAL rather than EQ for the
; key comparison, matching the reference's own structural `==`.
(DEFUN ASSOC (KEY ALIST)
  (IF (NULL ALIST)
      (QUOTE ())
      (IF (IF (ATOM (CAR ALIST)) (QUOTE ()) (EQUAL (CAR (CAR ALIST)) KEY))
          (CAR ALIST)
          (ASSOC KEY (CDR ALIST)))))

; MAPCAR — apply FN to each element of L, collecting the results.
; examples/fizzbuzz/main.lisp's own self-check uses this.
(DEFUN MAPCAR (FN L)
  (IF (NULL L) (QUOTE ()) (CONS (FN (CAR L)) (MAPCAR FN (CDR L)))))

; NUMBER->STRING — PRINC-TO-STRING already renders a fixnum as its
; plain decimal text (print_value's own fixnum case); this is just
; that primitive under the name examples/fizzbuzz/main.lisp expects.
(DEFUN NUMBER->STRING (N) (PRINC-TO-STRING N))

; GETP/PUTP — symbol property lists (KERNEL.md Part XI). SYMBOL-PLIST
; and SET-SYMBOL-PLIST! (compiler.asm/symtab.asm) are the only new
; kernel surface this needed: a symbol's plist slot is an ordinary
; CONS-built alist of (indicator . value) pairs, read/written exactly
; the way DEFINE/SETQ already read/write a symbol's separate value
; slot. GETP walks the alist looking for an EQ indicator match; PUTP
; always prepends a fresh (indicator . value) pair rather than
; searching for and replacing an existing one — GETP's own
; first-match-wins walk order means a later PUTP correctly shadows an
; earlier one for the same indicator, and cons cells staying immutable
; (Part XII axis 2) means there is no in-place update to do anyway.
;
; v0 scope, narrower than the reference on purpose: indicator equality
; here is EQ, not the reference's own name-text unification (its GETP/
; PUTP extract a symbol or string indicator's name text and key a
; per-symbol map by that text, so a symbol indicator and a string
; indicator spelling the same name are treated as identical property).
; This kernel's EQ is genuine value equality on strings now (see
; README "KERNEL.md conformance"), so two string indicators with the
; same text already unify correctly, and two symbol indicators of the
; same name already unify too (interning), but a *symbol* indicator
; and a *string* indicator sharing text do not unify with each other —
; an honest, narrower interpretation until a SYMBOL-NAME primitive
; exists to extract a symbol's name as its own string for GETP/PUTP to
; key on the same way the reference does.
(DEFUN GETP-ONTO (IND PL)
  (IF (NULL PL)
      (QUOTE ())
      (IF (EQ (CAR (CAR PL)) IND)
          (CDR (CAR PL))
          (GETP-ONTO IND (CDR PL)))))
(DEFUN GETP (SYM IND) (GETP-ONTO IND (SYMBOL-PLIST SYM)))
(DEFUN PUTP (SYM IND VAL)
  (SET-SYMBOL-PLIST! SYM (CONS (CONS IND VAL) (SYMBOL-PLIST SYM))))

; REMPROP — needs no new kernel primitive either, same as GETP/PUTP
; above: ordinary library code filtering SYMBOL-PLIST down and writing
; the result back via SET-SYMBOL-PLIST!. Removes every pair matching
; IND (by EQ, same indicator-equality rule as GETP/PUTP), not just the
; first — lib/00-core.lisp's own DEFUN macro calls this unconditionally
; on every expansion, before the indicator has necessarily ever been
; PUTP'd at all, so this must also be silently correct (a no-op) when
; IND isn't present.
(DEFUN REMPROP-ONTO (IND PL)
  (IF (NULL PL)
      (QUOTE ())
      (IF (EQ (CAR (CAR PL)) IND)
          (REMPROP-ONTO IND (CDR PL))
          (CONS (CAR PL) (REMPROP-ONTO IND (CDR PL))))))
(DEFUN REMPROP (SYM IND)
  (SET-SYMBOL-PLIST! SYM (REMPROP-ONTO IND (SYMBOL-PLIST SYM))))

; RPLACA/RPLACD — needs no new kernel primitive at all: the reference's
; own doc comment for both (evaluator/builtins_extra.rs) is explicit
; that "this implementation returns a NEW cons cell rather than
; modifying the original" precisely to keep cons cells immutable and
; circular lists impossible, which is exactly KERNEL.md Part XII axis
; 2's own requirement — a plain CONS of the replaced half onto the
; untouched other half already *is* that contract, character for
; character, with no compiler change needed.
(DEFUN RPLACA (C NEWCAR) (CONS NEWCAR (CDR C)))
(DEFUN RPLACD (C NEWCDR) (CONS (CAR C) NEWCDR))

; FUNCALL — needs no new kernel mechanism beyond APPLY (compiler.asm,
; a thin wrapper over invoke_macro, the same host routine DEFMACRO's
; own call sites already use to invoke an already-compiled closure
; with any number of argument values): by the time this ordinary
; DEFUN's own body runs, every one of its arguments is already
; evaluated, and &REST already collects the trailing ones into a
; fresh proper list — exactly the shape APPLY wants.
(DEFUN FUNCALL (FN &REST ARGS) (APPLY FN ARGS))

; >/>=/<=  — this kernel's only native comparison is `<` (and `=`);
; the rest are ordinary DEFUNs over it rather than new compiler
; special-cases, matching README's own "What's compiled" scope note
; that compile_binop only ever grew + - * < =.
(DEFUN > (A B) (< B A))
(DEFUN >= (A B) (NOT (< A B)))
(DEFUN <= (A B) (NOT (< B A)))

(DEFUN MAX (A B) (IF (< A B) B A))
(DEFUN MIN (A B) (IF (< A B) A B))

; DEF — the reference's own alternate top-level binding form
; (evaluator/special_forms.rs's SpecialForm::Def): like DEFINE, but
; evaluates to the *symbol* being defined rather than its value —
; `../examples/*/main.lisp`'s own idiom `(def $name expr)` relies on
; this being usable as an ordinary top-level statement whose own
; return value nobody cares about, freeing `$name` as the binding
; site. NAME is unevaluated syntax here (an ordinary macro parameter),
; matching the reference treating DEF's first operand as a literal
; symbol, never an expression to evaluate. The reference's optional
; third (docstring) operand is stored on the symbol's plist (indicator
; "docstring", a string — the same indicator-equality rule GETP/PUTP
; already document) — needed for `lib/00-core.lisp`'s own `defun`
; macro, whose `(def ,name ,lambda-expr ,doc)` expansion passes one
; whenever the DEFUN body led with a string literal.
(DEFMACRO DEF (NAME VAL &REST DOC)
  (IF (NULL DOC)
      (LIST (QUOTE PROGN)
            (LIST (QUOTE DEFINE) NAME VAL)
            (LIST (QUOTE QUOTE) NAME))
      (LIST (QUOTE PROGN)
            (LIST (QUOTE DEFINE) NAME VAL)
            (LIST (QUOTE PUTP) (LIST (QUOTE QUOTE) NAME) "docstring" (CAR DOC))
            (LIST (QUOTE QUOTE) NAME))))

; FOR-EACH/FILTER/SOME/EVERY — the reference's own versions
; (lib/29-protocols.lisp) are fn-first *protocols*, generically
; dispatching over lists, arrays, hash tables, and strings alike
; (DEFPROTOCOL/DEFINSTANCE, a full multi-type dispatch system this
; kernel doesn't have yet). v0 scope here, honestly narrower: plain
; recursive list-only versions, covering the overwhelmingly common
; case in practice (calling one of these on a list) without the
; generic-dispatch machinery. SOME/EVERY additionally simplify the
; reference's own contract of returning the *matching element* (SOME)
; or the last predicate result (EVERY) down to a plain T/NIL boolean.
(DEFUN FOR-EACH (FN L)
  (IF (NULL L)
      (QUOTE ())
      (PROGN (FN (CAR L)) (FOR-EACH FN (CDR L)))))

(DEFUN FILTER (PRED L)
  (IF (NULL L)
      (QUOTE ())
      (IF (PRED (CAR L))
          (CONS (CAR L) (FILTER PRED (CDR L)))
          (FILTER PRED (CDR L)))))

(DEFUN SOME (PRED L)
  (IF (NULL L)
      (QUOTE ())
      (IF (PRED (CAR L)) T (SOME PRED (CDR L)))))

(DEFUN EVERY (PRED L)
  (IF (NULL L)
      T
      (IF (PRED (CAR L)) (EVERY PRED (CDR L)) (QUOTE ()))))

; The real hash table (KERNEL.md Part IV/XI): MAKE-HASH-TABLE/SETHASH/
; GETHASH/REMHASH/KEYS, the exact names and arities Part XI requires —
; `(make-hash-table)` takes no arguments, `sethash`/`remhash` return
; `T`, `gethash` returns `NIL` for an absent key (indistinguishable
; from a stored `NIL`, per spec), `keys` returns the stored keys in
; unspecified order. Promoting tests/cases/022_hashtable_array.asm's
; own design to real library code: a fixed 61-bucket ARRAY, each slot
; an alist chain of (key . value) pairs, HASH-CODE+MOD picking the
; bucket, STORE mutating that one slot — no new kernel primitive
; needed, only ARRAY/FETCH/STORE/HASH-CODE/MOD plus the CONS/CAR/CDR/
; EQ/NULL this kernel already had. HT-* names below are this
; implementation's own private helpers, not part of the Part XI
; surface (a plain, unenforced naming convention — this kernel has no
; module system to actually hide them).
;
; v0 scope, narrower than the spec on purpose: key equality here is
; `EQ` (now genuine value equality on fixnums/floats/chars/strings/
; symbols, see README "KERNEL.md conformance"), not the spec's own
; full recursive `EQ`/`EQUAL` union, so a cons/array/lambda-shaped key
; does not yet find itself by structural content the way the reference
; requires; no resizing (a fixed 61 buckets, same as the array-alist
; demonstration this promotes); no bounded allocation ceiling
; (Part IV's array ceiling doesn't apply to a fixed-size bucket array
; anyway, since MAKE-HASH-TABLE takes no size argument to police).
(DEFUN HT-INDEX (KEY) (MOD (HASH-CODE KEY) 61))

(DEFUN HT-BUCKET-ASSOC (BUCKET KEY)
  (IF (NULL BUCKET)
      (QUOTE ())
      (IF (EQ (CAR (CAR BUCKET)) KEY)
          (CAR BUCKET)
          (HT-BUCKET-ASSOC (CDR BUCKET) KEY))))

(DEFUN HT-BUCKET-REMOVE (BUCKET KEY)
  (IF (NULL BUCKET)
      (QUOTE ())
      (IF (EQ (CAR (CAR BUCKET)) KEY)
          (HT-BUCKET-REMOVE (CDR BUCKET) KEY)
          (CONS (CAR BUCKET) (HT-BUCKET-REMOVE (CDR BUCKET) KEY)))))

(DEFUN HT-BUCKET-KEYS (BUCKET)
  (IF (NULL BUCKET)
      (QUOTE ())
      (CONS (CAR (CAR BUCKET)) (HT-BUCKET-KEYS (CDR BUCKET)))))

(DEFUN HT-KEYS-LOOP (TABLE I ACC)
  (IF (= I (ARRAY-LENGTH* TABLE))
      ACC
      (HT-KEYS-LOOP TABLE (+ I 1) (APPEND (HT-BUCKET-KEYS (FETCH TABLE I)) ACC))))

(DEFUN MAKE-HASH-TABLE () (ARRAY 61))

(DEFUN SETHASH (TABLE KEY VALUE)
  (STORE TABLE (HT-INDEX KEY)
         (CONS (CONS KEY VALUE) (HT-BUCKET-REMOVE (FETCH TABLE (HT-INDEX KEY)) KEY)))
  T)

(DEFUN GETHASH (TABLE KEY)
  (LET ((PAIR (HT-BUCKET-ASSOC (FETCH TABLE (HT-INDEX KEY)) KEY)))
    (IF (NULL PAIR) (QUOTE ()) (CDR PAIR))))

(DEFUN REMHASH (TABLE KEY)
  (STORE TABLE (HT-INDEX KEY) (HT-BUCKET-REMOVE (FETCH TABLE (HT-INDEX KEY)) KEY))
  T)

(DEFUN KEYS (TABLE) (HT-KEYS-LOOP TABLE 0 (QUOTE ())))

; QUASIQUOTE (KERNEL.md Part VII): the standard technique — a DEFMACRO
; whose transformer walks the (unevaluated) template at macro-expansion
; time and emits ordinary CONS/APPEND/QUOTE code that rebuilds it at
; runtime, evaluating each UNQUOTE'd subform in the caller's own
; environment when that generated code actually runs. No new kernel
; primitive needed — the reader's `` ` ``/`,`/`,@` macros
; (reader.asm) already produce (QUASIQUOTE tpl)/(UNQUOTE e)/
; (UNQUOTE-SPLICING e) forms exactly like `'` already produces
; (QUOTE x); everything past that is this macro, over CONS/CAR/CDR/EQ/
; APPEND/ATOM this kernel already had.
;
; QQ-EXPAND(form) mirrors the spec's own recursive rule exactly:
; - an atom rebuilds as itself: (QUOTE form).
; - a cons that IS (UNQUOTE e) (checked wherever this recursion reaches
;   a cons, not just at the top — this is what gives "no nesting-level
;   tracking" for free: an inner `,` is found and substituted the same
;   way regardless of how many enclosing (QUASIQUOTE ...) conses sit
;   around it, since QUASIQUOTE is just an ordinary symbol to this
;   walk, never special-cased) rebuilds as e itself, to be evaluated
;   when the generated code runs.
; - otherwise, if this cons's CAR is itself (UNQUOTE-SPLICING e) — "a
;   list element" position — rebuilds as (APPEND e (QQ-EXPAND cdr)):
;   e's value (which must be a proper list) is spliced in, followed by
;   the processed rest, which works before a dotted tail too, since
;   the tail is just wherever this same recursion's CDR call bottoms
;   out.
; - otherwise, rebuilds as (CONS (QQ-EXPAND car) (QQ-EXPAND cdr)) —
;   an ordinary cons, processed structurally on both halves.
(DEFUN QQ-UNQUOTE-FORM-P (FORM)
  (IF (ATOM FORM) (QUOTE ()) (EQ (CAR FORM) (QUOTE UNQUOTE))))

(DEFUN QQ-SPLICE-HEAD-P (X)
  (IF (ATOM X) (QUOTE ()) (EQ (CAR X) (QUOTE UNQUOTE-SPLICING))))

(DEFUN QQ-EXPAND (FORM)
  (IF (ATOM FORM)
      (LIST (QUOTE QUOTE) FORM)
      (IF (QQ-UNQUOTE-FORM-P FORM)
          (CAR (CDR FORM))
          (IF (QQ-SPLICE-HEAD-P (CAR FORM))
              (LIST (QUOTE APPEND) (CAR (CDR (CAR FORM))) (QQ-EXPAND (CDR FORM)))
              (LIST (QUOTE CONS) (QQ-EXPAND (CAR FORM)) (QQ-EXPAND (CDR FORM)))))))

(DEFMACRO QUASIQUOTE (TEMPLATE) (QQ-EXPAND TEMPLATE))

; FOR (KERNEL.md Part VII): "(for (var start end [step]) body...)
; evaluates start, end, and step once (fixnums; a zero step is an
; error), iterates var from start to end *inclusive* in one reused
; frame, and returns NIL" — explicitly licensed to be derived from
; LET/WHILE/tail-calls (Part XII axis 3), same recipe DOTIMES already
; uses: a LET binds var/end/step exactly once, WHILE re-tests a
; direction-aware continuation predicate, and the body's own trailing
; SETQ mutates var *in place* rather than rebinding it — which is
; exactly what gives "one reused frame every closure in the body
; shares" for free, the same way it already does for DOTIMES.
; GENSYM (not a fixed internal name) for the end/step bindings, so a
; nested FOR can't collide with an outer one's — the same hygiene fix
; DOTIMES itself needed (see "KERNEL.md conformance" above).
(DEFUN FOR-STEP-OF (SPEC)
  (IF (NULL (CDR (CDR (CDR SPEC))))
      1
      (CAR (CDR (CDR (CDR SPEC))))))

(DEFUN FOR-CHECK-STEP (STEP)
  (IF (= STEP 0) (ERROR "FOR: step must be non-zero" STEP) STEP))

; direction-aware inclusive bound test: counting up (step > 0)
; continues while var <= end; counting down continues while end <= var.
(DEFUN FOR-CONTINUE-P (VAR END STEP)
  (IF (< 0 STEP) (<= VAR END) (<= END VAR)))

(DEFMACRO FOR (SPEC &REST BODY)
  (LET ((VAR (CAR SPEC))
        (START (CAR (CDR SPEC)))
        (END (CAR (CDR (CDR SPEC))))
        (STEP-FORM (FOR-STEP-OF SPEC))
        (END-SYM (GENSYM))
        (STEP-SYM (GENSYM)))
    (LIST (QUOTE LET)
          (LIST (LIST VAR START)
                (LIST END-SYM END)
                (LIST STEP-SYM (LIST (QUOTE FOR-CHECK-STEP) STEP-FORM)))
          (LIST (QUOTE WHILE)
                (LIST (QUOTE FOR-CONTINUE-P) VAR END-SYM STEP-SYM)
                (CONS (QUOTE PROGN)
                      (APPEND BODY
                              (LIST (LIST (QUOTE SETQ) VAR
                                          (LIST (QUOTE +) VAR STEP-SYM))))))
          (QUOTE ()))))

; WITH-CAPABILITIES (KERNEL.md Part IX): "list-form is evaluated ...
; the new mask is the requested names when no mask is active,
; otherwise the intersection with the enclosing mask ... on exit by
; any path — completion, error, non-local exit — the previous mask is
; restored exactly." That exact "restore on any exit" guarantee is
; precisely what UNWIND-PROTECT (just added, see "KERNEL.md
; conformance" above) already provides, so this is a two-line
; derivation over it plus two small new kernel primitives
; (capabilities.asm): PUSH-CAPABILITY-MASK! evaluates list-form,
; computes the intersected (or outright, if unset) mask, saves the
; previous one, and installs the new one; POP-CAPABILITY-MASK!
; restores it. No new special-form machinery needed at all — Part XII
; axis 3's own license, applied here the same way DOTIMES/FOR/BLOCK
; already were.
(DEFMACRO WITH-CAPABILITIES (LIST-FORM &REST BODY)
  (LIST (QUOTE PROGN)
        (LIST (QUOTE PUSH-CAPABILITY-MASK!) LIST-FORM)
        (LIST (QUOTE UNWIND-PROTECT)
              (CONS (QUOTE PROGN) BODY)
              (LIST (QUOTE POP-CAPABILITY-MASK!)))))
