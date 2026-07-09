;;; npcs.lisp -- three kinds of NPC, one shared vocabulary, specialized voices.
;;;
;;; This example exercises the whole condensation stack at once:
;;;
;;;   - DEFRECORD seeds with row-typed fields (one form generates the
;;;     constructor, predicate, accessors, validator, equality, and lens --
;;;     each carrying a DECLARED row scheme the checker enforces at call
;;;     sites);
;;;   - a SHARED method, written exactly once, row-polymorphically: DAMAGE
;;;     works on "anything with an int64 hp", and the checker proves it;
;;;   - a SPECIALIZED method: each kind has its own GREET, dispatched by
;;;     METHOD (one deterministic name computation, no dispatch table);
;;;   - a DEFINTERFACE method set, with IMPLEMENTS! verifying each kind's
;;;     methods against the declared signatures by row unification.
;;;
;;; Run it:   cargo run -- -i examples/npcs.lisp
;;; Check it: cargo run -- --capability READ-FS -s '(check-file! "examples/npcs.lisp")'

;;; ---- the kinds --------------------------------------------------------
;; All NPC kinds share two fields, (name string) (hp int64) -- the shared
;; vocabulary. Each DEFRECORD defines a BRANDED record type (#308): nominal
;; in the checker (a goblin is never a merchant), row-subsumable at any
;; function asking only for some of its fields, accessed by NAME (not
;; position) through RECORD-REF.

(defrecord goblin
  (name string) (hp int64) (mischief int64)
  (:invariant (>= hp 0))
  (:derive equality lens))

(defrecord merchant
  (name string) (hp int64) (gold int64)
  (:invariant (and (>= hp 0) (>= gold 0)))
  (:derive equality lens))

(defrecord wisp
  (name string) (hp int64) (glow float64)
  (:invariant (>= hp 0))
  (:derive equality lens))

;;; ---- the shared vocabulary, as rows ------------------------------------
;; One accessor pair serves every kind. No axioms: RECORD-REF is a checker-
;; native primitive, so the row schemes below are INFERRED and PROVED --
;; "any record with a name / an hp, whatever else it carries."
;;
;;   (see-type 'npc-name)
;;   ; => (CHECKED (FORALL (A B) (-> ((RECORD ((NAME A)) B)) A)))

(defun npc-name (self) (record-ref self 'name))
(defun npc-hp (self) (record-ref self 'hp))

;;; ---- shared behavior: written once, typed for every kind ----------------
;; No annotations below -- the row schemes are INFERRED:
;;
;;   (see-type 'alive-p)
;;   ; => (CHECKED (FORALL (A) (-> ((RECORD ((HP INT64)) A)) BOOL)))
;;
;; ALIVE-P and HIT-POINTS-AFTER are one implementation each, shared by
;; goblins, merchants, wisps, and any future kind with an hp field.

(defun alive-p (npc)
  "True while NPC still stands. Works for any record with an int64 hp."
  (> (npc-hp npc) 0))

(defun hit-points-after (npc amount)
  "NPC's hp after taking AMOUNT damage, floored at zero."
  (let ((left (- (npc-hp npc) amount)))
    (if (> left 0) left 0)))

;;; ---- the method set ------------------------------------------------------
;; GREET is specialized per kind (each has a voice); DAMAGE was once
;; specialized in its rebuild step (each kind's lens), but RECORD-WITH is
;; brand-preserving typed update, so the per-kind bodies collapsed into one
;; shared shape. A method is an ordinary TYPE-OP function -- it type-checks,
;; edits, and traces like any other definition.

(definterface npc
  (:ops ((greet (-> (self) string))
         (damage (-> (self int64) self)))))

(defun goblin-greet (self)
  (concat (npc-name self) " sharpens a rusty dagger and cackles."))

(defun merchant-greet (self)
  (concat (npc-name self) " beams: fine wares, fair prices!"))

(defun wisp-greet (self)
  (concat (npc-name self) " flickers softly in the gloom."))

(defun goblin-damage (self amount)
  (record-with self 'hp (hit-points-after self amount)))

(defun merchant-damage (self amount)
  (record-with self 'hp (hit-points-after self amount)))

(defun wisp-damage (self amount)
  (record-with self 'hp (hit-points-after self amount)))

;; Verify the claims: each op's declared signature is unified against the
;; checker's verdict for each kind's method (rows and all). MISSING or
;; MISMATCH would error the load, right here.
(implements! 'goblin 'npc)
(implements! 'merchant 'npc)
(implements! 'wisp 'npc)

;;; ---- a scene --------------------------------------------------------------

(def *party*
  (list (make-goblin "Grix" 7 9)
        (make-merchant "Oleander" 12 240)
        (make-wisp "Sel" 3 0.8)))

(defun taunt-all (npcs)
  "Every NPC greets in its own voice -- one call site, three voices."
  (mapcar (lambda (n) (method 'greet n)) npcs))

(defun scuffle (npc amount)
  "Shared damage logic through each kind's own rebuild."
  (method 'damage npc amount))

(print (taunt-all *party*))
(print (mapcar (lambda (n) (npc-hp (scuffle n 5))) *party*))
(print (mapcar #'alive-p (mapcar (lambda (n) (scuffle n 5)) *party*)))

;; What the checker knows, without a single annotation in this file's logic:
;;
;;   (check-type (defun weaken (n) (scuffle n (npc-hp n))))
;;     -- fine: anything with hp can be weakened by its own hp.
;;
;;   (goblin-mischief (make-merchant "Oleander" 12 240))
;;     ; => TYPE-ERROR: cannot unify MERCHANT with GOBLIN
;;     -- cross-kind misuse is caught statically and NOMINALLY (brands,
;;        not just shapes), and EDIT! refuses (and rolls back) any edit
;;        that would introduce it.

'npcs-loaded
