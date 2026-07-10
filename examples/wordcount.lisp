;;; wordcount.lisp -- the classic word-frequency report, in 0.3 Lamedh.
;;;
;;; A deliberately ordinary program: read a file, split it into words,
;;; count them, print the top N right-aligned. Ordinary is the point --
;;; this is the pragmatics smoke test for the 0.3 staples working
;;; together:
;;;
;;;   - Option to make "no argument" explicit (`*ARGV*` handling);
;;;   - string-split / string-downcase / string-trim for tokenizing;
;;;   - FREQUENCIES for the counting (typed:
;;;     (forall (a) (-> ((list a)) (list (pair a int64)))));
;;;   - SORT-BY with a custom predicate for the descending order;
;;;   - TAKE + ENUMERATE + string-pad-left for the report;
;;;   - FOR-EACH (the sequence protocol) to drive the printing.
;;;
;;; Run it:  cargo run -- --capability READ-FS examples/wordcount.lisp README.md 10

(defun tokenize (text)
  "Lowercased words of TEXT: split on whitespace, strip empties."
  (filter (lambda (w) (> (string-length w) 0))
          (flatten
           (mapcar (lambda (line) (string-split (string-downcase line) " "))
                   (string-split text "\n")))))

(defun top-words (text n)
  "The N most frequent words of TEXT as ((word . count) ...), ties in
first-seen order (FREQUENCIES preserves it; the sort is stable)."
  (take (sort-by (frequencies (tokenize text)) #'cdr #'>) n))

(defun report (rows)
  "Print RANK. COUNT WORD lines, counts right-aligned."
  (for-each (enumerate rows 1)
            (lambda (row)
              (let ((rank (car row))
                    (cell (cadr row)))
                (format t "~a. ~a ~a~%"
                        rank
                        (string-pad-left (number->string (cdr cell)) 6)
                        (car cell))))))

(defun count-or-default (s)
  "Parse S as a count, defaulting to 10. STRING->NUMBER is nil-on-miss,
so the miss is handled HERE, at the edge, before the value flows onward."
  (let ((n (string->number s)))
    (if (and (numberp n) (not (floatp n))) n 10)))

(defun main-args ()
  "(path . n) from *ARGV*, as an Option."
  (if (null *ARGV*)
      (none)
      (some (cons (car *ARGV*)
                  (if (cdr *ARGV*) (count-or-default (cadr *ARGV*)) 10)))))

(variant-case (main-args)
  (some (args)
    (report (top-words (read-file (car args)) (cdr args))))
  (none ()
    (print "usage: lamedh --capability READ-FS examples/wordcount.lisp FILE [N]")))
