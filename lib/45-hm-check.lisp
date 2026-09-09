;;; A portable Hindley-Milner checker, in Lamedh, over a small core language
;;; (issue #451).
;;;
;;; ---- what this is -----------------------------------------------------
;;;
;;; `src/jit/infer.rs` (the `Infer` struct: fresh variables, a substitution,
;;; `unify` with an occurs-check, row-polymorphic record unification,
;;; `generalize`/`instantiate`) is the real HM core behind the native
;;; checker (`see-type`, `explain-compile`, `defun`'s quiet `jit-optimize`
;;; attempt). It has no host dependency at all — no representation access,
;;; no native syscalls, nothing Rust-specific — which is exactly what makes
;;; it a good fit for a portable Lamedh library instead of code duplicated
;;; in every host language a Lamedh port targets (the SBCL port, #449, is
;;; the immediate second consumer).
;;;
;;; This file ports that *algorithm* (not the Rust syntax) faithfully:
;;; `hm-walk`/`hm-occurs-p`/`hm-bind!`/`hm-unify!` mirror `Infer::{walk,
;;; occurs, bind, unify}` line for line, and `hm-unify-rows!` mirrors
;;; `Infer::unify_rows`'s Rémy-style row algorithm exactly (shared labels
;;; unify pointwise; each side's private labels must be absorbed by the
;;; other side's row tail; two open tails that disagree get a fresh shared
;;; tail so both remain a single row). `hm-generalize`/`hm-instantiate`
;;; mirror `Infer::generalize`/`instantiate`.
;;;
;;; ---- what is honestly NOT here (scoped out, see #451's own warning
;;; against overclaiming) ----------------------------------------------
;;;
;;; `src/jit/{types,elaboration,registry}.rs` together are the bidirectional
;;; elaborator over full Lamedh surface syntax (`defun`/`defun*`'s several
;;; parameter grammars, every special form, structs, generic records and
;;; variants, protocol dispatch, arrays, the compileable/checkable tier
;;; split, JIT codegen) — several thousand lines, not the ~1600 the issue
;;; estimated for a single `check.rs` (that file turned out, on inspection,
;;; to be the *unrelated* static lint tool — unbound-function/arity
;;; checking — not the HM checker at all; see the issue-tracking note in
;;; the PR that added this file). Porting that whole surface is future
;;; work, not attempted here.
;;;
;;; Instead, this is the increment the issue's own "suggested next step"
;;; asks for: a concrete Lamedh-level data representation for type schemes,
;;; substitution, and the unification core, wired to a REAL (not stubbed)
;;; `algorithm W`-style inferencer over a small explicit core expression
;;; language — `lambda`, application, `if`, `let` (with let-polymorphism),
;;; literals, closed record construction, open-row field access, and a
;;; `the` type-ascription form whose surface type syntax includes named
;;; (nominal) type application — chosen specifically to demonstrate the two
;;; things #451 asks to confirm before committing to a full port:
;;;
;;;   - record ROW POLYMORPHISM: `(lambda (r) (field-ref r x))` infers to
;;;     `(forall (a b) (-> ((record ((x . a)) b))) a))` — accepts any record
;;;     naming at least an `x` field, exactly the property
;;;     `lib/20-condensation.lisp`'s row-typed record accessors need;
;;;   - NOMINAL type application, the representation protocol dispatch
;;;     needs: `(named point int64 int64)`-style types unify with each other
;;;     by name, arguments pairwise (mirroring one case of `Ty::App`), so a
;;;     declared instance type can name a protocol/record's identity, not
;;;     just its shape. Two things are honestly NOT implemented, both
;;;     deferred because they need a definition registry (a `GenericDef`/
;;;     `VariantDef` table) this standalone checker has no notion of: a
;;;     `named` application does not row-subsume into a `record` (unlike
;;;     `Ty::App`'s two-way expansion against a `Record` in `infer.rs`,
;;;     which looks up the nominal's declared fields), and *resolving*
;;;     protocol dispatch (matching a call to the right `definstance`) is
;;;     not attempted — only the type representation is confirmed
;;;     expressive enough to carry both.
;;;
;;; This is NOT wired into `defun`'s `$defun-auto-compile` hook, `defrecord`,
;;; `defvariant`, or protocol instances — that wiring is real future work,
;;; deliberately deferred: the surface-syntax elaborator this checker would
;;; need to consume real `defun` bodies is exactly the large remaining port,
;;; and wiring a partial checker into the one-door `defun` compile path
;;; without it would risk false rejections on ordinary code. Call the
;;; functions below directly (`hm-check-lambda`, `hm-check-expr`) to exercise
;;; the checker; nothing here changes what any existing `defun` does.
;;;
;;; ---- type representation ------------------------------------------------
;;;
;;;   int64 | float64 | bool | string          scalars (bare symbols)
;;;   (-> (T1 T2 ...) Tret)                    function / arrow
;;;   (list T)                                 homogeneous list
;;;   (pair T1 T2)                             cons cell, independently typed
;;;   (record ((label . T) ...) rest)          row: rest is NIL (closed),
;;;                                             a row variable (open), or
;;;                                             (pre-flatten) another record
;;;   (named Name T1 T2 ...)                   nominal application; unifies
;;;                                             by NAME, arguments pairwise
;;;   (tvar N)                                 type variable (internal only
;;;                                             -- never appears in a
;;;                                             rendered/reported scheme)
;;;   any                                      gradual top; absorbs anything
;;;
;;; A scheme is `(forall (id ...) ty)` internally (raw integer ids, as
;;; `Infer::generalize` produces); `hm-render-scheme` renders one for
;;; display with `A`/`B`/`C` ... standing in for the bound ids, matching
;;; `src/jit/infer.rs`'s `scheme_name`/`var_letter` and, not coincidentally,
;;; `lib/20-condensation.lisp`'s `condense-vacuous-p` shape
;;; (`(forall (vars) (-> (args) ret))` with plain symbol vars) -- a rendered
;;; CHECKED scheme from this checker is a drop-in argument to
;;; `condense-classify`.
;;;
;;; ---- verdicts -------------------------------------------------------
;;;
;;; `hm-check-lambda`/`hm-check-expr` return one of:
;;;   (checked scheme)     -- well-typed; SCHEME is the rendered principal
;;;                           type, e.g. (forall (a) (-> (a) a))
;;;   (type-error "msg")   -- a genuine clash on the checker's core language
;;;   (dynamic "reason")   -- outside this checker's scope (e.g. a variadic
;;;                           parameter list, or a form this core language
;;;                           does not model) -- NOT a claim of type safety
;;;                           either way, matching the axiom-only DYNAMIC
;;;                           verdict `src/check.rs`'s SBCL-port sibling
;;;                           reports honestly for anything it cannot see.

