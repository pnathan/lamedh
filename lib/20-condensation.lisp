;;; Condensation: compact seeds, deterministic expansions, first-class change.
;;;
;;; The contract of this file, one line per idea:
;;;   - a definition's provenance (seed -> expansion -> generated symbols) is
;;;     ordinary data on the defining symbol's plist;
;;;   - the checker's verdict on every generated symbol is data (SEE-TYPE),
;;;     classified honestly: TYPED / CHECKED / VACUOUS / DYNAMIC / TYPE-ERROR.
;;;     VACUOUS means the checker found no contradiction but proved nothing;
;;;     it counts toward the unproven frontier, never as verified;
;;;   - change is data: CONDENSE-DIFF and SEXPR-PATCH are inverses, and EDIT!
;;;     applies a minimal path-addressed change to a live definition with the
;;;     HM checker as the barrier -- an edit that introduces a type error into
;;;     previously clean code is rolled back and rejected.

;;; ---- metadata --------------------------------------------------------------

(defun condense-put (sym key value)
  "Store condensation metadata VALUE for SYM under KEY."
  (putp sym key value))

(defun condense-get (sym key)
  "Retrieve condensation metadata for SYM under KEY."
  (getp sym key))

(defun condense-kind (sym)
  "Return the condensation kind recorded for SYM."
  (condense-get sym "condense.kind"))

(defun condense-source (sym)
  "Return the source form recorded for SYM."
  (condense-get sym "condense.source"))

(defun condense-expansion (sym)
  "Return the expansion recorded for SYM."
  (condense-get sym "condense.expansion"))

(defun condense-generated (sym)
  "Return the generated symbols recorded for SYM."
  (condense-get sym "condense.generated"))

(defun condense-record! (sym kind source expansion generated)
  "Record the core condensation trace for SYM."
  (condense-put sym "condense.kind" kind)
  (condense-put sym "condense.source" source)
  (condense-put sym "condense.expansion" expansion)
  (condense-put sym "condense.generated" generated)
  (condense-put sym "condense.contracts" nil)
  (condense-put sym "condense.laws" nil)
  (condense-put sym "condense.check-status" nil)
  (condense-put sym "condense.dynamic-frontier" nil)
  sym)

(defun condense-append-new (xs ys)
  (cond
    ((null ys) xs)
    ((member (car ys) xs) (condense-append-new xs (cdr ys)))
    (t (condense-append-new (append xs (list (car ys))) (cdr ys)))))

(defun condense-section-value (section)
  (if (null section) nil (cadr section)))

;;; ---- the sexpr change plane ------------------------------------------------
;;;
;;; A change is a list of (path old new) triples. Paths are positions from the
;;; root. CONDENSE-DIFF produces them; SEXPR-PATCH consumes them, guarding each
;;; edit on OLD so a stale patch fails loudly instead of applying silently.

(defun sexpr-ref (form path)
  "Return the subform of FORM at PATH, a list of positions from the root."
  (if (null path)
      form
      (sexpr-ref (nth (car path) form) (cdr path))))

(defun sexpr-set-nth (form n value)
  (if (= n 0)
      (cons value (cdr form))
      (cons (car form) (sexpr-set-nth (cdr form) (- n 1) value))))

(defun sexpr-set (form path value)
  "Return FORM with the subform at PATH replaced by VALUE (non-mutating)."
  (if (null path)
      value
      (sexpr-set-nth form (car path)
                     (sexpr-set (nth (car path) form) (cdr path) value))))

(defun condense-proper-list-p (x)
  (cond
    ((null x) t)
    ((consp x) (condense-proper-list-p (cdr x)))
    (t nil)))

(defun condense-diff-children (old new index path)
  (if (null old)
      nil
      (append (condense-diff-node (car old) (car new) (cons index path))
              (condense-diff-children (cdr old) (cdr new) (+ index 1) path))))

(defun condense-diff-node (old new path)
  (cond
    ((equal old new) nil)
    ((and (consp old)
          (consp new)
          (condense-proper-list-p old)
          (condense-proper-list-p new)
          (equal (length old) (length new)))
     (condense-diff-children old new 0 path))
    (t (list (list (reverse path) old new)))))

(defun condense-diff (old new)
  "Return the change from OLD to NEW as (path old new) triples.
Equal-length proper lists are compared element-wise so a change localizes to
a path; any shape change reports the enclosing node whole."
  (condense-diff-node old new nil))

