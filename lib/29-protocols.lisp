;;; 29-protocols.lisp -- TYPED PROTOCOLS: one name, many typed instances,
;;; resolved by inference (0.3 census; Paul's "specialist functions").
;;;
;;;   (defprotocol measure "docstring")
;;;   (definstance measure ((xs (list a))) int64 (length xs))
;;;   (definstance measure ((s string)) int64 (string-length s))
;;;
;;; One name, three resolutions:
;;;   - CHECKER: a call site whose first argument's type is known selects
;;;     the matching instance and gets its precise scheme; a known type
;;;     with NO instance is a static error ("no `measure` instance for
;;;     (pair int64 string)"); an unknown argument stays gradual, but when
;;;     every instance agrees on one ground result type, the result is
;;;     still typed.
;;;   - RUNTIME: the protocol name is bound to a dispatcher that keys on
;;;     the VALUE's kind (list/string/array/hash/scalars, record brands,
;;;     variants) and applies the matching instance.
;;;   - COMPILER: each instance body is an ordinary defun under a hidden
;;;     name, so the one-door pipeline compiles eligible instances
;;;     natively; the dispatcher stays dynamic.
;;;
;;; DEFPROTOCOL captures any PRIOR binding of the name as the fallback
;;; instance, so protocolizing an existing builtin (like LENGTH) keeps
;;; everything it already handled while instances extend it to new types.

(def $protocol-instances (make-hash-table))
(def $protocol-fallbacks (make-hash-table))

(defun $protocol-type-key (v)
  "The dispatch key for VALUE: its record brand (or variant), else its kind."
  (let ((brand (record-brand v)))
    (cond
      (brand brand)
      ((null v) 'list)
      ((consp v) 'list)
      ((stringp v) 'string)
      ((arrayp v) 'array)
      ((hash-table-p v) 'hash)
      ((floatp v) 'float64)
      ((charp v) 'char)
      ((numberp v) 'int64)
      ((symbolp v) 'symbol)
      (t 'any))))

(defun $protocol-shape-key (ty)
  "The dispatch key an instance's first-parameter TYPE registers under."
  (cond
    ((eq ty 'string) 'string)
    ((eq ty 'int64) 'int64)
    ((eq ty 'float64) 'float64)
    ((eq ty 'char) 'char)
    ((eq ty 'symbol) 'symbol)
    ((eq ty 'hash) 'hash)
    ((symbolp ty) ty)                     ; record/variant brand
    ((consp ty)
     (cond ((eq (car ty) 'list) 'list)
           ((eq (car ty) 'array) 'array)
           ((eq (car ty) 'record) 'record)
           (t (car ty))))                 ; generic application head
    (t 'any)))

(defun $protocol-lookup (table key v)
  "Instance for KEY, trying the value's variant and the fallback in turn."
  (let ((hit (assoc key table)))
    (if hit
        (cdr hit)
        (let ((variant (variant-of v)))
          (let ((vhit (if variant (assoc variant table) ())))
            (if vhit (cdr vhit) ()))))))

(defvau defprotocol (x e)
  "(DEFPROTOCOL name [docstring]) -- declare NAME as a typed protocol. Any
existing binding of NAME becomes the fallback instance (so builtins keep
working for everything they already handled); DEFINSTANCE adds typed
instances. The name is rebound to the dispatcher."
  (let* ((name (car x))
         ;; errorset-wrapped: (value) when bound, () when not.
         (prior (errorset name)))
    (sethash $protocol-instances name ())
    (if prior (sethash $protocol-fallbacks name (car prior)) ())
    (set name
         (lambda (&rest args)
           (let* ((table (gethash $protocol-instances name))
                  (key ($protocol-type-key (car args)))
                  (inst ($protocol-lookup table key (car args))))
             (cond
               (inst (apply inst args))
               ((gethash $protocol-fallbacks name)
                (apply (gethash $protocol-fallbacks name) args))
               (t (error (concat "no " (princ-to-string name)
                                 " instance for " (princ-to-string key))))))))
    (condense-put name "condense.kind" 'protocol)
    name))

(defvau definstance (x e)
  "(DEFINSTANCE name ((arg type) more-args...) result-type body...) --
add a typed instance to protocol NAME. The first parameter's type selects
the instance (record/variant brands, (list a), string, (array t), hash,
scalars); the full scheme is registered with the checker, and the body
becomes an ordinary (compilable) function."
  (let* ((name (car x))
         (params (cadr x))
         (result-ty (caddr x))
         (body (cdr (cdr (cdr x))))
         (argnames (mapcar (lambda (p) (if (consp p) (car p) p)) params))
         (argtys (mapcar (lambda (p) (if (consp p) (cadr p) 'any)) params))
         (first-ty (car argtys))
         (key ($protocol-shape-key first-ty))
         (impl-name (intern (concat "$" (princ-to-string name) "@"
                                    (princ-to-string key))))
         ;; Type variables: any bare lowercase symbol in the types that is
         ;; not a known type word becomes a FORALL variable.
         (vars ($instance-collect-vars (cons result-ty argtys) ())))
    (if (has-key-p $protocol-instances name)
        ()
        (eval (list 'defprotocol name) e))
    ;; The implementation: an ordinary defun (one-door compiled when
    ;; eligible), registered in the dispatch table under the shape key.
    (eval `(defun ,impl-name ,argnames ,@body) e)
    (sethash $protocol-instances name
             (cons (cons key (eval impl-name e))
                   (gethash $protocol-instances name)))
    ;; The checker instance: hash has no checker type, so hash-keyed
    ;; instances are runtime-only.
    (if (eq key 'hash)
        ()
        (declare-instance! name
                           (if vars
                               `(forall ,vars (-> ,argtys ,result-ty))
                               `(-> ,argtys ,result-ty))))
    name))

(defun $instance-collect-vars (tys acc)
  "Bare symbols in TYS that are not type words or registered nominals:
the instance's FORALL variables."
  (cond
    ((null tys) (reverse acc))
    ((consp (car tys))
     ($instance-collect-vars
      (append (cdr (car tys)) (cdr tys)) acc))
    ((and (symbolp (car tys))
          (not (member (car tys)
                       '(int64 float64 bool char string symbol any hash
                         list array pair record ->)))
          (null (condense-get (car tys) "condense.kind"))
          (not (member (car tys) acc)))
     ($instance-collect-vars (cdr tys) (cons (car tys) acc)))
    (t ($instance-collect-vars (cdr tys) acc))))

;;; ---- the pilot: LENGTH as a protocol ---------------------------------------
;;;
;;; The kernel LENGTH already handles lists, strings, arrays, and hash
;;; tables -- it becomes the fallback; the typed instances give the checker
;;; per-shape schemes, and user types can now join ((definstance length
;;; ((sc scene)) int64 ...)).

(defprotocol length
  "Number of elements in a sized collection (protocol: extend with
DEFINSTANCE for your own types).")

(definstance length ((xs (list a))) int64
  (funcall (gethash $protocol-fallbacks 'length) xs))
(definstance length ((s string)) int64 (string-length s))
(definstance length ((a (array any))) int64 (array-length a))

;;; ---- map and for-each: the sequence protocols -------------------------------
;;;
;;; Collection FIRST (protocols dispatch on their first argument, and it
;;; matches the container convention): (map coll fn), (for-each coll fn).
;;; MAP is kind-preserving — a list maps to a list, an array to an array.
;;; FOR-EACH visits for effect and returns (); the hash instance visits
;;; (key value) pairs.

(defprotocol map
  "Kind-preserving map: (map coll fn). Extend with DEFINSTANCE.")

(definstance map ((xs (list a)) (f (-> (a) b))) (list b)
  (mapcar f xs))
(definstance map ((arr (array any)) (f any)) (array any)
  (array-map arr f))
(definstance map ((s string) (f (-> (string) string))) string
  (list->string (mapcar f (string->list s))))

(defprotocol for-each
  "Visit each element for effect: (for-each coll fn) => (). The hash
instance calls (fn key value).")

(definstance for-each ((xs (list a)) (f (-> (a) b))) (list b)
  (mapc f xs))
(definstance for-each ((arr (array any)) (f any)) any
  (progn (array-map arr f) ()))
(definstance for-each ((h hash) (f any)) any
  (progn (maphash h f) ()))
(definstance for-each ((s string) (f (-> (string) b))) any
  (progn (mapc f (string->list s)) ()))

;;; ---- ref, put!, copy: the access protocols ----------------------------------
;;;
;;; One vocabulary over the per-type access zoo (fetch/gethash/nth/elt/
;;; record-ref; store/sethash; copy-list/array-copy). All collection
;;; FIRST. REF is STRICT -- an absent index or key is an ERROR, which is
;;; what lets every instance carry an honest result type (the lenient
;;; nil-on-miss reads keep their old names: gethash, nth, elt).

(defun $ref-nth (xs i)
  (cond ((null xs) (error "REF: list index out of bounds"))
        ((= i 0) (car xs))
        (t ($ref-nth (cdr xs) (- i 1)))))

(defun ref (c k)
  "Strict read of C at index/key K (this pre-protocol binding handles
records, by brand, as the fallback instance)."
  (if (record-brand c)
      (record-ref c k)
      (error (concat "no REF instance for " (princ-to-string c)))))

(defprotocol ref
  "Strict read at an index or key: (ref coll k). Absence is an error;
use gethash/nth/elt when nil-on-miss is wanted. Records read by field
name. Extend with DEFINSTANCE.")

(definstance ref ((xs (list a)) (i int64)) a
  (if (< i 0) (error "REF: list index out of bounds") ($ref-nth xs i)))
(definstance ref ((arr (array a)) (i int64)) a
  (fetch arr i))
(definstance ref ((s string) (i int64)) string
  (if (and (>= i 0) (< i (string-length s)))
      (substring s i (+ i 1))
      (error "REF: string index out of bounds")))
(definstance ref ((h hash) (k any)) any
  (if (has-key-p h k)
      (gethash h k)
      (error (concat "REF: missing key " (princ-to-string k)))))

(defprotocol put!
  "Write V at index/key K: (put! coll k v) => V. Arrays and hash tables
(the mutable containers); records are values -- use record-with.")

(definstance put! ((arr (array a)) (i int64) (v a)) a
  (store arr i v))
(definstance put! ((h hash) (k any) (v any)) any
  (progn (sethash h k v) v))

;;; COPY: the pre-protocol binding (lib/01's Lisp 1.5 structure copy,
;;; atoms pass through) becomes the fallback; instances give each kind a
;;; typed scheme. Conses are immutable, so the list copy is shallow with
;;; no observable difference; strings are immutable, so copy is identity.

(defprotocol copy
  "A copy of a collection: (copy x). Fresh array/hash/list; identity on
immutable strings and atoms. Extend with DEFINSTANCE.")

(definstance copy ((xs (list a))) (list a) (copy-list xs))
(definstance copy ((arr (array a))) (array a) (array-copy arr))
(definstance copy ((s string)) string s)
(definstance copy ((h hash)) any (copy-hash h))
