(declaim (optimize (speed 3) (safety 0) (debug 0)))

(defun fibonacci (n)
  (declare (fixnum n))
  (if (< n 2)
      n
      (the fixnum (+ (the fixnum (fibonacci (- n 1)))
                    (the fixnum (fibonacci (- n 2)))))))

(defun fibonacci-sum (n)
  (declare (fixnum n))
  (let ((result 0))
    (declare (fixnum result))
    (loop for i fixnum from 1 below n do
      (setf result (the fixnum (+ result (the fixnum (fibonacci i))))))
    result))

(defun now-ms ()
  (/ (* (get-internal-real-time) 1000.0d0)
     internal-time-units-per-second))

(defun std-dev (times mean)
  (let ((len (length times)))
    (if (< len 2)
        0.0d0
        (sqrt (/ (loop for x in times
                       for d double-float = (- x mean)
                       sum (* d d) double-float)
                 (- len 1))))))

(defun bench (run-ms n)
  (let ((times '())
        (elapsed-total 0.0d0)
        (result 0))
    (loop while (< elapsed-total run-ms) do
      (let* ((start (now-ms))
             (value (fibonacci-sum n))
             (elapsed (- (now-ms) start)))
        (setf result value)
        (incf elapsed-total elapsed)
        (push elapsed times)))
    (values (nreverse times) result)))

(defun emit (times result)
  (let* ((len (length times))
         (mean (/ (reduce #'+ times) len))
         (min (reduce #'min times))
         (max (reduce #'max times)))
    (format t "~,6f,~,6f,~,6f,~,6f,~d,~d~%"
            mean (std-dev times mean) min max len result)))

(let ((args sb-ext:*posix-argv*))
  (unless (= (length args) 4)
    (format *error-output* "Usage: sbcl --script fibonacci.lisp <run_ms> <warmup_ms> <n>~%")
    (sb-ext:exit :code 1))
  (let ((run-ms (parse-integer (second args)))
        (warmup-ms (parse-integer (third args)))
        (n (parse-integer (fourth args))))
    (when (> warmup-ms 0)
      (bench warmup-ms n))
    (when (> run-ms 0)
      (multiple-value-bind (times result) (bench run-ms n)
        (emit times result)))))
