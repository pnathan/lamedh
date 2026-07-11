;;; towers-of-hanoi -- the recursion poster child.
;;; Shows: the 2^n-1 move recurrence, an accumulator of moves, and
;;; simulating the moves against real stacks to prove legality.
;;; Run: cargo run -- examples/towers-of-hanoi/main.lisp

(defun hanoi (n from to via)
  "The move list ((disk from . to) ...) solving N disks."
  (if (= n 0)
      ()
      (append (hanoi (- n 1) from via to)
              (list (list n from to))
              (hanoi (- n 1) via to from))))

(def $moves (hanoi 4 'a 'c 'b))
(for-each (lambda (m) (format t "move disk ~a: ~a -> ~a~%" (car m) (cadr m) (caddr m))) (take $moves 5))
(format t "... ~a moves total~%" (length $moves))

;; self-check: 2^4-1 moves, and replaying them never puts a larger disk
;; on a smaller one and ends with all disks on C.
(defun replay (moves pegs)
  "PEGS is a hash of peg -> list of disks (top first)."
  (if (null moves)
      pegs
      (let* ((m (car moves))
             (disk (car m)) (from (cadr m)) (to (caddr m))
             (src (gethash pegs from))
             (dst (gethash pegs to)))
        (if (or (null src) (not (= (car src) disk))
                (and dst (< (car dst) disk)))
            (error "illegal move")
            (progn
              (put! pegs from (cdr src))
              (put! pegs to (cons disk dst))
              (replay (cdr moves) pegs))))))

(def $pegs (make-hash-table))
(put! $pegs 'a (list 1 2 3 4))
(put! $pegs 'b ())
(put! $pegs 'c ())
(replay $moves $pegs)

(if (and (= (length $moves) 15)
         (equal (gethash $pegs 'c) (list 1 2 3 4))
         (null (gethash $pegs 'a)))
    (print 'ok)
    (error "hanoi self-check failed"))
