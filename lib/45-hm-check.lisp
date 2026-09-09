;;; 45-hm-check.lisp -- the PORTABLE Hindley-Milner type checker, in Lamedh
;;; (issue #451).
;;;
;;; ---- what this is ---------------------------------------------------------
;;;
;;; A complete port of the reference implementation's native HM checker --
;;; `src/jit/infer.rs` (the inference substrate), `src/jit/types.rs` (the type
;;; vocabulary), `src/jit/elaboration.rs` in its CHECKING mode (the
;;; bidirectional elaborator over real Lamedh surface syntax), and
;;; `src/jit/registry.rs`'s declaration plane (the struct / variant / generic /
;;; declared-scheme / protocol-instance tables) -- into portable Lamedh source,
;;; operating on Lamedh code-as-data exactly the way `lib/23-match.lisp`'s
;;; pattern matcher and `lib/24-rules.lisp`'s rulebook optimizer already do.
;;;
;;; None of that logic has any host dependency: no representation access, no
;;; native syscalls, nothing Rust-specific. That is what makes it a better fit
;;; as ONE shared library than as native code duplicated in every host language
;;; a Lamedh port targets (the SBCL port, #449, is the immediate second
;;; consumer, and the reason #451 exists).
;;;
;;; What is deliberately NOT ported: `src/jit/elaboration.rs`'s CODEGEN mode
;;; (`checking: false`) and everything downstream of it -- `Core` lowering,
;;; Cranelift native codegen, the compileable-type gate, the stride/inline
;;; layout rewrites. Those are, by definition, host-specific machine-code
;;; concerns; a portable Lamedh library cannot and should not emit them. The
;;; checker half is the whole of what is portable, and it is here in full.
;;;
;;; ---- how it is wired in ---------------------------------------------------
;;;
;;; Not a library nobody calls. Section 9 wraps the five declaration entry
;;; points the checker's registry is fed through -- `declare-type!`,
;;; `record-declare`, `variant-declare`, `declare-instance!`,
;;; `declare-protocol-dispatch!` -- so every call ALREADY in the standard
;;; library feeds the portable registry in lockstep with the host's own:
;;;
;;;   - `defrecord` (lib/20-condensation.lisp), both tiers -- the dynamic and
;;;     parametric tiers through `record-declare`, and the compiled tier
;;;     (which expands to the host-only `defstruct-typed` special form)
;;;     through an explicit `hm-declare-record!` in `record-expansion`;
;;;   - `defvariant` (lib/25-variants.lisp) through `variant-declare` and its
;;;     constructors' `record-declare`s;
;;;   - `defprotocol`/`definstance` (lib/29-protocols.lisp) through
;;;     `declare-instance!`/`declare-protocol-dispatch!`;
;;;   - lib/28-types.lisp's whole axiom table through `declare-type!`.
;;;
;;; `defun` reaches this file through `$defun-auto-compile` (lib/00-core.lisp),
;;; the one door every definition in the language routes through: it calls
;;; `$HM-ON-DEFUN`, which under `(hm-check-policy! 'eager)` checks the
;;; definition on the spot. Verdicts are never cached -- see section 9's "why
;;; there is no verdict cache".
;;;
;;; And `condense-verdict` (lib/20-condensation.lisp) is the seam through
;;; which the condensation layer's honesty machinery -- `condense-classify`,
;;; the dynamic frontier, `edit!`'s type barrier -- consumes THIS checker's
;;; verdicts, on every host.
;;;
;;; That load order is why this file loads early (right after
;;; `21-cl-compat.lisp`, ahead of `20-condensation.lisp`): the wrappers have to
;;; be installed before the first declaration is made. `STDLIB_SOURCES`' order
;;; is explicit rather than numeric, as `20-condensation.lisp` itself already
;;; demonstrates.
;;;
;;; No new host hook was needed for any of it. The two pieces of reflection
;;; this checker needs -- a callee's parameters and body, and (optionally) a
;;; natively typed function's monomorphic signature -- come from `see-source`
;;; and `signature`, primitives that already existed.
;;;
;;; ---- fidelity discipline --------------------------------------------------
;;;
;;; Every rule below mirrors its native counterpart deliberately, including
;;; the native checker's own gradual-typing choices, so both hosts agree:
;;;
;;;   - `HM-WALK`/`HM-OCCURS-P`/`HM-BIND!`/`HM-UNIFY!` mirror `Infer::{walk,
;;;     occurs, bind, unify}`, arm for arm and in the same order (`any` is
;;;     absorbing and is checked BEFORE variable binding, so a variable meeting
;;;     `any` stays free);
;;;   - `HM-UNIFY-ROWS!` mirrors `Infer::unify_rows` (Remy-style rows) and
;;;     `HM-UNIFY-FIELDS-ROW!` mirrors `Infer::unify_fields_row` -- nominal
;;;     records and applied parametric records SUBSUME into row types, which is
;;;     what `lib/20-condensation.lisp`'s row-typed accessors depend on;
;;;   - `HM-GENERALIZE`/`HM-GENERALIZE-AVOIDING`/`HM-INSTANTIATE` mirror
;;;     `Infer::{generalize, generalize_avoiding, instantiate}`;
;;;   - `HM-RENDER-SCHEME` mirrors `scheme_name`/`ty_name_vars`/`ty_name`
;;;     exactly, so a rendered scheme is character-for-character the sexpr the
;;;     native `SEE-TYPE` reports and `CONDENSE-VACUOUS-P` already classifies;
;;;   - `HM-ELAB` and its `HM-ELAB-*` rules mirror `Cx::elab`'s dispatch table
;;;     under `checking: true`, INCLUDING which heads the native checker does
;;;     not model (`SETQ`, `WHILE`, `FOR`, `LET*`, the float intrinsics, the
;;;     compiled bitwise family) and therefore routes through the ordinary call
;;;     path to a declared scheme or the gradual `any` frontier. Reproducing
;;;     those non-rules is as load-bearing as reproducing the rules: a checker
;;;     that "improved" on them would disagree with the reference host.
;;;
;;; ---- the honesty rule -----------------------------------------------------
;;;
;;; A construct outside this checker's coverage reports `ANY` (the gradual
;;; frontier) inside a type, and `DYNAMIC` as a whole-function verdict. Neither
;;; is a claim of type safety in either direction -- exactly the discipline the
;;; SBCL port's axiom-only `SEE-TYPE` already keeps. Nothing here ever reports
;;; `CHECKED` for something it did not actually check.
;;;
;;; ---- type representation --------------------------------------------------
;;;
;;;   int64 float64 bool char symbol string any     scalars (bare symbols)
;;;   (tvar N)                                      type variable
;;;   (-> (T ...) R)                                arrow
;;;   (list T) / (array T)                          homogeneous containers
;;;   (pair A B)                                    cons cell, halves typed
;;;   (record ((label . T) ...) REST)               row: REST is NIL (closed),
;;;                                                  a (tvar N) (open), or --
;;;                                                  before flattening --
;;;                                                  another record
;;;   (struct NAME)                                 nominal branded record
;;;   (variant NAME)                                declared sum union
;;;   (app NAME (T ...))                            applied parametric nominal
;;;
;;; A scheme is `(forall (id ...) TY)` with raw integer ids, as
;;; `Infer::generalize` produces; `HM-RENDER-SCHEME` renders one for display
;;; with a, b, c ... standing in for the bound ids.
;;;
;;; ---- verdicts -------------------------------------------------------------
;;;
;;;   (checked SCHEME)     well-typed; SCHEME is the rendered principal type
;;;   (declared SCHEME)    an axiom asserted via DECLARE-TYPE!, not derived
;;;   (type-error "msg")   a genuine clash
;;;   (dynamic "reason")   outside coverage -- NOT a safety claim either way

;;; ==========================================================================
;;; 1. The declaration plane: the portable mirror of `Jit`'s registry.
;;; ==========================================================================
;;;
;;; `src/jit/registry.rs` keeps six maps that the CHECKER (never codegen)
;;; consults: `structs`, `variants`, `generics`, `declared`,
;;; `protocol_instances`, `protocol_dispatch`. They are populated entirely from
;;; the Lisp layer -- `record-declare`, `variant-declare`, `declare-type!`,
;;; `declare-instance!`, `declare-protocol-dispatch!` -- so mirroring them here
;;; needs no new host hook at all: this file wraps those same entry points (see
;;; section 9) and every existing call site in `lib/20-condensation.lisp`,
;;; `lib/25-variants.lisp`, `lib/28-types.lisp` and `lib/29-protocols.lisp`
;;; feeds both planes in lockstep.

(def $hm-structs (make-hash-table))     ; NAME -> ((FIELD . ty) ...)
(def $hm-variants (make-hash-table))    ; NAME -> (CTOR ...)
(def $hm-generics (make-hash-table))    ; NAME -> (arity fields ctors variant)
(def $hm-declared (make-hash-table))    ; NAME -> scheme
(def $hm-protocols (make-hash-table))   ; NAME -> (scheme ...)
(def $hm-pdispatch (make-hash-table))   ; NAME -> index

(defun hm-struct-def (name)
  "Field alist of the nominal record NAME, or NIL when it is not registered.
An EMPTY registered record is distinguished from an unregistered one by
HM-STRUCT-P (a provisional/forward-declared brand has no fields yet)."
  (gethash $hm-structs name))

(defun hm-struct-p (name)
  (has-key-p $hm-structs name))

(defun hm-variant-p (name)
  (has-key-p $hm-variants name))

(defun hm-variant-ctors (name)
  (gethash $hm-variants name))

(defun hm-generic-p (name)
  (has-key-p $hm-generics name))

(defun hm-generic-def (name)
  (gethash $hm-generics name))

(defun hm-generic-arity (def) (nth 0 def))
(defun hm-generic-fields (def) (nth 1 def))
(defun hm-generic-ctors (def) (nth 2 def))
(defun hm-generic-variant (def) (nth 3 def))

(defun hm-declared-scheme (name)
  (gethash $hm-declared name))

(defun hm-protocol-p (name)
  (has-key-p $hm-protocols name))

(defun hm-protocol-instances (name)
  (gethash $hm-protocols name))

(defun hm-protocol-dispatch-index (name)
  (let ((hit (gethash $hm-pdispatch name)))
    (if hit hit 0)))

