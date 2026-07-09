;;; oo-patterns.lisp — classic OO design patterns, without classes.
;;;
;;; The thesis: many Gang-of-Four patterns are workarounds for something a
;;; NOMINAL type system (Java, C#, C++) cannot say — "any value that has
;;; these members." Lamedh's ROW types say exactly that, and the checker
;;; proves it. So several patterns stop being ceremony and become one line.
;;;
;;; The one mechanism behind everything below:
;;;
;;;   (defun field-of (self) (record-ref self 'field))
;;;
;;; RECORD-REF is a checker-native primitive, so the row scheme
;;; (forall (a r) (-> ((record ((field a)) r)) a)) is INFERRED and PROVED —
;;; "field, of any record that has it, and whatever else." No axioms
;;; anywhere in this file: every contract below is derived from the code.
;;;
;;;   Run:   cargo run -- -i examples/oo-patterns.lisp
;;;   Check: each section prints its own (check-type ...) verdicts.

;;; ======================================================================
;;; Foundation — Duck Typing, statically checked
;;; ======================================================================
;;; "If it has an hp field, it can be wounded." No base class, no interface
;;; declaration — just name the field you need. Every concept below leads
;;; with a shared vocabulary so one accessor serves all of them.
;;;
;;; Our worked object, one seed form. DEFCONCEPT generates make-npc, npc-p,
;;; the four accessors, a validator for the :invariant, and (from :derive)
;;; equality + a lens. Note LOOT is a compound field, (list string) — a
;;; whole type flows into the row language unchanged.

(defconcept npc
  (:fields ((name string)
            (hp int64)
            (affiliation string)
            (loot (list string))))
  (:invariant (>= hp 0))
  (:derive equality lens))

;; Generic row accessors: one RECORD-REF each. Access is by NAME on the
;; value's own brand — no positions, no axioms, schemes inferred.
(defun the-name (self) (record-ref self 'name))
(defun the-hp (self) (record-ref self 'hp))
(defun the-affiliation (self) (record-ref self 'affiliation))
(defun the-loot (self) (record-ref self 'loot))

;; Behavior over the shared vocabulary — every row below is INFERRED, and
;; each is one implementation shared by npcs and any future kind with the
;; named field. No annotations in the logic.
(defun wounded-p (thing) (< (the-hp thing) 5))       ; (record ((hp int64)) A) -> bool
(defun carrying-p (thing) (not (null (the-loot thing))))
(defun allied-p (a b) (equal (the-affiliation a) (the-affiliation b)))

(print (list 'wounded-p-inferred (see-type 'wounded-p)))
;; => (CHECKED (FORALL (A) (-> ((RECORD ((HP INT64)) A)) BOOL)))
(print (list 'carrying-p-inferred (see-type 'carrying-p)))
;; The compound loot field rides into the row unchanged:
;; => (... (RECORD ((LOOT (LIST STRING))) A)) BOOL)
(print (list 'allied-p-inferred (see-type 'allied-p)))
;; Two INDEPENDENT rows (A and B): the arguments need not be the same kind,
;; only both affiliation-bearing — the row-vs-nominal distinction in one
;; signature. A nominal `boolean allied(Affiliated, Affiliated)` forces both
;; into one hierarchy; the row asks each for the field on its own.

(def *grix* (make-npc "Grix" 7 "Redfang" (list "dagger" "3 gold")))
(print (list 'npc (the-name *grix*) (the-hp *grix*)
             (the-affiliation *grix*) (the-loot *grix*)
             (wounded-p *grix*) (carrying-p *grix*)))

;;; ======================================================================
;;; Pattern 1 — STRATEGY
;;; ======================================================================
;;; GoF: encapsulate interchangeable algorithms behind a common interface,
;;; select at runtime. In a Lisp-1 a strategy is just a function value; the
;;; row angle is that the CONTEXT it operates on is duck-typed. One PRICE
;;; function, any pricing strategy, any record that carries a base amount.

(defun the-amount (self) (record-ref self 'amount))

(defconcept order (:fields ((amount int64) (qty int64))))

(defun full-price (o) (the-amount o))
(defun half-price (o) (/ (the-amount o) 2))
(defun bulk-price (o) (if (> (the-amount o) 100) (- (the-amount o) 20) (the-amount o)))

(defun checkout (o strategy)
  "The Context: it holds the data, the Strategy varies the algorithm.
STRATEGY is any function from an amount-bearing record to an int."
  (funcall strategy o))

(print (list 'strategy
             (checkout (make-order 200 3) #'full-price)
             (checkout (make-order 200 3) #'half-price)
             (checkout (make-order 200 3) #'bulk-price)))
;; => (STRATEGY 200 100 180)

;;; ======================================================================
;;; Pattern 2 — COMPOSITE + interface method dispatch
;;; ======================================================================
;;; GoF: treat individual objects and compositions uniformly through a
;;; shared interface. Here a Shape interface with AREA; a leaf DISC and a
;;; composite GROUP that holds children. METHOD dispatches on the value's
;;; own brand — one call site, both node kinds, recursively.

(definterface shape
  (:ops ((area (-> (self) int64)))))

(defconcept disc   (:fields ((r int64))))
(defconcept group  (:fields ((kids int64))))   ; kids held out-of-band below

;; A leaf computes directly.
(defun disc-area (self)
  (let ((r (record-ref self 'r))) (* 3 (* r r))))  ; ~pi r^2

;; A composite sums its children — the SAME (method 'area ...) call works on
;; a disc or a nested group, so the recursion is uniform.
(def *group-children* (make-hash-table))
(defun group-of (children)
  (let ((g (make-group (length children))))
    (sethash *group-children* g children)
    g))
(defun group-area (self)
  (apply #'+ (mapcar (lambda (k) (method 'area k)) (gethash *group-children* self))))

(implements! 'disc 'shape)
(implements! 'group 'shape)

(def *scene*
  (group-of (list (make-disc 2)
                  (make-disc 3)
                  (group-of (list (make-disc 1) (make-disc 1))))))

(print (list 'composite-total-area (method 'area *scene*)))
;; disc 2 -> 12, disc 3 -> 27, inner group (1->3, 1->3) -> 6 ; total 45

;;; ======================================================================
;;; Pattern 3 — DECORATOR
;;; ======================================================================
;;; GoF: attach responsibilities to an object dynamically, keeping the same
;;; interface, by wrapping. A beverage has a COST and a TAG; each decorator
;;; wraps a beverage and still IS a beverage (same row), so decorators stack.

(defun the-cost (self) (record-ref self 'cost))

(defconcept espresso (:fields ((cost int64) (tag string))))
(defconcept milk     (:fields ((cost int64) (tag string))))
(defconcept sugar    (:fields ((cost int64) (tag string))))

;; Decorators keep their wrapped beverage out-of-band and add to the cost.
(def *wrapped* (make-hash-table))
(defun with-milk (bev)
  (let ((d (make-milk (+ (the-cost bev) 2) "milk"))) (sethash *wrapped* d bev) d))
(defun with-sugar (bev)
  (let ((d (make-sugar (+ (the-cost bev) 1) "sugar"))) (sethash *wrapped* d bev) d))

;; total-cost works on ANY record with a cost field — espresso or decorator.
;; The row (record ((cost int64)) r) is the "beverage" contract, inferred.
(defun total-cost (bev) (the-cost bev))
(print (list 'total-cost-inferred (see-type 'total-cost)))

(def *drink* (with-sugar (with-milk (make-espresso 10 "espresso"))))
(print (list 'decorator-cost (total-cost *drink*)))   ; 10 + 2 + 1 = 13

;;; ======================================================================
;;; Pattern 4 — OBSERVER
;;; ======================================================================
;;; GoF: a subject notifies a set of observers of state changes. The row
;;; contract is the Observer method set; NOTIFY-ALL is one call site over a
;;; heterogeneous list, each observer reacting in its own voice.

(definterface observer
  (:ops ((notify (-> (self int64) string))))) ; receives an event code

(defconcept logger  (:fields ((tag string))))
(defconcept counter (:fields ((seen int64))))

(defun logger-notify (self code)
  (concat "[" (record-ref self 'tag) "] saw event " (number->string code)))
(defun counter-notify (self code)
  (concat "count+1 on event " (number->string code)))

(implements! 'logger 'observer)
(implements! 'counter 'observer)

(defun notify-all (observers code)
  "One subject, many observers, each reacting via METHOD dispatch."
  (mapcar (lambda (o) (method 'notify o code)) observers))

(print (list 'observer
             (notify-all (list (make-logger "audit") (make-counter 0)) 7)))

;;; ======================================================================
;;; Pattern 5 — STATE (state as data)
;;; ======================================================================
;;; GoF: an object alters behavior when its internal state changes,
;;; delegating to state objects. Here each STATE is a value; NEXT is the
;;; transition. A traffic light is data + a pure transition function.

(defconcept light (:fields ((color string) (go-p int64))))

(def *red*    (make-light "red" 0))
(def *green*  (make-light "green" 1))
(def *yellow* (make-light "yellow" 1))

(defun the-color (self) (record-ref self 'color))

(defun next-light (s)
  "Transition delegated to state — no giant conditional on a status flag."
  (let ((c (the-color s)))
    (cond ((equal c "red") *green*)
          ((equal c "green") *yellow*)
          (t *red*))))

(defun run-lights (s n)
  (if (< n 1) (list (the-color s))
      (cons (the-color s) (run-lights (next-light s) (- n 1)))))

(print (list 'state (run-lights *red* 4)))
;; => (STATE ("red" "green" "yellow" "red" "green"))

;;; ======================================================================
;;; The payoff — the checker catches cross-kind misuse statically
;;; ======================================================================
;;; A disc has no cost; asking for its beverage cost is a compile-time row
;;; error, not a runtime surprise. This is the safety class-based OO gets
;;; from nominal typing — here it falls out of structure alone.

(print (list 'cross-kind-guard
             (check-type (the-cost (make-disc 2)))))
;; => "type error: in call to `THE-COST`: struct DISC has no field cost"
;;
;; A disc is a BRANDED record with one field, r. THE-COST demands a record
;; with a cost field — and the demand travels through the helper's INFERRED
;; row scheme, no axiom anywhere. The checker rejects the call: the same
;; safety a class-based language gets from nominal typing, here from
;; structure plus brands. In a dynamically-typed OO language this would be
;; an AttributeError at runtime; here it never runs.

'oo-patterns-loaded
