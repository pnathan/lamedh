;;; ai.lisp -- entity behaviour for the Lamedh game demo.
;;;
;;; This script is loaded by examples/game_demo.rs. It knows nothing about Rust;
;;; it only uses the host primitives the engine registered:
;;;
;;;   (spawn-entity name x y hp atk)  (entity-name e)   (entity-x e)
;;;   (entity-y e)   (entity-hp e)    (entity-attack e) (entity-alive? e)
;;;   (move-entity! e dx dy)          (damage! e n)     (heal! e n)
;;;   (game-log ...)
;;;
;;; ...plus everything in the standard library (defun, let, cond, abs, max, ...).

;; The reader only has single-character operators (< > = + - * /), so there is
;; no `<=`; we express "<= 1" as "not (> ... 1)" below.

(defun sign (n)
  "Return -1, 0, or 1 with the same sign as N."
  (cond ((> n 0) 1)
        ((< n 0) -1)
        (t 0)))

(defun chebyshev (a b)
  "King-move distance between entities A and B.
   Two creatures are adjacent (can strike) when this is 1."
  (max (abs (- (entity-x a) (entity-x b)))
       (abs (- (entity-y a) (entity-y b)))))

;; --- Faster variants of the same distance ------------------------------------
;; The stdlib `max` is `&rest` AND recurses through `(apply #'max ...)`, and
;; `abs` calls `minusp` (another defun). Fixed-arity helpers built straight from
;; builtins do the identical math with far less interpreter overhead.
(defun abs1 (x)
  "Single-argument abs with no helper calls."
  (if (< x 0) (- 0 x) x))

(defun max2 (a b)
  "Two-argument max with no &rest / apply."
  (if (> a b) a b))

(defun chebyshev-fast (a b)
  "Same result as CHEBYSHEV, using the fixed-arity helpers."
  (max2 (abs1 (- (entity-x a) (entity-x b)))
        (abs1 (- (entity-y a) (entity-y b)))))

(defun adjacent? (a b)
  (not (> (chebyshev a b) 1)))

(defun step-toward (self target)
  "Move SELF one cell (8-directional) toward TARGET."
  (let ((dx (sign (- (entity-x target) (entity-x self))))
        (dy (sign (- (entity-y target) (entity-y self)))))
    (progn
      (move-entity! self dx dy)
      (game-log (entity-name self) "advances to"
                (entity-x self) (entity-y self)))))

(defun strike (self target)
  "SELF attacks TARGET for its attack rating."
  (let ((dmg (entity-attack self)))
    (progn
      (damage! target dmg)
      (game-log (entity-name self) "strikes" (entity-name target)
                "for" dmg "damage  (" (entity-name target)
                "now at" (entity-hp target) "hp )"))))

(defun take-turn (self target)
  "One turn of behaviour: if next to the target, attack; otherwise close in.
   This is the single entry point the Rust engine calls each round."
  (cond
    ((not (entity-alive? self)) nil)
    ((not (entity-alive? target)) nil)
    ((adjacent? self target) (strike self target))
    (t (step-toward self target))))
