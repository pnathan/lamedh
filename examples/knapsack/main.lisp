;;; knapsack -- 0/1 knapsack by dynamic programming over capacities.
;;; Shows: records for the items (defrecord), a DP array indexed by
;;; capacity, and reduce over items updating state.
;;; Run: cargo run -- examples/knapsack/main.lisp

(defrecord item (name symbol) (weight int64) (value int64))

(def $items
  (list (make-item 'map 9 150) (make-item 'compass 13 35)
        (make-item 'water 153 200) (make-item 'sandwich 50 160)
        (make-item 'glucose 15 60) (make-item 'tin 68 45)
        (make-item 'banana 27 60) (make-item 'apple 39 40)
        (make-item 'cheese 23 30) (make-item 'beer 52 10)
        (make-item 'camera 32 30) (make-item 'towel 18 12)))

(defun knapsack (items capacity)
  "Best total value within CAPACITY (0/1 per item)."
  (let ((best (array (1+ capacity))))
    (dotimes (c (1+ capacity)) (put! best c 0))
    (for-each items
      (lambda (it)
        (let ((w (item-weight it)) (v (item-value it))
              (c capacity))
          (while (>= c w)
            (put! best c (max (ref best c) (+ v (ref best (- c w)))))
            (setq c (- c 1))))))
    (ref best capacity)))

(def $best (knapsack $items 400))
(format t "best value at capacity 400: ~a~%" $best)

;; Independent oracle: exhaustive search over all 2^12 subsets.
(defun subsets-best (items cap)
  (if (null items)
      0
      (let ((skip (subsets-best (cdr items) cap))
            (w (item-weight (car items))))
        (if (<= w cap)
            (max skip (+ (item-value (car items))
                         (subsets-best (cdr items) (- cap w))))
            skip))))

;; self-check: DP agrees with brute force (780 for this instance).
(if (and (= $best 780)
         (= $best (subsets-best $items 400))
         (= (knapsack $items 0) 0)
         (= (knapsack (list (make-item 'x 5 10)) 4) 0))
    (print 'ok)
    (error "knapsack self-check failed"))
