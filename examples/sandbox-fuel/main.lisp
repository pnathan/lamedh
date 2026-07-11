;;; sandbox-fuel -- run untrusted code on a budget you set.
;;; Shows: with-fuel halting a runaway loop, fuel budgets = kernel steps
;;; (the one-ruler identity), with-capabilities attenuation, and spawn
;;; fanning work to a share-nothing child under both limits.
;;; Run: cargo run -- examples/sandbox-fuel/main.lisp

(defun spin (n)
  "Busy loop -- the stand-in for untrusted code."
  (if (= n 0) 'done (spin (- n 1))))

;; Enough fuel: completes.
(def $fine (with-fuel 1000000 (spin 1000)))
(format t "with fuel:    ~a~%" $fine)

;; Too little fuel: halted, not hung. errorset sees the fuel error.
(def $halted (errorset '(with-fuel 500 (spin 1000000))))
(format t "out of fuel:  ~a~%" (if (null $halted) 'halted 'escaped!))

;; The ruler is real: fuel budgets and step-count share one unit.
;; (The tested identity: 10x the measured steps runs, half of them dies.)
(def $cost (car (step-count (spin 100))))
(def $tight (errorset (list 'with-fuel (max 1 (/ $cost 2)) '(spin 100))))
(def $loose (errorset (list 'with-fuel (* $cost 10) '(spin 100))))
(format t "spin(100) costs ~a steps; half halts: ~a, 10x runs: ~a~%"
        $cost (null $tight) (not (null $loose)))

;; Capability attenuation composes with fuel.
(def $refused
  (with-capabilities ()
    (errorset '(read-file "README.md"))))

;; A spawned child: fresh interpreter, granted nothing, fueled.
(def $child (spawn (:capabilities () :fuel 1000000) (* 6 7)))
(def $child-value (await $child))
(format t "spawned child says ~a~%" $child-value)

;; self-check: completion, halting, the +-5 identity, refusal, child.
(if (and (equal $fine 'done)
         (null $halted)
         (null $tight)
         (equal $loose (list 'done))
         (null $refused)
         (= $child-value 42))
    (print 'ok)
    (error "sandbox-fuel self-check failed"))