(defun sexpr-locate-children (forms target index path acc)
  (if (null forms)
      acc
      (sexpr-locate-children (cdr forms) target (+ index 1) path
                             (sexpr-locate-walk (car forms) target
                                                (cons index path) acc))))

(defun sexpr-locate-walk (form target path acc)
  (let ((acc (if (equal form target) (cons (reverse path) acc) acc)))
    (if (and (consp form) (condense-proper-list-p form))
        (sexpr-locate-children form target 0 path acc)
        acc)))

(defun sexpr-locate (form target)
  "Return the unique path of subform TARGET within FORM.
Absence and ambiguity are both errors: an edit must name its site uniquely."
  (let ((paths (sexpr-locate-walk form target nil nil)))
    (cond
      ((null paths)
       (error (concat "sexpr-locate: not found: " (princ-to-string target))))
      ((cdr paths)
       (error (concat "sexpr-locate: ambiguous, "
                      (princ-to-string (length paths))
                      " occurrences of "
                      (princ-to-string target))))
      (t (car paths)))))

(defun condense-normalize-edit (form edit)
  "Normalize EDIT to (path old new). A two-element (old new) edit locates OLD
uniquely in FORM, so callers can name the subform instead of counting paths."
  (if (null (cddr edit))
      (list (sexpr-locate form (car edit)) (car edit) (cadr edit))
      edit))

(defun sexpr-patch (form edits)
  "Apply EDITS to FORM. An edit is (path old new) -- as produced by
CONDENSE-DIFF -- or (old new), which locates OLD uniquely. Each edit's OLD
must match the current subform, or the patch is an error.
Inverse of CONDENSE-DIFF: (sexpr-patch old (condense-diff old new)) = NEW."
  (if (null edits)
      form
      (let* ((edit (condense-normalize-edit form (car edits)))
             (path (car edit))
             (old (cadr edit))
             (new (caddr edit))
             (current (sexpr-ref form path)))
        (if (equal current old)
            (sexpr-patch (sexpr-set form path new) (cdr edits))
            (error (concat "sexpr-patch: expected "
                           (princ-to-string old)
                           " at "
                           (princ-to-string path)
                           ", found "
                           (princ-to-string current)))))))

;;; ---- honest checker verdicts -------------------------------------------
;;;
;;; SEE-TYPE (a builtin) reports the checker's verdict structurally:
;;;   (TYPED sig COMPILED|INTERPRETED) | (CHECKED scheme)
;;;   | (TYPE-ERROR msg) | (DYNAMIC reason)
;;; CONDENSE-CLASSIFY refines CHECKED: a scheme whose result type contains a
;;; variable unconstrained by any argument -- e.g.
;;; (FORALL (A B C) (-> (A B) C)) -- is VACUOUS: no contradiction, no promise.

(defun condense-occurs-p (x form)
  (cond
    ((eq form x) t)
    ((consp form) (or (condense-occurs-p x (car form))
                      (condense-occurs-p x (cdr form))))
    (t nil)))

