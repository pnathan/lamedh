;;; Intent layer: subject / means / outcome triples over typeclass dispatch.
;;;
;;; An intent names WHAT is acted on (:subject — a concept or ground type, or
;;; a type variable for polymorphic intents), HOW (:means — a typeclass
;;; operation, so intents sharing a means share dispatch through the instance
;;; table), and WHAT MUST HOLD afterwards (:outcome — a contract over *it*,
;;; the input subject, and *result*).
;;;
;;; Dispatch is two-tier, mirroring the execution-tier architecture:
;;;   - dynamic: INTENT-APPLY resolves the means through the typeclass
;;;     dictionary at the runtime subject type;
;;;   - static: INTENT-REALIZE lowers a ground-subject intent to a plain,
;;;     dictionary-free function named after the intent, then runs CHECK-TYPE
;;;     over it and records the result in the condensation trace. A missing
;;;     ground instance is an error at realize time, not a latent runtime
;;;     failure.

;;; ---- intent metadata -------------------------------------------------------

(def *intent-kind-key* "intent.kind")
(def *intent-subject-key* "intent.subject")
(def *intent-means-key* "intent.means")
(def *intent-outcome-key* "intent.outcome")
(def *intent-outcome-fn-key* "intent.outcome-fn")
(def *intent-source-key* "intent.source")
(def *intent-registry-key* "intent.registry")

