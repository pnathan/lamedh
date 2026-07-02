;;; npcs.lisp -- polymorphic NPCs without typeclasses, dictionaries, or
;;; dispatch tables.
;;;
;;; Run it:   cargo run -- -i examples/game/npcs.lisp
;;; Read it:  docs/npc_polymorphism.md walks through this file with the
;;;           transcript it prints.
;;;
;;; Three layers cooperate, and each is ordinary data:
;;;   - defconcept   (lib/20-condensation.lisp): one seed per NPC kind
;;;     generates constructor, predicate, accessors, validator -- and
;;;     declares row schemes so the checker knows the fields.
;;;   - definterface (lib/21-interfaces.lisp): the shared method set. A
;;;     method is the ordinary function <TYPE>-<OP>; satisfaction is
;;;     structural (Go), assertion is explicit (implements!, Rust).
;;;   - rows (the checker): a function written over accessors infers
;;;     "any record with these fields", so one definition is *proven*
;;;     shared across every NPC kind that carries the fields.
;;;
;;; Layout convention: every NPC kind lists the shared fields first, in
;;; the same order -- (name string) (hp int64) -- then its own fields.
;;; Accessors are positional at runtime while their declared row schemes
;;; are by-name, so the convention is what keeps the two in agreement.

;;; ---- two kinds of NPC ------------------------------------------------------

(defconcept goblin
  (:fields ((name string) (hp int64) (ferocity int64)))
  (:invariant (>= hp 0))
  (:derive equality))

(defconcept merchant
  (:fields ((name string) (hp int64) (gold int64)))
  (:invariant (and (>= hp 0) (>= gold 0)))
  (:derive equality))

;;; ---- the shared method set -------------------------------------------------
;;;
;;; NAME and HP are satisfied for free: the generated accessors GOBLIN-NAME,
;;; MERCHANT-HP, ... already follow the <TYPE>-<OP> convention. GREET is the
;;; op every kind must specialize by hand.

(definterface npc
  (:ops ((name  (-> (self) string))
         (hp    (-> (self) int64))
         (greet (-> (self) string)))))

;; The specialized method: same op name, one ordinary function per kind.
(defun goblin-greet (self)
  "A goblin's greeting is a threat."
  (concat "Grr. " (goblin-name self) " waves a rusty knife."))

(defun merchant-greet (self)
  "A merchant's greeting is a pitch."
  (concat "Welcome! " (merchant-name self) " opens a pack of "
          (princ-to-string (merchant-gold self)) " gold worth of wares."))

;; Check now, record the claim, error loudly if an op is missing or its
;; checker verdict contradicts the declared signature.
(implements! 'goblin 'npc)
(implements! 'merchant 'npc)

;;; ---- shared methods --------------------------------------------------------
;;;
;;; Flavor 1: row-typed. NPC-HP is one line over a concept accessor; the
;;; checker generalizes it to (forall (a) (-> ((record ((hp int64)) a)) int64))
;;; -- "any record with an int64 hp" -- so WOUNDED-P below is *statically
;;; proven* to apply to goblins, merchants, and every future kind that keeps
;;; the shared fields. No annotation anywhere.

(defun npc-hp (n)
  "HP of any NPC: any record with an int64 HP field."
  (goblin-hp n))

(defun wounded-p (n)
  "T when an NPC is close to death. One definition, row-typed, all kinds."
  (< (npc-hp n) 3))

;;; Flavor 2: late-bound. INTRODUCE is written once and calls the *specialized*
;;; GREET through METHOD -- one deterministic name computation from the value's
;;; concept tag, not a dispatch table. The trade: it works for every kind that
;;; implements NPC including ones defined after it, but the checker cannot see
;;; through METHOD, so its verdict is honestly unproven.

(defun introduce (n)
  "Introduce any NPC: shared skeleton, specialized greeting."
  (concat (method 'name n)
          " [" (princ-to-string (method 'hp n)) " hp]: "
          (method 'greet n)))

;;; ---- a kind that fails the contract ----------------------------------------
;;;
;;; A training dummy has the shared fields (so NAME and HP conform for free)
;;; but nobody taught it to speak: GREET is MISSING and the structural check
;;; fails. IMPLEMENTS! would error here; IMPLEMENTS? just reports.

(defconcept training-dummy
  (:fields ((name string) (hp int64))))

;;; ---- transcript --------------------------------------------------------------

(print (method 'greet (make-goblin "Snag" 7 3)))
(print (method 'greet (make-merchant "Oren" 12 250)))

(print (introduce (make-goblin "Snag" 2 3)))
(print (introduce (make-merchant "Oren" 12 250)))

(print (wounded-p (make-goblin "Snag" 2 3)))     ; T   -- 2 hp
(print (wounded-p (make-merchant "Oren" 12 250))) ; NIL -- 12 hp

;; The checker's view of the shared projection: a row scheme, inferred.
(print (see-type 'npc-hp))

;; Cross-concept misuse is a static type error, not a runtime surprise.
(defun rob () (merchant-gold (make-goblin "Snag" 7 3)))
(print (see-type 'rob))

;; Conformance is graded, honestly: T with per-op evidence for merchant,
;; a MISSING greet for the dummy.
(print (car (implements? 'merchant 'npc)))
(print (implements? 'training-dummy 'npc))

;; Invariants came with the seed: a goblin cannot have negative hp.
(print (validate-goblin (make-goblin "Snag" 7 3)))
(print (validate-goblin (make-goblin "Snag" -1 3)))
