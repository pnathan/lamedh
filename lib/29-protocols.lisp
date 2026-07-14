;;; 29-protocols.lisp -- TYPED PROTOCOLS: one name, many typed instances,
;;; resolved by inference (0.3 census; Paul's "specialist functions").
;;;
;;;   (defprotocol measure "docstring")
;;;   (definstance measure ((xs (list a))) int64 (length xs))
;;;   (definstance measure ((s string)) int64 (string-length* s))
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
;; Which argument position each protocol dispatches on (default 0).
;; Fn-first protocols (the CL convention for HOFs: map, for-each)
;; dispatch on 1.
(def $protocol-dispatch (make-hash-table))

(defun $protocol-dispatch-idx (name)
  (let ((hit (gethash $protocol-dispatch name)))
    (if hit hit 0)))

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
  "(DEFPROTOCOL name [docstring] [(:dispatch n)]) -- declare NAME as a
typed protocol. Any existing binding of NAME becomes the fallback
instance (so builtins keep working for everything they already handled);
DEFINSTANCE adds typed instances. The name is rebound to the dispatcher.
(:dispatch n) selects the argument position that drives dispatch --
fn-first protocols like MAP dispatch on 1; the default is 0."
  (let* ((name (car x))
         (dispatch-section (assoc ':dispatch (filter #'consp (cdr x))))
         (idx (if dispatch-section (cadr dispatch-section) 0))
         ;; errorset-wrapped: (value) when bound, () when not.
         (prior (errorset name)))
    (sethash $protocol-instances name ())
    (sethash $protocol-dispatch name idx)
    (declare-protocol-dispatch! name idx)
    (if prior (sethash $protocol-fallbacks name (car prior)) ())
    (set name
         (lambda (&rest args)
           (let* ((table (gethash $protocol-instances name))
                  (dval (nth ($protocol-dispatch-idx name) args))
                  (key ($protocol-type-key dval))
                  (inst ($protocol-lookup table key dval)))
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
add a typed instance to protocol NAME. The dispatch parameter's type
(position 0 unless the protocol declared (:dispatch n)) selects the
instance (record/variant brands, (list a), string, (array t), hash,
scalars); the full scheme is registered with the checker, and the body
becomes an ordinary (compilable) function."
  (let* ((name (car x))
         (params (cadr x))
         (result-ty (caddr x))
         (body (cdr (cdr (cdr x))))
         (argnames (mapcar (lambda (p) (if (consp p) (car p) p)) params))
         (argtys (mapcar (lambda (p) (if (consp p) (cadr p) 'any)) params))
         (dispatch-ty (nth ($protocol-dispatch-idx name) argtys))
         (key ($protocol-shape-key (if dispatch-ty dispatch-ty (car argtys))))
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
(definstance length ((s string)) int64 (string-length* s))
(definstance length ((a (array any))) int64 (array-length* a))

;;; ---- map and for-each: the sequence protocols -------------------------------
;;;
;;; Function FIRST, the CL convention for higher-order functions
;;; (mapcar/mapc heritage): (map fn coll), (for-each fn coll). These
;;; protocols dispatch on argument position 1 (the collection). MAP is
;;; kind-preserving — a list maps to a list, an array to an array, a
;;; string to a string. FOR-EACH visits for effect and returns (); the
;;; hash instance visits (key value) pairs.

(defprotocol map
  "Kind-preserving map: (map fn coll). Extend with DEFINSTANCE."
  (:dispatch 1))

(definstance map ((f (-> (a) b)) (xs (list a))) (list b)
  (mapcar f xs))
(definstance map ((f any) (arr (array any))) (array any)
  (array-map* arr f))
(definstance map ((f (-> (string) string)) (s string)) string
  (list->string (mapcar f (string->list s))))

(defprotocol for-each
  "Visit each element for effect: (for-each fn coll) => (). The hash
instance calls (fn key value)."
  (:dispatch 1))

(definstance for-each ((f (-> (a) b)) (xs (list a))) (list b)
  (mapc f xs))
(definstance for-each ((f any) (arr (array any))) any
  (progn (array-map* arr f) ()))
(definstance for-each ((f any) (h hash)) any
  (progn (maphash h f) ()))
(definstance for-each ((f (-> (string) b)) (s string)) any
  (progn (mapc f (string->list s)) ()))

;;; FILTER goes generic the same way -- it was ALREADY fn-first, so the
;;; existing list-only binding becomes the fallback and nothing breaks;
;;; arrays and strings gain kind-preserving instances.

(defprotocol filter
  "Kind-preserving filter: (filter pred coll). Extend with DEFINSTANCE."
  (:dispatch 1))

(definstance filter ((f (-> (a) bool)) (xs (list a))) (list a)
  (funcall (gethash $protocol-fallbacks 'filter) f xs))
(definstance filter ((f any) (arr (array any))) (array any)
  (list->array (funcall (gethash $protocol-fallbacks 'filter)
                        f (array->list arr))))
(definstance filter ((f (-> (string) bool)) (s string)) string
  (string-join (funcall (gethash $protocol-fallbacks 'filter)
                        f (string->list s))
               ""))

;;; ---- ref, put!, copy: the access protocols ----------------------------------
;;;
;;; One vocabulary over the per-type access zoo (fetch/gethash/nth/elt/
;;; record-ref; store/sethash; copy-list*/array-copy*). All collection
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
  (if (and (>= i 0) (< i (string-length* s)))
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

(definstance copy ((xs (list a))) (list a) (copy-list* xs))
(definstance copy ((arr (array a))) (array a) (array-copy* arr))
(definstance copy ((s string)) string s)
(definstance copy ((h hash)) any (copy-hash* h))

;;; ---- conformance: implements! over protocols ---------------------------------
;;;
;;; The interface half of the old two-system split (definterface, removed
;;; 0.3), re-seated on THE dispatch system: a "contract" is just a set of
;;; protocol names, and conformance is instances existing for a brand --
;;; graded with the checker's verdict on each instance's implementation.

(defun $protocol-instance-for (protocol key)
  "The registered instance entry for PROTOCOL under KEY, or ()."
  (assoc key (gethash $protocol-instances protocol)))

(defun $implements-status (type protocol)
  "(protocol . status): INSTANCE when TYPE has one and its implementation
carries no type error; MISMATCH when the implementation's verdict is a
type error; MISSING otherwise."
  (if ($protocol-instance-for protocol type)
      (let* ((impl (intern (concat "$" (princ-to-string protocol) "@"
                                   (princ-to-string type))))
             (verdict (see-type impl)))
        (if (eq (car verdict) 'type-error)
            (cons protocol 'mismatch)
            (cons protocol 'instance)))
      (cons protocol 'missing)))

(defun implements-p (type &rest protocols)
  "T when TYPE (a brand or kind key) has a clean instance for every
named protocol."
  (every (lambda (p) (eq (cdr ($implements-status type p)) 'instance))
         protocols))

(defun implements! (type &rest protocols)
  "Assert TYPE implements every named protocol: returns the per-protocol
report ((protocol . status)...) or errors naming the failures."
  (let* ((report (mapcar (lambda (p) ($implements-status type p)) protocols))
         (bad (filter (lambda (r) (not (eq (cdr r) 'instance))) report)))
    (if (null bad)
        report
        (error (concat "implements!: " (princ-to-string type)
                       " fails " (princ-to-string bad))))))

;;; REQUIRE-ABLE (issue #256): `(require 'protocols)` on a with_prelude()
;;; environment loads exactly this file. with_stdlib() still loads it
;;; unconditionally, unchanged.
;;; Registered as a module for introspection (issue #56). The dispatch
;;; vocabulary stays FLAT (language-defining: `defprotocol`/`definstance`
;;; and conformance are THE dispatch system); this DEFMODULE only records
;;; metadata -- no WITH-MODULE body rewrite.
(require 'modules)
(defmodule protocols
  (:export defprotocol definstance ref implements-p implements!))
(provide 'protocols)
