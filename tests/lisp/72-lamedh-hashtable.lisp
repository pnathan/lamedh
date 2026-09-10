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

(deftest lht-hash-spreads-distinct-opaque-keys
  ;; Issue #474 closed the degenerate-bucketing gap this file originally
  ;; shipped with: before HASH-CODE existed, every host-opaque
  ;; identity-compared key (arrays, closures, hash tables, ...) hashed to one
  ;; constant bucket. Distinct *simultaneously live* arrays must now hash to
  ;; different values -- both bound here so neither's allocation can be
  ;; reused for the other's address (HASH-CODE hashes the live allocation,
  ;; not a global identity: a freed then reused address is a documented,
  ;; harmless caveat, not something this test should trip over).
  (let ((a1 (array 1))
        (a2 (array 1)))
    (assert-true (/= (lht-hash a1) (lht-hash a2)))))

(deftest lht-array-keys-spread-not-single-bucket-chain
  ;; Issue #474 follow-up, flagged explicitly in #472's own body: before
  ;; HASH-CODE existed, every identity-compared key (arrays included)
  ;; hashed into the SAME constant bucket, so N distinct array keys
  ;; degenerated into one O(N) linear-probe chain -- no better than an
  ;; alist. HASH-CODE gives each live array its own allocation-derived
  ;; hash, so LHT-HASH now spreads them like any other key type. This
  ;; inserts many distinct, simultaneously-live arrays and bounds the
  ;; worst *actual* probe-chain length (distance from a key's ideal bucket
  ;; to the bucket it actually lands in), which the old constant-bucket
  ;; branch could never satisfy -- its worst case grew as N-1, unboundedly
  ;; with N, since every key shared one ideal bucket.
  (let* ((n 200)
         (h (make-lht))
         (keys (lht-make-distinct-arrays n)))
    (progn
      (mapc (lambda (k) (lht-put! h k 'lht-arr-present)) keys)
      (assert-equal (lht-count h) n)
      ;; Spreading must not break lookup: every key is still found.
      (assert-nil (lht-array-key-mismatches h keys))
      ;; A generous bound -- well above the O(log N) chain length expected
      ;; under real hashing at this table's 0.7 load factor -- that only a
      ;; genuinely degenerate, single-bucket hash could blow through.
      (assert-true (< (lht-max-probe-chain h keys) 40)))))

(defun lht-make-distinct-arrays (n)
  (if (<= n 0) nil (cons (array 1) (lht-make-distinct-arrays (- n 1)))))

(defun lht-array-key-mismatches (h keys)
  (if (null keys)
      nil
      (if (eq (lht-get h (car keys)) 'lht-arr-present)
          (lht-array-key-mismatches h (cdr keys))
          (cons (car keys) (lht-array-key-mismatches h (cdr keys))))))

;; Probe-chain length for KEY: distance (mod CAPACITY) from KEY's ideal
;; bucket (LHT-INDEX of its hash) to the bucket LHT-FIND actually lands it
;; in. A degenerate hash (every key sharing one ideal bucket) forces this
;; toward the full linear-probe length as more keys are inserted; a
;; well-spread hash keeps it small regardless of table size.
(defun lht-probe-chain-length (h key)
  (let* ((cap (lht--capacity h))
         (start (lht-index (lht-hash key) cap))
         (r (lht-find h key))
         ;; Packed probe result (see lib/45-hashtable.lisp, probing section):
         ;; a hit is the bucket itself, a miss is (- -1 bucket).
         (idx (if (>= r 0) r (- -1 r))))
    (mod (- idx start) cap)))

(defun lht-max-probe-chain (h keys)
  (if (null keys)
      0
      (max (lht-probe-chain-length h (car keys))
           (lht-max-probe-chain h (cdr keys)))))

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
  (assert-equal (see-type 'lht-index) '(TYPED (-> (INT64 INT64) INT64) COMPILED)))

(deftest lht-probe-is-typed-and-compiled
  ;; The per-step hot path (issue #476): BUCKETS crosses as a zero-copy
  ;; (array int64), KEYS and KEY as opaque boxed handles, and the result is one
  ;; packed int64 -- so the whole probe loop compiles.
  (assert-equal (see-type 'lht-probe)
                '(TYPED (-> ((ARRAY INT64) BOXED INT64 BOXED INT64 INT64 INT64) INT64)
                        COMPILED)))

(deftest lht-insert-empty-is-typed-and-compiled
  (assert-equal (see-type 'lht-insert-empty!)
                '(TYPED (-> ((ARRAY INT64) INT64 INT64 INT64 INT64) INT64) COMPILED)))

(deftest lht-hash-and-mixer-are-typed-and-compiled
  (progn
    (assert-equal (see-type 'lht-mix64) '(TYPED (-> (INT64) INT64) COMPILED))
    (assert-equal (see-type 'lht-hash) '(TYPED (-> (BOXED) INT64) COMPILED))))

(defun lht-reference-mix64 (x0)
  ;; The mixer as an interpreted body over the module's own constants: the
  ;; typed LHT-MIX64 spells them as literals and must agree bit for bit,
  ;; including the wrapping multiplies.
  (let* ((x (logand x0 $lht-mask63))
         (x (logand (* (logxor x (ash x -30)) $lht-c1) $lht-mask63))
         (x (logand (* (logxor x (ash x -27)) $lht-c2) $lht-mask63))
         (x (logxor x (ash x -31))))
    (logand x $lht-mask63)))

(deftest lht-mix64-matches-reference-mixer
  (let ((samples (list 0 1 -1 2 -2 42 123456789012345 -987654321098765
                       #x7FFFFFFFFFFFFFFF (- 0 #x7FFFFFFFFFFFFFFF)
                       (- -1 #x7FFFFFFFFFFFFFFF)
                       (hash-code 'lht-mix-sym) (hash-code "lht-mix-str")
                       (hash-code 3.5) (hash-code nil) (hash-code (cons 1 2)))))
    (progn
      (mapc (lambda (x) (assert-equal (lht-mix64 x) (lht-reference-mix64 x)))
            samples)
      (clear-flag 'overflow))))

(deftest lht-sentinels-match-typed-literals
  ;; The typed bodies of LHT-PROBE and LHT-INSERT-EMPTY! cannot read a global,
  ;; so they spell the EMPTY/TOMBSTONE sentinels as literals. Pin the globals
  ;; to those literals so the two cannot drift apart silently.
  (progn
    (assert-equal $lht-empty -1)
    (assert-equal $lht-tombstone -2)))

(deftest lht-probe-packed-result-encoding
  ;; Direct exercise of the three result classes on hand-built inputs.
  ;; Capacity 4; buckets[2] -> payload 0 holds the key.
  (let ((buckets (typed-array 4 'int64))
        (keys (array 4)))
    (progn
      (array-fill buckets $lht-empty)
      (store buckets 2 0)
      (store keys 0 'lht-enc-key)
      ;; HIT: start at the key's own bucket -> that bucket.
      (assert-equal (lht-probe buckets keys 4 'lht-enc-key 2 0 -1) 2)
      ;; HIT after one collision step: start one before, bucket 1 is EMPTY so
      ;; a different key MISSES there, encoded (- -1 1).
      (assert-equal (lht-probe buckets keys 4 'lht-enc-other 1 0 -1) -2)
      ;; MISS through a tombstone: the first tombstone on the chain is the
      ;; insertion slot even though a later slot is EMPTY.
      (store buckets 0 $lht-tombstone)
      (assert-equal (lht-probe buckets keys 4 'lht-enc-other 0 0 -1) -1)
      ;; FULL: every bucket a tombstone or a non-matching key, no EMPTY slot.
      (array-fill buckets $lht-tombstone)
      (assert-equal (lht-probe buckets keys 4 'lht-enc-other 0 0 -1) (- -1 4))
      ;; The public decoders agree with the encoding end to end.
      (let ((h (make-lht)))
        (progn
          (lht-put! h 'lht-enc-a 1)
          (assert-true (>= (lht-find h 'lht-enc-a) 0))
          (assert-equal (fetch (lht--keys h)
                               (fetch (lht--buckets h) (lht-find h 'lht-enc-a)))
                        'lht-enc-a)
          (assert-true (< (lht-find h 'lht-enc-absent) 0))
          (assert-true (>= (lht-find h 'lht-enc-absent) (- (lht--capacity h)))))))))

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

;;; ---- the MAP wrapper ------------------------------------------------------

(deftest map-make-picks-native-backend-on-this-host
  ;; This host (the Rust reference binary) always has MAKE-HASH-TABLE
  ;; bound, so MAKE-MAP must prefer it over LHT.
  (assert-true (hash-table-p (make-map))))

(deftest map-p-accepts-either-backend
  (progn
    (assert-true (map-p (make-hash-table)))
    (assert-true (map-p (make-lht)))
    (assert-false (map-p (array 8)))
    (assert-false (map-p 42))))

(deftest map-ops-work-on-a-native-table
  (let ((m (make-hash-table)))
    (progn
      (map-put! m 'map-native-key 1)
      (assert-equal (map-get m 'map-native-key) 1)
      (assert-true (map-has-key-p m 'map-native-key))
      (assert-equal (map-count m) 1)
      (assert-equal (map-keys m) '(map-native-key))
      (map-remove! m 'map-native-key)
      (assert-false (map-has-key-p m 'map-native-key))
      (assert-equal (map-count m) 0))))

(deftest map-ops-work-on-an-lht-table
  ;; The same MAP-* calls, unmodified, against the pure-Lamedh backend --
  ;; this is the whole point: callers need not know which backend a table
  ;; they were handed actually is.
  (let ((m (make-lht)))
    (progn
      (map-put! m 'map-lht-key 1)
      (assert-equal (map-get m 'map-lht-key) 1)
      (assert-true (map-has-key-p m 'map-lht-key))
      (assert-equal (map-count m) 1)
      (assert-equal (map-keys m) '(map-lht-key))
      (map-remove! m 'map-lht-key)
      (assert-false (map-has-key-p m 'map-lht-key))
      (assert-equal (map-count m) 0))))

(deftest map-each-and-map-alist-agree-across-backends
  (let ((native (make-hash-table))
        (lht (make-lht))
        (native-acc nil)
        (lht-acc nil))
    (progn
      (map-put! native 'map-each-a 1)
      (map-put! native 'map-each-b 2)
      (map-put! lht 'map-each-a 1)
      (map-put! lht 'map-each-b 2)
      (map-each native (lambda (k v) (csetq native-acc (cons (cons k v) native-acc))))
      (map-each lht (lambda (k v) (csetq lht-acc (cons (cons k v) lht-acc))))
      (assert-equal (sort-by native-acc (lambda (kv) (princ-to-string (car kv))) #'string<)
                     (sort-by lht-acc (lambda (kv) (princ-to-string (car kv))) #'string<))
      (assert-equal (sort-by (map->alist native) (lambda (kv) (princ-to-string (car kv))) #'string<)
                     (sort-by (map->alist lht) (lambda (kv) (princ-to-string (car kv))) #'string<)))))
