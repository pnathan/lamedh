;;; A hash table implemented from scratch in pure Lamedh (issue #458).
;;;
;;; This is NOT a wrapper over the native `LispVal::HashTable` builtin that
;;; backs `make-hash-table`/`gethash`/`sethash` (and that `lib/15-sets-hash.lisp`
;;; is a convenience layer over). Every primitive this file calls is one
;;; KERNEL.md Part XI already requires a host to provide natively: CONS/CAR/CDR,
;;; symbols, fixnum arithmetic and bitwise ops, `ARRAY`/`FETCH`/`STORE`/
;;; `ARRAY-LENGTH*`, `TYPED-ARRAY` (Part IV's `'INT64` element type), the
;;; reader/printer (`PRIN1-TO-STRING`), and `DEFUN`/`DEFUN-TYPED`. No `HASH-TABLE`,
;;; `GETHASH`, `SETHASH`, `MAKE-HASH-TABLE`, `KEYS`, or `REMHASH` call appears
;;; anywhere below.
;;;
;;; ---- Typing: five DEFUN-TYPED functions, DECLARE-TYPE! for the rest -------
;;;
;;; Issue #458 says to type-check this through the portable HM checker (#451)
;;; and/or `lib/29-protocols.lisp`'s typed protocols. #451 -- porting
;;; `src/check.rs`'s inference engine to a portable `lib/*.lisp` file so it
;;; also runs, unmodified, on hosts (like the SBCL port, #449) that don't have
;;; `src/check.rs` natively -- is still OPEN. On *this* host the typed compiler
;;; exists natively, and this file uses it exactly where it pays. Five functions
;;; carry explicit DEFUN-TYPED signatures and compile: the per-step loops
;;; LHT-INDEX, LHT-PROBE and LHT-INSERT-EMPTY!, and the per-operation LHT-HASH
;;; and LHT-MIX64, which are pure INT64 arithmetic once HASH-CODE has run
;;; (`(see-type 'lht-probe)` reads `COMPILED`; the tests pin all five).
;;; LHT-PROBE is what issue #476's `boxed` type exists for: its KEYS and KEY
;;; arguments are arbitrary LispVals, carried through compiled code as opaque
;;; handles (see the probing section below). Every other function here is
;;; plain `defun` plus an
;;; explicit `declare-type!` axiom (the same pattern `lib/28-types.lisp` uses
;;; for the rest of the stdlib) rather than `defun*`: probed by hand, `defun*`
;;; on e.g. the `AND`/`EQ`-composed predicate `LHT-P` comes back `CHECKED
;;; (FORALL (A B) (-> ((ARRAY A)) B))` -- a checker artifact around `AND`/`EQ`'s
;;; short-circuit value that claims a fully generic result type for a function
;;; that always returns a boolean. That is exactly the kind of unsound-looking
;;; automatic inference this module would rather not surface silently in
;;; library code real programs depend on; a hand-written,
;;; verified-against-behavior axiom is the honest choice there, matching
;;; `lib/28-types.lisp`'s own stated rule of only declaring what has actually
;;; been checked against evaluator behavior. It does not use
;;; `lib/29-protocols.lisp`'s dispatch machinery: there is exactly one
;;; concrete representation here (no multiple types implementing one
;;; interface), so protocol dispatch would add indirection without adding
;;; type confidence over what DEFUN-TYPED/`declare-type!` already gives.
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
;;; are EQUAL. LHT-HASH gets this for free from the native `HASH-CODE`
;;; primitive (issue #474): `(equal a b)` implies `(= (hash-code a) (hash-code
;;; b))` is HASH-CODE's own documented contract, backed by the same `Hash for
;;; LispVal` impl the native `HashTable` builtin's `HashMap<LispVal, LispVal>`
;;; already relies on -- so LHT-HASH is just that value re-mixed through
;;; LHT-MIX64 for this table's own avalanche/distribution needs, not a
;;; hand-rolled EQUAL-agreement proof. In particular this closes the
;;; degenerate-bucketing gap this file originally shipped with: HASH-CODE
;;; hashes host-opaque, identity-compared types (arrays, hash tables,
;;; environments, closures, ...) by their underlying allocation's address, so
;;; a table keyed heavily by e.g. distinct arrays now spreads across buckets
;;; instead of colliding into one. One caveat worth knowing, not a
;;; correctness gap: an allocation's address is only unique among
;;; simultaneously-live objects, so two host-opaque keys that are never alive
;;; at the same time can, by allocator coincidence, share a HASH-CODE. This
;;; is harmless here -- LHT-PROBE always confirms a bucket match with EQUAL,
;;; never trusts a hash match alone -- and cannot happen for any key actually
;;; stored in a live table (its slot in KEYS holds a live reference for as
;;; long as it stays a key).
;;;
;;; The integer mixer (LHT-MIX64) is a SplitMix64-style finalizer, with its
;;; constants pre-masked to 63 bits (so they parse as fixnum literals rather
;;; than overflowing to floats per Part II's literal-overflow rule) and every
;;; intermediate value masked back to 63 bits after each step -- including
;;; the initial HASH-CODE input, which is a full-range (possibly negative)
;;; 64-bit value masked down by the first `LOGAND` below. That mask matters
;;; beyond literal parsing: `ASH` with a negative shift is Lisp's usual
;;; ARITHMETIC (sign-extending) right shift, not a logical one -- there is no
;;; unsigned-shift primitive in Part XI's inventory either -- so mixing a
;;; full 64-bit two's-complement value would sign-extend the top bit through
;;; every right shift and measurably weaken the avalanche. Keeping the
;;; accumulator non-negative (top bit always 0) makes every `ASH ... -N` in
;;; this file behave as a logical shift, at the cost of one bit of hash space
;;; -- immaterial once capacity masks it down to a handful of bits anyway.
;;; Multiplying by odd constants intentionally wraps mod 2^64 (Part V's
;;; fixed-width model); the reference prints a stderr `integer overflow`
;;; warning and sets the global OVERFLOW flag on each wraparound, which is
;;; expected and harmless here -- callers who care can `(clear-flag
;;; 'overflow)` themselves.

;;; ---- tunable constants ------------------------------------------------------

;; LHT-MIX64 is DEFUN-TYPED and a typed body cannot read a global, so it
;; repeats these three as literals; the test LHT-MIX64-MATCHES-REFERENCE-MIXER
;; recomputes the mixer from the globals and pins the two spellings together.
(def $lht-mask63 #x7FFFFFFFFFFFFFFF)  ; 2^63 - 1: keeps the mixer non-negative
(def $lht-c1 #x1E3779B97F4A7C15)      ; SplitMix64's gamma, masked to 63 bits
(def $lht-c2 #x3F58476D1CE4E5B9)      ; SplitMix64 mix constant 1, masked
(def $lht-empty -1)
(def $lht-tombstone -2)
(def $lht-initial-capacity 16)
;; Grow when (live + tombstones) * 10 >= capacity * 7, i.e. load factor 0.7.
(def $lht-grow-num 10)
(def $lht-grow-den 7)

;;; ---- the integer mixer --------------------------------------------------

;; SplitMix64-style avalanche finalizer over a 63-bit non-negative fixnum.
;; Typed: the multiplies wrap mod 2^64 on every tier (Part V's fixed-width
;; model), and the compiled edition sets the same OVERFLOW flag the
;; tree-walker does, so the result is bit-identical to the interpreted body
;; (pinned by LHT-MIX64-MATCHES-REFERENCE-MIXER).
(defun-typed (lht-mix64 int64) ((x0 int64))
  (let ((x (logand x0 #x7FFFFFFFFFFFFFFF)))
    (let ((x (logand (* (logxor x (ash x -30)) #x1E3779B97F4A7C15)
                     #x7FFFFFFFFFFFFFFF)))
      (let ((x (logand (* (logxor x (ash x -27)) #x3F58476D1CE4E5B9)
                       #x7FFFFFFFFFFFFFFF)))
        (logand (logxor x (ash x -31)) #x7FFFFFFFFFFFFFFF)))))

;;; ---- the LispVal hash function -----------------------------------------------

;; Hash any LispVal so that (EQUAL A B) implies (= (lht-hash A) (lht-hash B)).
;; HASH-CODE (issue #474) already guarantees that agreement natively -- this
;; just re-mixes its output through LHT-MIX64 for this table's own avalanche.
;; V is `boxed` (issue #476): any LispVal crosses into the compiled body as an
;; opaque handle, and HASH-CODE on a handle is an intrinsic that runs the same
;; Rust hasher the builtin does.
(defun-typed (lht-hash int64) ((v boxed))
  (lht-mix64 (hash-code v)))

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
;;;
;;; The probe loop is the per-STEP hot path -- everything else in this file
;;; runs once per operation -- so it is the part that must compile. Three
;;; things make that possible:
;;;
;;;   * BUCKETS is a TYPED-ARRAY of INT64 and crosses the typed boundary as a
;;;     zero-copy `(array int64)`.
;;;   * KEYS (a general ARRAY of arbitrary LispVals) and KEY (an arbitrary
;;;     LispVal) cross as `boxed` handles (issue #476): opaque words the
;;;     compiled code only moves, and looks inside solely through the
;;;     `fetch`-on-a-handle and `equal` intrinsics.
;;;   * The result is one packed INT64, not a `(STATUS BUCKET PAYLOAD)` list.
;;;     A cons per probe step keeps a function interpreted regardless of its
;;;     types; the decoders below run once per operation, not once per step.
;;;
;;; Result encoding, for a table of capacity CAP:
;;;
;;;   r >= 0          HIT at bucket r; the payload index is (fetch buckets r).
;;;   -cap <= r < 0   MISS; insert at bucket (- -1 r), the first empty or
;;;                   tombstone slot on the chain.
;;;   r < -cap        FULL: every bucket probed without an empty slot. Only
;;;                   possible if the grow policy is violated; an
;;;                   internal-error safety net.
;;;
;;; The literals -1 and -2 in the typed bodies below ARE $LHT-EMPTY and
;;; $LHT-TOMBSTONE. A typed body cannot read a global, so the values are
;;; repeated here and pinned by the test LHT-SENTINELS-MATCH-TYPED-LITERALS.

(defun-typed (lht-index int64) ((h int64) (cap int64))
  (logand h (- cap 1)))

(defun-typed (lht-probe int64)
    ((buckets (array int64)) (keys boxed) (cap int64) (key boxed)
     (start int64) (i int64) (tomb int64))
  (if (>= i cap)
      (- -1 cap)
      (let ((idx (lht-index (+ start i) cap)))
        (let ((b (fetch buckets idx)))
          (if (= b -1)
              (if (>= tomb 0) (- -1 tomb) (- -1 idx))
              (if (= b -2)
                  (lht-probe buckets keys cap key start (+ i 1)
                             (if (>= tomb 0) tomb idx))
                  (if (equal (fetch keys b) key)
                      idx
                      (lht-probe buckets keys cap key start (+ i 1) tomb))))))))

(defun lht-find (ht key)
  (let* ((cap (lht--capacity ht))
         (h (lht-hash key))
         (start (lht-index h cap)))
    (lht-probe (lht--buckets ht) (lht--keys ht) cap key start 0 -1)))

;;; ---- growth / rehashing ---------------------------------------------------

(defun lht-should-grow-p (ht)
  "Gate growth on NEXT-SLOT, not COUNT+TOMBSTONES: LHT-PUT! always claims a
fresh payload slot on a miss (pidx = NEXT-SLOT), even when the bucket it
claims was a tombstone -- reusing a tombstoned bucket index does not reuse
its old payload index. So NEXT-SLOT is a strict upper bound on
COUNT+TOMBSTONES that keeps climbing under insert/delete churn even while
COUNT+TOMBSTONES itself stays flat, and it is NEXT-SLOT -- not
COUNT+TOMBSTONES -- that indexes the KEYS/VALS payload arrays (each sized
CAPACITY). Gating on the wrong one lets NEXT-SLOT walk past CAPACITY under
churn with no growth ever triggering, and STORE then faults out of bounds."
  (>= (* (lht--next-slot ht) $lht-grow-num)
      (* (lht--capacity ht) $lht-grow-den)))

(defun lht-next-capacity (cap count)
  "Smallest power-of-two capacity, doubling from CAP, giving COUNT+1 live
entries headroom under the 0.7 load factor with zero tombstones."
  (if (< (* (+ count 1) $lht-grow-num) (* cap $lht-grow-den))
      cap
      (lht-next-capacity (* cap 2) count)))

;; Linear-probe from START for an EMPTY (-1, see the probing section) slot in
;; a table known to hold no duplicate keys yet -- used only during rehash --
;; and claim it for PAYLOAD-IDX. Typed and compiled for the same reason as
;; LHT-PROBE: it runs once per live entry per rehash.
(defun-typed (lht-insert-empty! int64)
    ((buckets (array int64)) (cap int64) (start int64) (i int64)
     (payload-idx int64))
  (let ((idx (lht-index (+ start i) cap)))
    (if (= (fetch buckets idx) -1)
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
  "Rehash HT in place, compacting away tombstones -- capacity grows only if
the live count needs it (LHT-NEXT-CAPACITY starts its doubling search from
the current capacity), so a tombstone-heavy table under insert/delete churn
recompacts at its existing size instead of growing unboundedly."
  (let* ((old-buckets (lht--buckets ht))
         (old-keys (lht--keys ht))
         (old-vals (lht--vals ht))
         (old-cap (lht--capacity ht))
         (new-cap (lht-next-capacity old-cap (lht-count ht)))
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
    (if (>= r 0) (fetch (lht--vals ht) (fetch (lht--buckets ht) r)) nil)))

(defun lht-has-key-p (ht key)
  (>= (lht-find ht key) 0))

(defun lht-put! (ht key val)
  "Insert or replace KEY -> VAL in HT; returns T (mirrors native SETHASH)."
  (if (lht-should-grow-p ht) (lht-grow! ht) nil)
  (let* ((cap (lht--capacity ht))
         (r (lht-find ht key)))
    (cond
      ((>= r 0)
       (store (lht--vals ht) (fetch (lht--buckets ht) r) val)
       t)
      ((>= r (- cap))
       (let* ((bidx (- -1 r))
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
    (if (>= r 0)
        (progn
          (store (lht--buckets ht) r $lht-tombstone)
          (store ht 5 (- (lht-count ht) 1))
          (store ht 6 (+ (lht--tombstones ht) 1))
          t)
        t)))

(declare-type! 'lht-has-key-p '(forall (a b) (-> (a b) bool)))
(declare-type! 'lht-put! '(forall (a b c) (-> (a b c) bool)))
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

;;; ---- MAP: a portable wrapper over native-or-LHT --------------------------
;;;
;;; Not every host provides the native HASH-TABLE builtin -- it's a
;;; convenience layer (`lib/15-sets-hash.lisp`) over `LispVal::HashTable`,
;;; not one of KERNEL.md Part XI's minimal required primitives -- which is
;;; the whole reason LHT exists (issue #458). MAKE-MAP/MAP-GET/etc. below
;;; let calling code stay agnostic to which backend a given host (and hence
;;; a given table object) actually has, without paying for it: this is
;;; deliberately NOT built on `lib/29-protocols.lisp`'s DEFPROTOCOL/
;;; DEFINSTANCE machinery (a per-call type-key computation plus a
;;; HASH-TABLE-lookup plus an ASSOC over the instance list) -- LHT already
;;; earns back real overhead by using HASH-CODE (#474) directly, and
;;; layering a general dispatch registry back on top of every MAP-GET/
;;; MAP-PUT! call would give a meaningful fraction of that back for no
;;; reason. A protocol dispatch would also not even discriminate LHT from
;;; any other value here: `$protocol-type-key` keys on `(record-brand v)`
;;; else structural kind, and LHT is deliberately a bare ARRAY (see this
;;; file's header), not a DEFRECORD, so it has no brand to dispatch on --
;;; giving it one would mean depending on the condensation layer this file
;;; otherwise avoids on principle. Below is instead one HASH-TABLE-P
;;; branch per call: on any host, this compiles to (and should stay) the
;;; cheapest possible dispatch -- one comparison, one conditional jump.
;;;
;;; MAKE-MAP alone decides which *backend* a fresh table uses (once, via
;;; BOUNDP, at construction); MAP-GET/MAP-PUT!/etc. below instead re-check
;;; the concrete TABLE argument's own kind on every call, so they work
;;; uniformly on a table built by MAKE-HASH-TABLE or MAKE-LHT directly, not
;;; only one built by MAKE-MAP.
;;;
;;; MAP-GET/MAP-PUT!/etc. are DEFMACROs, not DEFUNs: a DEFUN wrapper adds a
;;; full extra interpreted call frame on top of the HASH-TABLE-P branch and
;;; the winning op's own call -- measured at ~1.4x the direct-call cost on
;;; this host, not egregious but not the "one comparison, one jump" this
;;; section promises either. Since #460 (main, cached per compiled call
;;; site), a macro call's expansion is only run once per call SITE, not
;;; once per call: after the first hit, evaluating the site re-executes the
;;; already-expanded `(if (hash-table-p ...) ...)` directly, with no
;;; wrapper frame at all -- including every iteration of a loop whose body
;;; contains the call, exactly like every mainstream Lisp's DEFMACRO.
;;; Each macro below LET-binds its table argument once (a GENSYM'd temp,
;;; following this file's own `lib/12-control.lisp` DOTIMES precedent) --
;;; required because the expansion references it twice (once in the
;;; HASH-TABLE-P test, once in the taken branch), and naive substitution
;;; would silently double-evaluate a table argument with side effects (e.g.
;;; `(map-get (side-effecting-lookup) k)`). The key/value/fn arguments need
;;; no such protection: each appears once per branch, and the branches are
;;; mutually exclusive, so exactly one evaluation happens at runtime
;;; regardless of the textual duplication across the expansion's two arms.

(defun map-p (x)
  "True if X is a table this MAP-* API accepts -- either backend."
  (or (hash-table-p x) (lht-p x)))

(declare-type! 'map-p '(forall (a) (-> (a) bool)))

(defun make-map ()
  "Construct a table using the fastest backend this host provides: the
native HASH-TABLE builtin when bound, else the pure-Lamedh LHT."
  (if (boundp 'make-hash-table) (make-hash-table) (make-lht)))

(defmacro map-get (m k)
  "Portable GETHASH/LHT-GET, dispatched by M's own kind."
  (let ((tmp (gensym)))
    (list 'let (list (list tmp m))
          (list 'if (list 'hash-table-p tmp)
                (list 'gethash tmp k)
                (list 'lht-get tmp k)))))

(defmacro map-put! (m k v)
  "Portable SETHASH/LHT-PUT!, dispatched by M's own kind."
  (let ((tmp (gensym)))
    (list 'let (list (list tmp m))
          (list 'if (list 'hash-table-p tmp)
                (list 'sethash tmp k v)
                (list 'lht-put! tmp k v)))))

(defmacro map-remove! (m k)
  "Portable REMHASH/LHT-REMOVE!, dispatched by M's own kind."
  (let ((tmp (gensym)))
    (list 'let (list (list tmp m))
          (list 'if (list 'hash-table-p tmp)
                (list 'remhash tmp k)
                (list 'lht-remove! tmp k)))))

(defmacro map-has-key-p (m k)
  "Portable HAS-KEY-P/LHT-HAS-KEY-P, dispatched by M's own kind."
  (let ((tmp (gensym)))
    (list 'let (list (list tmp m))
          (list 'if (list 'hash-table-p tmp)
                (list 'has-key-p tmp k)
                (list 'lht-has-key-p tmp k)))))

(defmacro map-count (m)
  "Portable HASH-TABLE-COUNT*/LHT-COUNT, dispatched by M's own kind."
  (let ((tmp (gensym)))
    (list 'let (list (list tmp m))
          (list 'if (list 'hash-table-p tmp)
                (list 'hash-table-count* tmp)
                (list 'lht-count tmp)))))

(defmacro map-keys (m)
  "Portable KEYS/LHT-KEYS, dispatched by M's own kind."
  (let ((tmp (gensym)))
    (list 'let (list (list tmp m))
          (list 'if (list 'hash-table-p tmp)
                (list 'keys tmp)
                (list 'lht-keys tmp)))))

(defmacro map-each (m fn)
  "Portable MAPHASH/LHT-EACH, dispatched by M's own kind. Calls (FN key
value) for each entry; returns NIL."
  (let ((tmp (gensym)))
    (list 'let (list (list tmp m))
          (list 'if (list 'hash-table-p tmp)
                (list 'maphash tmp fn)
                (list 'lht-each tmp fn)))))

(defmacro map->alist (m)
  "Portable HASH->ALIST/LHT->ALIST, dispatched by M's own kind."
  (let ((tmp (gensym)))
    (list 'let (list (list tmp m))
          (list 'if (list 'hash-table-p tmp)
                (list 'hash->alist tmp)
                (list 'lht->alist tmp)))))