(defun condense-vacuous-p (scheme)
  "T when SCHEME's result contains a type variable no argument constrains."
  (if (and (consp scheme) (eq (car scheme) 'forall))
      (let ((vars (cadr scheme))
            (arrow (caddr scheme)))
        (if (and (consp arrow) (eq (car arrow) '->))
            (let* ((args (cadr arrow))
                   (ret (caddr arrow))
                   (ret-vars (filter (lambda (v) (condense-occurs-p v ret)) vars))
                   (loose (filter (lambda (v) (not (condense-occurs-p v args)))
                                  ret-vars)))
              (not (null loose)))
            nil))
      nil))

(defun condense-classify (verdict)
  "Classify a SEE-TYPE verdict:
TYPED / CHECKED / DECLARED / VACUOUS / DYNAMIC / TYPE-ERROR.
DECLARED is an axiom asserted via DECLARE-TYPE! (e.g. a row-typed concept
accessor): trusted by the checker at call sites, generated in lockstep with
its implementation, but not derived from the body."
  (cond
    ((eq (car verdict) 'typed) 'typed)
    ((eq (car verdict) 'declared) 'declared)
    ((eq (car verdict) 'checked)
     (if (condense-vacuous-p (cadr verdict)) 'vacuous 'checked))
    ((eq (car verdict) 'type-error) 'type-error)
    (t 'dynamic)))

(defun condense-verified-p (status)
  "T for statuses that carry a guarantee (TYPED, informative CHECKED) or a
generator-backed axiom (DECLARED)."
  (if (member status '(typed checked declared)) t nil))

(defun condense-check-type-one (sym)
  (let ((verdict (see-type sym)))
    (list sym (condense-classify verdict) verdict)))

(defun condense-check-type (sym)
  "Classify every generated symbol of SYM; store per-symbol verdicts.
Anything not TYPED or informative CHECKED joins the unproven frontier."
  (let* ((targets (if (condense-generated sym)
                      (condense-generated sym)
                      (list sym)))
         (results (mapcar #'condense-check-type-one targets))
         (frontier (filter (lambda (r) (not (condense-verified-p (cadr r))))
                           results)))
    (condense-put sym "condense.check-status" results)
    (condense-put sym "condense.dynamic-frontier" frontier)
    results))

;;; ---- fingerprints and staleness ------------------------------------------
;;;
;;; Condensation is a one-way lens: the seed cannot be recovered from an edited
;;; expansion. The discipline is regenerate-only, enforced by detection: every
;;; generated definition is snapshotted, and drift is flagged in the trace so
;;; it never silently vouches for code the seed no longer describes.

(defun condense-definition-of (sym)
  "Return SYM's current definition form, or NIL when it has none."
  (car (errorset (list 'see-source (list 'quote sym)))))

(defun condense-fingerprint! (sym)
  "Snapshot the current definitions of SYM's generated symbols."
  (condense-put sym "condense.fingerprints"
                (mapcar (lambda (g) (cons g (condense-definition-of g)))
                        (condense-generated sym)))
  sym)

(defun condense-drifted (sym)
  (filter (lambda (fp) (not (equal (cdr fp) (condense-definition-of (car fp)))))
          (condense-get sym "condense.fingerprints")))

(defun condense-stale (sym)
  "Generated symbols of SYM whose definitions drifted since fingerprinting."
  (mapcar #'car (condense-drifted sym)))

(defun condense-drift (sym)
  "Return (generated-symbol . diff) pairs for drifted definitions of SYM."
  (mapcar (lambda (fp)
            (cons (car fp)
                  (condense-diff (cdr fp) (condense-definition-of (car fp)))))
          (condense-drifted sym)))

;;; ---- trace -----------------------------------------------------------------

(def *condense-trace-keys*
  '((kind . "condense.kind")
    (source . "condense.source")
    (expansion . "condense.expansion")
    (generated . "condense.generated")
    (contracts . "condense.contracts")
    (laws . "condense.laws")
    (check-status . "condense.check-status")
    (dynamic-frontier . "condense.dynamic-frontier")
    (fields . "condense.fields")
    (invariant . "condense.invariant")
    (derivations . "condense.derivations")
    (examples . "condense.examples")
    (concept . "condense.concept")
    (assert . "condense.assert")
    (given . "condense.given")
    (expect . "condense.expect")
    (edits . "condense.edits")
    (last-diff . "condense.last-diff")))

(defun condense-trace (sym)
  "Return an alist describing the condensation trace recorded for SYM."
  (append
    (mapcar (lambda (entry) (cons (car entry) (condense-get sym (cdr entry))))
            *condense-trace-keys*)
    (list (cons 'stale (condense-stale sym)))))

;;; ---- edit!: minimum change, checker as the barrier -------------------------

(defun condense-edit-report (sym before verdict edits)
  (list (cons 'symbol sym)
        (cons 'was before)
        (cons 'now (condense-classify verdict))
        (cons 'applied edits)))

(defun condense-edit-function! (sym edits e)
  (let ((lam (condense-definition-of sym)))
    (if (null lam)
        (error (concat "edit!: no editable source for "
                       (princ-to-string sym)))
        (let* ((patched (sexpr-patch lam edits))
               (before (condense-classify (see-type sym))))
          (eval (cons 'defun (cons sym (cdr patched))) e)
          (let ((verdict (see-type sym)))
            (if (and (eq (condense-classify verdict) 'type-error)
                     (not (eq before 'type-error)))
                (progn
                  (eval (cons 'defun (cons sym (cdr lam))) e)
                  (error (concat "edit!: rejected, introduces a type error: "
                                 (princ-to-string (cadr verdict)))))
                (progn
                  (condense-put sym "condense.edits"
                                (cons edits (condense-get sym "condense.edits")))
                  (condense-edit-report sym before verdict edits))))))))

(defun condense-edit-concept! (sym edits e)
  (let* ((source (condense-source sym))
         (patched (sexpr-patch source edits))
         (laws (condense-get sym "condense.laws"))
         (derivations (condense-get sym "condense.derivations")))
    (eval patched e)
    (condense-put sym "condense.laws" laws)
    (if derivations (eval (cons 'derive (cons sym derivations)) e) nil)
    (condense-check-type sym)
    (condense-fingerprint! sym)
    (condense-put sym "condense.edits"
                  (cons edits (condense-get sym "condense.edits")))
    (list (cons 'symbol sym)
          (cons 'last-diff (condense-get sym "condense.last-diff"))
          (cons 'dynamic-frontier (condense-get sym "condense.dynamic-frontier"))
          (cons 'checks (if (condense-get sym "condense.examples")
                            (condense-check sym)
                            (cons t nil)))
          (cons 'applied edits))))

(defvau edit! (x e)
  "Apply EDITS, (path old new) triples, to SYM's live definition and re-check.

  (edit! 'sym edits)

The minimum-change verb; both operands are evaluated, so edit lists may be
computed (e.g. from CONDENSE-DIFF). Paths address subforms of
(SEE-SOURCE SYM); each edit is guarded on OLD so stale patches fail instead
of applying silently. For a plain function the HM checker is the barrier: an
edit that introduces a TYPE-ERROR into a definition that previously had none
is rolled back and rejected. For a concept the *seed* is patched: the
DEFCONCEPT source is re-evaluated, recorded derivations re-derived, and every
check re-run -- one edit to the seed regenerates and re-verifies the whole
artifact. Returns a report alist."
  (let ((sym (eval (car x) e))
        (edits (eval (cadr x) e)))
    (if (eq (condense-kind sym) 'concept)
        (condense-edit-concept! sym edits e)
        (condense-edit-function! sym edits e))))

;;; ---- defconcept ------------------------------------------------------------

(defun condense-symbol-append3 (a b c)
  (intern (concat a (princ-to-string b) c)))

(defun condense-constructor-symbol (concept)
  (condense-symbol-append3 "MAKE-" concept ""))

(defun condense-predicate-symbol (concept)
  (condense-symbol-append3 "" concept "-P"))

(defun condense-validator-symbol (concept)
  (condense-symbol-append3 "VALIDATE-" concept ""))

(defun condense-printer-symbol (concept)
  (condense-symbol-append3 "" concept "->PLIST"))

(defun condense-builder-symbol (concept)
  (condense-symbol-append3 "PLIST->" concept ""))

(defun condense-equality-symbol (concept)
  (condense-symbol-append3 "" concept "-EQUAL"))

(defun condense-lens-law-symbol (concept)
  (condense-symbol-append3 "" concept "-LENS-ROUNDTRIP"))

(defun condense-accessor-symbol (concept field)
  (intern (concat (princ-to-string concept) "-" (princ-to-string field))))

(defun condense-field-names (field-specs)
  (mapcar #'car field-specs))

;; Concept fields become defrecord fields. A field type outside the checker's
;; language (a symbol naming no record) degrades to ANY -- gradual, not an
;; error: the field is stored and accessed identically, just unchecked.
(defun condense-concept-field-ty (ty)
  (if (or (eq ty 'any)
          (condense-row-type-p ty)
          (and (atom ty) (condense-get ty "condense.kind")))
      ty
      'any))

(defun condense-concept-record-specs (field-specs)
  (mapcar (lambda (spec)
            (list (car spec) (condense-concept-field-ty (cadr spec))))
          field-specs))

(defun condense-validator-bindings (concept fields)
  (mapcar (lambda (field)
            (list field (list (condense-accessor-symbol concept field) 'self)))
          fields))

(defun condense-invariant-expression (invariant-section)
  (cond
    ((null invariant-section) t)
    ((null (cdr invariant-section)) t)
    ((null (cddr invariant-section)) (cadr invariant-section))
    (t (cons 'and (cdr invariant-section)))))

(defun condense-concept-generated (concept fields)
  (append
    (list (condense-constructor-symbol concept)
          (condense-predicate-symbol concept)
          (condense-validator-symbol concept))
    (mapcar (lambda (field) (condense-accessor-symbol concept field)) fields)))

(defun condense-concept-expansion (concept field-specs invariant-section)
  "Concept = DEFRECORD (branded, gradual-typed, tier-dispatched -- issue
#308) + a validator closing over the invariant. Constructor, predicate, and
accessors all come from the record core; the concept layer adds meaning."
  (let ((predicate (condense-predicate-symbol concept))
        (validator (condense-validator-symbol concept))
        (fields (condense-field-names field-specs))
        (invariant (condense-invariant-expression invariant-section)))
    `(progn
       (defrecord ,concept ,@(condense-concept-record-specs field-specs))
       (defun ,validator (self)
         (and (,predicate self)
              (let ,(condense-validator-bindings concept fields)
                ,invariant)))
       ;; Brand-in/bool-out, declared in lockstep (the invariant is
       ;; arbitrary user code the checker can't always see through).
       (declare-type! ',validator '(-> (,concept) bool))
       ',concept)))

;; Row declarations (experimental): when every field type maps into the
;; checker's type language, the generated operations get DECLARED row schemes,
;; so the checker types uses of accessors/constructors across the whole
;; program -- (defun total (x) (invoice-amount x)) infers
;; (forall (r) (-> ((record ((amount int64)) r)) int64)), and passing the
;; wrong concept to an accessor is a static type error.
(def *condense-row-types* '(int64 float64 bool char string symbol))

;; Compound field types that also map into the checker's type language — the
;; declared-scheme parser already accepts these inside a record, so a field
;; carrying one gets a row scheme too. This lifts the former "every field must
;; be a flat scalar or the concept gets no rows at all" limitation: a record
;; with a (list string) or (array int64) field is now fully row-typed.
(def *condense-row-compounds* '(list array pair record))

(defun condense-row-type-p (ty)
  "T when field type TY maps into the checker's row type language."
  (or (member ty *condense-row-types*)
      (and (consp ty) (member (car ty) *condense-row-compounds*))))

(defun condense-row-field (spec)
  (if (condense-row-type-p (cadr spec))
      (list (car spec) (cadr spec))
      nil))

(defun condense-row-fields (field-specs)
  "Map FIELD-SPECS into row fields, or NIL if any field type is unmappable."
  (let ((mapped (mapcar #'condense-row-field field-specs)))
    (if (member nil mapped) nil mapped)))

(defun condense-declare-rows! (concept field-specs)
  (let ((fields (condense-row-fields field-specs)))
    (if (null fields)
        nil
        (progn
          (declare-type! (condense-constructor-symbol concept)
                         `(-> ,(mapcar #'cadr fields) (record ,fields)))
          (declare-type! (condense-predicate-symbol concept)
                         '(forall (a) (-> (a) bool)))
          (declare-type! (condense-validator-symbol concept)
                         `(forall (r) (-> ((record ,fields r)) bool)))
          (mapc (lambda (field)
                  (declare-type! (condense-accessor-symbol concept (car field))
                                 `(forall (r)
                                    (-> ((record (,field) r)) ,(cadr field)))))
                fields)
          t))))

(defun condense-declare-derive-rows! (concept derivations)
  "Declare BRANDED schemes for derived operations: since every concept is a
registered record type (#308), the derived equality/printer/lens signatures
name the concept itself -- nominal in, row-subsumption still applies out."
  (mapc (lambda (d)
          (cond
            ((eq d 'equality)
             (declare-type! (condense-equality-symbol concept)
                            `(-> (,concept ,concept) bool)))
            ;; The printer builds dotted pairs from typed components, which
            ;; the checker's list-biased CONS rule rejects; its scheme is
            ;; declared (generated in lockstep) instead.
            ((eq d 'printer)
             (declare-type! (condense-printer-symbol concept)
                            `(-> (,concept) (list (pair symbol any)))))
            ((eq d 'lens)
             (progn
               (declare-type! (condense-printer-symbol concept)
                              `(-> (,concept) (list (pair symbol any))))
               (declare-type! (condense-builder-symbol concept)
                              `(forall (a) (-> ((list a)) ,concept)))
               (declare-type! (condense-lens-law-symbol concept)
                              `(-> (,concept) bool))))
            (t nil)))
        derivations))

;;; ---- defrecord: the one-door gradual-typed record --------------------------
;;;
;;; (defrecord Name (field type)...) is the recommended way to define a
;;; record — ONE BODY across both tiers (issue #308):
;;;
;;;   - every defrecord registers a branded StructDef: the name is a real,
;;;     DENOTABLE type, nominal in the checker (chest /= crate even with the
;;;     same fields), and row-subsumable (#299) — one row-polymorphic
;;;     function accepts every record naming a subset of its fields;
;;;   - every defrecord value is a StructObj — one runtime representation,
;;;     read and functionally updated through RECORD-REF / RECORD-WITH,
;;;     whose checker rules DERIVE the row types (no axioms);
;;;   - the tier is chosen by the compiler: all-native fields (scalars,
;;;     (array scalar)) compile via the defstruct-typed machinery (native
;;;     constructor and accessors); any other checkable field type (list,
;;;     pair, nested record, string, symbol) gets dynamic constructor and
;;;     accessors over RECORD-NEW/RECORD-REF with DECLARED branded
;;;     signatures. Same surface either way. (record-compiled-p 'Name)
;;;     reports the tier.
;;;
;;; DEFCONCEPT layers invariants, derivations, and the condensation trace on
;;; this same core. DEFSTRUCT (untyped, mutable) and DEFSTRUCT-TYPED
;;; (native-only, with mutating setters) remain explicit escape hatches.

(defun record-native-field-p (spec)
  "T when field SPEC's type is natively storable (compiled tier)."
  (let ((ty (cadr spec)))
    (or (member ty '(int64 float64 bool char))
        (and (consp ty) (eq (car ty) 'array)
             (member (cadr ty) '(int64 float64 bool char))))))

(defun record-fields-native-p (field-specs)
  (not (member nil (mapcar #'record-native-field-p field-specs))))

(defun $record-getter-forms (name field-specs)
  "Dynamic-tier getters: NAME-FIELD over RECORD-REF, each with a DECLARED
branded signature (-> (NAME) field-type) — derived from the def in
lockstep, so axiom/implementation drift is impossible."
  (mapcar
   (lambda (spec)
     (let ((getter (condense-accessor-symbol name (car spec))))
       `(progn
          (defun ,getter (self) (record-ref self ',(car spec)))
          (declare-type! ',getter '(-> (,name) ,(cadr spec))))))
   field-specs))

(defvau defrecord (x e)
  "(DEFRECORD Name (field type)...) — define a gradual-typed record: a
branded, denotable, row-subsumable type whose tier (compiled or dynamic)
is chosen by the compiler from the field types. Generates make-Name,
Name-p, and Name-field accessors; values are read with the accessors or
generically with RECORD-REF and updated with RECORD-WITH."
  (let* ((name (car x))
         (field-specs (cdr x)))
    (if (null field-specs)
        (error "defrecord requires at least one field")
        (progn
          (if (record-fields-native-p field-specs)
              ;; Compiled tier: native branded type, constructor, accessors.
              (eval (cons 'defstruct-typed (cons name field-specs)) e)
              ;; Dynamic tier: branded type + StructObj values.
              (progn
                (eval (list 'record-declare (list 'quote name)
                            (list 'quote field-specs)) e)
                (let ((argnames (mapcar #'car field-specs))
                      (argtys (mapcar #'cadr field-specs))
                      (ctor (condense-constructor-symbol name)))
                  (eval `(progn
                           (defun ,ctor ,argnames (record-new ',name ,@argnames))
                           (declare-type! ',ctor '(-> ,argtys ,name)))
                        e))
                (mapc (lambda (form) (eval form e))
                      ($record-getter-forms name field-specs))))
          ;; Both tiers: a predicate (defstruct-typed generates none) and
          ;; condensation metadata.
          ;; record-brand is a dynamic primitive (works on anything), so the
          ;; predicate's scheme is declared in lockstep with its definition.
          (eval `(progn
                   (defun ,(condense-predicate-symbol name) (v)
                     (eq (record-brand v) ',name))
                   (declare-type! ',(condense-predicate-symbol name)
                                  '(forall (a) (-> (a) bool))))
                e)
          (condense-put name "condense.kind" 'record)
          (condense-put name "condense.fields" field-specs)
          name))))

(defun record-compile-eligible-p (name)
  "Whether record NAME's fields are all natively storable — i.e. which tier
DEFRECORD chose. Alias over the kernel's RECORD-COMPILED-P."
  (ignore-errors (record-compiled-p name)))

(defvau defconcept (x e)
  "Define a compact concept: fields, invariant, generated operations, trace.
An optional (:derive target ...) section derives support code in the same
form, so the whole artifact has a single seed to edit."
  (let* ((concept (car x))
         (sections (cdr x))
         (fields-section (assoc :fields sections))
         (invariant-section (assoc :invariant sections))
         (derive-section (assoc :derive sections)))
    (if (null fields-section)
        (error "defconcept requires a :fields section")
        (let* ((field-specs (cadr fields-section))
               (fields (condense-field-names field-specs))
               (expansion (condense-concept-expansion concept field-specs invariant-section))
               (generated (condense-concept-generated concept fields))
               (source (cons 'defconcept x))
               (previous (condense-expansion concept)))
          (eval expansion e)
          (condense-record! concept 'concept source expansion generated)
          (condense-put concept "condense.fields" field-specs)
          (condense-put concept "condense.invariant" invariant-section)
          (condense-put concept "condense.last-diff"
                        (if previous (condense-diff previous expansion) nil))
          (condense-fingerprint! concept)
          (if derive-section
              (eval (cons 'derive (cons concept (cdr derive-section))) e)
              nil)
          concept))))

;;; ---- derive ----------------------------------------------------------------

(defun condense-printer-pairs (concept fields)
  (mapcar (lambda (field)
            `(cons ',field (,(condense-accessor-symbol concept field) self)))
          fields))

(defun condense-builder-args (fields)
  (mapcar (lambda (field) `(alist-get view ',field)) fields))

(defun condense-derive-symbol-list (concept derivation)
  (cond
    ((eq derivation 'printer) (list (condense-printer-symbol concept)))
    ((eq derivation 'equality) (list (condense-equality-symbol concept)))
    ((eq derivation 'lens) (list (condense-printer-symbol concept)
                                 (condense-builder-symbol concept)
                                 (condense-lens-law-symbol concept)))
    (t (error "unknown derive target"))))

(defun condense-derive-form (concept derivation fields)
  (cond
    ((eq derivation 'printer)
     `(defun ,(condense-printer-symbol concept) (self)
        (list ,@(condense-printer-pairs concept fields))))
    ((eq derivation 'equality)
     `(defun ,(condense-equality-symbol concept) (a b)
        (and (,(condense-predicate-symbol concept) a)
             (,(condense-predicate-symbol concept) b)
             (equal a b))))
    ((eq derivation 'lens)
     `(progn
        (defun ,(condense-printer-symbol concept) (self)
          (list ,@(condense-printer-pairs concept fields)))
        (defun ,(condense-builder-symbol concept) (view)
          (,(condense-constructor-symbol concept)
           ,@(condense-builder-args fields)))))
    (t (error "unknown derive target"))))

(defun condense-derive-post-form (concept derivation)
  (if (eq derivation 'lens)
      (list `(deflaw ,(condense-lens-law-symbol concept)
               (:for ,concept)
               (:assert (equal (,(condense-builder-symbol concept)
                                (,(condense-printer-symbol concept) self))
                               self))))
      nil))

(defun condense-derive-symbols (concept derivations)
  (if (null derivations)
      nil
      (condense-append-new
        (condense-derive-symbol-list concept (car derivations))
        (condense-derive-symbols concept (cdr derivations)))))

(defvau derive (x e)
  "Generate deterministic support code from concept metadata, then re-verify.
Targets: printer (<c>->plist), equality (<c>-equal), lens (<c>->plist,
plist-><c>, and a <c>-lens-roundtrip law)."
  (let* ((concept (car x))
         (derivations (cdr x))
         (field-specs (condense-get concept "condense.fields")))
    (if (not (eq (condense-kind concept) 'concept))
        (error "derive requires a condensed concept")
        (let* ((fields (condense-field-names field-specs))
               (old-derivations (condense-get concept "condense.derivations"))
               (all-derivations (condense-append-new old-derivations derivations))
               (forms (mapcar (lambda (d) (condense-derive-form concept d fields))
                              derivations))
               (post (reduce #'append
                             (mapcar (lambda (d) (condense-derive-post-form concept d))
                                     derivations)
                             nil))
               (generated (condense-derive-symbols concept all-derivations))
               (base-generated (condense-concept-generated concept fields)))
          (eval (cons 'progn (append forms (list `',concept))) e)
          (mapc (lambda (form) (eval form e)) post)
          (condense-declare-derive-rows! concept derivations)
          (condense-put concept "condense.derivations" all-derivations)
          (condense-put concept "condense.generated"
                        (condense-append-new base-generated generated))
          (condense-check-type concept)
          (condense-fingerprint! concept)
          concept))))

;;; ---- laws, examples, and checks -----------------------------------------

(defun condense-require-concept (concept action)
  (if (eq (condense-kind concept) 'concept)
      concept
      (error (concat action " requires a condensed concept"))))

(defvau deflaw (x e)
  "Attach a named predicate law to a condensed concept."
  (let* ((name (car x))
         (sections (cdr x))
         (concept (condense-section-value (assoc :for sections)))
         (assertion (condense-section-value (assoc :assert sections))))
    (condense-require-concept concept "deflaw")
    (if (null assertion)
        (error "deflaw requires an :assert section")
        (let* ((fields (condense-field-names
                        (condense-get concept "condense.fields")))
               (expansion `(defun ,name (self)
                             (and (,(condense-predicate-symbol concept) self)
                                  (let ,(condense-validator-bindings concept fields)
                                    ,assertion)))))
          (eval expansion e)
          (condense-record! name 'law (cons 'deflaw x) expansion (list name))
          (condense-put name "condense.concept" concept)
          (condense-put name "condense.assert" assertion)
          (condense-put concept "condense.laws"
                        (condense-append-new
                          (condense-get concept "condense.laws")
                          (list name)))
          name))))

(defvau example (x e)
  "Attach an executable example check to a condensed concept."
  (let* ((name (car x))
         (sections (cdr x))
         (concept (condense-section-value (assoc :for sections)))
         (given (condense-section-value (assoc :given sections)))
         (expect (condense-section-value (assoc :expect sections))))
    (condense-require-concept concept "example")
    (if (null expect)
        (error "example requires an :expect section")
        (let ((expansion `(defun ,name ()
                            (let ((*it* ,given))
                              ,expect))))
          (eval expansion e)
          (condense-record! name 'example (cons 'example x) expansion (list name))
          (condense-put name "condense.concept" concept)
          (condense-put name "condense.given" given)
          (condense-put name "condense.expect" expect)
          (condense-put concept "condense.examples"
                        (condense-append-new
                          (condense-get concept "condense.examples")
                          (list name)))
          name))))

(defun condense-check (sym)
  "Run executable condensation checks for SYM and return (PASS . RESULTS)."
  (cond
    ((eq (condense-kind sym) 'concept)
     (let ((results (mapcar (lambda (name) (cons name (funcall name)))
                            (condense-get sym "condense.examples"))))
       (cons (null (filter (lambda (r) (null (cdr r))) results)) results)))
    ((eq (condense-kind sym) 'example)
     (let ((result (funcall sym)))
       (cons result (list (cons sym result)))))
    (t (error "condense-check requires a concept or example"))))

(defun condense-recheck! (sym)
  "Re-verify SYM: staleness, examples, and checker status. Updates metadata."
  (list
    (cons 'stale (condense-stale sym))
    (cons 'drift (condense-drift sym))
    (cons 'checks (if (and (eq (condense-kind sym) 'concept)
                           (condense-get sym "condense.examples"))
                      (condense-check sym)
                      (cons t nil)))
    (cons 'check-status (condense-check-type sym))))

;;; ---- check-file!: the agent loop -------------------------------------------
;;;
;;; Agents do not hold a live image: they edit files with their own tools and
;;; verify in batch. CHECK-FILE! is that verification step -- load the file,
;;; report an honest verdict for everything it defines. Reports are data, so
;;; two runs diff with CONDENSE-DIFF: edit, check, read the delta.

(defun condense-definition-name (form)
  "Return the symbol a top-level FORM defines, or NIL."
  (if (and (consp form)
           (member (car form) '(defun defun* defmacro defexpr defvau
                                defconcept defun-typed deflaw example))
           (consp (cdr form)))
      (if (consp (cadr form)) (car (cadr form)) (cadr form))
      nil))

(defun condense-check-targets (names)
  (reduce #'append
          (mapcar (lambda (n)
                    (if (eq (condense-kind n) 'concept)
                        (condense-generated n)
                        (list n)))
                  names)
          nil))

(defvau check-file! (x e)
  "Load a Lisp file and return honest checker verdicts for what it defines.

  (check-file! \"src.lisp\")

Evaluates every form in the file, then reports (name status verdict) for each
definition -- concepts expand to their generated symbols -- with the unproven
and broken remainder repeated under FRONTIER. Requires the READ-FS
capability. Diff two reports with CONDENSE-DIFF to see exactly what an edit
changed."
  (let* ((path (eval (car x) e))
         (forms (read-string (read-file path)))
         (names (filter (lambda (n) (not (null n)))
                        (mapcar #'condense-definition-name forms))))
    (mapc (lambda (form) (eval form e)) forms)
    (let* ((results (mapcar #'condense-check-type-one
                            (condense-check-targets names)))
           (frontier (filter (lambda (r) (not (condense-verified-p (cadr r))))
                             results)))
      (list (cons 'file path)
            (cons 'definitions results)
            (cons 'frontier frontier)))))
