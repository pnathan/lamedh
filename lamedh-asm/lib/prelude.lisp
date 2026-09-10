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

; --- &OPTIONAL / &KEY parameter lists ---------------------------------
;
; (DEFUN F (A &OPTIONAL B (C 10) &KEY (D 2) E &REST R) ...) expands to a
; variadic LAMBDA (only &REST is a real kernel parameter-list feature —
; see the file header above) plus a LET* prologue that peels optionals
; positionally off the rest-list (a later default may reference an
; earlier parameter, since LET* binds sequentially), binds &REST to
; whatever remains after that peeling, and reads &KEY parameters from
; that same remainder as a :KEYWORD plist. A bare LAMBDA still supports
; only &REST; this sugar is DEFUN-level only, ported line-for-line in
; spirit from the reference implementation's own lib/00-core.lisp
; ($split-params/$opt-bindings/$key-bindings/$extended-lambda) minus
; the JIT/purity machinery this project doesn't have. Every helper
; below is plain-arg-or-&REST only (no &OPTIONAL/&KEY of its own), so
; none of them depend on the very DEFMACRO they help implement, and
; each is built from CONS/QUOTE templates rather than backquote, which
; is itself only usable once QQ-EXPAND is defined much later in this
; file (see that section's own header comment).

; $PARAMS-EXTENDED-P(ps) -> T iff ps contains &OPTIONAL or &KEY.
(DEFINE $PARAMS-EXTENDED-P
  (LAMBDA (PS)
    (IF (ATOM PS)
        (QUOTE ())
        (IF (EQ (CAR PS) (QUOTE &OPTIONAL))
            T
            (IF (EQ (CAR PS) (QUOTE &KEY))
                T
                ($PARAMS-EXTENDED-P (CDR PS)))))))

; $KEY-LOOKUP(plist key default) — a plain :KEYWORD-plist lookup, EQ on
; the keyword. Generated code (from $KEY-BINDINGS below) calls this at
; runtime once per &KEY parameter; it is not itself part of any macro
; expansion.
(DEFINE $KEY-LOOKUP
  (LAMBDA (PLIST KEY DEFAULT)
    (IF (ATOM PLIST)
        DEFAULT
        (IF (EQ (CAR PLIST) KEY)
            (CAR (CDR PLIST))
            ($KEY-LOOKUP (CDR (CDR PLIST)) KEY DEFAULT)))))

; $PARAM-KEYWORD(sym) -> the :SYM keyword symbol a caller passes this
; &KEY parameter's value under. STRING-APPEND/PRINC-TO-STRING, not
; CONCAT, since CONCAT is itself a DEFUN defined later in this file —
; using it here would make $PARAM-KEYWORD depend on load order.
(DEFINE $PARAM-KEYWORD
  (LAMBDA (SYM) (INTERN (STRING-APPEND ":" (PRINC-TO-STRING SYM)))))

; $SPLIT-PARAMS(ps mode fixed opts rest keys) -> (fixed opts rest keys),
; each of opts/keys normalized to a (sym default) pair (an atom spec
; like plain B becomes (B ())). mode starts as (QUOTE FIX) and switches
; to OPT/KEY on seeing &OPTIONAL/&KEY; &REST is recognized in any mode
; and consumes exactly the one symbol after it.
; $CHECK-PARAM-SPEC — an &OPTIONAL/&KEY spec is (name default) or a
; bare name, exactly as in the reference. A third element (Common
; Lisp's supplied-p variable, `(B 10 B-P)`) is NOT supported by the
; reference either, and used to be silently ignored here — B-P then
; read as an unbound global. Now it is a compile-time error.
(DEFINE $CHECK-PARAM-SPEC
  (LAMBDA (SPEC)
    (IF (ATOM (CDR (CDR SPEC)))
        SPEC
        (ERROR "DEFUN: parameter spec must be (name default); supplied-p variables are not supported" SPEC))))

(DEFINE $SPLIT-PARAMS
  (LAMBDA (PS MODE FIXED OPTS REST KEYS)
    (IF (ATOM PS)
        (CONS FIXED (CONS OPTS (CONS REST (CONS KEYS (QUOTE ())))))
        (IF (EQ (CAR PS) (QUOTE &OPTIONAL))
            ($SPLIT-PARAMS (CDR PS) (QUOTE OPT) FIXED OPTS REST KEYS)
            (IF (EQ (CAR PS) (QUOTE &KEY))
                ($SPLIT-PARAMS (CDR PS) (QUOTE KEY) FIXED OPTS REST KEYS)
                (IF (EQ (CAR PS) (QUOTE &REST))
                    ($SPLIT-PARAMS (CDR (CDR PS)) MODE FIXED OPTS
                                   (CAR (CDR PS)) KEYS)
                    (IF (EQ MODE (QUOTE FIX))
                        ($SPLIT-PARAMS (CDR PS) MODE
                                       (APPEND FIXED (CONS (CAR PS) (QUOTE ())))
                                       OPTS REST KEYS)
                        (IF (EQ MODE (QUOTE OPT))
                            ($SPLIT-PARAMS (CDR PS) MODE FIXED
                                           (APPEND OPTS
                                                   (CONS (IF (ATOM (CAR PS))
                                                                  (CONS (CAR PS) (CONS (QUOTE ()) (QUOTE ())))
                                                                  ($CHECK-PARAM-SPEC (CAR PS)))
                                                         (QUOTE ())))
                                           REST KEYS)
                            ($SPLIT-PARAMS (CDR PS) MODE FIXED OPTS REST
                                           (APPEND KEYS
                                                   (CONS (IF (ATOM (CAR PS))
                                                                  (CONS (CAR PS) (CONS (QUOTE ()) (QUOTE ())))
                                                                  ($CHECK-PARAM-SPEC (CAR PS)))
                                                         (QUOTE ()))))))))))))

; $OPT-BINDINGS(opts g) -> a LET*-bindings list, two per optional: the
; parameter itself (CAR g, if g is still a cons — i.e. an argument was
; actually supplied there — else its default expression), then a
; rebinding of g to (CDR g) or NIL. The rebinding is why this must run
; inside a LET* (sequential), not a LET: each later optional's own
; "was there an argument here" test reads the g this same list already
; advanced. "g is still a cons" is tested as (NOT (ATOM g)), not
; (CONSP g) — CONSP isn't a real primitive in this kernel's own small
; prelude (ASSOC, above, makes exactly the same choice for the same
; reason).
(DEFINE $OPT-BINDINGS
  (LAMBDA (OPTS G)
    (IF (ATOM OPTS)
        (QUOTE ())
        (CONS (CONS (CAR (CAR OPTS))
                    (CONS (CONS (QUOTE IF)
                                (CONS (CONS (QUOTE NOT) (CONS (CONS (QUOTE ATOM) (CONS G (QUOTE ()))) (QUOTE ())))
                                      (CONS (CONS (QUOTE CAR) (CONS G (QUOTE ())))
                                            (CONS (CAR (CDR (CAR OPTS))) (QUOTE ())))))
                          (QUOTE ())))
              (CONS (CONS G
                          (CONS (CONS (QUOTE IF)
                                      (CONS (CONS (QUOTE NOT) (CONS (CONS (QUOTE ATOM) (CONS G (QUOTE ()))) (QUOTE ())))
                                            (CONS (CONS (QUOTE CDR) (CONS G (QUOTE ())))
                                                  (CONS (QUOTE ()) (QUOTE ())))))
                                (QUOTE ())))
                    ($OPT-BINDINGS (CDR OPTS) G))))))

; $KEY-BINDINGS(keys g) -> a LET*-bindings list, one per &KEY parameter,
; each reading g (by now the remainder after every optional has been
; peeled off it, i.e. the true &REST tail) as a :KEYWORD plist via
; $KEY-LOOKUP.
(DEFINE $KEY-BINDINGS
  (LAMBDA (KEYS G)
    (IF (ATOM KEYS)
        (QUOTE ())
        (CONS (CONS (CAR (CAR KEYS))
                    (CONS (CONS (QUOTE $KEY-LOOKUP)
                                (CONS G
                                      (CONS (CONS (QUOTE QUOTE)
                                                  (CONS ($PARAM-KEYWORD (CAR (CAR KEYS))) (QUOTE ())))
                                            (CONS (CAR (CDR (CAR KEYS))) (QUOTE ())))))
                          (QUOTE ())))
              ($KEY-BINDINGS (CDR KEYS) G)))))

; $EXTENDED-LAMBDA(params body) -> (LAMBDA (fixed... &REST g) (LET*
; (bindings...) . body)) — the whole point: only FIXED and a single
; synthetic &REST parameter ever reach the real kernel LAMBDA; every
; &OPTIONAL/&KEY parameter becomes an ordinary lexical LET* binding
; computed from that &REST tail.
(DEFINE $EXTENDED-LAMBDA
  (LAMBDA (PARAMS BODY)
    (LET* ((G (GENSYM))
           (SPLIT ($SPLIT-PARAMS PARAMS (QUOTE FIX) (QUOTE ()) (QUOTE ()) (QUOTE ()) (QUOTE ())))
           (FIXED (CAR SPLIT))
           (OPTS (CAR (CDR SPLIT)))
           (REST-SYM (CAR (CDR (CDR SPLIT))))
           (KEYS (CAR (CDR (CDR (CDR SPLIT)))))
           (BINDINGS (APPEND ($OPT-BINDINGS OPTS G)
                             (APPEND (IF REST-SYM
                                         (CONS (CONS REST-SYM (CONS G (QUOTE ()))) (QUOTE ()))
                                         (QUOTE ()))
                                     ($KEY-BINDINGS KEYS G)))))
      (CONS (QUOTE LAMBDA)
            (CONS (APPEND FIXED (CONS (QUOTE &REST) (CONS G (QUOTE ()))))
                  (CONS (CONS (QUOTE LET*) (CONS BINDINGS BODY)) (QUOTE ())))))))

(DEFMACRO DEFUN (NAME PARAMS &REST BODY)
  (CONS (QUOTE DEFINE)
        (CONS NAME
              (CONS (IF ($PARAMS-EXTENDED-P PARAMS)
                        ($EXTENDED-LAMBDA PARAMS BODY)
                        (CONS (QUOTE LAMBDA) (CONS PARAMS BODY)))
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

; $LENGTH — the reference's own Rust-level builtin (evaluator/builtins_
; core.rs) backing lib/01-list.lisp's LENGTH wrapper. No host-
; representation access is needed to count a proper list's own cons
; cells, so this is an ordinary recursive Lisp definition rather than a
; new kernel primitive, per this project's stated preference. Never
; called at all until something in the reference stdlib actually
; invokes LENGTH (lib/25-variants.lisp's DEFVARIANT does, via
; `(length params)`), which is why an entirely unbound $LENGTH went
; unnoticed until now.
(DEFUN $LENGTH (LST) (IF (NULL LST) 0 (+ 1 ($LENGTH (CDR LST)))))

; $LIST->ARRAY/$ARRAY->LIST — the reference's own Rust-level builtins
; (evaluator/builtins_core.rs) backing lib/17-arrays.lisp's LIST->ARRAY/
; ARRAY->LIST wrappers. Ordinary Lisp over this kernel's own ARRAY
; (= MAKE-ARRAY, allocates N NIL-filled slots), FETCH, STORE, and
; ARRAY-LENGTH* special forms — ARRAY mutation (STORE on a freshly
; allocated, not-yet-shared array) is exactly the documented allowed
; case (lib/17-arrays.lisp's own header comment: "arrays are mutable;
; only cons aliasing is the concern"). Never called until
; lib/30-text.lisp's STRING->UTF8 goes through LIST->ARRAY of one
; CODE-CHAR per byte.
(DEFUN $LIST->ARRAY-FILL! (ARR LST I)
  (IF (NULL LST)
      ARR
      (PROGN (STORE ARR I (CAR LST))
             ($LIST->ARRAY-FILL! ARR (CDR LST) (+ I 1)))))
(DEFUN $LIST->ARRAY (LST)
  ($LIST->ARRAY-FILL! (ARRAY ($LENGTH LST)) LST 0))
(DEFUN $ARRAY->LIST-LOOP (ARR I N)
  (IF (NOT (< I N)) (QUOTE ()) (CONS (FETCH ARR I) ($ARRAY->LIST-LOOP ARR (+ I 1) N))))
(DEFUN $ARRAY->LIST (ARR) ($ARRAY->LIST-LOOP ARR 0 (ARRAY-LENGTH* ARR)))

; STRING->UTF8*/UTF8->STRING*/UTF8->STRING-LOSSY* — the three genuine
; Rust-level builtins (evaluator/builtins_core.rs) lib/30-text.lisp's
; TEXT module wraps. This kernel's own STRING representation
; (tags.inc's HDR_STRING: "[8 len][len bytes, padded]") already stores
; a string's raw UTF-8 bytes directly — STRING-LENGTH is a BYTE count,
; not a codepoint count ("héllo" is 6, not 5) — so STRING->UTF8 is
; nearly a straight byte copy into an Array<Char>, and UTF8->STRING is
; its inverse. KNOWN LIMITATION: this kernel does no UTF-8
; well-formedness validation at all (no Rust `std::str::from_utf8`
; equivalent exists here) — UTF8->STRING* and UTF8->STRING-LOSSY* are
; therefore identical, unlike the reference, where UTF8->STRING errors
; on malformed input and UTF8->STRING-LOSSY replaces it with U+FFFD.
; Honest for this v0 scope (matching this project's stated preference
; for the Lisp layer over new kernel work), not a hidden gap.
;
; $CHAR-ARRAY-ELEM-BYTE normalizes one Array<Char> element to its raw
; byte value: found necessary (not just a defensive nicety) by
; lib/31-ports.lisp's own $READ-LINE-ACC!, which builds its
; Array<Char> from PORT-READ-BYTE!'s raw-integer results and hands it
; straight to TEXT:UTF8->STRING-LOSSY — so an element here is
; genuinely EITHER a one-character string (this kernel's own CODE-CHAR
; produces those, not a genuine Char immediate — see byte_value_of's
; own comment, ports.asm) OR a bare fixnum 0-255, never a Char
; immediate in any real call path this kernel's own Lisp code takes.
(DEFUN $CHAR-ARRAY-ELEM-BYTE (X)
  (IF (STRINGP X) (STRING-REF X 0) (IF (CHARP X) (CHAR-CODE X) X)))
(DEFUN $STRING->UTF8-LOOP (S I N)
  (IF (NOT (< I N))
      (QUOTE ())
      (CONS (CODE-CHAR (STRING-REF S I)) ($STRING->UTF8-LOOP S (+ I 1) N))))
(DEFUN STRING->UTF8* (S)
  ($LIST->ARRAY ($STRING->UTF8-LOOP S 0 (STRING-LENGTH S))))
(DEFUN $UTF8->STRING-LOOP (ARR I N)
  (IF (NOT (< I N))
      ""
      (STRING-APPEND (CODE-CHAR ($CHAR-ARRAY-ELEM-BYTE (FETCH ARR I)))
                     ($UTF8->STRING-LOOP ARR (+ I 1) N))))
(DEFUN UTF8->STRING* (ARR)
  ($UTF8->STRING-LOOP ARR 0 (ARRAY-LENGTH* ARR)))
(DEFUN UTF8->STRING-LOSSY* (ARR)
  (UTF8->STRING* ARR))

; CAR/CDR/CONS/ATOM/EQ/NULL as ordinary global closures, exactly the same
; bridge as +/-/*/</= above, for exactly the same reason: each of these is
; ALSO a compiler.asm special form dispatched purely at compile time in
; *operator* position (compile_form checks this dispatch before an
; ordinary call ever would, so `(CAR X)` etc. keep the fast inline path
; unconditionally) — but with no bridge, the bare symbol CAR/CDR/CONS/
; ATOM/EQ/NULL had nothing bound to it as a value, so `#'CAR` (FUNCTION,
; compiler.asm) compiled as an ordinary variable read of a permanently
; unbound global, and passing one as a higher-order argument (`(MAPCAR
; #'CAR field-specs)`, lib/20-condensation.lisp's condense-field-names,
; needed transitively by lib/25-variants.lisp's DEFVARIANT) trapped.
(DEFUN CAR (X) (CAR X))
(DEFUN CDR (X) (CDR X))
(DEFUN CONS (A B) (CONS A B))
(DEFUN ATOM (X) (ATOM X))
(DEFUN EQ (A B) (EQ A B))
(DEFUN NULL (X) (NULL X))

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

; RECORD-DECLARE/VARIANT-DECLARE/DECLARE-TYPE! — three genuine
; Rust-level builtins (environment.rs) that register CHECKER-ONLY
; metadata (field-name/type declarations for the reference's own HM
; type checker) with no runtime effect on values at all. This kernel
; has no type checker of any kind, so these are honest no-ops: every
; real call site (lib/20-condensation.lisp's DEFRECORD, lib/25-
; variants.lisp's DEFVARIANT) passes only QUOTE'd literal data with no
; side effects to lose by skipping the registration, and RECORD-NEW
; (compiler.asm) needs no field-count lookup from RECORD-DECLARE's own
; registry either — every generated constructor already keeps its own
; field count in sync with what it declares, by construction, so
; there is nothing here for a checker-less kernel to validate against.
; RECORD-DECLARE is NOT a pure no-op after all: it is the only place
; that ever learns a record brand's field NAMES in order (RECORD-NEW
; only ever sees positional values, tags.inc's own HDR_RECORD layout
; stores no field names at all) — and RECORD-REF (below) needs exactly
; that name-to-position mapping to resolve `(record-ref self 'field)`
; the way every DEFRECORD/DEFVARIANT-generated getter calls it. So this
; records field-name order on the brand symbol's plist (GETP/PUTP,
; above) as a byproduct of what is otherwise still checker-only
; metadata; CTOR-SPEC is either a bare brand symbol or `(brand
; params...)` for a parametric record/variant, and only the brand
; (the CAR, when it's a list) matters here.
(DEFUN RECORD-DECLARE (CTOR-SPEC FIELD-SPECS)
  (PUTP (IF (CONSP CTOR-SPEC) (CAR CTOR-SPEC) CTOR-SPEC)
        "RECORD-FIELD-NAMES"
        (MAPCAR #'CAR FIELD-SPECS)))
(DEFUN VARIANT-DECLARE (&REST IGNORED) T)
(DEFUN DECLARE-TYPE! (&REST IGNORED) T)

; DECLARE-PROTOCOL-DISPATCH! — another genuine Rust-level checker-only
; builtin (registers a protocol's dispatch argument position with the
; HM checker, lib/29-protocols.lisp's DEFPROTOCOL), same honest no-op
; story as DECLARE-TYPE!/RECORD-DECLARE's checker half: this kernel's
; runtime dispatch (DEFPROTOCOL's own generated lambda, using NTH on
; the already-known dispatch index) needs no registry lookup here at
; all.
(DEFUN DECLARE-PROTOCOL-DISPATCH! (&REST IGNORED) T)

; DECLARE-INSTANCE! — another genuine Rust-level checker-only builtin
; (registers one DEFINSTANCE's scheme with the HM checker,
; lib/29-protocols.lisp), same honest no-op story as DECLARE-TYPE!/
; DECLARE-PROTOCOL-DISPATCH!: this kernel's runtime dispatch table
; (DEFINSTANCE's own SETHASH into $PROTOCOL-INSTANCES) needs no
; checker registry at all.
(DEFUN DECLARE-INSTANCE! (&REST IGNORED) T)

; RECORD-REF/RECORD-WITH (KERNEL.md condensation layer) — generic
; by-name field access over any record, used by every DEFRECORD/
; DEFVARIANT-generated getter (`(defun ,getter (self) (record-ref self
; ',(car spec)))`). Resolves FIELD to a position via the name order
; RECORD-DECLARE stashed on the brand's plist, then indexes into
; RECORD-FIELDS' positional value list (record_fields_tagged,
; arrays.asm) — no new kernel primitive needed, same as GETP/PUTP.
(DEFUN $RECORD-FIELD-INDEX (NAMES FIELD IDX)
  (IF (NULL NAMES)
      -1
      (IF (EQ (CAR NAMES) FIELD)
          IDX
          ($RECORD-FIELD-INDEX (CDR NAMES) FIELD (+ IDX 1)))))
(DEFUN $NTH (LST N)
  (IF (= N 0) (CAR LST) ($NTH (CDR LST) (- N 1))))

; NTH — a genuine Rust-level builtin (evaluator/builtins_core.rs),
; `(nth n list)` 0-indexed (lib/99-help-data.lisp's own documented
; signature/arg order — note the reverse of $NTH just above). Used by
; lib/29-protocols.lisp's DEFPROTOCOL dispatch (`(nth (protocol-
; dispatch-idx name) args)`) and never called before that file, same
; story as $LENGTH.
(DEFUN NTH (N LST)
  (IF (= N 0) (CAR LST) (NTH (- N 1) (CDR LST))))

(DEFUN RECORD-REF (SELF FIELD)
  ($NTH (RECORD-FIELDS SELF)
        ($RECORD-FIELD-INDEX (GETP (RECORD-BRAND SELF) "RECORD-FIELD-NAMES")
                              FIELD 0)))

; CONCAT — a genuine Rust-level builtin (evaluator/builtins_core.rs):
; variadic string concatenation, needed by lib/27-modules.lisp's own
; $MODULE-QUALIFY (`(concat (princ-to-string module) ":" (princ-to-
; string name))`, three arguments). Ordinary library code over the
; existing 2-argument STRING-APPEND — no new kernel primitive needed.
(DEFUN CONCAT (&REST STRS)
  (IF (NULL STRS)
      ""
      (IF (NULL (CDR STRS))
          (CAR STRS)
          (STRING-APPEND (CAR STRS) (APPLY #'CONCAT (CDR STRS))))))

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

; SEXPR-RENAME — a genuine Rust-level builtin in the reference
; (evaluator/builtins_core.rs, backing lib/27-modules.lisp's own
; WITH-MODULE), rebuilding FORM with every symbol that is (a) a key in
; TABLE (an ordinary hash table, symbol -> symbol) and (b) not already
; qualified (its name contains ":", which also exempts keywords)
; replaced by its mapped value; a cons headed by the literal symbol
; QUOTE or QUASIQUOTE is returned untouched, checked at every cons
; level (not just the top), matching the reference's own semantics —
; forward-references GETHASH/STRING-INDEX-OF (this file's own GETHASH
; already exists; STRING-INDEX-OF is reference stdlib, lib/14-
; strings.lisp, loaded well before lib/27-modules.lisp's own first use
; of this), the same tolerance every other forward reference in this
; project already relies on (a global call resolves through the
; self-patching inline cache at actual call time, not at the calling
; function's own compile time).
(DEFUN SEXPR-RENAME (FORM TABLE)
  (IF (ATOM FORM)
      (IF (AND (SYMBOLP FORM)
               (GETHASH TABLE FORM)
               (NULL (STRING-INDEX-OF (PRINC-TO-STRING FORM) ":")))
          (GETHASH TABLE FORM)
          FORM)
      (IF (IF (SYMBOLP (CAR FORM))
              (OR (EQ (CAR FORM) (QUOTE QUOTE)) (EQ (CAR FORM) (QUOTE QUASIQUOTE)))
              ())
          FORM
          (CONS (SEXPR-RENAME (CAR FORM) TABLE) (SEXPR-RENAME (CDR FORM) TABLE)))))

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

; MAX/MIN — the reference's own MAX/MIN take any number of arguments
; (a variadic fold); an earlier version of these was fixed 2-argument,
; silently WRONG (not even an error) on a third argument — `(max 1 3
; 9)` returned `3`, since calling a 2-parameter DEFUN with a third
; argument here simply never reads it, no arity check at all. Folding
; over the 2-argument core below is exactly the same idiom REDUCE
; already establishes for a variadic reference builtin over a fixed
; binary primitive.
(DEFUN $MAX2 (A B) (IF (< A B) B A))
(DEFUN $MAX-FOLD (ACC REST)
  (IF (NULL REST) ACC ($MAX-FOLD ($MAX2 ACC (CAR REST)) (CDR REST))))
(DEFUN MAX (FIRST &REST REST) ($MAX-FOLD FIRST REST))
(DEFUN $MIN2 (A B) (IF (< A B) A B))
(DEFUN $MIN-FOLD (ACC REST)
  (IF (NULL REST) ACC ($MIN-FOLD ($MIN2 ACC (CAR REST)) (CDR REST))))
(DEFUN MIN (FIRST &REST REST) ($MIN-FOLD FIRST REST))

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
  (IF (= I 61)
      ACC
      (HT-KEYS-LOOP TABLE (+ I 1) (APPEND (HT-BUCKET-KEYS (FETCH TABLE I)) ACC))))

; A hash table is a 62-slot ARRAY: 61 buckets plus a marker symbol in
; slot 61, which is what lets HASH-TABLE-P (a reference builtin the
; reference's own lib/35-json.lisp stringifier dispatches on) tell one
; apart from an ordinary array. HT-KEYS-LOOP walks the 61 buckets only.
(DEFUN MAKE-HASH-TABLE ()
  (LET ((TABLE (ARRAY 62)))
    (STORE TABLE 61 (QUOTE $HASH-TABLE-MARKER))
    TABLE))
(DEFUN HASH-TABLE-P (X)
  (IF (ARRAYP X)
      (IF (EQ (ARRAY-LENGTH* X) 62)
          (EQ (FETCH X 61) (QUOTE $HASH-TABLE-MARKER))
          (QUOTE ()))
      (QUOTE ())))

(DEFUN SETHASH (TABLE KEY VALUE)
  (STORE TABLE (HT-INDEX KEY)
         (CONS (CONS KEY VALUE) (HT-BUCKET-REMOVE (FETCH TABLE (HT-INDEX KEY)) KEY)))
  T)

; SET-BANG — the reference's own name for this exact same builtin
; (environment.rs registers "SETHASH" and "SET-BANG" as two names for
; the identical BuiltinFunc::Set), surfaced by lib/98-help-system.lisp's
; own REGISTER-DOC (`(set-bang help-db name entry)`). A pure alias, no
; new behavior.
(DEFUN SET-BANG (TABLE KEY VALUE) (SETHASH TABLE KEY VALUE))

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

; --- Reference builtins the stdlib calls at runtime -------------------
; Each of these is a Rust builtin in the reference (environment.rs) that
; some file of the reference's own stdlib calls, so a program loading
; that stdlib through lamedhc (tests/run.sh's stdlib_conformance) needs
; it bound here first. Plain Lisp over the kernel's own primitives,
; each matching the reference's own argument order and result; the
; reference stdlib may later redefine some (that is fine).
(DEFUN ADD1 (X) (+ X 1))
(DEFUN SUB1 (X) (- X 1))
(DEFUN PLUS (A B) (+ A B))
(DEFUN TIMES (A B) (* A B))
(DEFUN LESSP (A B) (< A B))
(DEFUN ZEROP (X) (EQ X 0))
(DEFUN PLUSP (X) (< 0 X))
(DEFUN EVENP (X) (EQ (REMAINDER X 2) 0))
(DEFUN ODDP (X) (NOT (EQ (REMAINDER X 2) 0)))
(DEFUN SIGNUM (X) (IF (< X 0) -1 (IF (< 0 X) 1 0)))
(DEFUN LAST (L) (IF (NULL L) (QUOTE ()) (IF (NULL (CDR L)) L (LAST (CDR L)))))
(DEFUN NTHCDR (N L) (IF (EQ N 0) L (NTHCDR (- N 1) (CDR L))))
(DEFUN EXPT (B E)
  (IF (< E 0)
      (ERROR "EXPT: negative exponent is not supported for fixnums" E)
      (IF (EQ E 0) 1 (* B (EXPT B (- E 1))))))
(DEFUN GCD (A B)
  (LET ((A (IF (< A 0) (- 0 A) A)) (B (IF (< B 0) (- 0 B) B)))
    (IF (EQ B 0) A (GCD B (REMAINDER A B)))))
(DEFUN LCM (A B) (IF (EQ (* A B) 0) 0 (LET ((P (* A B))) (QUOTIENT-ABS P (GCD A B)))))
(DEFUN QUOTIENT-ABS (P D) ($QUOTIENT (IF (< P 0) (- 0 P) P) D))
; $QUOTIENT — integer division by repeated subtraction is far too slow;
; binary long division over the kernel's ASH instead.
(DEFUN $QUOTIENT (N D)
  (IF (EQ D 0)
      (ERROR "division by zero" N)
      (LET ((NEG (IF (< N 0) (NOT (< D 0)) (< D 0)))
            (N (IF (< N 0) (- 0 N) N))
            (D (IF (< D 0) (- 0 D) D)))
        (LET ((Q ($QUOTIENT-LOOP N D 0)))
          (IF NEG (- 0 Q) Q)))))
(DEFUN $QUOTIENT-LOOP (N D Q)
  (IF (< N D)
      Q
      (LET ((SHIFT ($QUOTIENT-SHIFT N D 0)))
        ($QUOTIENT-LOOP (- N (ASH D SHIFT)) D (+ Q (ASH 1 SHIFT))))))
(DEFUN $QUOTIENT-SHIFT (N D S)
  (IF (< N (ASH D (+ S 1))) S ($QUOTIENT-SHIFT N D (+ S 1))))
(DEFUN ISQRT (N)
  (IF (< N 0)
      (ERROR "ISQRT: expected a non-negative integer" N)
      (IF (< N 2) N ($ISQRT-NEWTON N (ASH 1 (+ 1 ($ISQRT-BITS N 0)))))))
(DEFUN $ISQRT-BITS (N B) (IF (EQ N 0) (ASH B -1) ($ISQRT-BITS (ASH N -1) (+ B 1))))
(DEFUN $ISQRT-NEWTON (N X)
  (LET ((Y (ASH (+ X ($QUOTIENT N X)) -1)))
    (IF (< Y X) ($ISQRT-NEWTON N Y) X)))
(DEFUN TERPRI () (NEWLINE))
(DEFUN PRINC (X) (PRINT X))
(DEFUN PRIN1 (X) (PRINT (PRIN1-TO-STRING X)))
(DEFUN SPACES (N) (IF (< 0 N) (PROGN (PRINT " ") (SPACES (- N 1))) (QUOTE ())))
(DEFUN DELETE (ITEM L)
  (IF (NULL L)
      (QUOTE ())
      (IF (EQUAL ITEM (CAR L))
          (DELETE ITEM (CDR L))
          (CONS (CAR L) (DELETE ITEM (CDR L))))))
(DEFUN EFFACE (ITEM L)
  (IF (NULL L)
      (QUOTE ())
      (IF (EQUAL ITEM (CAR L))
          (CDR L)
          (CONS (CAR L) (EFFACE ITEM (CDR L))))))
(DEFUN SUBST (NEW OLD TREE)
  (IF (EQUAL OLD TREE)
      NEW
      (IF (ATOM TREE)
          TREE
          (CONS (SUBST NEW OLD (CAR TREE)) (SUBST NEW OLD (CDR TREE))))))
; SORT — (sort list pred), collection first like the reference: a stable
; merge sort, PRED called as (pred a b).
(DEFUN SORT (L PRED)
  (IF (NULL L)
      (QUOTE ())
      (IF (NULL (CDR L))
          L
          (LET ((HALVES ($SORT-SPLIT L (QUOTE ()) (QUOTE ()))))
            ($SORT-MERGE (SORT (CAR HALVES) PRED) (SORT (CDR HALVES) PRED) PRED)))))
(DEFUN $SORT-SPLIT (L A B)
  (IF (NULL L)
      (CONS (REVERSE A) (REVERSE B))
      ($SORT-SPLIT (CDR L) B (CONS (CAR L) A))))
(DEFUN $SORT-MERGE (A B PRED)
  (IF (NULL A)
      B
      (IF (NULL B)
          A
          (IF (PRED (CAR B) (CAR A))
              (CONS (CAR B) ($SORT-MERGE A (CDR B) PRED))
              (CONS (CAR A) ($SORT-MERGE (CDR A) B PRED))))))
; INDEX — (index string i): the one-character string at byte index i
; (the reference indexes by character; this kernel's strings are byte
; buffers — README "v0 limits").
(DEFUN INDEX (S I)
  (IF (< I (STRING-LENGTH S))
      (SUBSTRING S I (+ I 1))
      (ERROR "INDEX: index out of bounds" I)))
(DEFUN MAKNAM (L) (INTERN ($MAKNAM-CONCAT L "")))
(DEFUN $MAKNAM-CONCAT (L ACC)
  (IF (NULL L) ACC ($MAKNAM-CONCAT (CDR L) (STRING-APPEND ACC (PRINC-TO-STRING (CAR L))))))
; PLIST — the reference returns a flat (key value ...) list; this
; kernel's own plist (GETP/PUTP above) is an alist, flattened here.
(DEFUN PLIST (SYM) ($PLIST-FLATTEN (SYMBOL-PLIST SYM)))
(DEFUN $PLIST-FLATTEN (AL)
  (IF (NULL AL) (QUOTE ()) (CONS (CAR (CAR AL)) (CONS (CDR (CAR AL)) ($PLIST-FLATTEN (CDR AL))))))
(DEFUN EVLIS (L) (IF (NULL L) (QUOTE ()) (CONS (EVAL (CAR L)) (EVLIS (CDR L)))))
(DEFUN EVCON (CLAUSES)
  (IF (NULL CLAUSES)
      (QUOTE ())
      (IF (EVAL (CAR (CAR CLAUSES)))
          (EVAL (CAR (CDR (CAR CLAUSES))))
          (EVCON (CDR CLAUSES)))))
; / — the reference's division: truncating integer division on fixnums
; (Rust's `/`), float division when either operand is a float, variadic
; as a left fold like the reference's own.
(DEFUN / (A &REST MORE)
  (IF (NULL MORE)
      ($DIVIDE2 1 A)
      ($DIVIDE-FOLD A MORE)))
(DEFUN $DIVIDE-FOLD (ACC L)
  (IF (NULL L) ACC ($DIVIDE-FOLD ($DIVIDE2 ACC (CAR L)) (CDR L))))
(DEFUN $DIVIDE2 (A B)
  (IF (FLOATP A)
      (F/ A (IF (FLOATP B) B (FLOAT B)))
      (IF (FLOATP B)
          (F/ (FLOAT A) B)
          ($QUOTIENT A B))))
(DEFUN GET (TABLE KEY) (GETHASH TABLE KEY))
; --- bare-symbol bindings for the kernel's function-like keywords -----
; Every one of these is compiled inline when it appears in operator
; position (compile_form dispatches on the keyword before an ordinary
; call is ever considered), but nothing bound the *symbol* to a callable
; value, so passing one as a value — `(mapcar #'code-char codes)`, as
; the reference's lib/32-base64.lisp does — handed MAPCAR an unbound
; cell. Same idiom as the +/-/*/</= bindings above.
(DEFUN CODE-CHAR (X) (CODE-CHAR X))
(DEFUN CHAR-CODE (X) (CHAR-CODE X))
(DEFUN MAKE-CHAR (X) (MAKE-CHAR X))
(DEFUN STRING-LENGTH (X) (STRING-LENGTH X))
(DEFUN STRING-REF (S I) (STRING-REF S I))
(DEFUN STRING-APPEND (A B) (STRING-APPEND A B))
(DEFUN SUBSTRING (S A B) (SUBSTRING S A B))
(DEFUN STRINGP (X) (STRINGP X))
(DEFUN SYMBOLP (X) (SYMBOLP X))
(DEFUN FIXP (X) (FIXP X))
(DEFUN FLOATP (X) (FLOATP X))
(DEFUN ARRAYP (X) (ARRAYP X))
(DEFUN CHARP (X) (CHARP X))
(DEFUN PRINC-TO-STRING (X) (PRINC-TO-STRING X))
(DEFUN PRIN1-TO-STRING (X) (PRIN1-TO-STRING X))
(DEFUN FLOAT (X) (FLOAT X))
(DEFUN F+ (A B) (F+ A B))
(DEFUN F- (A B) (F- A B))
(DEFUN F* (A B) (F* A B))
(DEFUN F/ (A B) (F/ A B))
(DEFUN F< (A B) (F< A B))
(DEFUN MOD (A B) (MOD A B))
(DEFUN REMAINDER (A B) (REMAINDER A B))
(DEFUN ASH (A B) (ASH A B))
(DEFUN LOGNOT (X) (LOGNOT X))
(DEFUN HASH-CODE (X) (HASH-CODE X))
(DEFUN INTERN (X) (INTERN X))
(DEFUN PRINT (X) (PRINT X))
(DEFUN ERROR-MESSAGE (X) (ERROR-MESSAGE X))
(DEFUN ERROR-DATA (X) (ERROR-DATA X))
(DEFUN ERROR-P (X) (ERROR-P X))
(DEFUN FETCH (A I) (FETCH A I))
(DEFUN STORE (A I V) (STORE A I V))
(DEFUN SYMBOL-PLIST (X) (SYMBOL-PLIST X))
(DEFUN NUMBERP (X) (IF (FIXP X) T (FLOATP X)))
; STRING->NUMBER — the reference's builtin: the number a string spells,
; or NIL when it spells anything else (lib/35-json.lisp's parser).
(DEFUN STRING->NUMBER (S)
  (HANDLER-CASE
      (LET ((V (READ-FROM-STRING S))) (IF (NUMBERP V) V (QUOTE ())))
    (ERROR (E) (QUOTE ()))))

; Value bindings for the math-library keywords (same idiom as above).
(DEFUN SQRT (X) (SQRT X))
(DEFUN SIN (X) (SIN X))
(DEFUN COS (X) (COS X))
(DEFUN TAN (X) (TAN X))
(DEFUN EXP (X) (EXP X))
(DEFUN LOG (X &REST BASE) (IF (NULL BASE) (LOG X) (LOG X (CAR BASE))))
(DEFUN FLOOR (X) (FLOOR X))
(DEFUN CEILING (X) (CEILING X))
(DEFUN ROUND (X) (ROUND X))
(DEFUN TRUNCATE (X) (TRUNCATE X))
(DEFUN ROT (X N) (ROT X N))

; --- Streams: the fd layer -------------------------------------------
; The kernel has exactly two I/O primitives beyond PRINT: (FD-READ fd n)
; -> the string ONE read(2) returned (up to n bytes, "" at end of
; input) and (FD-WRITE fd string) -> writes every byte. An fd is a
; plain fixnum, so the process's standard streams are fds 0/1/2 with
; no primitive of their own; everything stream-shaped is Lisp here,
; over those two: line reading, the reference's READ (one line from
; stdin, one datum parsed from it), READ-LINE, WRITE-STRING and
; WRITE-LINE. Reading fd 0 needs the IO capability (fileio.asm).
(DEFINE *STDIN* 0)
(DEFINE *STDOUT* 1)
(DEFINE *STDERR* 2)
(DEFINE $NEWLINE-STRING (CODE-CHAR 10))
; FD-READ-LINE — the bytes up to (excluding) the next newline, one
; read(2) of a single byte at a time so nothing past the newline is
; consumed from a shared fd (what the reference's read_line contract
; needs); NIL at end of input with nothing read, a final unterminated
; line returned once.
(DEFUN FD-READ-LINE (FD) ($FD-READ-LINE-LOOP FD "" (QUOTE ())))
(DEFUN $FD-READ-LINE-LOOP (FD ACC ANY)
  (LET ((B (FD-READ FD 1)))
    (IF (EQ (STRING-LENGTH B) 0)
        (IF ANY ACC (QUOTE ()))
        (IF (EQ (STRING-REF B 0) 10)
            ACC
            ($FD-READ-LINE-LOOP FD (STRING-APPEND ACC B) T)))))
(DEFUN READ-LINE (&OPTIONAL FD) (FD-READ-LINE (IF FD FD *STDIN*)))
; READ — one datum from the next line of FD (default stdin): the
; reference's READ builtin (read_line, then parse). End of input, or a
; line holding no datum, is a condition.
(DEFUN READ (&OPTIONAL FD)
  (LET ((LINE (FD-READ-LINE (IF FD FD *STDIN*))))
    (IF (NULL LINE)
        (ERROR "READ: end of input")
        (LET ((V (READ-FROM-STRING LINE)))
          (IF (EQ V (READ-FROM-STRING ""))
              (ERROR "READ: no datum on the line" LINE)
              V)))))
(DEFUN WRITE-STRING (S &OPTIONAL FD) (FD-WRITE (IF FD FD *STDOUT*) S))
(DEFUN WRITE-LINE (S &OPTIONAL FD)
  (LET ((F (IF FD FD *STDOUT*)))
    (FD-WRITE F S)
    (FD-WRITE F $NEWLINE-STRING)
    S))
; PRINT-TO — PRINT's text (PRINC-TO-STRING) to any fd: (PRINT-TO
; *STDERR* x) is the error-stream PRINT.
(DEFUN PRINT-TO (FD X) (FD-WRITE FD (PRINC-TO-STRING X)) X)

; --- The OS layer, over SYSCALL --------------------------------------
; (SYSCALL nr arg...) is the kernel's one OS primitive (syscall.asm):
; every OS-facing function below is Lisp over it, so adding one needs
; no assembly. Argument conversion: fixnums as-is, a string as the
; address of its (NUL-terminated) bytes — a path, or a buffer the
; kernel writes into — NIL as NULL, a list of strings as a
; NULL-terminated char*[] (execve). Result: the raw return, negative =
; -errno. SYSCALL itself requires the SHELL capability, so everything
; here does too.
(DEFINE SYS-READ 0) (DEFINE SYS-WRITE 1) (DEFINE SYS-OPEN 2) (DEFINE SYS-CLOSE 3)
(DEFINE SYS-STAT 4) (DEFINE SYS-PIPE 22) (DEFINE SYS-DUP2 33) (DEFINE SYS-FORK 57)
(DEFINE SYS-EXECVE 59) (DEFINE SYS-EXIT 60) (DEFINE SYS-WAIT4 61) (DEFINE SYS-CHMOD 90)
(DEFINE SYS-UNLINK 87) (DEFINE SYS-GETPID 39)

; MAKE-STRING — N zero bytes: a buffer for a system call to fill.
(DEFUN MAKE-STRING (N) ($MAKE-STRING-LOOP N ""))
(DEFUN $MAKE-STRING-LOOP (N ACC)
  (IF (< N 1) ACC ($MAKE-STRING-LOOP (- N 1) (STRING-APPEND ACC (CODE-CHAR 0)))))
; $LE32 — the little-endian 32-bit integer at byte offset I of buffer S
; (the int[2] pipe(2) fills, wait4's status word, stat's st_mode).
(DEFUN $LE32 (S I)
  (+ (STRING-REF S I)
     (+ (* 256 (STRING-REF S (+ I 1)))
        (+ (* 65536 (STRING-REF S (+ I 2)))
           (* 16777216 (STRING-REF S (+ I 3)))))))

; FILE-P — T iff PATH names a regular file: stat(2) into a 144-byte
; struct stat, st_mode at offset 24, S_IFMT/S_IFREG = 0xF000/0x8000.
(DEFUN FILE-P (PATH)
  (LET ((BUF (MAKE-STRING 144)))
    (IF (< (SYSCALL SYS-STAT PATH BUF) 0)
        (QUOTE ())
        (EQ (LOGAND ($LE32 BUF 24) 61440) 32768))))

; CHMOD — (chmod path mode), mode a fixnum (e.g. 493 = #o755) or an
; octal-digit string like "755", as in the reference. T on success,
; else a condition carrying -errno.
(DEFUN CHMOD (PATH MODE)
  (LET ((R (SYSCALL SYS-CHMOD PATH (IF (STRINGP MODE) ($PARSE-OCTAL MODE 0 0) MODE))))
    (IF (< R 0) (ERROR "CHMOD failed (data: -errno)" R) T)))
(DEFUN $PARSE-OCTAL (S I ACC)
  (IF (< I (STRING-LENGTH S))
      ($PARSE-OCTAL S (+ I 1) (+ (* ACC 8) (- (STRING-REF S I) 48)))
      ACC))

; $FD-READ-ALL — everything an fd yields until end of input, as one
; string (4 KiB per read(2)).
(DEFUN $FD-READ-ALL (FD ACC)
  (LET ((CHUNK (FD-READ FD 4096)))
    (IF (EQ (STRING-LENGTH CHUNK) 0) ACC ($FD-READ-ALL FD (STRING-APPEND ACC CHUNK)))))

; SHELL — (shell "cmd") runs it through /bin/sh -c; (shell "prog" "a"
; "b") runs prog directly with those arguments. Returns (code stdout
; stderr) exactly like the reference: pipe(2) twice, fork(2), the child
; dup2(2)s the write ends onto 1 and 2 and execve(2)s (exiting 127 if
; that fails), the parent reads both pipes to end of input then
; wait4(2)s. v0: stdout is drained before stderr, so a child that
; writes more than a pipe buffer (64 KiB) to stderr before finishing
; its stdout can stall; nothing this project runs does.
(DEFUN SHELL (CMD &REST ARGS)
  (LET* ((ARGV (IF (NULL ARGS) (LIST "/bin/sh" "-c" CMD) (CONS CMD ARGS)))
         (OUTP (MAKE-STRING 8))
         (ERRP (MAKE-STRING 8)))
    (SYSCALL SYS-PIPE OUTP)
    (SYSCALL SYS-PIPE ERRP)
    (LET ((OUT-R ($LE32 OUTP 0)) (OUT-W ($LE32 OUTP 4))
          (ERR-R ($LE32 ERRP 0)) (ERR-W ($LE32 ERRP 4)))
      (LET ((PID (SYSCALL SYS-FORK)))
        (IF (EQ PID 0)
            (PROGN
              (SYSCALL SYS-DUP2 OUT-W 1)
              (SYSCALL SYS-DUP2 ERR-W 2)
              (SYSCALL SYS-CLOSE OUT-R) (SYSCALL SYS-CLOSE ERR-R)
              (SYSCALL SYS-CLOSE OUT-W) (SYSCALL SYS-CLOSE ERR-W)
              (SYSCALL SYS-EXECVE (CAR ARGV) ARGV NIL)
              (SYSCALL SYS-EXIT 127))
            (PROGN
              (SYSCALL SYS-CLOSE OUT-W)
              (SYSCALL SYS-CLOSE ERR-W)
              (LET* ((OUT ($FD-READ-ALL OUT-R ""))
                     (ERR ($FD-READ-ALL ERR-R ""))
                     (STATUS (MAKE-STRING 4)))
                (SYSCALL SYS-CLOSE OUT-R)
                (SYSCALL SYS-CLOSE ERR-R)
                (SYSCALL SYS-WAIT4 PID STATUS 0 NIL)
                (LIST (LOGAND (ASH ($LE32 STATUS 0) -8) 255) OUT ERR))))))))
