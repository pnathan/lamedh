;; Coverage for lib/45-hashtable.lisp -- the pure-Lamedh hash table (issue #458).
;; Distinct symbol/string names are used throughout to avoid cross-test
;; interference, matching the convention in 70-hash-and-plist.lisp.

(deftest lht-make
  (let ((h (make-lht)))
    (progn
      (assert-true (lht-p h))
      (assert-equal (lht-count h) 0))))

(deftest lht-p-rejects-non-table
  (progn
    (assert-false (lht-p (array 8)))
    (assert-false (lht-p 42))
    (assert-false (lht-p nil))
    (assert-false (lht-p (make-hash-table)))))

(deftest lht-set-get
  (let ((h (make-lht)))
    (progn
      (lht-put! h 'lht-key-1 42)
      (assert-equal (lht-get h 'lht-key-1) 42))))

(deftest lht-get-missing-is-nil
  (let ((h (make-lht)))
    (assert-nil (lht-get h 'lht-nonexistent))))

(deftest lht-stored-nil-indistinguishable-from-absent
  ;; Matches the native GETHASH contract per KERNEL.md Part IV: no second
  ;; return value, so a key whose value IS nil looks like an absent key to
  ;; LHT-GET alone -- but LHT-HAS-KEY-P still tells them apart.
  (let ((h (make-lht)))
    (progn
      (lht-put! h 'lht-nil-val nil)
      (assert-nil (lht-get h 'lht-nil-val))
      (assert-true (lht-has-key-p h 'lht-nil-val))
      (assert-false (lht-has-key-p h 'lht-truly-absent)))))

(deftest lht-overwrite
  (let ((h (make-lht)))
    (progn
      (lht-put! h 'lht-key-2 1)
      (lht-put! h 'lht-key-2 2)
      (assert-equal (lht-get h 'lht-key-2) 2)
      (assert-equal (lht-count h) 1))))

(deftest lht-nil-as-key
  ;; KERNEL.md Part IV: "Nil as a key" is explicitly legal.
  (let ((h (make-lht)))
    (progn
      (lht-put! h nil 'lht-val-for-nil-key)
      (assert-true (lht-has-key-p h nil))
      (assert-equal (lht-get h nil) 'lht-val-for-nil-key))))

(deftest lht-delete
  (let ((h (make-lht)))
    (progn
      (lht-put! h 'lht-del-key 7)
      (lht-remove! h 'lht-del-key)
      (assert-nil (lht-get h 'lht-del-key))
      (assert-false (lht-has-key-p h 'lht-del-key))
      (assert-equal (lht-count h) 0))))

(deftest lht-delete-missing-returns-t
  ;; Matches native REMHASH: "removes and returns T whether or not the key
  ;; was present."
  (let ((h (make-lht)))
    (assert-true (lht-remove! h 'lht-never-inserted))))

(deftest lht-delete-then-reinsert
  ;; Exercises tombstone reclamation on the same bucket.
  (let ((h (make-lht)))
    (progn
      (lht-put! h 'lht-churn 1)
      (lht-remove! h 'lht-churn)
      (lht-put! h 'lht-churn 2)
      (assert-equal (lht-get h 'lht-churn) 2)
      (assert-equal (lht-count h) 1))))

(deftest lht-repeated-single-key-churn-does-not-overflow-payload-arrays
  ;; LHT-PUT! always claims a fresh payload slot (NEXT-SLOT) on a miss, even
  ;; when it reuses a tombstoned bucket -- so COUNT+TOMBSTONES stays flat
  ;; under repeated put/remove of one key while NEXT-SLOT keeps climbing.
  ;; Gating growth on COUNT+TOMBSTONES instead of NEXT-SLOT would let
  ;; NEXT-SLOT walk past CAPACITY with no growth ever triggering, faulting
  ;; STORE out of bounds well before this many ops.
  (let ((h (make-lht)))
    (progn
      (mapc (lambda (i)
              (progn
                (lht-put! h 'lht-single-churn i)
                (lht-remove! h 'lht-single-churn)))
            (iota 200))
      (lht-put! h 'lht-single-churn 'final)
      (assert-equal (lht-get h 'lht-single-churn) 'final)
      (assert-equal (lht-count h) 1))))

(deftest lht-string-keys
  (let ((h (make-lht)))
    (progn
      (lht-put! h "hello world" 1)
      (assert-equal (lht-get h "hello world") 1)
      ;; A freshly-built EQUAL-but-not-EQ string must still find the entry.
      (assert-equal (lht-get h (concat "hello" " " "world")) 1))))

(deftest lht-cons-keys-structural
  ;; Cons keys must be found by structural EQUAL, not identity.
  (let ((h (make-lht)))
    (progn
      (lht-put! h (list 1 2 3) 'lht-listkey)
      (assert-equal (lht-get h (list 1 2 3)) 'lht-listkey)
      (assert-equal (lht-get h (cons 1 (cons 2 (cons 3 nil)))) 'lht-listkey)
      (assert-nil (lht-get h (list 1 2 4))))))

(deftest lht-nested-cons-keys
  (let ((h (make-lht)))
    (progn
      (lht-put! h '((a . 1) (b . 2)) 'lht-nested)
      (assert-equal (lht-get h '((a . 1) (b . 2))) 'lht-nested))))

(deftest lht-int-and-float-keys-distinct
  ;; KERNEL.md Part IV: "1 and 1.0 are distinct keys."
  (let ((h (make-lht)))
    (progn
      (lht-put! h 1 'lht-int-one)
      (lht-put! h 1.0 'lht-float-one)
      (assert-equal (lht-get h 1) 'lht-int-one)
      (assert-equal (lht-get h 1.0) 'lht-float-one)
      (assert-equal (lht-count h) 2))))

(deftest lht-zero-and-negative-zero-same-key
  ;; KERNEL.md Part IV: "0.0 and -0.0 are the same key."
  (let ((h (make-lht)))
    (progn
      (lht-put! h 0.0 'lht-zero)
      (assert-equal (lht-get h -0.0) 'lht-zero)
      (assert-true (lht-has-key-p h -0.0))
      (lht-put! h -0.0 'lht-negzero-overwrite)
      (assert-equal (lht-get h 0.0) 'lht-negzero-overwrite)
      (assert-equal (lht-count h) 1))))

(deftest lht-nan-key-finds-nan
  ;; KERNEL.md Part IV: "NaN finds NaN."
  (let ((h (make-lht)))
    (progn
      (lht-put! h (/ 0.0 0.0) 'lht-nan-val)
      (assert-equal (lht-get h (/ -0.0 0.0)) 'lht-nan-val)
      (assert-true (lht-has-key-p h (/ 0.0 0.0))))))

(deftest lht-char-vs-number-distinct
  ;; KERNEL.md Part IV: Char and Number are never EQ/EQUAL even when the
  ;; char's code matches the number.
  (let ((h (make-lht)))
    (progn
      (lht-put! h (make-char 97) 'lht-char-a)
      (lht-put! h 97 'lht-num-97)
      (assert-equal (lht-get h (make-char 97)) 'lht-char-a)
      (assert-equal (lht-get h 97) 'lht-num-97)
      (assert-equal (lht-count h) 2))))

(deftest lht-array-keys-are-identity
  ;; KERNEL.md Part IV: arrays compare (and hence hash-collide-resolve) by
  ;; identity; two content-identical arrays are distinct keys.
  (let ((h (make-lht))
        (a1 (array 3))
        (a2 (array 3)))
    (progn
      (lht-put! h a1 'lht-arr1)
      (assert-equal (lht-get h a1) 'lht-arr1)
      (assert-nil (lht-get h a2))
      (assert-false (lht-has-key-p h a2)))))

(deftest lht-hash-agrees-with-equal-on-collisions
  ;; Two structurally-EQUAL-but-freshly-built compound keys must collide
  ;; into the SAME entry, exercising both the hash function and the probe's
  ;; EQUAL-based comparison together.
  (let ((h (make-lht)))
    (progn
      (lht-put! h (list 'a (+ 1 1) "x") 1)
      (lht-put! h (list 'a 2 (concat "x" "")) 2)
      (assert-equal (lht-get h (list 'a 2 "x")) 2)
      (assert-equal (lht-count h) 1))))

(deftest lht-many-keys-collide-by-capacity
  ;; Force real bucket collisions by inserting more distinct keys than the
  ;; initial capacity (16) before any resize would separate them, then read
  ;; every one back to prove probing/resize doesn't lose or cross-wire
  ;; entries.
  (let ((h (make-lht)))
    (progn
      (mapc (lambda (i) (lht-put! h (intern (concat "LHT-COLLIDE-" (number->string i))) i))
            (iota 40))
      (assert-nil (lht-collide-mismatches h 0 40)))))

(defun lht-collide-mismatches (h i n)
  (if (>= i n)
      nil
      (if (equal (lht-get h (intern (concat "LHT-COLLIDE-" (number->string i)))) i)
          (lht-collide-mismatches h (+ i 1) n)
          (cons i (lht-collide-mismatches h (+ i 1) n)))))

(deftest lht-resize-preserves-all-entries
  ;; Insert enough keys to force several rehashes (initial capacity 16,
  ;; 0.7 load factor -> several doublings by 500 entries) and verify every
  ;; single one is still reachable afterward.
  (let ((h (make-lht)))
    (progn
      (mapc (lambda (i) (lht-put! h (intern (concat "LHT-BIG-" (number->string i))) i))
            (iota 500))
      (assert-equal (lht-count h) 500)
      (assert-nil (lht-resize-mismatches h 0 500)))))

(defun lht-resize-mismatches (h i n)
  (if (>= i n)
      nil
      (if (equal (lht-get h (intern (concat "LHT-BIG-" (number->string i)))) i)
          (lht-resize-mismatches h (+ i 1) n)
          (cons i (lht-resize-mismatches h (+ i 1) n)))))

(deftest lht-delete-half-then-verify-rest
  (let ((h (make-lht)))
    (progn
      (mapc (lambda (i) (lht-put! h (intern (concat "LHT-DEL-" (number->string i))) i))
            (iota 200))
      (mapc (lambda (i) (lht-remove! h (intern (concat "LHT-DEL-" (number->string i)))))
            (filter evenp (iota 200)))
      (assert-equal (lht-count h) 100)
      (assert-nil (lht-get h (intern "LHT-DEL-0")))
      (assert-equal (lht-get h (intern "LHT-DEL-1")) 1)
      (assert-nil (lht-get h (intern "LHT-DEL-198")))
      (assert-equal (lht-get h (intern "LHT-DEL-199")) 199))))

(deftest lht-churn-does-not-corrupt
  ;; Interleave insert/delete/reinsert across a resize boundary.
  (let ((h (make-lht)))
    (progn
      (mapc (lambda (i) (lht-put! h (intern (concat "LHT-CHURN-" (number->string i))) i))
            (iota 60))
      (mapc (lambda (i) (lht-remove! h (intern (concat "LHT-CHURN-" (number->string i)))))
            (filter (lambda (i) (= (mod i 3) 0)) (iota 60)))
      (mapc (lambda (i) (lht-put! h (intern (concat "LHT-CHURN-" (number->string i))) (* i 100)))
            (filter (lambda (i) (= (mod i 3) 0)) (iota 60)))
      (assert-equal (lht-count h) 60)
      (assert-equal (lht-get h (intern "LHT-CHURN-0")) 0)
      (assert-equal (lht-get h (intern "LHT-CHURN-3")) 300)
      (assert-equal (lht-get h (intern "LHT-CHURN-59")) 59))))

(deftest lht-clear
  (let ((h (make-lht)))
    (progn
      (lht-put! h 'lht-clear-key-1 1)
      (lht-put! h 'lht-clear-key-2 2)
      (lht-clear! h)
      (assert-equal (lht-count h) 0)
      (assert-nil (lht-get h 'lht-clear-key-1))
      (lht-put! h 'lht-clear-key-3 3)
      (assert-equal (lht-get h 'lht-clear-key-3) 3))))

(deftest lht-keys-and-alist-roundtrip
  (let ((h (make-lht)))
    (progn
      (lht-put! h 'lht-rt-a 1)
      (lht-put! h 'lht-rt-b 2)
      (lht-put! h 'lht-rt-c 3)
      (assert-equal (sort (lht-keys h) (lambda (x y) (string< (princ-to-string x) (princ-to-string y))))
                     '(LHT-RT-A LHT-RT-B LHT-RT-C))
      (assert-equal (sort (mapcar (lambda (c) (list (car c) (cdr c))) (lht->alist h))
                            (lambda (x y) (string< (princ-to-string (car x)) (princ-to-string (car y)))))
                     '((LHT-RT-A 1) (LHT-RT-B 2) (LHT-RT-C 3))))))

(deftest lht-each-visits-every-entry
  (let ((h (make-lht))
        (seen nil))
    (progn
      (lht-put! h 'lht-each-a 10)
      (lht-put! h 'lht-each-b 20)
      (lht-each h (lambda (k v) (setq seen (cons (list k v) seen))))
      (assert-equal (sort seen (lambda (x y) (string< (princ-to-string (car x)) (princ-to-string (car y)))))
                     '((LHT-EACH-A 10) (LHT-EACH-B 20))))))

(deftest alist-to-lht-roundtrip
  (let ((h (alist->lht (list (cons 'lht-a2l-1 1) (cons 'lht-a2l-2 2)))))
    (progn
      (assert-equal (lht-get h 'lht-a2l-1) 1)
      (assert-equal (lht-get h 'lht-a2l-2) 2)
      (assert-equal (lht-count h) 2))))

(deftest lht-index-is-typed-and-compiled
  ;; The one function in the module simple enough for the native HM checker
  ;; (src/check.rs, run automatically by DEFUN*) to fully infer.
  (assert-equal (see-type 'lht-index) '(TYPED (-> (INT64 INT64) INT64) COMPILED)))

(deftest lht-hash-agrees-across-primitive-types
  ;; Sanity check on LHT-HASH directly: EQUAL keys must hash EQUAL, over a
  ;; sampling of every primitive type category the module special-cases.
  (progn
    (assert-equal (lht-hash nil) (lht-hash nil))
    (assert-equal (lht-hash 'lht-sym) (lht-hash 'lht-sym))
    (assert-equal (lht-hash "abc") (lht-hash (concat "ab" "c")))
    (assert-equal (lht-hash 5) (lht-hash 5))
    (assert-equal (lht-hash 0.0) (lht-hash -0.0))
    (assert-equal (lht-hash (/ 0.0 0.0)) (lht-hash (/ -0.0 0.0)))
    (assert-equal (lht-hash (make-char 65)) (lht-hash (make-char 65)))
    (assert-equal (lht-hash (cons 1 2)) (lht-hash (cons 1 2)))))
