;;; A hash table implemented from scratch in pure Lamedh (issue #458).
;;;
;;; This is NOT a wrapper over the native `LispVal::HashTable` builtin that
;;; backs `make-hash-table`/`gethash`/`sethash` (and that `lib/15-sets-hash.lisp`
;;; is a convenience layer over). Every primitive this file calls is one
;;; KERNEL.md Part XI already requires a host to provide natively: CONS/CAR/CDR,
;;; symbols, fixnum arithmetic and bitwise ops, `ARRAY`/`FETCH`/`STORE`/
;;; `ARRAY-LENGTH*`, `TYPED-ARRAY` (Part IV's `'INT64` element type), the
;;; reader/printer (`PRIN1-TO-STRING`), and `DEFUN`/`DEFUN*`. No `HASH-TABLE`,
;;; `GETHASH`, `SETHASH`, `MAKE-HASH-TABLE`, `KEYS`, or `REMHASH` call appears
;;; anywhere below.
;;;
;;; ---- Why not the portable HM checker (#451)? -------------------------------
;;;
;;; Issue #458 says to type-check this through the portable HM checker (#451)
;;; and/or `lib/29-protocols.lisp`'s typed protocols. As of this writing #451
;;; is still OPEN: it is about porting `src/check.rs`'s inference engine to a
;;; portable `lib/*.lisp` file so it also runs, unmodified, on hosts (like the
;;; SBCL port, #449) that don't have `src/check.rs` natively. It has not
;;; landed. But on *this* host -- the Rust reference `lamedh` binary this file
;;; is loaded into -- `src/check.rs`'s HM checker already exists and already
;;; runs natively via `defun*` (see `CLAUDE.md`/`AGENTS.md`: "`defun*` is the
;;; recommended default function definition form when HM-style type inference
;;; should be attempted automatically"). `LHT-INDEX`, the one leaf function
;;; simple enough for the inferencer to see through cleanly, uses `defun*` and
;;; is confirmed (via `(see-type 'lht-index)`) to come back `TYPED (-> (INT64
;;; INT64) INT64) [COMPILED]` -- checked *and* JIT-compiled, a real win from
;;; the native checker, not a nominal one. Every other function here is
;;; deliberately plain `defun` plus an explicit `declare-type!` axiom (the
;;; same pattern `lib/28-types.lisp` uses for the rest of the stdlib) rather
;;; than `defun*`: probed by hand, `defun*` on e.g. the `AND`/`EQ`-composed
;;; predicate `LHT-P` comes back `CHECKED (FORALL (A B) (-> ((ARRAY A)) B))`
;;; -- a checker artifact around `AND`/`EQ`'s short-circuit value that claims
;;; a fully generic result type for a function that always returns a boolean.
;;; That is exactly the kind of unsound-looking automatic inference this
;;; module would rather not surface silently in library code real programs
;;; depend on; a hand-written, verified-against-behavior axiom is the honest
;;; choice there, matching `lib/28-types.lisp`'s own stated rule of only
;;; declaring what has actually been checked against evaluator behavior. It
;;; does not use `lib/29-protocols.lisp`'s dispatch machinery: there is exactly one
;;; concrete representation here (no multiple types implementing one
;;; interface), so protocol dispatch would add indirection without adding
;;; type confidence over what `defun*`/`declare-type!` already gives.
;;;
;;; ---- Representation ---------------------------------------------------------
;;;
;;; A table is one 8-slot general ARRAY (a lightweight hand-rolled record --
;;; using ARRAY directly, rather than DEFRECORD, keeps this file's only
;;; dependency the primitives Part XI lists, not the condensation layer):
;;;
;;;   slot 0: the tag symbol LHT (identifies the array as a table to LHT-P)
;;;   slot 1: BUCKETS  -- (TYPED-ARRAY capacity 'INT64), the open-addressing
;;;           index: BUCKETS[i] is -1 (EMPTY), -2 (TOMBSTONE), or a
;;;           non-negative PAYLOAD index into KEYS/VALS.
;;;   slot 2: KEYS     -- (ARRAY capacity), general array of live/dead keys
;;;   slot 3: VALS     -- (ARRAY capacity), parallel array of values
;;;   slot 4: CAPACITY -- current length of BUCKETS/KEYS/VALS (a power of 2)
;;;   slot 5: COUNT    -- number of live entries
;;;   slot 6: TOMBSTONES -- number of deleted-but-not-yet-reclaimed buckets
;;;   slot 7: NEXT-SLOT -- next free index into KEYS/VALS (bump allocator,
;;;           reset to 0 on every rehash, which compacts away dead payload
;;;           slots)
;;;
;;; This is open addressing with indirection: BUCKETS is the probe structure
;;; (sized to capacity, cheap to rehash/scan) and KEYS/VALS hold the actual
;;; heterogeneous LispVal payload the issue calls out ("since keys/values are
;;; arbitrary LispVals, not themselves int64/float64 scalars"). Linear probing
;;; was chosen over Robin Hood hashing: Robin Hood needs a per-slot probe-
;;; sequence-length field and swap-on-insert/backward-shift-on-delete logic to
;;; bound worst-case probe length under clustering; linear probing needs
;;; neither, and at the load factor this table maintains (grows before the
;;; live+tombstone fraction reaches 0.7) its expected probe length stays
;;; small. Robin Hood's real advantage -- flatter probe-length variance under
;;; adversarial or highly clustered hashes -- is a legitimate future
;;; improvement, not a correctness requirement, so the simpler scheme was
;;; chosen for a first from-scratch implementation.
;;;
;;; ---- The hash function and EQUAL agreement -----------------------------
;;;
;;; KERNEL.md Part IV's key-equality contract is EQ/EQUAL: two keys collide
;;; in the same bucket-chain sense (i.e. MUST hash equal) exactly when they
;;; are EQUAL, with two canonicalizations spelled out explicitly: `0.0` and
;;; `-0.0` are the SAME key, and `NaN` finds `NaN`. LHT-HASH below handles
;;; both without needing raw IEEE-754 bit access (no such primitive is in
;;; Part XI's inventory, and none was added for this): floats are hashed by
;;; their *canonical printed form* -- `(prin1-to-string f)`, except any float
;;; equal to 0.0 (which per Part IV's `=` is true for both signs of zero)
;;; canonicalizes to the literal string "0.0" before hashing, and any NaN
;;; already prints as the single literal "NaN" regardless of its sign or
;;; payload bits (verified against the reference: `(prin1-to-string (/ 0.0
;;; 0.0))` and `(prin1-to-string (/ -0.0 0.0))` both print "NaN"). Every other
;;; type is hashed structurally in a way that agrees with `EQUAL`: symbols by
;;; their printed name, strings by their code points, conses recursively by
;;; CAR/CDR, fixnums and chars directly. Host-opaque types whose `EQUAL` is
;;; identity (hash tables, arrays, typed arrays, environments, closures,
;;; builtins) and structural-but-unintrospectable types (records, conditions)
;;; all hash to one constant bucket: this is *correct* (equal keys, which for
;;; an identity type means the same object, trivially still hash equal to
;;; themselves) but deliberately degenerate -- every such key collides with
;;; every other, so a table keyed heavily by e.g. distinct arrays performs no
;;; better than an alist. This is the concrete "large performance hit" gap
;;; the issue asked to be reported honestly if it exists; see the PR
;;; description's benchmark section. It is a direct consequence of Part XI
;;; not providing any host-independent identity-hash primitive (no address-of,
;;; no counter tied to allocation) -- exactly the kind of concrete, actionable
;;; gap the issue says is worth surfacing.
;;;
;;; The integer mixer (LHT-MIX64) is a SplitMix64-style finalizer, with its
;;; constants pre-masked to 63 bits (so they parse as fixnum literals rather
;;; than overflowing to floats per Part II's literal-overflow rule) and every
;;; intermediate value masked back to 63 bits after each step. That mask
;;; matters beyond literal parsing: `ASH` with a negative shift is Lisp's
;;; usual ARITHMETIC (sign-extending) right shift, not a logical one -- there
;;; is no unsigned-shift primitive in Part XI's inventory either -- so mixing
;;; a full 64-bit two's-complement value would sign-extend the top bit
;;; through every right shift and measurably weaken the avalanche. Keeping
;;; the accumulator non-negative (top bit always 0) makes every `ASH ... -N`
;;; in this file behave as a logical shift, at the cost of one bit of hash
;;; space -- immaterial once capacity masks it down to a handful of bits
;;; anyway. Multiplying by odd constants intentionally wraps mod 2^64 (Part
;;; V's fixed-width model); the reference prints a stderr `integer overflow`
;;; warning and sets the global OVERFLOW flag on each wraparound, which is
;;; expected and harmless here -- callers who care can `(clear-flag
;;; 'overflow)` themselves.

;;; ---- tunable constants ------------------------------------------------------

(def $lht-mask63 #x7FFFFFFFFFFFFFFF)  ; 2^63 - 1: keeps the mixer non-negative
(def $lht-c1 #x1E3779B97F4A7C15)      ; SplitMix64's gamma, masked to 63 bits
(def $lht-c2 #x3F58476D1CE4E5B9)      ; SplitMix64 mix constant 1, masked
(def $lht-empty -1)
(def $lht-tombstone -2)
(def $lht-initial-capacity 16)
;; Grow when (live + tombstones) * 10 >= capacity * 7, i.e. load factor 0.7.
(def $lht-grow-num 10)
(def $lht-grow-den 7)

(def $lht-tag-nil 1)
(def $lht-tag-sym 2)
(def $lht-tag-str 3)
(def $lht-tag-char 4)
(def $lht-tag-float 5)
(def $lht-tag-fix 6)
(def $lht-tag-opaque (logand #x2545F4914F6CDD1D $lht-mask63)) ; one fixed bucket

;;; ---- the integer/string mixer -----------------------------------------------

(defun lht-mix64 (x0)
  "SplitMix64-style avalanche finalizer over a 63-bit non-negative fixnum."
  (let* ((x (logand x0 $lht-mask63))
         (x (logand (* (logxor x (ash x -30)) $lht-c1) $lht-mask63))
         (x (logand (* (logxor x (ash x -27)) $lht-c2) $lht-mask63))
         (x (logxor x (ash x -31))))
    (logand x $lht-mask63)))

(defun lht-hash-string-loop (s i n acc)
  (if (>= i n)
      acc
      (lht-hash-string-loop
       s (+ i 1) n
       (lht-mix64 (logxor (* acc 31) (char-code (char-at s i)))))))

(defun lht-hash-string (s)
  "Hash a string by folding LHT-MIX64 over its code points."
  (lht-hash-string-loop s 0 (string-length* s) $lht-c1))

(defun lht-canon-float-string (f)
  "Canonical printed form of F for hashing: 0.0 and -0.0 hash identically."
  (if (= f 0.0) "0.0" (prin1-to-string f)))

;;; ---- the LispVal hash function -----------------------------------------------

(defun lht-hash (v)
  "Hash any LispVal so that (EQUAL A B) implies (= (lht-hash A) (lht-hash B))."
  (cond
    ((null v) (lht-mix64 $lht-tag-nil))
    ((consp v)
     (lht-mix64 (logxor (* 3 (lht-hash (car v))) (lht-hash (cdr v)))))
    ((symbolp v) (lht-mix64 (logxor $lht-tag-sym (lht-hash-string (princ-to-string v)))))
    ((stringp v) (lht-mix64 (logxor $lht-tag-str (lht-hash-string v))))
    ((charp v) (lht-mix64 (logxor $lht-tag-char (char-code v))))
    ((floatp v) (lht-mix64 (logxor $lht-tag-float (lht-hash-string (lht-canon-float-string v)))))
    ((fixp v) (lht-mix64 (logxor $lht-tag-fix v)))
    (t $lht-tag-opaque)))

(declare-type! 'lht-hash '(-> (any) int64))

;;; ---- the table record -------------------------------------------------------

(defun make-lht ()
  "Construct a fresh, empty pure-Lamedh hash table."
  (let ((a (array 8))
        (buckets (typed-array $lht-initial-capacity 'int64)))
    (array-fill buckets $lht-empty)
    (store a 0 'lht)
    (store a 1 buckets)
    (store a 2 (array $lht-initial-capacity))
    (store a 3 (array $lht-initial-capacity))
    (store a 4 $lht-initial-capacity)
    (store a 5 0)
    (store a 6 0)
    (store a 7 0)
    a))

(defun lht-p (x)
  "True if X is a pure-Lamedh hash table built by MAKE-LHT."
  (and (arrayp x) (= (array-length* x) 8) (eq (fetch x 0) 'lht)))

(declare-type! 'lht-p '(forall (a) (-> (a) bool)))

(defun lht--buckets (ht) (fetch ht 1))
(defun lht--keys (ht) (fetch ht 2))
(defun lht--vals (ht) (fetch ht 3))
(defun lht--capacity (ht) (fetch ht 4))
(defun lht-count (ht) (fetch ht 5))
(defun lht--tombstones (ht) (fetch ht 6))
(defun lht--next-slot (ht) (fetch ht 7))

(declare-type! 'lht-count '(-> (any) int64))

;;; ---- probing ------------------------------------------------------------

(defun* lht-index (h cap) (logand h (- cap 1)))

(defun lht-probe (buckets keys cap key start i tomb)
  "Walk the probe sequence from START. Returns (STATUS BUCKET-IDX PAYLOAD-IDX):
STATUS is 'HIT (KEY found; PAYLOAD-IDX valid), 'MISS (KEY absent; BUCKET-IDX
is the first empty-or-tombstone slot on the chain, for insertion), or 'FULL
(every bucket probed without an empty slot -- can only happen if the grow
policy below is violated; treated as an internal-error safety net)."
  (if (>= i cap)
      (list 'full -1 nil)
      (let* ((idx (lht-index (+ start i) cap))
             (b (fetch buckets idx)))
        (cond
          ((= b $lht-empty)
           (list 'miss (if (>= tomb 0) tomb idx) nil))
          ((= b $lht-tombstone)
           (lht-probe buckets keys cap key start (+ i 1)
                      (if (>= tomb 0) tomb idx)))
          ((equal (fetch keys b) key) (list 'hit idx b))
          (t (lht-probe buckets keys cap key start (+ i 1) tomb))))))

(defun lht-find (ht key)
  (let* ((cap (lht--capacity ht))
         (h (lht-hash key))
         (start (lht-index h cap)))
    (lht-probe (lht--buckets ht) (lht--keys ht) cap key start 0 -1)))

;;; ---- growth / rehashing ---------------------------------------------------

(defun lht-should-grow-p (ht)
  (>= (* (+ (lht-count ht) (lht--tombstones ht)) $lht-grow-num)
      (* (lht--capacity ht) $lht-grow-den)))

(defun lht-next-capacity (cap count)
  "Smallest power-of-two capacity, doubling from CAP, giving COUNT+1 live
entries headroom under the 0.7 load factor with zero tombstones."
  (if (< (* (+ count 1) $lht-grow-num) (* cap $lht-grow-den))
      cap
      (lht-next-capacity (* cap 2) count)))

(defun lht-insert-empty! (buckets cap start i payload-idx)
  "Linear-probe from START for an EMPTY slot in a table known to hold no
duplicate keys yet (used only during rehash), and claim it for PAYLOAD-IDX."
  (let ((idx (lht-index (+ start i) cap)))
    (if (= (fetch buckets idx) $lht-empty)
        (store buckets idx payload-idx)
        (lht-insert-empty! buckets cap start (+ i 1) payload-idx))))

(defun lht-rehash-into! (old-buckets old-keys old-vals old-cap
                          new-buckets new-keys new-vals new-cap
                          next-slot i)
  (if (>= i old-cap)
      next-slot
      (let ((b (fetch old-buckets i)))
        (if (>= b 0)
            (let* ((k (fetch old-keys b))
                   (v (fetch old-vals b))
                   (start (lht-index (lht-hash k) new-cap)))
              (lht-insert-empty! new-buckets new-cap start 0 next-slot)
              (store new-keys next-slot k)
              (store new-vals next-slot v)
              (lht-rehash-into! old-buckets old-keys old-vals old-cap
                                 new-buckets new-keys new-vals new-cap
                                 (+ next-slot 1) (+ i 1)))
            (lht-rehash-into! old-buckets old-keys old-vals old-cap
                               new-buckets new-keys new-vals new-cap
                               next-slot (+ i 1))))))

(defun lht-grow! (ht)
  "Double (or more) HT's capacity in place, compacting away tombstones."
  (let* ((old-buckets (lht--buckets ht))
         (old-keys (lht--keys ht))
         (old-vals (lht--vals ht))
         (old-cap (lht--capacity ht))
         (new-cap (lht-next-capacity (* old-cap 2) (lht-count ht)))
         (new-buckets (typed-array new-cap 'int64))
         (new-keys (array new-cap))
         (new-vals (array new-cap)))
    (array-fill new-buckets $lht-empty)
    (let ((final-next (lht-rehash-into! old-buckets old-keys old-vals old-cap
                                         new-buckets new-keys new-vals new-cap
                                         0 0)))
      (store ht 1 new-buckets)
      (store ht 2 new-keys)
      (store ht 3 new-vals)
      (store ht 4 new-cap)
      (store ht 6 0)
      (store ht 7 final-next)
      ht)))

;;; ---- public operations ------------------------------------------------------

(defun lht-get (ht key)
  "Return the value stored for KEY in HT, or NIL if absent (same
no-second-value contract as the native GETHASH: a stored NIL is
indistinguishable from absence)."
  (let ((r (lht-find ht key)))
    (if (eq (car r) 'hit) (fetch (lht--vals ht) (caddr r)) nil)))

(defun lht-has-key-p (ht key)
  (eq (car (lht-find ht key)) 'hit))

(defun lht-put! (ht key val)
  "Insert or replace KEY -> VAL in HT; returns T (mirrors native SETHASH)."
  (if (lht-should-grow-p ht) (lht-grow! ht) nil)
  (let ((r (lht-find ht key)))
    (cond
      ((eq (car r) 'hit)
       (store (lht--vals ht) (caddr r) val)
       t)
      ((eq (car r) 'miss)
       (let* ((bidx (cadr r))
              (was-tomb (= (fetch (lht--buckets ht) bidx) $lht-tombstone))
              (pidx (lht--next-slot ht)))
         (store (lht--keys ht) pidx key)
         (store (lht--vals ht) pidx val)
         (store (lht--buckets ht) bidx pidx)
         (store ht 7 (+ pidx 1))
         (store ht 5 (+ (lht-count ht) 1))
         (if was-tomb (store ht 6 (- (lht--tombstones ht) 1)) nil)
         t))
      (t (error "lht-put!: table full despite grow policy (internal error)")))))

(defun lht-remove! (ht key)
  "Remove KEY from HT if present; always returns T (mirrors native REMHASH)."
  (let ((r (lht-find ht key)))
    (if (eq (car r) 'hit)
        (progn
          (store (lht--buckets ht) (cadr r) $lht-tombstone)
          (store ht 5 (- (lht-count ht) 1))
          (store ht 6 (+ (lht--tombstones ht) 1))
          t)
        t)))

(declare-type! 'lht-has-key-p '(forall (a b) (-> (a b) bool)))
(declare-type! 'lht-put! '(forall (a b) (-> (a a b) bool)))
(declare-type! 'lht-remove! '(forall (a b) (-> (a b) bool)))

(defun lht-keys-loop (buckets keys cap i acc)
  (if (>= i cap)
      acc
      (let ((b (fetch buckets i)))
        (lht-keys-loop buckets keys cap (+ i 1)
                       (if (>= b 0) (cons (fetch keys b) acc) acc)))))

(defun lht-keys (ht)
  "Return HT's keys as a list, in unspecified order (matches native KEYS)."
  (lht-keys-loop (lht--buckets ht) (lht--keys ht) (lht--capacity ht) 0 nil))

(defun lht-each (ht fn)
  "Call (FN key value) for each entry of HT; return NIL."
  (mapc (lambda (k) (funcall fn k (lht-get ht k))) (lht-keys ht))
  nil)

(defun lht->alist (ht)
  "Return HT's entries as an alist of (key . value)."
  (mapcar (lambda (k) (cons k (lht-get ht k))) (lht-keys ht)))

(defun alist->lht (alist)
  "Build a fresh pure-Lamedh hash table from an alist of (key . value)."
  (let ((ht (make-lht)))
    (mapc (lambda (cell) (lht-put! ht (car cell) (cdr cell))) alist)
    ht))

(defun lht-clear! (ht)
  "Remove every entry from HT (rebuilds it at the initial capacity); return HT."
  (let ((buckets (typed-array $lht-initial-capacity 'int64)))
    (array-fill buckets $lht-empty)
    (store ht 1 buckets)
    (store ht 2 (array $lht-initial-capacity))
    (store ht 3 (array $lht-initial-capacity))
    (store ht 4 $lht-initial-capacity)
    (store ht 5 0)
    (store ht 6 0)
    (store ht 7 0)
    ht))
