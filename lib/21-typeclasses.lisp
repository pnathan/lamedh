;;; Typeclass dictionaries.
;;;
;;; This is an explicit dictionary layer, not an implicit-search engine and not
;;; a host sandbox capability.  It intentionally has no dependency on
;;; condensation: any symbol can be used as the represented type.

(def *typeclass-kind-key* "typeclass.kind")
(def *typeclass-source-key* "typeclass.source")
(def *typeclass-params-key* "typeclass.params")
(def *typeclass-ops-key* "typeclass.ops")
(def *typeclass-instances-key* "typeclass.instances")

(defun typeclass-put (sym key value)
  (putp sym key value))

(defun typeclass-get (sym key)
  (getp sym key))

(defun typeclass-kind (sym)
  (typeclass-get sym *typeclass-kind-key*))

(defun typeclass-ops (class)
  (typeclass-get class *typeclass-ops-key*))

(defun typeclass-instances (class)
  (typeclass-get class *typeclass-instances-key*))

(defun typeclass-normalize-op (op)
  "Normalize OP so keywords such as :eqv select the EQV operation."
  (let ((s (princ-to-string op)))
    (if (starts-with-p s ":")
        (intern (substring s 1))
        op)))

(defun typeclass-op-name (op-spec)
  (typeclass-normalize-op (car op-spec)))

(defun typeclass-op-names (op-specs)
  (if (null op-specs)
      nil
      (cons (typeclass-op-name (car op-specs))
            (typeclass-op-names (cdr op-specs)))))

(defun typeclass-member-p (x xs)
  (not (null (member x xs))))

(defun typeclass-missing-ops (required provided)
  (cond
    ((null required) nil)
    ((typeclass-member-p (car required) provided)
     (typeclass-missing-ops (cdr required) provided))
    (t (cons (car required)
             (typeclass-missing-ops (cdr required) provided)))))

(defun typeclass-unknown-ops (provided required)
  (cond
    ((null provided) nil)
    ((typeclass-member-p (car provided) required)
     (typeclass-unknown-ops (cdr provided) required))
    (t (cons (car provided)
             (typeclass-unknown-ops (cdr provided) required)))))

(defun typeclass-instance-op-pair (entry)
  (cons (typeclass-normalize-op (car entry)) (cadr entry)))

(defun typeclass-instance-op-pairs (entries)
  (if (null entries)
      nil
      (cons (typeclass-instance-op-pair (car entries))
            (typeclass-instance-op-pairs (cdr entries)))))

(defun typeclass-instance-op-names (entries)
  (if (null entries)
      nil
      (cons (typeclass-normalize-op (car (car entries)))
            (typeclass-instance-op-names (cdr entries)))))

(defun typeclass-require-class (class action)
  (if (eq (typeclass-kind class) 'typeclass)
      class
      (error (concat action " requires a declared typeclass"))))

(defun typeclass-validate-instance (class entries)
  (let* ((required (typeclass-op-names (typeclass-ops class)))
         (provided (typeclass-instance-op-names entries))
         (missing (typeclass-missing-ops required provided))
         (unknown (typeclass-unknown-ops provided required)))
    (cond
      ((not (null missing))
       (error (concat "definstance missing operation(s): "
                      (princ-to-string missing))))
      ((not (null unknown))
       (error (concat "definstance unknown operation(s): "
                      (princ-to-string unknown))))
      (t t))))

(defun typeclass-remove-instance (type instances)
  (cond
    ((null instances) nil)
    ((eq (car (car instances)) type)
     (typeclass-remove-instance type (cdr instances)))
    (t (cons (car instances)
             (typeclass-remove-instance type (cdr instances))))))

(defun typeclass-set-instance (class type ops)
  (let ((instances (typeclass-instances class)))
    (typeclass-put class *typeclass-instances-key*
                   (cons (cons type ops)
                         (typeclass-remove-instance type instances)))))

(defvau deftypeclass (x e)
  "Declare a typeclass and its required operations."
  (let* ((class (car x))
         (params (cadr x))
         (sections (cddr x))
         (ops-section (assoc :ops sections)))
    (if (null ops-section)
        (error "deftypeclass requires an :ops section")
        (let ((ops (cadr ops-section)))
          (typeclass-put class *typeclass-kind-key* 'typeclass)
          (typeclass-put class *typeclass-source-key* (cons 'deftypeclass x))
          (typeclass-put class *typeclass-params-key* params)
          (typeclass-put class *typeclass-ops-key* ops)
          (typeclass-put class *typeclass-instances-key* nil)
          class))))

(defvau defcap (x e)
  "Compatibility alias for DEFTYPECLASS."
  (eval (cons 'deftypeclass x) e))

(defvau definstance (x e)
  "Attach an explicit dictionary instance to a declared typeclass."
  (let* ((class (car x))
         (type (cadr x))
         (entries (cddr x)))
    (typeclass-require-class class "definstance")
    (typeclass-validate-instance class entries)
    (typeclass-set-instance class type
                            (typeclass-instance-op-pairs entries))
    type))

(defun resolve-instance (class type)
  "Return the explicit dictionary for CLASS at TYPE, or signal a clear error."
  (typeclass-require-class class "resolve-instance")
  (let ((cell (assoc type (typeclass-instances class))))
    (if cell
        cell
        (error (concat "missing typeclass instance: "
                       (princ-to-string class)
                       " for "
                       (princ-to-string type))))))

(defun resolve-cap (class type)
  "Compatibility alias for RESOLVE-INSTANCE."
  (resolve-instance class type))

(defun typeclass-op (class type op)
  "Resolve operation OP for CLASS at TYPE and return the implementing function."
  (let* ((instance (resolve-instance class type))
         (ops (cdr instance))
         (name (typeclass-normalize-op op))
         (cell (assoc name ops)))
    (if cell
        (cdr cell)
        (error (concat "missing typeclass operation: "
                       (princ-to-string name))))))

(defun cap-op (class type op)
  "Compatibility alias for TYPECLASS-OP."
  (typeclass-op class type op))

(defun typeclass-call (class type op &rest args)
  "Resolve OP and apply it to ARGS."
  (apply (typeclass-op class type op) args))

(defun typeclass-trace (class)
  "Return inspectable metadata for CLASS."
  (list
    (cons 'kind (typeclass-kind class))
    (cons 'source (typeclass-get class *typeclass-source-key*))
    (cons 'params (typeclass-get class *typeclass-params-key*))
    (cons 'ops (typeclass-ops class))
    (cons 'instances (typeclass-instances class))))

(defun instance-trace (class type)
  "Return inspectable metadata for CLASS at TYPE."
  (let ((instance (resolve-instance class type)))
    (list
      (cons 'class class)
      (cons 'type type)
      (cons 'ops (cdr instance)))))

;;; ---- standard typeclasses --------------------------------------------------
;;;
;;; The classes DERIVE knows how to install instances for
;;; (lib/20-condensation.lisp): equality -> EQV, printer -> SHOW, lens -> LENS.
;;; NOTE: the :ops signatures are declarative metadata. They are not enforced
;;; against instances by any checker today; only the arity is consumed.

(deftypeclass eqv (a)
  (:ops ((eqv (-> (a a) bool)))))

(deftypeclass show (a)
  (:ops ((show (-> (a) list)))))

(deftypeclass lens (a)
  (:ops ((view (-> (a) list))
         (build (-> (list) a)))))
