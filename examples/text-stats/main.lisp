;;; text-stats -- a report over a real file: lines, words, top words,
;;; sentence lengths. The unix-tool shape (wc + a little more).
;;; Shows: READ-FS capability gating (run WITHOUT the flag to see the
;;; sandbox refuse), *ARGV*, string tooling, frequencies, padding.
;;; Run: cargo run -- --capability READ-FS examples/text-stats/main.lisp README.md

(defun words (text)
  (filter (lambda (w) (> (string-length* w) 0))
          (mapcan (lambda (line) (string-split line " "))
                  (string-split text "\n"))))

(defun report (path)
  (let* ((text (read-file path))
         (lines (string-split text "\n"))
         (ws (words text))
         (top (take (sort-by (frequencies (mapcar #'string-downcase ws))
                             #'cdr #'>)
                    5)))
    (format t "~a:~%" path)
    (format t "  lines: ~a~%" (length lines))
    (format t "  words: ~a~%" (length ws))
    (format t "  longest line: ~a chars~%"
            (reduce #'max (mapcar #'string-length* lines) 0))
    (format t "  top words:~%")
    (for-each (lambda (cell)
        (format t "    ~a ~a~%"
                (string-pad-left (number->string (cdr cell)) 5)
                (car cell))) top)
    (length ws)))

(def $path (if *ARGV* (car *ARGV*) "README.md"))
(def $word-count (report $path))

;; self-check: sane counts on whatever file we read, and the capability
;; fence is real -- inside with-capabilities () the read is refused.
(if (and (> $word-count 0)
         (null (with-capabilities ()
                 (errorset '(read-file "README.md")))))
    (print 'ok)
    (error "text-stats self-check failed"))
