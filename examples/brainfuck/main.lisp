;;; brainfuck -- an interpreter for the 8-instruction classic.
;;; Shows: interpreting a foreign language, bracket matching as a
;;; precomputed jump table, a tape as an array, and building output.
;;; Run: cargo run -- examples/brainfuck/main.lisp

(defun jump-table (prog)
  "Hash mapping each [ and ] index to its partner."
  (let ((jumps (make-hash-table)))
    (jump-aux prog 0 () jumps)
    jumps))

(defun jump-aux (prog i stack jumps)
  (cond ((= i (string-length* prog)) jumps)
        ((equal (ref prog i) "[")
         (jump-aux prog (1+ i) (cons i stack) jumps))
        ((equal (ref prog i) "]")
         (progn (put! jumps (car stack) i)
                (put! jumps i (car stack))
                (jump-aux prog (1+ i) (cdr stack) jumps)))
        (t (jump-aux prog (1+ i) stack jumps))))

(defun bf-run (prog)
  "Run PROG, return its output string. Cells are 0-255, tape 3000 wide."
  (let ((tape (array 3000))
        (jumps (jump-table prog))
        (out (list->array (list ""))))
    (dotimes (i 3000) (put! tape i 0))
    (bf-aux prog 0 tape 0 jumps out)
    (ref out 0)))

(defun bf-aux (prog pc tape ptr jumps out)
  (if (>= pc (string-length* prog))
      ()
      (let ((op (ref prog pc)))
        (cond
          ((equal op ">") (bf-aux prog (1+ pc) tape (1+ ptr) jumps out))
          ((equal op "<") (bf-aux prog (1+ pc) tape (- ptr 1) jumps out))
          ((equal op "+")
           (put! tape ptr (mod (1+ (ref tape ptr)) 256))
           (bf-aux prog (1+ pc) tape ptr jumps out))
          ((equal op "-")
           (put! tape ptr (mod (+ 255 (ref tape ptr)) 256))
           (bf-aux prog (1+ pc) tape ptr jumps out))
          ((equal op ".")
           (put! out 0 (concat (ref out 0) (code-char (ref tape ptr))))
           (bf-aux prog (1+ pc) tape ptr jumps out))
          ((equal op "[")
           (bf-aux prog
                   (if (= 0 (ref tape ptr)) (1+ (gethash jumps pc)) (1+ pc))
                   tape ptr jumps out))
          ((equal op "]")
           (bf-aux prog (gethash jumps pc) tape ptr jumps out))
          (t (bf-aux prog (1+ pc) tape ptr jumps out))))))

(def $hello
  "++++++++[>++++[>++>+++>+++>+<<<<-]>+>+>->>+[<]<-]>>.>---.+++++++..+++.>>.<-.<.+++.------.--------.>>+.>++.")

(def $output (bf-run $hello))
(format t "~a~%" $output)

;; self-check: the canonical hello program, and a computed one (2+5 as
;; a digit).
(if (and (equal $output "Hello World!\n")
         (equal (bf-run "++>+++++[<+>-]++++++++[<++++++>-]<.") "7"))
    (print 'ok)
    (error "brainfuck self-check failed"))