(def *intent-ground-types* '(int64 float64 char string symbol list))
(def *intent-param-pool* '(a b c d e f g h))

(defun intent-p (name)
  "Return T when NAME is a defined intent."
  (eq (getp name *intent-kind-key*) 'intent))

(defun intent-require (name)
  (if (intent-p name)
      name
      (error (concat "not a defined intent: " (princ-to-string name)))))

(defun intent-subject (name)
  "Return the :subject of intent NAME."
  (getp name *intent-subject-key*))

(defun intent-means (name)
  "Return the (class op) :means of intent NAME."
  (getp name *intent-means-key*))

(defun intent-outcome (name)
  "Return the :outcome form of intent NAME, or NIL."
  (getp name *intent-outcome-key*))

(defun intent-means-class (name)
  (car (intent-means name)))

(defun intent-means-op (name)
  (cadr (intent-means name)))

(defun intent-registry ()
  "Return the list of defined intents."
  (getp 'intents *intent-registry-key*))

(defun intent-register! (name)
  (putp 'intents *intent-registry-key*
        (condense-append-new (intent-registry) (list name))))

;;; ---- subjects --------------------------------------------------------------

(defun intent-ground-subject-p (subject)
  "Return T when SUBJECT names a concept or ground builtin type."
  (if (or (member subject *intent-ground-types*)
          (eq (condense-kind subject) 'concept))
      t
      nil))

(defun intent-subject-type (value)
  "Return the dispatch type of VALUE: its concept tag or a ground builtin type."
  (cond
    ((and (consp value)
          (symbolp (car value))
          (eq (condense-kind (car value)) 'concept))
     (car value))
    ((consp value) 'list)
    ((null value) 'list)
    ((charp value) 'char)
    ((floatp value) 'float64)
    ((numberp value) 'int64)
    ((stringp value) 'string)
    ((symbolp value) 'symbol)
    (t (error "intent-subject-type: unsupported subject value"))))

(defun intent-subject-check (name value)
  (let ((subject (intent-subject name)))
    (cond
      ((eq (condense-kind subject) 'concept)
       (if (funcall (condense-predicate-symbol subject) value)
           t
           (error (concat "intent subject mismatch: expected "
                          (princ-to-string subject)))))
      ((member subject *intent-ground-types*)
       (if (eq (intent-subject-type value) subject)
           t
           (error (concat "intent subject mismatch: expected "
                          (princ-to-string subject)))))
      (t t))))

;;; ---- definition ------------------------------------------------------------

(defun intent-outcome-symbol (name)
  (intern (concat (princ-to-string name) "-OUTCOME-P")))

(defun intent-outcome-expansion (name outcome)
  `(defun ,(intent-outcome-symbol name) (*it* *result*)
     ,outcome))

(defvau defintent (x e)
  "Define a named intent as a :subject / :means / :outcome triple."
  (let* ((name (car x))
         (sections (cdr x))
         (subject (condense-section-value (assoc :subject sections)))
         (means (condense-section-value (assoc :means sections)))
         (outcome (condense-section-value (assoc :outcome sections))))
    (cond
      ((null subject) (error "defintent requires a :subject section"))
      ((null means) (error "defintent requires a :means section"))
      (t (progn
           (if outcome
               (progn
                 (eval (intent-outcome-expansion name outcome) e)
                 (putp name *intent-outcome-fn-key*
                       (intent-outcome-symbol name)))
               (putp name *intent-outcome-fn-key* nil))
           (putp name *intent-kind-key* 'intent)
           (putp name *intent-subject-key* subject)
           (putp name *intent-means-key* means)
           (putp name *intent-outcome-key* outcome)
           (putp name *intent-source-key* (cons 'defintent x))
           (intent-register! name)
           name)))))

;;; ---- dynamic dispatch ------------------------------------------------------

(defun intent-dispatch (name value)
  "Resolve the implementing function for intent NAME at VALUE's subject type."
  (intent-require name)
  (let* ((subject (intent-subject name))
         (type (if (intent-ground-subject-p subject)
                   subject
                   (intent-subject-type value))))
    (typeclass-op (intent-means-class name) type (intent-means-op name))))

(defun intent-check-outcome (name input result)
  (let ((outcome-fn (getp name *intent-outcome-fn-key*)))
    (if (null outcome-fn)
        result
        (if (funcall outcome-fn input result)
            result
            (error (concat "intent outcome failed: "
                           (princ-to-string name)))))))

(defun intent-apply (name subject-value &rest args)
  "Apply intent NAME: dispatch its means by subject type, then check its outcome."
  (intent-require name)
  (intent-subject-check name subject-value)
  (let* ((fn (intent-dispatch name subject-value))
         (result (apply fn (cons subject-value args))))
    (intent-check-outcome name subject-value result)))

;;; ---- static lowering -------------------------------------------------------

(defun intent-op-signature (class op)
  (let ((spec (assoc (typeclass-normalize-op op) (typeclass-ops class))))
    (if spec (cadr spec) nil)))

(defun intent-op-arity (class op)
  "Return the argument count declared for OP in CLASS, defaulting to 1."
  (let ((sig (intent-op-signature class op)))
    (if (and (consp sig) (eq (car sig) '->))
        (length (cadr sig))
        1)))

(defun intent-params (n pool)
  (if (or (< n 1) (null pool))
      nil
      (cons (car pool) (intent-params (- n 1) (cdr pool)))))

(defun intent-realize-expansion (name method params outcome-fn)
  (if outcome-fn
      `(defun ,name ,params
         (let ((result (,method ,@params)))
           (if (,outcome-fn ,(car params) result)
               result
               (error ,(concat "intent outcome failed: "
                               (princ-to-string name))))))
      `(defun ,name ,params
         (,method ,@params))))

(defvau intent-realize (x e)
  "Lower a ground intent to a direct, dictionary-free function named after it."
  (let ((name (car x)))
    (intent-require name)
    (let ((subject (intent-subject name)))
      (if (not (intent-ground-subject-p subject))
          (error "intent-realize requires a ground subject")
          (let* ((class (intent-means-class name))
                 (op (intent-means-op name))
                 (method (typeclass-op class subject op))
                 (outcome-fn (getp name *intent-outcome-fn-key*))
                 (params (intent-params (intent-op-arity class op)
                                        *intent-param-pool*))
                 (expansion (intent-realize-expansion name method params
                                                      outcome-fn))
                 (generated (if outcome-fn
                                (list name outcome-fn)
                                (list name))))
            (eval expansion e)
            (condense-record! name 'intent
                              (getp name *intent-source-key*)
                              expansion generated)
            (condense-check-type name)
            (condense-fingerprint! name)
            name)))))

;;; ---- sharing and traces ----------------------------------------------------

(defun intent-filter (names key value)
  (cond
    ((null names) nil)
    ((equal (getp (car names) key) value)
     (cons (car names) (intent-filter (cdr names) key value)))
    (t (intent-filter (cdr names) key value))))

(defun intents-for-subject (subject)
  "Return intents whose :subject is SUBJECT."
  (intent-filter (intent-registry) *intent-subject-key* subject))

(defun intents-for-means (means)
  "Return intents whose :means equals MEANS, a (class op) pair."
  (intent-filter (intent-registry) *intent-means-key* means))

(defun intents-for-outcome (outcome)
  "Return intents whose :outcome form equals OUTCOME."
  (intent-filter (intent-registry) *intent-outcome-key* outcome))

(defun intent-trace (name)
  "Return inspectable metadata for intent NAME."
  (intent-require name)
  (list
    (cons 'kind 'intent)
    (cons 'subject (intent-subject name))
    (cons 'means (intent-means name))
    (cons 'outcome (intent-outcome name))
    (cons 'ground (intent-ground-subject-p (intent-subject name)))
    (cons 'realized (if (eq (condense-kind name) 'intent) t nil))
    (cons 'source (getp name *intent-source-key*))))
