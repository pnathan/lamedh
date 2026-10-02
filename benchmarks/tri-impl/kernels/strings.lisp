;; String building: append a 2-byte piece 2000 times (quadratic copying), 150 rounds.
;; STRING-APPEND/STRING-LENGTH are the asm port's names; run.sh rewrites them
;; to CONCAT/LENGTH for the Rust and SBCL hosts.
(defun build (n)
  (let ((s "") (i 0))
    (while (< i n) (setq s (STRING-APPEND s "xy")) (setq i (+ i 1)))
    (STRING-LENGTH s)))
(defun run (reps)
  (let ((r 0) (tot 0))
    (while (< r reps) (setq tot (+ tot (build 2000))) (setq r (+ r 1)))
    tot))
(print (run 150))