;;; ---- inference state: fresh variables + substitution -------------------

(defun hm-new-state ()
  "A fresh checker state: a monotonic variable counter plus a substitution
(a hash table from variable id to TY). Threaded explicitly (not a global)
so independent checks never share variables or bindings."
  (let ((st (make-hash-table)))
    (sethash st 'counter 0)
    (sethash st 'subst (make-hash-table))
    st))

(defun hm-fresh (state)
  "A fresh, currently-unbound type variable."
  (let ((n (gethash state 'counter)))
    (sethash state 'counter (+ n 1))
    (list 'tvar n)))

(defun hm-tvar-p (ty)
  (and (consp ty) (eq (car ty) 'tvar)))

(defun hm-walk (state ty)
  "Follow the substitution to TY's representative: chase a bound variable's
chain until reaching a non-variable or an unbound variable. Shallow -- does
not descend into a compound type's components (mirrors Infer::walk)."
  (if (hm-tvar-p ty)
      (let ((bound (gethash (gethash state 'subst) (cadr ty))))
        (if bound (hm-walk state bound) ty))
      ty))

(defun hm-occurs-p (state v ty)
  "Does variable id V occur anywhere in TY (under the current substitution)?
The occurs-check that keeps HM-BIND! from building an infinite type such as
a = (list a). Mirrors Infer::occurs."
  (let ((w (hm-walk state ty)))
    (cond
      ((hm-tvar-p w) (= v (cadr w)))
      ((and (consp w) (eq (car w) '->))
       (or (exists (lambda (p) (hm-occurs-p state v p)) (cadr w))
           (hm-occurs-p state v (caddr w))))
      ((and (consp w) (member (car w) '(list)))
       (hm-occurs-p state v (cadr w)))
      ((and (consp w) (eq (car w) 'pair))
       (or (hm-occurs-p state v (cadr w)) (hm-occurs-p state v (caddr w))))
      ((and (consp w) (eq (car w) 'named))
       (exists (lambda (a) (hm-occurs-p state v a)) (cddr w)))
      ((and (consp w) (eq (car w) 'record))
       (or (exists (lambda (f) (hm-occurs-p state v (cdr f))) (cadr w))
           (and (caddr w) (hm-occurs-p state v (caddr w)))))
      (t nil))))

(defun hm-bind! (state v ty)
  "Bind variable id V to TY in STATE's substitution, rejecting a cyclic
binding via the occurs-check. Mirrors Infer::bind."
  (if (hm-occurs-p state v ty)
      (error (concat "occurs-check: type variable ?" (princ-to-string v)
                      " occurs in itself"))
      (progn (sethash (gethash state 'subst) v ty) nil)))

;;; ---- unification ---------------------------------------------------------

(defun hm-scalar-p (ty)
  (member ty '(int64 float64 bool string symbol any)))

(defun hm-tag (ty)
  (if (consp ty) (car ty) nil))

(defun hm-unify! (state a b)
  "Unify TY A and TY B, extending STATE's substitution so they become equal.
Signals an error describing the clash on failure. Variables bind (with an
occurs-check); ANY is absorbing (checked before variable binding, so a
variable meeting ANY is left free); scalars/records/functions/lists/pairs/
named applications unify structurally. Mirrors Infer::unify."
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
      ((and (eq (hm-tag wa) 'pair) (eq (hm-tag wb) 'pair))
       (progn (hm-unify! state (cadr wa) (cadr wb))
              (hm-unify! state (caddr wa) (caddr wb))))
      ((and (eq (hm-tag wa) '->) (eq (hm-tag wb) '->)
            (= (length (cadr wa)) (length (cadr wb))))
       (progn (mapcar (lambda (x y) (hm-unify! state x y)) (cadr wa) (cadr wb))
              (hm-unify! state (caddr wa) (caddr wb))))
      ((and (eq (hm-tag wa) 'named) (eq (hm-tag wb) 'named)
            (eq (cadr wa) (cadr wb)) (= (length (cddr wa)) (length (cddr wb))))
       (mapcar (lambda (x y) (hm-unify! state x y)) (cddr wa) (cddr wb)))
      ((and (eq (hm-tag wa) 'record) (eq (hm-tag wb) 'record))
       (hm-unify-rows! state (cadr wa) (caddr wa) (cadr wb) (caddr wb)))
      (t (error (concat "cannot unify " (princ-to-string wa)
                         " with " (princ-to-string wb)))))))

(defun hm-flatten-row (state fields rest)
  "Walk a row tail bound to another record under STATE, merging its fields
in, until the tail is an unbound variable or absent. Returns (FIELDS . TAIL)
with FIELDS sorted by label. Mirrors Infer::flatten_row."
  (let ((fs fields) (tail (if rest (hm-walk state rest) nil)))
    (while (and tail (consp tail) (eq (car tail) 'record))
      (setq fs (append fs (cadr tail)))
      (setq tail (if (caddr tail) (hm-walk state (caddr tail)) nil)))
    (cons (sort-fields fs) tail)))

(defun sort-fields (fields)
  "A freshly SORTed copy of FIELDS (an alist of LABEL . TY) by label name.
SORT itself returns a fresh list (it converts to a Vec internally), so no
separate copy is needed."
  (sort fields (lambda (a b) (string< (princ-to-string (car a))
                                       (princ-to-string (car b))))))

(defun hm-unify-rows! (state fa ra fb rb)
  "Row unification (Remy-style): shared labels unify pointwise; each side's
private labels must be absorbed by the other side's row tail, with two
disagreeing open tails sharing one fresh tail so `rest` stays a single row.
A closed record (REST = NIL) absorbs nothing. Mirrors Infer::unify_rows."
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

;;; ---- zonking, generalization, instantiation -----------------------------

(defun hm-zonk (state ty)
  "Deeply apply STATE's substitution, keeping free variables in place (never
errors). The read-back used before generalizing a still-polymorphic type.
Mirrors Infer::zonk."
  (let ((w (hm-walk state ty)))
    (cond
      ((and (consp w) (eq (car w) 'list)) (list 'list (hm-zonk state (cadr w))))
      ((and (consp w) (eq (car w) 'pair))
       (list 'pair (hm-zonk state (cadr w)) (hm-zonk state (caddr w))))
      ((and (consp w) (eq (car w) '->))
       (list '-> (mapcar (lambda (p) (hm-zonk state p)) (cadr w))
             (hm-zonk state (caddr w))))
      ((and (consp w) (eq (car w) 'named))
       (cons 'named (cons (cadr w) (mapcar (lambda (a) (hm-zonk state a)) (cddr w)))))
      ((and (consp w) (eq (car w) 'record))
       (let* ((flat (hm-flatten-row state (cadr w) (caddr w)))
              (fs (car flat)) (tail (cdr flat)))
         (list 'record
               (mapcar (lambda (f) (cons (car f) (hm-zonk state (cdr f)))) fs)
               tail)))
      (t w))))

(defun hm-free-vars-into (ty acc)
  "Collect the free (TVAR) ids of ZONKED TY, first-seen order, into ACC."
  (cond
    ((hm-tvar-p ty) (if (member (cadr ty) acc) acc (append acc (list (cadr ty)))))
    ((and (consp ty) (eq (car ty) 'list)) (hm-free-vars-into (cadr ty) acc))
    ((and (consp ty) (eq (car ty) 'pair))
     (hm-free-vars-into (caddr ty) (hm-free-vars-into (cadr ty) acc)))
    ((and (consp ty) (eq (car ty) '->))
     (hm-free-vars-into (caddr ty)
                        (reduce (lambda (a p) (hm-free-vars-into p a)) (cadr ty) acc)))
    ((and (consp ty) (eq (car ty) 'named))
     (reduce (lambda (a p) (hm-free-vars-into p a)) (cddr ty) acc))
    ((and (consp ty) (eq (car ty) 'record))
     (let ((acc2 (reduce (lambda (a f) (hm-free-vars-into (cdr f) a)) (cadr ty) acc)))
       (if (caddr ty) (hm-free-vars-into (caddr ty) acc2) acc2)))
    (t acc)))

(defun hm-free-vars (ty)
  (hm-free-vars-into ty nil))

(defun hm-scheme-free-vars (scheme)
  "Free variables of SCHEME: TY's free vars minus its own bound (FORALL) ids."
  (set-difference (hm-free-vars (caddr scheme)) (cadr scheme)))

(defun hm-env-free-vars (tyenv)
  "Union of every binding's scheme-free-vars in TYENV (an alist of
name -> scheme). Excluded from a nested LET's generalization, exactly as
Infer::generalize_avoiding excludes variables entangled with an enclosing
in-flight check."
  (reduce (lambda (acc entry) (union acc (hm-scheme-free-vars (cdr entry))))
          tyenv nil))

(defun hm-generalize (state tyenv ty)
  "Generalize (zonked) TY into a scheme, closing over its free variables
except those still free (monomorphic) in TYENV. Mirrors
Infer::generalize_avoiding, applied with TYENV's own free variables as the
avoid set (the whole-function top-level check avoids nothing, matching
plain Infer::generalize, since TYENV is empty there)."
  (let* ((z (hm-zonk state ty))
         (blocked (hm-env-free-vars tyenv))
         (vars (set-difference (hm-free-vars z) blocked)))
    (list 'forall vars z)))

(defun hm-subst-vars (ty mapping)
  "Pure substitution: replace every (TVAR id) in TY per alist MAPPING
(id -> TY), leaving unmapped ids as-is. Mirrors Infer::subst_vars."
  (cond
    ((hm-tvar-p ty) (let ((hit (assoc (cadr ty) mapping))) (if hit (cdr hit) ty)))
    ((and (consp ty) (eq (car ty) 'list)) (list 'list (hm-subst-vars (cadr ty) mapping)))
    ((and (consp ty) (eq (car ty) 'pair))
     (list 'pair (hm-subst-vars (cadr ty) mapping) (hm-subst-vars (caddr ty) mapping)))
    ((and (consp ty) (eq (car ty) '->))
     (list '-> (mapcar (lambda (p) (hm-subst-vars p mapping)) (cadr ty))
           (hm-subst-vars (caddr ty) mapping)))
    ((and (consp ty) (eq (car ty) 'named))
     (cons 'named (cons (cadr ty) (mapcar (lambda (a) (hm-subst-vars a mapping)) (cddr ty)))))
    ((and (consp ty) (eq (car ty) 'record))
     (list 'record
           (mapcar (lambda (f) (cons (car f) (hm-subst-vars (cdr f) mapping))) (cadr ty))
           (if (caddr ty) (hm-subst-vars (caddr ty) mapping) nil)))
    (t ty)))

(defun hm-instantiate (state scheme)
  "Instantiate SCHEME with fresh variables for each bound id. Mirrors
Infer::instantiate."
  (let ((mapping (mapcar (lambda (v) (cons v (hm-fresh state))) (cadr scheme))))
    (hm-subst-vars (caddr scheme) mapping)))

;;; ---- rendering (letter-named variables, matching src/jit/infer.rs) ------

(def $hm-alphabet
  '("a" "b" "c" "d" "e" "f" "g" "h" "i" "j" "k" "l" "m" "n" "o" "p" "q" "r" "s"
    "t" "u" "v" "w" "x" "y" "z"))

(defun hm-var-letter (i)
  "The I-th letter name (a, b, c, ... z, a1, b1, ...), matching var_letter."
  (let ((base (nth (mod i 26) $hm-alphabet)))
    (if (< i 26) base (concat base (princ-to-string (floor (/ i 26)))))))

(defun hm-render-ty (ty names)
  "TY with every (TVAR id) present in alist NAMES (id -> letter symbol)
replaced by its letter; a free id absent from NAMES renders as ?id.
Mirrors ty_name_vars."
  (cond
    ((hm-tvar-p ty)
     (let ((hit (assoc (cadr ty) names)))
       (if hit (cdr hit) (intern (concat "?" (princ-to-string (cadr ty)))))))
    ((and (consp ty) (eq (car ty) 'list)) (list 'list (hm-render-ty (cadr ty) names)))
    ((and (consp ty) (eq (car ty) 'pair))
     (list 'pair (hm-render-ty (cadr ty) names) (hm-render-ty (caddr ty) names)))
    ((and (consp ty) (eq (car ty) '->))
     (list '-> (mapcar (lambda (p) (hm-render-ty p names)) (cadr ty))
           (hm-render-ty (caddr ty) names)))
    ((and (consp ty) (eq (car ty) 'named))
     (cons 'named (cons (cadr ty) (mapcar (lambda (a) (hm-render-ty a names)) (cddr ty)))))
    ((and (consp ty) (eq (car ty) 'record))
     (list 'record
           (mapcar (lambda (f) (cons (car f) (hm-render-ty (cdr f) names))) (cadr ty))
           (if (caddr ty) (hm-render-ty (caddr ty) names) nil)))
    (t ty)))

(defun hm-render-scheme (scheme)
  "Render SCHEME for display: bound ids renamed to a, b, c, ... Mirrors
scheme_name -- the result is a `(forall (vars) ty)` sexpr with plain symbol
vars, directly consumable by lib/20-condensation.lisp's CONDENSE-VACUOUS-P."
  (let* ((vars (cadr scheme))
         (names (let ((i -1))
                  (mapcar (lambda (v) (setq i (+ i 1)) (cons v (intern (hm-var-letter i))))
                          vars)))
         (body (hm-render-ty (caddr scheme) names)))
    (if (null vars)
        body
        (list 'forall (mapcar #'cdr names) body))))

;;; ---- the core inferencer (algorithm W over a small explicit language) --
;;;
;;; Supported forms: literals (integer/float/string/T), a bound symbol
;;; (variable reference), (LAMBDA (p...) body...), application (f a...),
;;; (IF c then else), (LET ((n v)...) body...) with let-polymorphism,
;;; (MK-RECORD (label expr)...) closed record construction, (FIELD-REF
;;; expr label) open-row field access, and (THE type-form expr) ascription
;;; (its surface type syntax accepts named/nominal application).

(defun hm-lookup-var (state tyenv sym)
  (let ((hit (assoc sym tyenv)))
    (if hit
        (hm-instantiate state (cdr hit))
        (error (concat "unbound variable " (princ-to-string sym))))))

(defun hm-infer-seq (state tyenv forms)
  (if (null (cdr forms))
      (hm-infer state tyenv (car forms))
      (progn (hm-infer state tyenv (car forms))
             (hm-infer-seq state tyenv (cdr forms)))))

(defun hm-infer-lambda (state tyenv expr)
  (let ((params (cadr expr)) (body (cddr expr)))
    (if (null body)
        (error "lambda: empty body is not supported by this core checker")
        (let* ((ptys (mapcar (lambda (p) (hm-fresh state)) params))
               (bindings (mapcar (lambda (p pt) (cons p (list 'forall nil pt))) params ptys))
               (inner (append bindings tyenv))
               (rty (hm-infer-seq state inner body)))
          (list '-> ptys rty)))))

(defun hm-infer-app (state tyenv expr)
  (let* ((fty (hm-infer state tyenv (car expr)))
         (argtys (mapcar (lambda (a) (hm-infer state tyenv a)) (cdr expr)))
         (rty (hm-fresh state)))
    (hm-unify! state fty (list '-> argtys rty))
    rty))

(defun hm-infer-if (state tyenv expr)
  (let ((cty (hm-infer state tyenv (cadr expr)))
        (tty (hm-infer state tyenv (caddr expr)))
        (ety (hm-infer state tyenv (cadddr expr))))
    (hm-unify! state cty 'bool)
    (hm-unify! state tty ety)
    tty))

(defun hm-infer-let-bindings (state tyenv bindings)
  "Sequential (LET*-style) LET bindings: each RHS is checked and generalized
before the next binding is added, so later bindings -- and the body -- may
use an earlier one polymorphically."
  (if (null bindings)
      tyenv
      (let* ((b (car bindings))
             (vty (hm-infer state tyenv (cadr b)))
             (scheme (hm-generalize state tyenv vty)))
        (hm-infer-let-bindings state (cons (cons (car b) scheme) tyenv) (cdr bindings)))))

(defun hm-infer-let (state tyenv expr)
  (let* ((bindings (cadr expr)) (body (cddr expr))
         (inner (hm-infer-let-bindings state tyenv bindings)))
    (if (null body)
        (error "let: empty body is not supported by this core checker")
        (hm-infer-seq state inner body))))

(defun hm-infer-mk-record (state tyenv expr)
  (let ((fields (mapcar (lambda (f) (cons (car f) (hm-infer state tyenv (cadr f))))
                        (cdr expr))))
    (list 'record (sort-fields fields) nil)))

(defun hm-infer-field-ref (state tyenv expr)
  (let* ((rty (hm-infer state tyenv (cadr expr)))
         (label (caddr expr))
         (fty (hm-fresh state))
         (rho (hm-fresh state)))
    (hm-unify! state rty (list 'record (list (cons label fty)) rho))
    fty))

(defun hm-parse-ty (form)
  "Parse a closed (variable-free) surface type FORM into the internal
representation. Scalars are bare symbols; (-> (args) ret), (list T),
(pair T1 T2), and (record ((label T)...)) [always closed here -- a THE
ascription names a concrete type] are recognized; anything else headed by
a symbol is a NOMINAL (named) application, e.g. (point int64 int64)."
  (cond
    ((member form '(int64 float64 bool string any)) form)
    ((and (consp form) (eq (car form) '->))
     (list '-> (mapcar #'hm-parse-ty (cadr form)) (hm-parse-ty (caddr form))))
    ((and (consp form) (eq (car form) 'list)) (list 'list (hm-parse-ty (cadr form))))
    ((and (consp form) (eq (car form) 'pair))
     (list 'pair (hm-parse-ty (cadr form)) (hm-parse-ty (caddr form))))
    ((and (consp form) (eq (car form) 'record))
     (list 'record
           (mapcar (lambda (f) (cons (car f) (hm-parse-ty (cadr f)))) (cadr form))
           nil))
    ((and (consp form) (symbolp (car form)))
     (cons 'named (cons (car form) (mapcar #'hm-parse-ty (cdr form)))))
    (t (error (concat "hm-parse-ty: cannot parse type " (princ-to-string form))))))

(defun hm-infer-the (state tyenv expr)
  (let ((declared (hm-parse-ty (cadr expr)))
        (actual (hm-infer state tyenv (caddr expr))))
    (hm-unify! state declared actual)
    declared))

(defun hm-infer (state tyenv expr)
  (cond
    ((and (numberp expr) (floatp expr)) 'float64)
    ((numberp expr) 'int64)
    ((stringp expr) 'string)
    ((eq expr t) 'bool)
    ((symbolp expr) (hm-lookup-var state tyenv expr))
    ((not (consp expr)) (error (concat "hm-infer: cannot classify literal "
                                        (princ-to-string expr))))
    ((eq (car expr) 'lambda) (hm-infer-lambda state tyenv expr))
    ((eq (car expr) 'if) (hm-infer-if state tyenv expr))
    ((eq (car expr) 'let) (hm-infer-let state tyenv expr))
    ((eq (car expr) 'mk-record) (hm-infer-mk-record state tyenv expr))
    ((eq (car expr) 'field-ref) (hm-infer-field-ref state tyenv expr))
    ((eq (car expr) 'the) (hm-infer-the state tyenv expr))
    (t (hm-infer-app state tyenv expr))))

;;; ---- public entry points: verdicts --------------------------------------

(defun hm-check-expr (expr)
  "Check EXPR (in an empty type environment) with a fresh checker state.
Returns (CHECKED rendered-scheme) | (TYPE-ERROR \"msg\")."
  (let ((state (hm-new-state)))
    (handler-case
        (let* ((ty (hm-infer state nil expr))
               (scheme (hm-generalize state nil ty)))
          (list 'checked (hm-render-scheme scheme)))
      (error (e) (list 'type-error (error-message e))))))

(defun hm-check-lambda (params body)
  "Check a function of PARAMS (a flat list of bare symbols -- no &optional/
&rest/&key: those are DYNAMIC here, exactly as the native checker's
`checker_lambda_source` excludes a variadic lambda from checking) and BODY
(a list of body forms; the last is the return expression). Returns
(CHECKED rendered-scheme) | (TYPE-ERROR \"msg\") | (DYNAMIC \"reason\"),
matching the shape of the native checker's SEE-TYPE verdict."
  (cond
    ((null body) (list 'dynamic "empty body"))
    ((not (every #'symbolp params)) (list 'dynamic "non-symbol parameter"))
    ((member '&rest params) (list 'dynamic "variadic parameter list"))
    ((member '&optional params) (list 'dynamic "variadic parameter list"))
    ((member '&key params) (list 'dynamic "variadic parameter list"))
    (t (let ((state (hm-new-state)))
         (handler-case
             (let* ((ptys (mapcar (lambda (p) (hm-fresh state)) params))
                    (tyenv (mapcar (lambda (p pt) (cons p (list 'forall nil pt))) params ptys))
                    (rty (hm-infer-seq state tyenv body))
                    (scheme (hm-generalize state nil (list '-> ptys rty))))
               (list 'checked (hm-render-scheme scheme)))
           (error (e) (list 'type-error (error-message e))))))))