;;; Type words a bare field type must never be turned into a forward stub for
;;; -- either the declared-type parser resolves them itself (the scalars,
;;; string, symbol, any) or they are structural heads. Mirrors exactly the
;;; reserved set inside `Jit::declare_record`'s two-phase registration, so a
;;; field type outside it becomes a provisional empty record on BOTH sides.
(def $hm-reserved-type-names
  '(int64 float64 bool char u8 byte symbol string any list array pair record))

;;; ==========================================================================
;;; 2. Inference state: fresh variables, substitution, per-run memo tables.
;;; ==========================================================================
;;;
;;; One STATE per check, threaded explicitly (never a global), so independent
;;; checks share no variables or bindings. Mirrors `Infer` plus the per-run
;;; fields `Cx` carries (`derived`, `assumptions`, `avoid_gen`).

(defun hm-new-state ()
  "A fresh checker state."
  (let ((st (make-hash-table)))
    (sethash st 'counter 0)
    (sethash st 'subst (make-hash-table))
    ;; Memo of schemes derived on demand through the resolver. A recorded NIL
    ;; (as the marker DYNAMIC) means "could not be derived" so it is not
    ;; re-attempted; such calls stay gradual, mirroring `Cx::derived`.
    (sethash st 'derived (make-hash-table))
    ;; Monotype arrow assumptions for callees being checked up-stack (self and
    ;; mutual recursion), consulted before re-entering the resolver so cycles
    ;; terminate. Mirrors `Cx::assumptions`.
    (sethash st 'assumptions (make-hash-table))
    ;; Type-variable ids of enclosing in-flight checks; a nested callee's
    ;; scheme generalizes AVOIDING these. Mirrors `Cx::avoid_gen`.
    (sethash st 'avoid nil)
    st))

(defun hm-fresh (state)
  "A fresh, currently-unbound type variable. Mirrors Infer::fresh."
  (let ((n (gethash state 'counter)))
    (sethash state 'counter (+ n 1))
    (list 'tvar n)))

(defun hm-tvar-p (ty)
  (and (consp ty) (eq (car ty) 'tvar)))

(defun hm-tag (ty)
  (if (consp ty) (car ty) nil))

(defun hm-walk (state ty)
  "Follow the substitution to TY's representative: chase a bound variable's
chain until reaching a non-variable or an unbound variable. Shallow -- it does
not descend into a compound type's components. Mirrors Infer::walk."
  (if (hm-tvar-p ty)
      (let ((bound (gethash (gethash state 'subst) (cadr ty))))
        (if bound (hm-walk state bound) ty))
      ty))

(defun hm-occurs-p (state v ty)
  "Does variable id V occur anywhere in TY under the current substitution?
The occurs-check that keeps HM-BIND! from building an infinite type such as
a = (list a). Mirrors Infer::occurs -- note VARIANT and STRUCT contain no
variables (a variant is a set of brand names; a struct's fields are declared
monotypes), exactly as the native arms have it."
  (let ((w (hm-walk state ty)))
    (cond
      ((hm-tvar-p w) (= v (cadr w)))
      ((eq (hm-tag w) '->)
       (or (exists (lambda (p) (hm-occurs-p state v p)) (cadr w))
           (hm-occurs-p state v (caddr w))))
      ((member (hm-tag w) '(list array)) (hm-occurs-p state v (cadr w)))
      ((eq (hm-tag w) 'pair)
       (or (hm-occurs-p state v (cadr w)) (hm-occurs-p state v (caddr w))))
      ((eq (hm-tag w) 'app)
       (exists (lambda (a) (hm-occurs-p state v a)) (caddr w)))
      ((eq (hm-tag w) 'record)
       (or (exists (lambda (f) (hm-occurs-p state v (cdr f))) (cadr w))
           (and (caddr w) (hm-occurs-p state v (caddr w)))))
      (t nil))))

(defun hm-bind! (state v ty)
  "Bind variable id V to TY, rejecting a cyclic binding. Mirrors Infer::bind."
  (if (hm-occurs-p state v ty)
      (error (concat "occurs-check: type variable ?" (princ-to-string v)
                     " occurs in itself"))
      (progn (sethash (gethash state 'subst) v ty) nil)))

(defun hm-force-any! (state v)
  "Force variable id V's binding to ANY, OVERWRITING any existing binding.
Mirrors Infer::force_any -- used only by the self-recursion honesty rule in
HM-CHECK-CALLEE, where V is a fresh, definition-scoped variable."
  (sethash (gethash state 'subst) v 'any))

;;; ==========================================================================
;;; 3. Unification.
;;; ==========================================================================

(defun hm-scalar-p (ty)
  (member ty '(int64 float64 bool char symbol string any)))

(defun hm-nominal-name (ty)
  "The nominal NAME of a (struct N) / (variant N) / (app N args) type."
  (cadr ty))

(defun hm-type-name (ty)
  "TY as its surface name, for diagnostics. Mirrors ty_name."
  (princ-to-string (hm-render-ty ty nil)))

(defun hm-unify! (state a b)
  "Unify TY A and TY B, extending STATE's substitution so they become equal;
signal an error describing the clash on failure. Mirrors Infer::unify arm for
arm and IN THE SAME ORDER -- ANY is absorbing and is tested before variable
binding, so a variable meeting ANY is left free rather than pinned."
  (let ((wa (hm-walk state a)) (wb (hm-walk state b)))
    (cond
      ((and (hm-tvar-p wa) (hm-tvar-p wb) (= (cadr wa) (cadr wb))) nil)
      ((eq wa 'any) nil)
      ((eq wb 'any) nil)
      ((hm-tvar-p wa) (hm-bind! state (cadr wa) wb))
      ((hm-tvar-p wb) (hm-bind! state (cadr wb) wa))
      ((and (hm-scalar-p wa) (eq wa wb)) nil)
      ((and (eq (hm-tag wa) 'list) (eq (hm-tag wb) 'list))
       (hm-unify! state (cadr wa) (cadr wb)))
      ((and (eq (hm-tag wa) 'array) (eq (hm-tag wb) 'array))
       (hm-unify! state (cadr wa) (cadr wb)))
      ((and (eq (hm-tag wa) 'pair) (eq (hm-tag wb) 'pair))
       (progn (hm-unify! state (cadr wa) (cadr wb))
              (hm-unify! state (caddr wa) (caddr wb))))
      ((and (eq (hm-tag wa) '->) (eq (hm-tag wb) '->)
            (= (length (cadr wa)) (length (cadr wb))))
       (progn (hm-unify-list! state (cadr wa) (cadr wb))
              (hm-unify! state (caddr wa) (caddr wb))))
      ;; Sum types: two variants are the same type iff same name; a
      ;; constructor brand is a member of its variant, so a CIRCLE unifies
      ;; where a SHAPE is demanded (one-way absorption, both argument orders).
      ((and (eq (hm-tag wa) 'variant) (eq (hm-tag wb) 'variant))
       (if (eq (hm-nominal-name wa) (hm-nominal-name wb))
           nil
           (hm-clash wa wb)))
      ((and (eq (hm-tag wa) 'struct) (eq (hm-tag wb) 'variant))
       (hm-unify-ctor-variant! wa wb))
      ((and (eq (hm-tag wa) 'variant) (eq (hm-tag wb) 'struct))
       (hm-unify-ctor-variant! wb wa))
      ((and (eq (hm-tag wa) 'app) (eq (hm-tag wb) 'app))
       (hm-unify-app! state wa wb))
      ;; A parametric RECORD application meets a row: expand its fields with
      ;; the arguments substituted. Variants do not row-subsume.
      ((and (eq (hm-tag wa) 'app) (eq (hm-tag wb) 'record))
       (hm-unify-app-row! state wa wb))
      ((and (eq (hm-tag wa) 'record) (eq (hm-tag wb) 'app))
       (hm-unify-app-row! state wb wa))
      ;; Nominal identity is the NAME: a provisional/stale def and the current
      ;; def of the same record are the same type, which is what makes
      ;; recursive field types unify with values of the real type.
      ((and (eq (hm-tag wa) 'struct) (eq (hm-tag wb) 'struct))
       (if (eq (hm-nominal-name wa) (hm-nominal-name wb)) nil (hm-clash wa wb)))
      ((and (eq (hm-tag wa) 'record) (eq (hm-tag wb) 'record))
       (hm-unify-rows! state (cadr wa) (caddr wa) (cadr wb) (caddr wb)))
      ;; A nominal struct meets a row: the struct IS its closed row of fields
      ;; plus identity, so it subsumes into any record it structurally
      ;; satisfies -- this is what lets one row-typed function accept every
      ;; conforming record while two same-shaped brands stay nominally
      ;; distinct (the struct/struct arm above is untouched).
      ((and (eq (hm-tag wa) 'struct) (eq (hm-tag wb) 'record))
       (hm-unify-struct-row! state wa (cadr wb) (caddr wb)))
      ((and (eq (hm-tag wa) 'record) (eq (hm-tag wb) 'struct))
       (hm-unify-struct-row! state wb (cadr wa) (caddr wa)))
      (t (hm-clash wa wb)))))

(defun hm-clash (wa wb)
  (error (concat "cannot unify " (hm-type-name wa) " with " (hm-type-name wb))))

(defun hm-unify-list! (state xs ys)
  "Unify two type lists pairwise, stopping at the shorter (matching the
native `zip`)."
  (if (or (null xs) (null ys))
      nil
      (progn (hm-unify! state (car xs) (car ys))
             (hm-unify-list! state (cdr xs) (cdr ys)))))

(defun hm-unify-ctor-variant! (sty vty)
  "A constructor brand absorbs into its owning variant."
  (if (member (hm-nominal-name sty) (hm-variant-ctors (hm-nominal-name vty)))
      nil
      (error (concat (princ-to-string (hm-nominal-name sty))
                     " is not a constructor of variant "
                     (princ-to-string (hm-nominal-name vty))))))

(defun hm-unify-app! (state wa wb)
  "Two applied parametric nominals. Same name: arguments unify pairwise. A
constructor application absorbs into its owning variant's application (both
orders); SIBLING constructors of one variant meet at their variant. Mirrors
Infer::unify's two Ty::App/Ty::App arms."
  (let ((na (hm-nominal-name wa)) (nb (hm-nominal-name wb)))
    (if (eq na nb)
        (hm-unify-list! state (caddr wa) (caddr wb))
        (let ((va (hm-generic-variant (hm-generic-def na)))
              (vb (hm-generic-variant (hm-generic-def nb))))
          (if (or (eq va nb) (eq vb na) (and va (eq va vb)))
              (hm-unify-list! state (caddr wa) (caddr wb))
              (hm-clash wa wb))))))

(defun hm-canonical-subst (args)
  "Alist mapping canonical parameter ids 0..n-1 to ARGS -- the substitution a
parametric nominal's declared field types are read under."
  (let ((i -1))
    (mapcar (lambda (a) (setq i (+ i 1)) (cons i a)) args)))

(defun hm-unify-app-row! (state aty rty)
  "An applied parametric RECORD meets a record row: substitute the arguments
into its declared fields and subsume. Mirrors Infer::unify's App/Record arm."
  (let* ((name (hm-nominal-name aty))
         (def (hm-generic-def name)))
    (if (hm-generic-ctors def)
        (error (concat "variant " (princ-to-string name)
                       " does not subsume into a record row"))
        (let* ((m (hm-canonical-subst (caddr aty)))
               (sfields (mapcar (lambda (f)
                                  (cons (car f) (hm-subst-vars (cdr f) m)))
                                (hm-generic-fields def))))
          (hm-unify-fields-row! state name sfields (cadr rty) (caddr rty))))))

(defun hm-unify-struct-row! (state sty fields rest)
  "A nominal struct against a record row. Mirrors Infer::unify_struct_row --
the def is re-resolved BY NAME through the registry so a provisional def
embedded in a recursive field type expands to the current definition."
  (let ((name (hm-nominal-name sty)))
    (hm-unify-fields-row! state name (hm-struct-def name) fields rest)))

(defun hm-unify-fields-row! (state name sfields fields rest)
  "The shared half of nominal-into-row subsumption: SFIELDS (a struct's or an
instantiated generic's declared fields) against a record row. Every row field
must exist on the nominal with a unifying type; the nominal's remaining fields
form a CLOSED remainder bound to the row tail, so a closed record requires an
exact field-set match. Mirrors Infer::unify_fields_row."
  (let* ((flat (hm-flatten-row state fields rest))
         (fs (car flat))
         (tail (cdr flat)))
    (mapc (lambda (entry)
            (let ((hit (assoc (car entry) sfields)))
              (if hit
                  (hm-unify! state (cdr hit) (cdr entry))
                  (error (concat "struct " (princ-to-string name)
                                 " has no field "
                                 (princ-to-string (car entry)))))))
          fs)
    (let ((remainder (hm-sort-fields
                      (filter (lambda (sf) (not (assoc (car sf) fs))) sfields))))
      (cond
        ((null tail)
         (if (null remainder)
             nil
             (error (concat "closed record does not mention struct "
                            (princ-to-string name) "'s field "
                            (princ-to-string (car (car remainder)))))))
        (t (let ((w (hm-walk state tail)))
             (cond
               ((hm-tvar-p w) (hm-bind! state (cadr w) (list 'record remainder nil)))
               ((eq (hm-tag w) 'record)
                (hm-unify-rows! state remainder nil (cadr w) (caddr w)))
               (t (error "malformed record row tail")))))))))

(defun hm-flatten-row (state fields rest)
  "Walk a row tail bound to another record, merging its fields in, until the
tail is an unbound variable or absent. Returns (FIELDS . TAIL) with FIELDS
sorted by label. Mirrors Infer::flatten_row."
  (let ((fs fields) (tail (if rest (hm-walk state rest) nil)))
    (while (eq (hm-tag tail) 'record)
      (setq fs (append fs (cadr tail)))
      (setq tail (if (caddr tail) (hm-walk state (caddr tail)) nil)))
    (cons (hm-sort-fields fs) tail)))

(defun hm-sort-fields (fields)
  "A freshly SORTed copy of FIELDS (an alist of LABEL . TY) by label name."
  (sort fields (lambda (a b) (string< (princ-to-string (car a))
                                      (princ-to-string (car b))))))

(defun hm-unify-rows! (state fa ra fb rb)
  "Row unification (Remy-style): shared labels unify pointwise; each side's
private labels must be absorbed by the other side's row tail, with two
disagreeing open tails sharing one fresh tail so `rest` stays a single row. A
closed record (REST = NIL) absorbs nothing. Mirrors Infer::unify_rows."
  (let* ((flat-a (hm-flatten-row state fa ra))
         (flat-b (hm-flatten-row state fb rb))
         (fa (car flat-a)) (ra (cdr flat-a))
         (fb (car flat-b)) (rb (cdr flat-b)))
    (mapc (lambda (entry)
            (let ((other (assoc (car entry) fb)))
              (if other (hm-unify! state (cdr entry) (cdr other)) nil)))
          fa)
    (let ((only-a (filter (lambda (e) (not (assoc (car e) fb))) fa))
          (only-b (filter (lambda (e) (not (assoc (car e) fa))) fb)))
      (cond
        ((and (null ra) (null rb))
         (if (and (null only-a) (null only-b))
             nil
             (error (concat "record fields disagree: {"
                            (hm-field-names only-a) "} vs {"
                            (hm-field-names only-b) "}"))))
        ((and (hm-tvar-p ra) (null rb))
         (if only-a
             (error (concat "closed record lacks field(s) "
                            (hm-field-names only-a)))
             (hm-bind! state (cadr ra) (list 'record only-b nil))))
        ((and (null ra) (hm-tvar-p rb))
         (if only-b
             (error (concat "closed record lacks field(s) "
                            (hm-field-names only-b)))
             (hm-bind! state (cadr rb) (list 'record only-a nil))))
        ((and (hm-tvar-p ra) (hm-tvar-p rb))
         (if (= (cadr ra) (cadr rb))
             (if (and (null only-a) (null only-b))
                 nil
                 (error "record rows with a shared tail disagree on fields"))
             (let ((shared (hm-fresh state)))
               (hm-bind! state (cadr ra) (list 'record only-b shared))
               (hm-bind! state (cadr rb) (list 'record only-a shared)))))
        (t (error "malformed record row tail"))))))

(defun hm-field-names (fields)
  (string-join (mapcar (lambda (f) (princ-to-string (car f))) fields) ", "))

;;; ==========================================================================
;;; 4. Zonking, generalization, instantiation.
;;; ==========================================================================

(defun hm-zonk (state ty)
  "Deeply apply STATE's substitution, KEEPING free variables in place (never
errors) -- the read-back used before generalizing a still-polymorphic type.
Mirrors Infer::zonk."
  (let ((w (hm-walk state ty)))
    (cond
      ((eq (hm-tag w) 'list) (list 'list (hm-zonk state (cadr w))))
      ((eq (hm-tag w) 'array) (list 'array (hm-zonk state (cadr w))))
      ((eq (hm-tag w) 'pair)
       (list 'pair (hm-zonk state (cadr w)) (hm-zonk state (caddr w))))
      ((eq (hm-tag w) '->)
       (list '-> (mapcar (lambda (p) (hm-zonk state p)) (cadr w))
             (hm-zonk state (caddr w))))
      ((eq (hm-tag w) 'app)
       (list 'app (cadr w) (mapcar (lambda (a) (hm-zonk state a)) (caddr w))))
      ((eq (hm-tag w) 'record)
       (let* ((flat (hm-flatten-row state (cadr w) (caddr w)))
              (fs (car flat))
              (tail (cdr flat)))
         (list 'record
               (mapcar (lambda (f) (cons (car f) (hm-zonk state (cdr f)))) fs)
               tail)))
      (t w))))

(defun hm-free-vars-into (ty acc)
  "Collect the free (TVAR) ids of a ZONKED TY, first-seen order, into ACC.
Mirrors Infer::free_vars."
  (cond
    ((hm-tvar-p ty) (if (member (cadr ty) acc) acc (append acc (list (cadr ty)))))
    ((member (hm-tag ty) '(list array)) (hm-free-vars-into (cadr ty) acc))
    ((eq (hm-tag ty) 'pair)
     (hm-free-vars-into (caddr ty) (hm-free-vars-into (cadr ty) acc)))
    ((eq (hm-tag ty) '->)
     (hm-free-vars-into (caddr ty)
                        (reduce (lambda (a p) (hm-free-vars-into p a)) (cadr ty) acc)))
    ((eq (hm-tag ty) 'app)
     (reduce (lambda (a p) (hm-free-vars-into p a)) (caddr ty) acc))
    ((eq (hm-tag ty) 'record)
     (let ((acc2 (reduce (lambda (a f) (hm-free-vars-into (cdr f) a)) (cadr ty) acc)))
       (if (caddr ty) (hm-free-vars-into (caddr ty) acc2) acc2)))
    (t acc)))

(defun hm-free-vars (ty)
  (hm-free-vars-into ty nil))

(defun hm-generalize (state ty)
  "Generalize (zonked) TY into a scheme, closing over its free variables.
Mirrors Infer::generalize."
  (let ((z (hm-zonk state ty)))
    (list 'forall (hm-free-vars z) z)))

(defun hm-generalize-avoiding (state ty avoid)
  "Like HM-GENERALIZE but never quantifies a variable reachable from AVOID
(each avoid entry expanded through the current substitution). Used when
generalizing a callee checked INSIDE another check: variables entangled with
the enclosing in-flight function must stay free monotypes, or instantiation
would sever the link. Mirrors Infer::generalize_avoiding."
  (let* ((z (hm-zonk state ty))
         (vars (hm-free-vars z))
         (blocked (reduce (lambda (acc v)
                            (hm-free-vars-into (hm-zonk state (list 'tvar v)) acc))
                          avoid nil)))
    (list 'forall (filter (lambda (v) (not (member v blocked))) vars) z)))

(defun hm-subst-vars (ty mapping)
  "Pure substitution: replace every (TVAR id) in TY per alist MAPPING
(id -> TY), leaving unmapped ids as-is. Mirrors Infer::subst_vars."
  (cond
    ((hm-tvar-p ty) (let ((hit (assoc (cadr ty) mapping))) (if hit (cdr hit) ty)))
    ((eq (hm-tag ty) 'list) (list 'list (hm-subst-vars (cadr ty) mapping)))
    ((eq (hm-tag ty) 'array) (list 'array (hm-subst-vars (cadr ty) mapping)))
    ((eq (hm-tag ty) 'pair)
     (list 'pair (hm-subst-vars (cadr ty) mapping)
           (hm-subst-vars (caddr ty) mapping)))
    ((eq (hm-tag ty) '->)
     (list '-> (mapcar (lambda (p) (hm-subst-vars p mapping)) (cadr ty))
           (hm-subst-vars (caddr ty) mapping)))
    ((eq (hm-tag ty) 'app)
     (list 'app (cadr ty)
           (mapcar (lambda (a) (hm-subst-vars a mapping)) (caddr ty))))
    ((eq (hm-tag ty) 'record)
     (list 'record
           (mapcar (lambda (f) (cons (car f) (hm-subst-vars (cdr f) mapping)))
                   (cadr ty))
           (if (caddr ty) (hm-subst-vars (caddr ty) mapping) nil)))
    (t ty)))

(defun hm-instantiate (state scheme)
  "Instantiate SCHEME with fresh variables for each bound id. Mirrors
Infer::instantiate."
  (hm-subst-vars (caddr scheme)
                 (mapcar (lambda (v) (cons v (hm-fresh state))) (cadr scheme))))

;;; ==========================================================================
;;; 5. Rendering (letter-named variables) -- mirrors scheme_name / ty_name.
;;; ==========================================================================

(def $hm-alphabet
  '("a" "b" "c" "d" "e" "f" "g" "h" "i" "j" "k" "l" "m" "n" "o" "p" "q" "r" "s"
    "t" "u" "v" "w" "x" "y" "z"))

(defun hm-var-letter (i)
  "The I-th letter name (a, b, ... z, a1, b1, ...). Mirrors var_letter."
  (let ((base (nth (mod i 26) $hm-alphabet)))
    (if (< i 26) base (concat base (princ-to-string (floor (/ i 26)))))))

(defun hm-render-ty (ty names)
  "TY with every (TVAR id) present in alist NAMES (id -> letter symbol)
replaced by its letter; a free id absent from NAMES renders as ?id. Mirrors
ty_name_vars, including its record-field spelling ((label type), NOT a dotted
pair) and its bare rendering of struct/variant nominals."
  (cond
    ((hm-tvar-p ty)
     (let ((hit (assoc (cadr ty) names)))
       (if hit (cdr hit) (intern (concat "?" (princ-to-string (cadr ty)))))))
    ((eq (hm-tag ty) 'list) (list 'list (hm-render-ty (cadr ty) names)))
    ((eq (hm-tag ty) 'array) (list 'array (hm-render-ty (cadr ty) names)))
    ((eq (hm-tag ty) 'pair)
     (list 'pair (hm-render-ty (cadr ty) names) (hm-render-ty (caddr ty) names)))
    ((eq (hm-tag ty) '->)
     (list '-> (mapcar (lambda (p) (hm-render-ty p names)) (cadr ty))
           (hm-render-ty (caddr ty) names)))
    ;; A struct/variant renders as its bare nominal name; an application as
    ;; (name arg...).
    ((eq (hm-tag ty) 'struct) (cadr ty))
    ((eq (hm-tag ty) 'variant) (cadr ty))
    ((eq (hm-tag ty) 'app)
     (cons (cadr ty) (mapcar (lambda (a) (hm-render-ty a names)) (caddr ty))))
    ((eq (hm-tag ty) 'record)
     (let ((fs (mapcar (lambda (f) (list (car f) (hm-render-ty (cdr f) names)))
                       (cadr ty))))
       (if (caddr ty)
           (list 'record fs (hm-render-ty (caddr ty) names))
           (list 'record fs))))
    (t ty)))

(defun hm-render-scheme (scheme)
  "Render SCHEME for display: bound ids renamed a, b, c ... Mirrors
scheme_name -- the result is exactly the sexpr the native SEE-TYPE reports,
directly consumable by lib/20-condensation.lisp's CONDENSE-VACUOUS-P."
  (let* ((vars (cadr scheme))
         (names (let ((i -1))
                  (mapcar (lambda (v)
                            (setq i (+ i 1))
                            (cons v (intern (hm-var-letter i))))
                          vars)))
         (body (hm-render-ty (caddr scheme) names)))
    (if (null vars) body (list 'forall (mapcar #'cdr names) body))))

;;; ==========================================================================
;;; 6. Surface type parsing -- mirrors parse_declared_ty / parse_scheme_form.
;;; ==========================================================================

(defun hm-parse-ty (form vars)
  "Parse a surface type FORM into the internal representation. VARS is an
alist of type-variable name -> canonical id. Mirrors parse_declared_ty,
including its BARE-generic sugar (a bare parametric name means the all-ANY
application, the gradual reading pre-parametric code wrote)."
  (cond
    ((symbolp form)
     (let ((hit (assoc form vars)))
       (cond
         (hit (list 'tvar (cdr hit)))
         ((eq form 'int64) 'int64)
         ((eq form 'float64) 'float64)
         ((eq form 'bool) 'bool)
         ((eq form 'char) 'char)
         ((eq form 'symbol) 'symbol)
         ((eq form 'string) 'string)
         ((eq form 'any) 'any)
         ((hm-struct-p form) (list 'struct form))
         ((hm-variant-p form) (list 'variant form))
         ((hm-generic-p form)
          (list 'app form
                (mapcar (lambda (i) 'any)
                        (hm-iota (hm-generic-arity (hm-generic-def form))))))
         (t (error (concat "declare-type!: unknown type `"
                           (princ-to-string form) "'"))))))
    ((consp form)
     (let ((head (car form)) (n (length form)))
       (cond
         ((not (symbolp head))
          (error "declare-type!: malformed compound type"))
         ((and (eq head 'list) (= n 2))
          (list 'list (hm-parse-ty (cadr form) vars)))
         ((and (eq head 'array) (= n 2))
          (list 'array (hm-parse-ty (cadr form) vars)))
         ((and (eq head 'pair) (= n 3))
          (list 'pair (hm-parse-ty (cadr form) vars)
                (hm-parse-ty (caddr form) vars)))
         ((and (eq head '->) (= n 3))
          (list '-> (mapcar (lambda (a) (hm-parse-ty a vars)) (cadr form))
                (hm-parse-ty (caddr form) vars)))
         ((and (eq head 'record) (or (= n 2) (= n 3)))
          (hm-parse-record-ty form vars n))
         ((hm-generic-p head)
          (let ((def (hm-generic-def head)))
            (if (= (- n 1) (hm-generic-arity def))
                (list 'app head
                      (mapcar (lambda (a) (hm-parse-ty a vars)) (cdr form)))
                (error (concat "declare-type!: `" (princ-to-string head)
                               "' takes " (princ-to-string (hm-generic-arity def))
                               " type argument(s), got "
                               (princ-to-string (- n 1)))))))
         (t (error (concat "declare-type!: unknown type constructor `"
                           (princ-to-string head) "'"))))))
    (t (error "declare-type!: malformed type"))))

(defun hm-parse-record-ty (form vars n)
  (let ((fields (hm-sort-fields
                 (mapcar (lambda (f)
                           (if (and (consp f) (symbolp (car f)))
                               (cons (car f) (hm-parse-ty (cadr f) vars))
                               (error "declare-type!: record field must be (label type)")))
                         (cadr form)))))
    (mapc (lambda (w)
            (if (and (cdr w) (eq (car (car w)) (car (cadr w))))
                (error (concat "declare-type!: duplicate record label "
                               (princ-to-string (car (car w)))))
                nil))
          (hm-windows2 fields))
    (let ((rest (if (= n 3)
                    (let ((tail (hm-parse-ty (caddr form) vars)))
                      (if (hm-tvar-p tail)
                          tail
                          (error "declare-type!: record row tail must be a type variable")))
                    nil)))
      (list 'record fields rest))))

(defun hm-windows2 (lst)
  "Successive 2-element windows of LST (the duplicate-label scan)."
  (if (or (null lst) (null (cdr lst)))
      nil
      (cons (list (car lst) (cadr lst)) (hm-windows2 (cdr lst)))))

(defun hm-iota (n)
  "(0 1 ... N-1)."
  (if (or (null n) (<= n 0)) nil (append (hm-iota (- n 1)) (list (- n 1)))))

(defun hm-parse-scheme (form)
  "Parse a surface scheme: (forall (v ...) TY) or a bare TY. Mirrors
parse_scheme_form -- the bound ids are canonical 0..n-1."
  (let* ((forall-p (and (consp form) (eq (car form) 'forall) (= (length form) 3)))
         (names (if forall-p (cadr form) nil))
         (body (if forall-p (caddr form) form))
         (vars (let ((i -1))
                 (mapcar (lambda (v) (setq i (+ i 1)) (cons v i)) names))))
    (list 'forall (mapcar #'cdr vars) (hm-parse-ty body vars))))

;;; ==========================================================================
;;; 7. The elaborator -- mirrors src/jit/elaboration.rs in CHECKING mode.
;;; ==========================================================================
;;;
;;; TYENV is an alist of NAME -> TY holding MONOTYPES only, exactly like the
;;; native `Scope`: the native checker has no let-polymorphism (an `elab_let`
;;; binding pushes a plain type). Polymorphism enters only through generalized
;;; CALLEE schemes (declared, protocol instances, or derived on demand).
;;;
;;; A name not in TYENV is the gradual frontier (`any`), matching the native
;;; free-symbol arm; TRUE/FALSE are the two exceptions.

(defun hm-known-non-numeric (w)
  "A resolved operand type the EVALUATOR would reject for arithmetic or
numeric comparison. CHAR is numeric (byte arithmetic); an applied parametric
nominal is deliberately absent, matching Cx::known_non_numeric."
  (or (member w '(string symbol bool))
      (member (hm-tag w) '(list pair record struct variant))))

(defun hm-known-non-list (w)
  "Ground types that can never be a list: a CONS onto one of these is a dotted
pair, not a list extension. Mirrors Cx::known_non_list."
  (or (member w '(int64 float64 bool char string symbol))
      (member (hm-tag w) '(array struct variant -> record pair))))

(defun hm-lookup (tyenv name)
  (assoc name tyenv))

(defun hm-elab (state tyenv expr)
  "Elaborate EXPR and return its type. Mirrors Cx::elab's checking-mode
dispatch table."
  (cond
    ;; A bare nil/() literal is an empty list of unknown element type.
    ((null expr) (list 'list (hm-fresh state)))
    ((and (numberp expr) (floatp expr)) 'float64)
    ((numberp expr) 'int64)
    ((stringp expr) 'string)
    ((charp expr) 'char)
    ((symbolp expr) (hm-elab-symbol state tyenv expr))
    ((not (consp expr)) 'any)
    (t (let ((head (car expr)) (args (cdr expr)))
         (if (not (symbolp head))
             (error "typed core: call head must be a symbol")
             (hm-elab-form state tyenv head args))))))

(defun hm-elab-symbol (state tyenv sym)
  (cond
    ((eq sym 'true) 'bool)
    ((eq sym 'false) 'bool)
    (t (let ((hit (hm-lookup tyenv sym)))
         ;; A free symbol in checker mode is a global we don't track: the
         ;; gradual frontier.
         (if hit (cdr hit) 'any)))))

(defun hm-elab-all (state tyenv args)
  "Elaborate every argument for its side effects on the substitution (so a
type error nested inside one surfaces), discarding the types."
  (progn (mapc (lambda (a) (hm-elab state tyenv a)) args) nil))

(defun hm-elab-form (state tyenv head args)
  (cond
    ((member head '(+ - * / mod)) (hm-elab-bin state tyenv head args))
    ((member head '(< > <= >= = /=)) (hm-elab-cmp state tyenv head args))
    ((eq head 'not) (hm-elab-not state tyenv args))
    ((member head '(and or)) (progn (hm-elab-all state tyenv args) 'any))
    ((eq head 'if) (hm-elab-if state tyenv args))
    ((member head '(let let-typed)) (hm-elab-let state tyenv args))
    ((eq head 'progn) (hm-elab-body state tyenv args))
    ((eq head 'char-code) (hm-elab-char-code state tyenv args))
    ((eq head 'code-char) (hm-elab-code-char state tyenv args))
    ((member head '(array make-array)) (hm-elab-array-new state tyenv args))
    ((member head '(fetch aref)) (hm-elab-fetch state tyenv args))
    ((member head '(store aset)) (hm-elab-store state tyenv args))
    ((eq head 'array-length*) (hm-elab-array-len state tyenv args))
    ((eq head 'cons) (hm-elab-cons state tyenv args))
    ((member head '(car first)) (hm-elab-car state tyenv args))
    ((member head '(cdr rest)) (hm-elab-cdr state tyenv args))
    ((eq head 'list) (hm-elab-list state tyenv args))
    ((member head '(null null? endp)) (hm-elab-null state tyenv args))
    ((eq head 'record-ref) (hm-elab-record-ref state tyenv args))
    ((eq head 'record-new) (hm-elab-record-new state tyenv args))
    ((eq head 'record-with) (hm-elab-record-with state tyenv args))
    ((eq head 'append) (hm-elab-append state tyenv args))
    ((eq head 'concat) (hm-elab-mono-variadic state tyenv args 'string "concat"))
    ((member head '(logand logior logxor gcd lcm))
     (hm-elab-mono-variadic state tyenv args 'int64 "bitwise/gcd"))
    ((member head '(min max)) (hm-elab-min-max state tyenv args))
    ((eq head 'quote) (hm-elab-quote state args))
    ((eq head 'cond) (hm-elab-cond state tyenv args))
    ((eq head 'variant-case) (hm-elab-variant-case state tyenv args))
    ((member head '(when unless)) (hm-elab-when state tyenv args))
    (t (hm-elab-call state tyenv head args))))

(defun hm-elab-body (state tyenv forms)
  "A body: every form elaborated in order, the last one's type is the result.
Mirrors Cx::elab_body."
  (if (null forms)
      (error "empty body")
      (if (null (cdr forms))
          (hm-elab state tyenv (car forms))
          (progn (hm-elab state tyenv (car forms))
                 (hm-elab-body state tyenv (cdr forms))))))

;;; ---- arithmetic and comparison -------------------------------------------

(defun hm-elab-bin (state tyenv op args)
  "`+ - * / mod`. `/` and `mod` are strictly BINARY in the evaluator and must
be rejected at every other arity here too; `-` needs at least one operand.
Mirrors Cx::elab_bin's checking path."
  (cond
    ((and (member op '(/ mod)) (not (= (length args) 2)))
     (error (concat "`" (princ-to-string op) "` requires exactly 2 arguments, got "
                    (princ-to-string (length args)))))
    ((and (eq op '-) (null args))
     (error "`-` requires at least 1 argument"))
    ((null args) 'int64)
    ((null (cdr args)) (hm-walk state (hm-elab state tyenv (car args))))
    (t (let ((ty (hm-elab state tyenv (car args))))
         (mapc (lambda (a)
                 (let ((tb (hm-elab state tyenv a)))
                   (if (hm-unifies-p state ty tb)
                       (setq ty (hm-walk state ty))
                       (error (concat "`" (princ-to-string op)
                                      "` operands disagree")))))
               (cdr args))
         (let ((w (hm-walk state ty)))
           (if (hm-known-non-numeric w)
               (error (concat "`" (princ-to-string op)
                              "` expects numeric operands, got " (hm-type-name w)))
               w))))))

(defun hm-unifies-p (state a b)
  "T when A and B unify (extending the substitution); NIL on a clash -- the
Lamedh spelling of the native `self.unify(..).is_err()` idiom. NOTE that a
failed unification may still have extended the substitution with the bindings
it made before the clash, exactly as the native `Infer::unify` does: neither
implementation rolls back, and every caller that can see the difference
reports a type error immediately afterwards."
  (handler-case (progn (hm-unify! state a b) t)
    (error (e) nil)))

(defun hm-elab-cmp (state tyenv op args)
  "`< > <= >= = /=`: exactly two operands that unify; result BOOL. Known
non-comparable operand kinds are rejected as the evaluator would at runtime."
  (if (not (= (length args) 2))
      (error (concat "`" (princ-to-string op) "` expects 2 args, got "
                     (princ-to-string (length args))))
      (let ((ta (hm-elab state tyenv (car args)))
            (tb (hm-elab state tyenv (cadr args))))
        (if (hm-unifies-p state ta tb)
            (let ((w (hm-walk state ta)))
              (if (hm-known-non-numeric w)
                  (error (concat "`" (princ-to-string op)
                                 "` expects comparable (numeric or char) operands, got "
                                 (hm-type-name w)))
                  'bool))
            (error (concat "`" (princ-to-string op) "` operands disagree"))))))

(defun hm-elab-not (state tyenv args)
  "`not` follows Lisp truthiness in checker mode: any operand, BOOL result."
  (if (not (= (length args) 1))
      (error (concat "`not` expects 1 arg, got " (princ-to-string (length args))))
      (progn (hm-elab state tyenv (car args)) 'bool)))

;;; ---- conditionals ---------------------------------------------------------

(defun hm-bare-nil-p (expr)
  "Does a branch SOURCE expression look like a bare nil/() literal (as opposed
to a computed value that merely happens to type as a list)?"
  (null expr))

(defun hm-elab-if (state tyenv args)
  "`(if c then else)`. The condition follows Lisp truthiness (any type)."
  (if (not (= (length args) 3))
      (error (concat "`if` expects (if cond then else), got "
                     (princ-to-string (length args)) " args"))
      (progn
        (hm-elab state tyenv (car args))
        (let* ((tt (hm-elab state tyenv (cadr args)))
               (te (hm-elab state tyenv (caddr args)))
               (lhs-nil (hm-bare-nil-p (cadr args)))
               (rhs-nil (hm-bare-nil-p (caddr args))))
          (hm-join-branches state lhs-nil tt rhs-nil te
                            "`if` branches disagree")))))

(defun hm-join-branches (state lhs-nil lty rhs-nil rty disagreement)
  "Join two branch result types -- the nil-on-miss honesty rule (mirrors
Cx::join_branch_types). A bare nil literal types as (list _) purely so a nil
branch meeting a genuine list branch still unifies as a list. But when a
literal-nil branch meets a branch that is NOT itself a list -- a ground
scalar, or a still-free variable nothing has pinned down -- forcing that
unification either hard-errors (the guard idiom, where the other branch flowed
from a nil-on-miss function) or silently commits that branch's free variable
to `list of something`. Both outcomes are exactly the bias the honesty rule
forbids, so such a join degrades the whole IF to ANY instead. Two branches
that are both nil, both non-nil, or one nil meeting an already-list-or-ANY
branch fall through to ordinary unification, so a genuine conflict between two
concrete branches still errors."
  (if (and (not (eq (if lhs-nil t nil) (if rhs-nil t nil)))
           (let ((other (hm-walk state (if lhs-nil rty lty))))
             (not (or (eq (hm-tag other) 'list) (eq other 'any)))))
      'any
      (if (hm-unifies-p state lty rty)
          (hm-walk state lty)
          (error disagreement))))

(defun hm-elab-cond (state tyenv clauses)
  "`(cond (test body...) ...)`: every clause body unifies to one result type;
tests follow Lisp truthiness. With no clause, ANY. Mirrors Cx::elab_cond --
including its deliberate NON-application of the nil honesty rule (see that
function's comment: a self-recursive nil-on-miss helper's own clause join
happens before COND's result type is computed, so degrading here arrives too
late and regresses honest CHECKED verdicts into hard TYPE-ERRORs)."
  (let ((result (hm-fresh state)) (had nil))
    (mapc (lambda (clause)
            ;; A non-cons clause has no parts at all (the native
            ;; `list_to_vec` yields an empty vector and the clause is
            ;; skipped); only a real clause list contributes a result type.
            (if (not (consp clause))
                nil
                (let* ((parts clause)
                       (test-ty (hm-elab state tyenv (car parts)))
                       (bt (if (null (cdr parts))
                               test-ty
                               (hm-elab-body state tyenv (cdr parts)))))
                  (if (hm-unifies-p state bt result)
                      (setq had t)
                      (error "`cond` clauses disagree")))))
          clauses)
    (if had (hm-walk state result) 'any)))

(defun hm-elab-when (state tyenv args)
  "`(when test body...)` / `(unless ...)`: the value is the body OR nil
(heterogeneous), so the result is ANY."
  (if (null args)
      (error "`when`/`unless` need a condition")
      (progn (hm-elab state tyenv (car args))
             (if (cdr args) (hm-elab-body state tyenv (cdr args)) nil)
             'any)))

(defun hm-elab-quote (state args)
  "`(quote x)`: a quoted symbol is SYMBOL, quoted () a list, anything else ANY
-- the checker does not model quoted structure."
  (cond
    ((null args) 'any)
    ((null (car args)) (list 'list (hm-fresh state)))
    ((symbolp (car args)) 'symbol)
    (t 'any)))

;;; ---- binding forms --------------------------------------------------------

(defun hm-parse-annotation (state form)
  "Parse a LET-TYPED type ANNOTATION. Mirrors `src/jit/parse.rs`'s `parse_ty`,
which is a deliberately different and much smaller grammar than the
DECLARE-TYPE! one HM-PARSE-TY implements: a scalar keyword (`int64`,
`float64`, `bool`, and `char`/`u8`/`byte` all naming the byte scalar), a
registered struct name, the bare `array` keyword with the element type left as
a fresh inference variable, or `(array T)` with the element pinned. Nothing
else -- notably not `(list T)`, `string`, `symbol` or `any`, none of which the
native annotation parser accepts."
  (cond
    ((symbolp form)
     (cond
       ((eq form 'array) (list 'array (hm-fresh state)))
       ((hm-struct-p form) (list 'struct form))
       ((eq form 'int64) 'int64)
       ((eq form 'float64) 'float64)
       ((eq form 'bool) 'bool)
       ((member form '(char u8 byte)) 'char)
       (t (error (concat "unknown type `" (princ-to-string form) "'")))))
    ((consp form)
     (if (and (eq (car form) 'array) (= (length form) 2))
         (list 'array (hm-parse-annotation state (cadr form)))
         (error "type must be a scalar, struct, `array`, or `(array T)`")))
    (t (error "bad type annotation"))))

(defun hm-elab-let (state tyenv args)
  "`(let ((name init) ...) body...)`, and LET-TYPED's `(name type init)` shape
which pins the type explicitly. Bindings are MONOTYPES and all enter scope
together for the body (matching the native Scope discipline)."
  (if (null (cdr args))
      (error "`let-typed` needs a body")
      (let ((inner tyenv))
        (mapc (lambda (b)
                (let ((parts (if (consp b) b (list b))))
                  (cond
                    ((and (= (length parts) 3) (symbolp (car parts)))
                     (let ((d (hm-parse-annotation state (cadr parts)))
                           (init-ty (hm-elab state inner (caddr parts))))
                       (if (hm-unifies-p state d init-ty)
                           (setq inner (cons (cons (car parts) d) inner))
                           (error (concat "binding `" (princ-to-string (car parts))
                                          "` declared type disagrees with its init")))))
                    ((and (= (length parts) 2) (symbolp (car parts)))
                     ;; A fresh variable constrained by the initializer -- NOT
                     ;; resolved here; the var flows to every reference.
                     (let ((init-ty (hm-elab state inner (cadr parts)))
                           (v (hm-fresh state)))
                       (hm-unify! state v init-ty)
                       (setq inner (cons (cons (car parts) v) inner))))
                    (t (error "`let-typed` binding must be (name type init) or (name init)")))))
              (car args))
        (hm-elab-body state inner (cdr args)))))

;;; ---- list and pair rules --------------------------------------------------

(defun hm-elab-cons (state tyenv args)
  "`(cons x xs)` : a (list a) -> (list a). A tail already known to be a
non-list ground type makes an improper PAIR (the alist-cell case); an unknown
tail keeps the list-cons view, the useful default for inference through
recursion."
  (if (not (= (length args) 2))
      (error (concat "`cons` expects 2 args, got " (princ-to-string (length args))))
      (let ((tx (hm-elab state tyenv (car args)))
            (txs (hm-elab state tyenv (cadr args))))
        (if (hm-known-non-list (hm-walk state txs))
            (list 'pair tx txs)
            (let ((lst (list 'list tx)))
              (if (hm-unifies-p state txs lst)
                  (hm-walk state lst)
                  (error "`cons`: tail is not a list of the head's type")))))))

(defun hm-elab-car (state tyenv args)
  "`(car xs)` : (list a) -> a, or the CAR half of a known pair."
  (if (not (= (length args) 1))
      (error (concat "`car` expects 1 arg, got " (princ-to-string (length args))))
      (let ((txs (hm-elab state tyenv (car args))))
        (if (eq (hm-tag (hm-walk state txs)) 'pair)
            (cadr (hm-walk state txs))
            (let ((elem (hm-fresh state)))
              (if (hm-unifies-p state txs (list 'list elem))
                  (hm-walk state elem)
                  (error "`car` expects a list")))))))

(defun hm-elab-cdr (state tyenv args)
  "`(cdr xs)` : (list a) -> (list a), or the CDR half of a known pair."
  (if (not (= (length args) 1))
      (error (concat "`cdr` expects 1 arg, got " (princ-to-string (length args))))
      (let ((txs (hm-elab state tyenv (car args))))
        (if (eq (hm-tag (hm-walk state txs)) 'pair)
            (caddr (hm-walk state txs))
            (let ((lst (list 'list (hm-fresh state))))
              (if (hm-unifies-p state txs lst)
                  (hm-walk state lst)
                  (error "`cdr` expects a list")))))))

(defun hm-elab-list (state tyenv args)
  "`(list e0 e1 ...)`: all elements unified to a -> (list a)."
  (let ((elem (hm-fresh state)))
    (mapc (lambda (a)
            (let ((ta (hm-elab state tyenv a)))
              (if (hm-unifies-p state ta elem)
                  nil
                  (error "`list` elements disagree"))))
          args)
    (list 'list (hm-walk state elem))))

(defun hm-elab-null (state tyenv args)
  "`(null xs)` : (list a) -> bool."
  (if (not (= (length args) 1))
      (error (concat "`null` expects 1 arg, got " (princ-to-string (length args))))
      (let ((txs (hm-elab state tyenv (car args))))
        (if (hm-unifies-p state txs (list 'list (hm-fresh state)))
            'bool
            (error "`null` expects a list")))))

(defun hm-elab-append (state tyenv args)
  "`(append l1 ... ln)` : every argument (list a), result (list a)."
  (let ((want (list 'list (hm-fresh state))))
    (mapc (lambda (a)
            (let ((ta (hm-elab state tyenv a)))
              (if (hm-unifies-p state ta want) nil (error "`append`: type mismatch"))))
          args)
    (hm-walk state want)))

(defun hm-elab-mono-variadic (state tyenv args ty what)
  "A variadic operator whose arguments and result share one monomorphic type."
  (progn
    (mapc (lambda (a)
            (let ((ta (hm-elab state tyenv a)))
              (if (hm-unifies-p state ta ty)
                  nil
                  (error (concat "`" what "`: type mismatch")))))
          args)
    ty))

(defun hm-elab-min-max (state tyenv args)
  "`(min a b ...)` / `(max ...)`: a numeric chain like `+`."
  (if (null args)
      (error "`min`/`max` require at least one argument")
      (let ((ty (hm-elab state tyenv (car args))))
        (mapc (lambda (a)
                (let ((tb (hm-elab state tyenv a)))
                  (if (hm-unifies-p state ty tb)
                      (setq ty (hm-walk state ty))
                      (error "`min`/`max`: operands disagree"))))
              (cdr args))
        (let ((w (hm-walk state ty)))
          (if (hm-known-non-numeric w)
              (error (concat "`min`/`max` expect numeric operands, got "
                             (hm-type-name w)))
              w)))))

;;; ---- array rules ----------------------------------------------------------

(defun hm-elab-array-new (state tyenv args)
  "`(array n)` / `(make-array n)` : int64 -> (array a)."
  (if (not (= (length args) 1))
      (error (concat "`array` expects 1 arg (size), got "
                     (princ-to-string (length args))))
      (let ((tn (hm-elab state tyenv (car args))))
        (if (hm-unifies-p state tn 'int64)
            (list 'array (hm-fresh state))
            (error "`array` size must be int64")))))

(defun hm-elab-fetch (state tyenv args)
  "`(fetch a i)` : (array a) int64 -> a."
  (if (not (= (length args) 2))
      (error (concat "`fetch` expects 2 args, got " (princ-to-string (length args))))
      (let ((ta (hm-elab state tyenv (car args)))
            (ti (hm-elab state tyenv (cadr args)))
            (elem (hm-fresh state)))
        (if (not (hm-unifies-p state ta (list 'array elem)))
            (error "`fetch` expects an array")
            (if (not (hm-unifies-p state ti 'int64))
                (error "`fetch` index must be int64")
                (hm-walk state elem))))))

(defun hm-elab-store (state tyenv args)
  "`(store a i v)` : (array a) int64 a -> a."
  (if (not (= (length args) 3))
      (error (concat "`store` expects 3 args, got " (princ-to-string (length args))))
      (let ((ta (hm-elab state tyenv (car args)))
            (ti (hm-elab state tyenv (cadr args)))
            (tv (hm-elab state tyenv (caddr args)))
            (elem (hm-fresh state)))
        (cond
          ((not (hm-unifies-p state ta (list 'array elem)))
           (error "`store` expects an array"))
          ((not (hm-unifies-p state ti 'int64))
           (error "`store` index must be int64"))
          ((not (hm-unifies-p state tv elem))
           (error "`store` value type does not match the element type"))
          (t (hm-walk state elem))))))

(defun hm-elab-array-len (state tyenv args)
  "`(array-length* a)` : (array a) -> int64."
  (if (not (= (length args) 1))
      (error (concat "`array-length*` expects 1 arg, got "
                     (princ-to-string (length args))))
      (let ((ta (hm-elab state tyenv (car args))))
        (if (hm-unifies-p state ta (list 'array (hm-fresh state)))
            'int64
            (error "`array-length*` expects an array")))))

(defun hm-elab-char-code (state tyenv args)
  "`(char-code c)` : char -> int64. The evaluator also accepts a non-empty
STRING (the first char's code point), so the checker must too -- it must never
reject a program the interpreter would run."
  (if (not (= (length args) 1))
      (error (concat "`char-code` expects 1 arg, got " (princ-to-string (length args))))
      (let* ((ta (hm-elab state tyenv (car args)))
             (w (hm-walk state ta))
             (accepts (cond
                        ((member w '(char string any)) t)
                        ((eq (hm-tag w) 'array)
                         (or (eq (cadr w) 'char) (eq (cadr w) 'any)
                             (hm-tvar-p (cadr w))))
                        ((hm-tvar-p w) (hm-unifies-p state ta 'char))
                        (t nil))))
        (if accepts 'int64 (error "`char-code` expects char or string")))))

(defun hm-elab-code-char (state tyenv args)
  "`(code-char n)` : int64 -> char."
  (if (not (= (length args) 1))
      (error (concat "`code-char` expects 1 arg, got " (princ-to-string (length args))))
      (let ((ta (hm-elab state tyenv (car args))))
        (if (hm-unifies-p state ta 'int64)
            'char
            (error "`code-char` expects int64")))))

;;; ---- record rules (the row-typing core the condensation layer needs) ------

(defun hm-quoted-field (arg)
  "The quoted field symbol of a `(quote f)` argument, or NIL for a computed
field expression (which falls back to the dynamic path)."
  (if (and (consp arg) (eq (car arg) 'quote) (symbolp (cadr arg)))
      (cadr arg)
      nil))

(defun hm-elab-record-ref (state tyenv args)
  "`(record-ref x 'f)` : (record ((f a)) r) -> a -- the checker-native ROW
rule. This is what makes row types DERIVED end to end: any function reading a
field through the primitive infers an open record requirement with no
declare-type! axioms. A nominal record argument satisfies it by subsumption."
  (if (not (= (length args) 2))
      (error (concat "`record-ref` expects 2 args, got "
                     (princ-to-string (length args))))
      (let ((tx (hm-elab state tyenv (car args)))
            (field (hm-quoted-field (cadr args))))
        (if (null field)
            'any
            (let ((alpha (hm-fresh state)) (rho (hm-fresh state)))
              (if (hm-unifies-p state tx (list 'record (list (cons field alpha)) rho))
                  (hm-walk state alpha)
                  (error (concat "`record-ref`: no field "
                                 (princ-to-string field)))))))))

(defun hm-elab-record-with (state tyenv args)
  "`(record-with x 'f v)` : typed functional update, same record type out."
  (if (not (= (length args) 3))
      (error (concat "`record-with` expects 3 args, got "
                     (princ-to-string (length args))))
      (let ((tx (hm-elab state tyenv (car args)))
            (tv (hm-elab state tyenv (caddr args)))
            (field (hm-quoted-field (cadr args))))
        (if (null field)
            'any
            (let ((alpha (hm-fresh state)) (rho (hm-fresh state)))
              (cond
                ((not (hm-unifies-p state tx (list 'record (list (cons field alpha)) rho)))
                 (error (concat "`record-with`: no field " (princ-to-string field))))
                ((not (hm-unifies-p state tv alpha))
                 (error "`record-with`: replacement value type mismatch"))
                (t (hm-walk state tx))))))))

(defun hm-elab-record-new (state tyenv args)
  "`(record-new 'brand v1 ... vn)` : the BRANDED constructor rule -- looks the
brand up in the registry, unifies each argument with its field type, and
returns the NOMINAL type. This is what makes record-new values carry their
brand in checked code. An unquoted or unregistered brand degrades to ANY."
  (if (null args)
      (error "`record-new` expects a brand and field values")
      (let ((brand (hm-quoted-field (car args)))
            (vals (cdr args)))
        (cond
          ((null brand) (progn (hm-elab-all state tyenv vals) 'any))
          ((hm-struct-p brand)
           (let ((fields (hm-struct-def brand)))
             (if (not (= (length vals) (length fields)))
                 (error (concat "`record-new`: " (princ-to-string brand)
                                " has " (princ-to-string (length fields))
                                " field(s), got " (princ-to-string (length vals))
                                " value(s)"))
                 (progn
                   (mapcar (lambda (v f)
                             (let ((ta (hm-elab state tyenv v)))
                               (if (hm-unifies-p state ta (cdr f))
                                   nil
                                   (error (concat "`record-new`: field "
                                                  (princ-to-string (car f)))))))
                           vals fields)
                   (list 'struct brand)))))
          ((hm-generic-p brand)
           (let* ((def (hm-generic-def brand))
                  (fields (hm-generic-fields def)))
             (if (not (= (length vals) (length fields)))
                 (error (concat "`record-new`: " (princ-to-string brand)
                                " has " (princ-to-string (length fields))
                                " field(s), got " (princ-to-string (length vals))
                                " value(s)"))
                 (let* ((fresh (mapcar (lambda (i) (hm-fresh state))
                                       (hm-iota (hm-generic-arity def))))
                        (m (hm-canonical-subst fresh)))
                   (progn
                     (mapcar (lambda (v f)
                               (let ((ta (hm-elab state tyenv v))
                                     (want (hm-subst-vars (cdr f) m)))
                                 (if (hm-unifies-p state ta want)
                                     nil
                                     (error (concat "`record-new`: field "
                                                    (princ-to-string (car f)))))))
                             vals fields)
                     (list 'app brand fresh))))))
          (t (progn (hm-elab-all state tyenv vals) 'any))))))

;;; ---- the sum eliminator ---------------------------------------------------

(defun hm-owning-variant (ctor)
  "The declared (non-parametric) variant owning constructor brand CTOR."
  (let ((hit (filter (lambda (n) (member ctor (hm-variant-ctors n)))
                     (keys $hm-variants))))
    (if hit (car hit) nil)))

(defun hm-elab-variant-case (state tyenv args)
  "`(variant-case x (ctor (vars...) body...) ... [(else body...)])` -- the sum
eliminator. The scrutinee unifies with each clause ctor's OWNING variant (so
mixed-variant clauses clash); clause vars bind positionally to the ctor's
field types; every clause body joins to one result type. Exhaustiveness stays
a runtime concern. Mirrors Cx::elab_variant_case."
  (if (null args)
      (error "`variant-case` expects a scrutinee and clauses")
      (let ((tx (hm-elab state tyenv (car args)))
            (result (hm-fresh state))
            (had nil))
        (mapc (lambda (clause)
                (let ((parts (if (consp clause) clause nil)))
                  (if (or (null parts) (not (symbolp (car parts))))
                      (error "`variant-case`: clause must start with a constructor or `else`")
                      (let* ((ctor (car parts))
                             (bound (if (eq ctor 'else)
                                        (cons tyenv (cdr parts))
                                        (hm-variant-clause state tyenv tx ctor parts)))
                             (inner (car bound))
                             (body (cdr bound))
                             (bt (if (null body) 'any (hm-elab-body state inner body))))
                        (if (hm-unifies-p state bt result)
                            (setq had t)
                            (error "`variant-case` clauses disagree"))))))
              (cdr args))
        (if had (hm-walk state result) 'any))))

(defun hm-variant-clause (state tyenv tx ctor parts)
  "Bind one VARIANT-CASE clause's positional variables; returns
(TYENV . BODY-FORMS)."
  (if (< (length parts) 2)
      (error (concat "`variant-case`: clause for " (princ-to-string ctor)
                     " needs a binding list"))
      (let ((vars (cadr parts)))
        (if (not (every #'symbolp vars))
            (error (concat "`variant-case`: clause for " (princ-to-string ctor)
                           " binds a non-symbol"))
            (let ((field-tys (hm-variant-clause-fields state tx ctor vars)))
              (if (not (= (length vars) (length field-tys)))
                  (error (concat "`variant-case`: clause for " (princ-to-string ctor)
                                 " binds " (princ-to-string (length vars))
                                 " var(s) but " (princ-to-string ctor) " has "
                                 (princ-to-string (length field-tys)) " field(s)"))
                  (cons (append (mapcar #'cons vars field-tys) tyenv)
                        (cddr parts))))))))

(defun hm-variant-clause-fields (state tx ctor vars)
  "The clause constructor's field types, after unifying the scrutinee with the
constructor's owning variant (or, for an unregistered brand, the gradual
frontier)."
  (cond
    ((hm-struct-p ctor)
     (let* ((owner (hm-owning-variant ctor))
            (want (if owner (list 'variant owner) (list 'struct ctor))))
       (if (hm-unifies-p state tx want)
           (mapcar #'cdr (hm-struct-def ctor))
           (error "`variant-case`: scrutinee does not match the clause constructor"))))
    ((hm-generic-p ctor)
     (let* ((def (hm-generic-def ctor))
            (fresh (mapcar (lambda (i) (hm-fresh state))
                           (hm-iota (hm-generic-arity def))))
            (m (hm-canonical-subst fresh))
            (owner (hm-generic-variant def))
            (want (if (and owner (hm-generic-p owner))
                      (list 'app owner fresh)
                      (list 'app ctor fresh))))
       (if (hm-unifies-p state tx want)
           (mapcar (lambda (f) (hm-subst-vars (cdr f) m)) (hm-generic-fields def))
           (error "`variant-case`: scrutinee does not match the clause constructor"))))
    ;; Unregistered at check time: the gradual frontier.
    (t (mapcar (lambda (v) 'any) vars))))

;;; ---- calls: protocols, declared axioms, derived schemes, the frontier ----

;;; The host's typed-function registry, consulted through SIGNATURE when the
;;; host has one. This is the portable stand-in for `Cx::elab_call`'s FIRST
;;; arm (`by_name`): a natively typed/compiled function has a concrete,
;;; MONOMORPHIC signature, and the native checker demands exactly that
;;; signature at every call site rather than re-deriving a polymorphic scheme
;;; from the callee's body. Consulting it keeps both hosts' verdicts aligned
;;; wherever the host can actually answer.
;;;
;;; It is strictly OPTIONAL, and its absence is not a gap: a host with no
;;; typed registry has no monomorphic signature to report in the first place,
;;; so the DERIVED path below (which reads the callee's own body) is the
;;; complete and correct answer there. SIGNATURE is an existing introspection
;;; primitive, not a hook added for this file.
(def $hm-host-signature (if (boundp 'signature) (eval 'signature) nil))

(defun hm-before-arrow (sig)
  "SIG's elements up to (not including) the `->` marker; NIL if absent.
Written out rather than reaching for POSITION/SUBSEQ so this file depends
only on the kernel and the core list vocabulary."
  (cond
    ((null sig) nil)
    ((eq (car sig) '->) nil)
    (t (cons (car sig) (hm-before-arrow (cdr sig))))))

(defun hm-after-arrow (sig)
  "SIG's tail starting AT the `->` marker, or NIL if there is none."
  (cond
    ((null sig) nil)
    ((eq (car sig) '->) sig)
    (t (hm-after-arrow (cdr sig)))))

(defun hm-parse-signature (sig)
  "The surface signature `(t1 t2 -> r)` / `(-> r)` as an internal arrow."
  (let ((tail (hm-after-arrow sig)))
    (if (or (null tail) (null (cdr tail)))
        (error "not a signature")
        (list '-> (mapcar (lambda (a) (hm-parse-ty a nil)) (hm-before-arrow sig))
              (hm-parse-ty (cadr tail) nil)))))

(defun hm-host-arrow (name)
  "NAME's host-registered MONOMORPHIC arrow, or NIL when the host has no
typed registry, does not know NAME, or reports a signature this checker's
type language cannot express."
  (if (null $hm-host-signature)
      nil
      (let ((sig (handler-case (funcall $hm-host-signature name)
                   (error (e) nil))))
        (if (consp sig)
            (handler-case (hm-parse-signature sig) (error (e) nil))
            nil))))

(defun hm-elab-call (state tyenv name args)
  "A call to NAME. Mirrors Cx::elab_call's checking-mode order: the host's
typed registry first (see HM-HOST-ARROW), then a typed PROTOCOL (several
instance schemes, selected by the dispatch argument's shape), then a DECLARED
axiom, then an on-demand DERIVED scheme (a recursion assumption, a memoized
result, or a check of the callee's own lambda body), and only then the
gradual frontier."
  (let ((host (hm-host-arrow name)))
    (cond
      (host (hm-apply-arrow state tyenv name host args "registered"))
      ((hm-protocol-p name) (hm-elab-protocol-call state tyenv name args))
      ((hm-declared-scheme name) (hm-elab-declared-call state tyenv name args))
      (t (let ((derived (hm-elab-derived-call state tyenv name args)))
           (if derived
               (cdr derived)
               ;; Gradual frontier: an unknown/untyped callee yields ANY. The
               ;; arguments are still elaborated so type errors INSIDE them
               ;; surface, but left unconstrained (the callee makes no demand).
               (progn (hm-elab-all state tyenv args) 'any)))))))

(defun hm-apply-arrow (state tyenv name arrow args what)
  "Instantiated ARROW applied to ARGS: arity-check, then unify each argument
against its parameter. Returns the result type."
  (if (not (eq (hm-tag arrow) '->))
      ;; A declared non-arrow type says nothing useful about a call.
      (progn (hm-elab-all state tyenv args) 'any)
      (if (not (= (length (cadr arrow)) (length args)))
          (error (concat "`" (princ-to-string name) "` expects "
                         (princ-to-string (length (cadr arrow))) " args, got "
                         (princ-to-string (length args)) " (" what " type)"))
          (progn
            (mapcar (lambda (a p)
                      (let ((at (hm-elab state tyenv a)))
                        (if (hm-unifies-p state at p)
                            nil
                            (error (concat "in call to `" (princ-to-string name)
                                           "`: argument type mismatch")))))
                    args (cadr arrow))
            (caddr arrow)))))

(defun hm-elab-declared-call (state tyenv name args)
  (hm-apply-arrow state tyenv name
                  (hm-instantiate state (hm-declared-scheme name))
                  args "declared"))

(defun hm-instance-shape-matches (param arg)
  "Does an argument's WALKED type structurally match an instance scheme's
dispatch-parameter shape? Selection only -- real unification follows once an
instance is chosen. Mirrors Cx::instance_shape_matches."
  (let ((pt (hm-tag param)) (at (hm-tag arg)))
    (cond
      ((and (null pt) (null at)) (and (hm-scalar-p param) (eq param arg)
                                      (not (eq param 'any))))
      ((and (eq pt 'struct) (eq at 'struct)) (eq (cadr param) (cadr arg)))
      ((and (eq pt 'app) (eq at 'app))
       (or (eq (cadr param) (cadr arg))
           (eq (hm-generic-variant (hm-generic-def (cadr arg))) (cadr param))))
      ((and (eq pt 'variant) (eq at 'variant)) (eq (cadr param) (cadr arg)))
      ((and (eq pt 'variant) (eq at 'struct))
       (if (member (cadr arg) (hm-variant-ctors (cadr param))) t nil))
      ((and pt at (eq pt at) (member pt '(list array pair record))) t)
      (t nil))))

(defun hm-elab-protocol-call (state tyenv name args)
  "Select the instance whose dispatch-position parameter shape matches the
corresponding argument, then unify every argument against the instantiated
instance scheme. Fn-first protocols (MAP) dispatch on position 1. An
unresolved dispatch argument stays gradual -- but when every instance agrees
on one GROUND result type, the result is still known. Mirrors
Cx::elab_protocol_call."
  (let* ((instances (hm-protocol-instances name))
         (d (hm-protocol-dispatch-index name)))
    (if (<= (length args) d)
        (error (concat "`" (princ-to-string name)
                       "`: protocol calls need at least "
                       (princ-to-string (+ d 1)) " arguments"))
        (let* ((arg-tys (mapcar (lambda (a) (hm-elab state tyenv a)) args))
               (w (hm-walk state (nth d arg-tys))))
          (if (or (hm-tvar-p w) (eq w 'any))
              (hm-protocol-shared-result instances)
              (hm-protocol-select state name instances d w arg-tys args))))))

(defun hm-protocol-shared-result (instances)
  "When every instance's return type is the SAME ground type, that is still
the result even with an unresolved dispatch argument; otherwise ANY."
  (let ((shared 'none) (ok t))
    (mapc (lambda (inst)
            (let ((ty (caddr inst)))
              (if (eq (hm-tag ty) '->)
                  (let ((r (caddr ty)))
                    (cond
                      ((hm-tvar-p r) (setq ok nil))
                      ((eq shared 'none) (setq shared r))
                      ((equal shared r) nil)
                      (t (setq ok nil))))
                  nil)))
          instances)
    (if (and ok (not (eq shared 'none))) shared 'any)))

(defun hm-protocol-select (state name instances d w arg-tys args)
  (let ((chosen nil))
    (mapc (lambda (inst)
            (if chosen
                nil
                (let ((ty (caddr inst)))
                  (if (and (eq (hm-tag ty) '->)
                           (> (length (cadr ty)) d)
                           (hm-instance-shape-matches (nth d (cadr ty)) w))
                      (setq chosen inst)
                      nil))))
          instances)
    (if (null chosen)
        (error (concat "no `" (princ-to-string name) "` instance for "
                       (hm-type-name w)))
        (let ((ty (hm-instantiate state chosen)))
          (if (not (= (length (cadr ty)) (length args)))
              (error (concat "`" (princ-to-string name) "`: this instance expects "
                             (princ-to-string (length (cadr ty))) " args, got "
                             (princ-to-string (length args))))
              (progn
                (mapcar (lambda (at p)
                          (if (hm-unifies-p state at p)
                              nil
                              (error (concat "in call to `" (princ-to-string name)
                                             "`: argument type mismatch"))))
                        arg-tys (cadr ty))
                (caddr ty)))))))

;;; ---- on-demand derivation of a callee's own scheme ------------------------

(defun hm-elab-derived-call (state tyenv name args)
  "The derived-scheme path for an unknown callee. Returns (T . TY) or NIL when
nothing can be derived (the caller then stays gradual). Mirrors
Cx::derived_call."
  (let ((assumed (gethash (gethash state 'assumptions) name)))
    (if assumed
        ;; A callee currently being checked up-stack: use its in-flight
        ;; monotype arrow (the standard monomorphic-recursion assumption).
        (if (not (= (length (cadr assumed)) (length args)))
            ;; The function under check is the one the native checker reaches
            ;; through its provisional REGISTRY entry rather than through the
            ;; recursion assumption, and that arm rejects a wrong-arity call
            ;; outright instead of conceding the gradual frontier. A nested
            ;; callee keeps the assumption's own permissive behaviour.
            (if (and name (eq name (gethash state 'self)))
                (error (concat "`" (princ-to-string name) "` expects "
                               (princ-to-string (length (cadr assumed)))
                               " args, got " (princ-to-string (length args))))
                nil)
            (progn
              (mapcar (lambda (a p)
                        (let ((at (hm-elab state tyenv a)))
                          (if (hm-unifies-p state at p)
                              nil
                              (error (concat "in call to `" (princ-to-string name)
                                             "`: argument type mismatch")))))
                      args (cadr assumed))
              (cons t (hm-walk state (caddr assumed)))))
        (let ((scheme (hm-derived-scheme state name)))
          (if (eq scheme 'none)
              nil
              (let ((inst (hm-instantiate state scheme)))
                (if (eq (hm-tag inst) '->)
                    (cons t (hm-apply-arrow state tyenv name inst args "inferred"))
                    nil)))))))

(defun hm-derived-scheme (state name)
  "NAME's scheme derived on demand from its own lambda body, memoized for this
run. The marker NONE records a callee that could not be derived (variadic, not
a plain lambda, or failing its own check) so it is not re-attempted; such
calls stay gradual -- the callee's own error is reported at its own
definition, not here."
  (let ((memo (gethash state 'derived)))
    (if (has-key-p memo name)
        (gethash memo name)
        (let* ((src (hm-lambda-source name))
               (scheme (if (null src)
                           'none
                           (handler-case
                               (hm-check-callee state name (car src) (cdr src))
                             (error (e) 'none)))))
          (progn (sethash memo name scheme) scheme)))))

(defun hm-check-callee (state name params body)
  "Check a callee's own body inside this run and return its generalized
scheme. The callee's fresh variables join the AVOID set for the duration (so
deeper callees do not quantify them), and its arrow is generalized avoiding
every ENCLOSING in-flight variable. Mirrors Cx::check_callee."
  (if (null body)
      (error "empty body")
      (let* ((ptys (mapcar (lambda (p) (hm-fresh state)) params))
             (ret (hm-fresh state))
             (own-vars (mapcar #'cadr (cons ret ptys)))
             (arrow (list '-> ptys ret))
             (tyenv (mapcar #'cons params ptys)))
        (progn
          (sethash (gethash state 'assumptions) name arrow)
          (sethash state 'avoid (append (gethash state 'avoid) own-vars))
          (let ((outcome (handler-case
                             (hm-check-callee-body state tyenv body ret)
                           (error (e) nil))))
            ;; The assumption and the avoid-set entries are unwound whether or
            ;; not the body checked, so a failed callee never leaks state into
            ;; the enclosing check.
            (progn
              (remhash (gethash state 'assumptions) name)
              (sethash state 'avoid
                       (filter (lambda (v) (not (member v own-vars)))
                               (gethash state 'avoid)))
              (if (null outcome)
                  (error "callee check failed")
                  (hm-generalize-avoiding state arrow (gethash state 'avoid)))))))))

(defun hm-check-callee-body (state tyenv body ret)
  "Elaborate a callee body and tie it to its assumed return variable.

The self-recursion honesty rule: for a SELF-RECURSIVE callee a recursive call
site unifies against RET while the body is still being elaborated, so an
ordinary sibling clause can concretize RET before the body's OWN top-level
nil-vs-non-list join decides the honest answer is ANY. When that happens BT
(the body's final, authoritative type) is ANY even though RET was already
pinned; unify(any, ret) alone cannot undo that, because ANY only absorbs a
still-FREE variable. Trust BT and force RET back to ANY rather than let the
internal concretization leak into the generalized scheme."
  (let* ((bt (hm-elab-body state tyenv body))
         (wbt (hm-walk state bt)))
    (if (and (eq wbt 'any) (not (hm-tvar-p (hm-walk state ret))))
        (progn (hm-force-any! state (cadr ret)) t)
        (if (hm-unifies-p state bt ret)
            t
            (error "return type mismatch across branches")))))

;;; ==========================================================================
;;; 8. Source resolution and the public verdict entry points.
;;; ==========================================================================

(defun hm-progn-body (forms)
  "Unwrap a single `(progn ...)` wrapper, matching the native resolver's own
body normalization."
  (if (and (= (length forms) 1)
           (consp (car forms))
           (eq (car (car forms)) 'progn))
      (cdr (car forms))
      forms))

(defun hm-lambda-source (name)
  "(PARAMS . BODY-FORMS) for NAME's live plain lambda, or NIL.

Uses only SEE-SOURCE -- an existing, portable reflection primitive the
condensation layer already depends on -- so no new host hook is needed. A
variadic lambda, a non-lambda value and an unbound name all yield NIL: none of
them is a plain lambda whose body this checker can see. This is exactly the
reference host's own `checker_lambda_source` rule.

SEE-SOURCE is asked about NAME'S CURRENT VALUE, never about the symbol. That
distinction is load-bearing, not stylistic: asked about a SYMBOL, SEE-SOURCE
answers from a `source-form` property, and several host paths write that
property and then rebind the name later without clearing it. On the reference
host, `jit-optimize` records a `source-form` for every function it natively
compiles, so after

    (defun f (n) (+ n 1))            ; auto-compiled; source-form recorded
    (def f (lambda (s) (concat s \"!\")))   ; rebound; property NOT cleared

the symbol still carries the OLD body. Checking that would report a confident
CHECKED scheme for code the name no longer runs -- a fabricated verdict, which
is precisely what this checker's honesty discipline exists to prevent. The
live value cannot lie about itself.

The cost is that a name whose current value is an opaque host object -- a
natively compiled function's membrane, a builtin -- reports DYNAMIC rather
than being checked through. That is the honest answer for a body this checker
cannot see, and on such a host `condense-verdict` already asks the host, which
reports TYPED for exactly those names."
  (let* ((value (handler-case (eval name) (error (e) nil)))
         (src (if value
                  (handler-case (see-source value) (error (e) nil))
                  nil)))
    (if (and (consp src)
             (eq (car src) 'lambda)
             (not (member '&rest (cadr src)))
             (not (member '&optional (cadr src)))
             (not (member '&key (cadr src)))
             (every #'symbolp (cadr src))
             (cddr src))
        (cons (cadr src) (hm-progn-body (cddr src)))
        nil)))

(defun hm-check-lambda (params body)
  "Check an ANONYMOUS function of PARAMS (a flat list of bare symbols) and
BODY (a list of body forms). Returns (CHECKED scheme) | (TYPE-ERROR \"msg\") |
(DYNAMIC \"reason\") -- the same verdict shape the native SEE-TYPE reports.

NIL for the name, not a sentinel symbol: an anonymous lambda has no name to
call itself by, and any symbol picked to stand in for one would be a name real
code could also use as a call head."
  (hm-check-named nil params body))

(defun hm-check-named (name params body)
  "HM-CHECK-LAMBDA with NAME seeded as a self-recursion assumption, so a
recursive body types against its own in-flight arrow (the native checker gets
this from its provisional registry entry; this is the portable equivalent)."
  (cond
    ((null body) (list 'dynamic "empty body"))
    ((not (every #'symbolp params)) (list 'dynamic "non-symbol parameter"))
    ((exists (lambda (m) (member m params)) '(&rest &optional &key))
     (list 'dynamic "variadic parameter list"))
    (t (let* ((state (hm-new-state))
              (ptys (mapcar (lambda (p) (hm-fresh state)) params))
              (ret (hm-fresh state))
              (arrow (list '-> ptys ret))
              (tyenv (mapcar #'cons params ptys)))
         (progn
           ;; NAME is the function under check, not merely a callee: the
           ;; native checker reaches it through a provisional registry entry
           ;; (see HM-ELAB-DERIVED-CALL's arity arm). An anonymous check
           ;; (NAME nil) has no such entry and seeds neither.
           (if name
               (progn (sethash (gethash state 'assumptions) name arrow)
                      (sethash state 'self name))
               nil)
           ;; This function's own in-flight variables seed the AVOID set so a
           ;; callee checked on demand never quantifies them.
           (sethash state 'avoid (mapcar #'cadr (cons ret ptys)))
           (handler-case
               (let ((bt (hm-elab-body state tyenv body)))
                 (if (hm-unifies-p state bt ret)
                     (list 'checked
                           (hm-render-scheme
                            (hm-generalize state (list '-> (mapcar (lambda (p) (hm-zonk state p)) ptys)
                                                       (hm-zonk state ret)))))
                     (list 'type-error "return type mismatch across branches")))
             (error (e) (list 'type-error (error-message e)))))))))

(defun hm-check-expr (expr)
  "Check a single EXPR in an empty environment. Returns (CHECKED scheme) |
(TYPE-ERROR \"msg\"). Mirrors the native CHECK-TYPE entry point."
  (let ((state (hm-new-state)))
    (handler-case
        (list 'checked (hm-render-scheme
                        (hm-generalize state (hm-elab state nil expr))))
      (error (e) (list 'type-error (error-message e))))))

(defun hm-see-type (sym)
  "The PORTABLE checker's verdict for SYM, in SEE-TYPE's own shape:

  (DECLARED scheme)    an axiom asserted via DECLARE-TYPE!
  (CHECKED scheme)     a lambda whose body the checker accepts
  (TYPE-ERROR \"msg\")   the checker rejects it
  (DYNAMIC \"reason\")   variadic, a builtin, or not a function at all

This is the whole of what a host with no native checker can honestly say --
and, on a host that HAS one, exactly the part of its answer that is portable.
DECLARED is checked first, mirroring see_type_form's own order."
  (let ((declared (hm-declared-scheme sym)))
    (if declared
        (list 'declared (hm-render-scheme declared))
        (let ((src (hm-lambda-source sym)))
          (if (null src)
              (list 'dynamic "variadic or not a plain lambda")
              (hm-check-named sym (car src) (cdr src)))))))

;;; ==========================================================================
;;; 9. Wiring: the declaration plane, and the DEFUN hook.
;;; ==========================================================================
;;;
;;; Every portable registration entry point below is defined FIRST as a plain
;;; portable function, then the corresponding kernel builtin (when the host has
;;; one) is wrapped so both planes are fed in lockstep from the existing call
;;; sites in lib/20-condensation.lisp, lib/25-variants.lisp, lib/28-types.lisp
;;; and lib/29-protocols.lisp. On a host with no native checker the wrappers
;;; simply become the whole implementation.

;;; A declaration the portable parser cannot express is DROPPED, not
;;; fabricated: the portable registry simply does not learn it, and
;;; HM-SEE-TYPE then honestly reports DYNAMIC where it would otherwise have
;;; reported DECLARED. Dropping rather than raising matters because a host may
;;; register a type through a channel with no portable declaration entry point
;;; at all -- the reference host's `defstruct-typed` special form is exactly
;;; that: it registers a brand straight into the native registry, so an axiom
;;; naming such a brand parses natively and cannot parse here.
;;;
;;; Every drop is recorded, so this can never quietly hide a parser bug:
;;; HM-DROPPED-DECLARATIONS lists them, and the test suite asserts the list is
;;; EMPTY after a full stdlib load -- i.e. that the portable registry really
;;; does learn every declaration the standard library makes.
(def $hm-dropped nil)

(defun hm-dropped-declarations ()
  "Declarations the portable registry could not express, most recent first:
(KIND NAME FORM) triples."
  $hm-dropped)

(defun hm-drop! (kind name form)
  (progn (setq $hm-dropped (cons (list kind name form) $hm-dropped)) nil))

(defun hm-declare-type! (name form)
  "Register a DECLARED scheme axiom for NAME. Portable half of DECLARE-TYPE!."
  (handler-case
      (progn (sethash $hm-declared name (hm-parse-scheme form)) name)
    (error (e) (hm-drop! 'declared name form))))

(defun hm-declare-instance! (name form)
  "Register one protocol INSTANCE scheme for NAME (additive)."
  (handler-case
      (progn
        (sethash $hm-protocols name
                 (append (hm-protocol-instances name)
                         (list (hm-parse-scheme form))))
        name)
    (error (e) (hm-drop! 'instance name form))))

(defun hm-declare-protocol-dispatch! (name idx)
  (progn (sethash $hm-pdispatch name idx) name))

(defun hm-declare-variant! (head ctors)
  "Register a sum type. HEAD is NAME, or (NAME param...) for a parametric
variant. Mirrors declare_variant / declare_generic_variant."
  (let ((name (if (consp head) (car head) head))
        (params (if (consp head) (cdr head) nil)))
    (progn
      (if params
          (sethash $hm-generics name (list (length params) nil ctors nil))
          (sethash $hm-variants name ctors))
      name)))

(defun hm-forward-stub! (ty)
  "Register a bare, unknown, non-reserved field-type symbol as a provisional
empty record, so self- and mutual references resolve NOMINALLY instead of
degrading. Mirrors declare_record's two-phase registration."
  (if (and (symbolp ty)
           ty
           (not (member ty $hm-reserved-type-names))
           (not (hm-struct-p ty))
           (not (hm-variant-p ty))
           (not (hm-generic-p ty)))
      (sethash $hm-structs ty nil)
      nil))

(defun hm-declare-record! (head field-specs)
  "Register a nominal record. HEAD is NAME, or (NAME param...) for a
parametric record/constructor. Two-phase like declare_record: a provisional
def for the record itself and a forward stub for any bare unknown field type
are registered BEFORE the field types are parsed, so recursive definitions
resolve by name."
  (let ((name (if (consp head) (car head) head))
        (params (if (consp head) (cdr head) nil)))
    (handler-case
        (if params
            (hm-declare-generic-record! name params field-specs)
            (progn
              (if (hm-struct-p name) nil (sethash $hm-structs name nil))
              (mapc (lambda (f) (hm-forward-stub! (cadr f))) field-specs)
              (sethash $hm-structs name
                       (mapcar (lambda (f)
                                 (cons (car f) (hm-parse-ty (cadr f) nil)))
                               field-specs))
              name))
      (error (e) (hm-drop! 'record name field-specs)))))

(defun hm-declare-generic-record! (name params field-specs)
  "Register a PARAMETRIC record or variant constructor. If NAME is already a
constructor of a declared parametric variant, the back-reference is recorded
(that is what drives App-into-App absorption)."
  (let* ((arity (length params))
         (owner (hm-generic-owner name))
         (vars (let ((i -1))
                 (mapcar (lambda (p) (setq i (+ i 1)) (cons p i)) params))))
    (progn
      ;; Provisional def so self-references resolve by name.
      (if (hm-generic-p name)
          nil
          (sethash $hm-generics name (list arity nil nil owner)))
      (sethash $hm-generics name
               (list arity
                     (mapcar (lambda (f) (cons (car f) (hm-parse-ty (cadr f) vars)))
                             field-specs)
                     nil
                     owner))
      name)))

(defun hm-generic-owner (ctor)
  "The parametric variant whose constructor list contains CTOR, or NIL."
  (let ((hit (filter (lambda (n)
                       (member ctor (hm-generic-ctors (hm-generic-def n))))
                     (keys $hm-generics))))
    (if hit (car hit) nil)))

;;; ---- kernel wrappers ------------------------------------------------------
;;;
;;; Each wrapper captures the kernel builtin (when present) by VALUE, then
;;; rebinds the name to a function that feeds the portable registry first and
;;; delegates to the kernel second. Definition-time errors from the kernel
;;; still propagate unchanged, so no existing behavior moves.

(def $hm-kernel-declare-type!
  (if (boundp 'declare-type!) (eval 'declare-type!) nil))
(def $hm-kernel-record-declare
  (if (boundp 'record-declare) (eval 'record-declare) nil))
(def $hm-kernel-variant-declare
  (if (boundp 'variant-declare) (eval 'variant-declare) nil))
(def $hm-kernel-declare-instance!
  (if (boundp 'declare-instance!) (eval 'declare-instance!) nil))
(def $hm-kernel-declare-protocol-dispatch!
  (if (boundp 'declare-protocol-dispatch!) (eval 'declare-protocol-dispatch!) nil))

(def declare-type!
  (lambda (name form)
    (progn (hm-declare-type! name form)
           (if $hm-kernel-declare-type!
               (funcall $hm-kernel-declare-type! name form)
               name))))

(def record-declare
  (lambda (head field-specs)
    (progn (hm-declare-record! head field-specs)
           (if $hm-kernel-record-declare
               (funcall $hm-kernel-record-declare head field-specs)
               (if (consp head) (car head) head)))))

(def variant-declare
  (lambda (head ctors)
    (progn (hm-declare-variant! head ctors)
           (if $hm-kernel-variant-declare
               (funcall $hm-kernel-variant-declare head ctors)
               (if (consp head) (car head) head)))))

(def declare-instance!
  (lambda (name form)
    (progn (hm-declare-instance! name form)
           (if $hm-kernel-declare-instance!
               (funcall $hm-kernel-declare-instance! name form)
               name))))

(def declare-protocol-dispatch!
  (lambda (name idx)
    (progn (hm-declare-protocol-dispatch! name idx)
           (if $hm-kernel-declare-protocol-dispatch!
               (funcall $hm-kernel-declare-protocol-dispatch! name idx)
               name))))

;;; ---- the DEFUN hook -------------------------------------------------------
;;;
;;; `lib/00-core.lisp`'s `$defun-auto-compile` -- the one-door hook every
;;; single `defun` in the language routes through -- calls `$HM-ON-DEFUN` when
;;; it is bound. What that does is governed by the check POLICY:
;;;
;;;   'lazy (the default) -- do nothing. Verdicts are computed on demand, the
;;;     same discipline `00-core.lisp` already documents for purity and the
;;;     call graph: "computed LAZILY on first query rather than eagerly at
;;;     definition time. This avoids multi-second startup costs during stdlib
;;;     loading." A tree-walked HM check is exactly that kind of cost.
;;;
;;;   'eager -- run the portable checker on the new definition immediately and
;;;     RECORD the verdict on the symbol's plist. This is what a host with no
;;;     native checker wants: type errors surface where they are introduced.
;;;
;;; ---- why there is no verdict cache ----------------------------------------
;;;
;;; There was one, and it was unsound in two ways that both produced dishonest
;;; CHECKED verdicts -- the exact failure this checker's honesty discipline
;;; exists to prevent -- so it is gone.
;;;
;;;   1. A verdict is derived from the whole world, not from one definition:
;;;      the callee schemes it derives on demand, the declared axioms, the
;;;      record/variant/protocol registries. Invalidating only the redefined
;;;      NAME left every CALLER holding a verdict computed from the callee's
;;;      old body. `(defun a (x) (concat x "!")) (defun b (y) (a y))` then
;;;      redefining `a` to `(+ x 1)` left `b` reporting the old string scheme.
;;;   2. Not every definition path can be hooked. `defun*`, `defun-typed`,
;;;      `def`, `setq` and `set` are all host special forms or builtins that
;;;      never reach `$defun-auto-compile`, so any cache keyed on that hook is
;;;      stale by construction on a host that has them.
;;;
;;; A recomputed verdict cannot be stale, and it is cheap: checking a typical
;;; stdlib function is sub-millisecond, and the consumers (the condensation
;;; layer's `condense-check-type`, `edit!`'s barrier) query a handful of
;;; symbols on demand, not in a loop.
;;;
;;; The EAGER record below is deliberately NOT a cache and nothing serves it as
;;; a current answer: it is a dated note of what the checker said the last time
;;; this hook saw the name defined. HM-SEE-TYPE is the only current answer.

(def $hm-policy 'lazy)
(def $hm-definition-verdict-key "hm.definition-verdict")

(defun hm-check-policy! (policy)
  "Set the portable checker's DEFUN policy: 'LAZY (verdicts computed on
demand) or 'EAGER (every definition checked as it is made). Returns the
previous policy."
  (let ((prev $hm-policy))
    (progn (setq $hm-policy policy) prev)))

(defun hm-check-policy ()
  $hm-policy)

(defun hm-verdict (sym)
  "SYM's portable verdict, computed fresh. An alias for HM-SEE-TYPE, kept as
the name consumers call so it stays obvious that there is nothing cached
behind it (see this section's header for why there must not be)."
  (hm-see-type sym))

(defun hm-definition-verdict (sym)
  "What the checker said about SYM the last time the DEFUN hook saw it defined
under the EAGER policy, or NIL. A DATED NOTE, not an answer: SYM may have been
redefined since, by a path that does not route through the hook. Ask
HM-SEE-TYPE for what is true now."
  (getp sym $hm-definition-verdict-key))

(def $hm-on-defun
  (lambda (name)
    (progn
      (if (eq $hm-policy 'eager)
          (putp name $hm-definition-verdict-key (hm-see-type name))
          nil)
      name)))

;;; ---- bulk auditing --------------------------------------------------------

(defun hm-audit (names)
  "Run the portable checker over every symbol in NAMES and return an alist of
(NAME . STATUS), STATUS being the verdict head. The stdlib-scale exercise the
port is validated against."
  (mapcar (lambda (n) (cons n (car (hm-verdict n)))) names))

(defun hm-audit-summary (names)
  "(status . count) tallies for HM-AUDIT over NAMES."
  (let ((tally nil))
    (progn
      (mapc (lambda (entry)
              (let ((hit (assoc (cdr entry) tally)))
                (if hit
                    (setq tally (mapcar (lambda (e)
                                          (if (eq (car e) (cdr entry))
                                              (cons (car e) (+ (cdr e) 1))
                                              e))
                                        tally))
                    (setq tally (append tally (list (cons (cdr entry) 1)))))))
            (hm-audit names))
      tally)))
