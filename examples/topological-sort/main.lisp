;;; topological-sort -- Kahn's algorithm over a build-dependency graph.
;;; Shows: in-degree bookkeeping in a hash, worklist iteration, cycle
;;; detection as the leftover, and an order-validity self-check.
;;; Run: cargo run -- examples/topological-sort/main.lisp

(def $deps
  ;; (target . prerequisites) -- a small build graph.
  '((binary . (obj-main obj-lib))
    (obj-main . (main-c headers))
    (obj-lib . (lib-c headers))
    (headers . (config))
    (main-c . ())
    (lib-c . ())
    (config . ())))

(defun topo-sort (deps)
  "Prerequisite-first order, or (error) on a cycle."
  (let ((indeg (make-hash-table))
        (nodes (mapcar #'car deps)))
    (for-each deps
      (lambda (d) (put! indeg (car d) (length (cdr d)))))
    (topo-aux deps indeg
              (filter (lambda (n) (= 0 (gethash indeg n))) nodes)
              ())))

(defun topo-aux (deps indeg ready acc)
  (if (null ready)
      (if (= (length acc) (length deps))
          (reverse acc)
          (error "dependency cycle"))
      (let ((n (car ready)))
        ;; every target that depends on n loses one in-degree
        (let ((newly
               (filter (lambda (d)
                         (if (member n (cdr d))
                             (progn
                               (put! indeg (car d) (- (gethash indeg (car d)) 1))
                               (= 0 (gethash indeg (car d))))
                             ()))
                       deps)))
          (topo-aux deps indeg
                    (append (cdr ready) (mapcar #'car newly))
                    (cons n acc))))))

(def $order (topo-sort $deps))
(format t "build order: ~a~%" $order)

;; self-check: every prerequisite appears before its target, and a
;; cyclic graph errors.
(defun before-p (a b lst) (member b (member a lst)))
(if (and (every (lambda (d)
                  (every (lambda (pre) (before-p pre (car d) $order))
                         (cdr d)))
                $deps)
         (equal (car (errorset '(topo-sort '((a . (b)) (b . (a))))))
                ()))
    (print 'ok)
    (error "topological-sort self-check failed"))
