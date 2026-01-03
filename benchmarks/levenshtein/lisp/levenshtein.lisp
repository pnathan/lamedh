;;; Levenshtein distance benchmark for lamedh
;;; Calculates edit distance between strings

(defun string-length (str)
  "Calculate length of a string"
  (cond
    ((null str) 0)
    ((atom str) 0)  ;; Not a string
    (t (prog (len s)
         (setq len 0)
         (setq s str)
         loop
         (cond ((null s) (return len)))
         (setq len (+ len 1))
         (setq s (cdr s))
         (go loop)))))

(defun string-char-at (str idx)
  "Get character at index in string (0-indexed)"
  (cond
    ((< idx 0) nil)
    ((null str) nil)
    ((eq idx 0) (car str))
    (t (string-char-at (cdr str) (- idx 1)))))

(defun make-array (size initial-value)
  "Create an array (list) of given size with initial value"
  (cond
    ((<= size 0) nil)
    (t (cons initial-value (make-array (- size 1) initial-value)))))

(defun array-ref (arr idx)
  "Get value from array at index"
  (cond
    ((< idx 0) nil)
    ((null arr) nil)
    ((eq idx 0) (car arr))
    (t (array-ref (cdr arr) (- idx 1)))))

(defun array-set (arr idx value)
  "Set value in array at index (returns new array)"
  (cond
    ((< idx 0) arr)
    ((null arr) nil)
    ((eq idx 0) (cons value (cdr arr)))
    (t (cons (car arr) (array-set (cdr arr) (- idx 1) value)))))

(defun min3 (a b c)
  "Return minimum of three numbers"
  (cond
    ((and (<= a b) (<= a c)) a)
    ((and (<= b a) (<= b c)) b)
    (t c)))

(defun char-eq (c1 c2)
  "Compare two characters for equality"
  (eq c1 c2))

(defun levenshtein-distance (str1 str2)
  "Calculate Levenshtein distance between two strings"
  (prog (m n prev curr i j cost c1 c2)
    (setq m (string-length str1))
    (setq n (string-length str2))

    ;; Swap if str2 is shorter
    (cond ((< n m)
           (return (levenshtein-distance str2 str1))))

    ;; Initialize prev array
    (setq prev (make-array (+ m 1) 0))
    (setq curr (make-array (+ m 1) 0))

    ;; Fill prev with 0, 1, 2, ...
    (setq i 0)
    init-loop
    (cond ((> i m) (go init-done)))
    (setq prev (array-set prev i i))
    (setq i (+ i 1))
    (go init-loop)

    init-done
    ;; Compute Levenshtein distance
    (setq i 1)
    outer-loop
    (cond ((> i n) (go outer-done)))

    (setq curr (array-set curr 0 i))

    (setq j 1)
    inner-loop
    (cond ((> j m) (go inner-done)))

    ;; Get characters at positions
    (setq c1 (string-char-at str1 (- j 1)))
    (setq c2 (string-char-at str2 (- i 1)))

    ;; Cost is 0 if characters match, 1 if they differ
    (setq cost (cond ((char-eq c1 c2) 0) (t 1)))

    ;; Calculate minimum
    (setq curr (array-set curr j
                          (min3
                            (+ (array-ref prev j) 1)        ;; deletion
                            (+ (array-ref curr (- j 1)) 1)  ;; insertion
                            (+ (array-ref prev (- j 1)) cost))))  ;; substitution

    (setq j (+ j 1))
    (go inner-loop)

    inner-done
    ;; Copy curr to prev
    (setq prev curr)
    (setq curr (make-array (+ m 1) 0))

    (setq i (+ i 1))
    (go outer-loop)

    outer-done
    (return (array-ref prev m))))

;; Note: This is a simplified version
;; In practice, lamedh doesn't have full string support built-in
;; So this would need to work with character lists
