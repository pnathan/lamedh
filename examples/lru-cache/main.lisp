;;; lru-cache -- bounded cache with least-recently-used eviction.
;;; Shows: hash for storage plus a recency list, closures wrapping the
;;; whole structure into an object-like interface, and a hit/miss trace.
;;; Run: cargo run -- examples/lru-cache/main.lisp

(defun make-lru (capacity)
  "Returns (get . put) closures sharing private state."
  (let ((store (make-hash-table))
        (recency (list->array (list ()))))  ; most recent first
    (let ((touch (lambda (k)
                   (put! recency 0
                         (cons k (remove k (ref recency 0)))))))
      (cons
       ;; get
       (lambda (k)
         (if (has-key-p store k)
             (progn (funcall touch k) (some (gethash store k)))
             (none)))
       ;; put
       (lambda (k v)
         (funcall touch k)
         (put! store k v)
         (if (> (length (ref recency 0)) capacity)
             (let ((victim (ref (ref recency 0) capacity)))
               (put! recency 0 (take (ref recency 0) capacity))
               (remhash store victim)
               victim)
             ()))))))

(def $cache (make-lru 3))
(def $get (car $cache))
(def $put (cdr $cache))

(for-each '((a . 1) (b . 2) (c . 3))
  (lambda (kv) (funcall $put (car kv) (cdr kv))))
(funcall $get 'a)                       ; a is now most recent
(def $evicted (funcall $put 'd 4))      ; b is the LRU -> evicted
(format t "evicted: ~a~%" $evicted)

;; self-check: b gone, a/c/d present with right values, eviction order
;; respects recency.
(if (and (equal $evicted 'b)
         (equal (funcall $get 'b) (none))
         (equal (funcall $get 'a) (some 1))
         (equal (funcall $get 'c) (some 3))
         (equal (funcall $get 'd) (some 4)))
    (print 'ok)
    (error "lru-cache self-check failed"))
