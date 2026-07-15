;;; Mixed "realistic" workload benchmark, matching
;;; benchmarks/realistic/realistic-array.lisp's run-arr-once / bench-arr:
;;;
;;;   1. array processing (n=300): square 1..n, sum evens, +2n
;;;   2. key/value lookup: build {1:3,...,250:750}, 1500 lookups, sum
;;;   3. naive recursive fibonacci(23)
;;;   4. tail-recursive sum 1..300
;;;   5. ackermann(2, 50)
;;;   6. record processing (n=300): categorize into 3 buckets, totals+count+max
;;;
;;; run_once sums all six; the benchmark repeats run_once 10 times and sums
;;; those into a checksum.
;;;
;;; Run: sbcl --script realistic.lisp

(declaim (optimize (speed 3) (safety 0) (debug 0)))

(defconstant +arr-n+ 300)
(defconstant +kv-n+ 250)
(defconstant +kv-reps+ 1500)
(defconstant +rec-n+ 300)

(defun bench-array-lists (n)
  (declare (fixnum n))
  (let ((arr (make-array n :element-type 'fixnum)))
    (dotimes (i n)
      (setf (aref arr i) (* (1+ i) (1+ i))))
    (let ((total 0))
      (declare (fixnum total))
      (dotimes (i n)
        (let ((v (aref arr i)))
          (declare (fixnum v))
          (when (evenp v)
            (incf total v))))
      (+ total (* 2 n)))))

(defun bench-kv-lookup (n reps)
  (declare (fixnum n reps))
  (let ((h (make-array (1+ n) :element-type 'fixnum)))
    (loop for i fixnum from 1 to n do (setf (aref h i) (* i 3)))
    (let ((total 0))
      (declare (fixnum total))
      (dotimes (i reps)
        (let ((key (+ 1 (mod i n))))
          (declare (fixnum key))
          (incf total (aref h key))))
      total)))

(defun fib (n)
  (declare (fixnum n))
  (if (< n 2)
      n
      (the fixnum (+ (the fixnum (fib (- n 1)))
                      (the fixnum (fib (- n 2)))))))

(defun tsum (n acc)
  (declare (fixnum n acc))
  (if (zerop n) acc (tsum (- n 1) (+ acc n))))

(defun ackermann (m n)
  (declare (fixnum m n))
  (cond ((zerop m) (1+ n))
        ((zerop n) (ackermann (- m 1) 1))
        (t (ackermann (- m 1) (ackermann m (- n 1))))))

(defun process-records (n)
  (declare (fixnum n))
  (let ((t1 0) (t2 0) (t3 0) (mx 0))
    (declare (fixnum t1 t2 t3 mx))
    (dotimes (i n)
      (let ((amt (mod (* (1+ i) 7) 100))
            (cat (+ 1 (mod (1+ i) 3))))
        (declare (fixnum amt cat))
        (when (> amt mx) (setf mx amt))
        (cond ((= cat 1) (incf t1 amt))
              ((= cat 2) (incf t2 amt))
              (t (incf t3 amt)))))
    (+ t1 t2 t3 n mx)))

(defun run-once ()
  (+ (bench-array-lists +arr-n+)
     (bench-kv-lookup +kv-n+ +kv-reps+)
     (fib 23)
     (tsum 300 0)
     (ackermann 2 50)
     (process-records +rec-n+)))

(defun now-ms ()
  (/ (* (get-internal-real-time) 1000.0d0)
     internal-time-units-per-second))

(defun main ()
  (let ((reps 10)
        (checksum 0))
    (declare (fixnum reps checksum))
    (let ((start (now-ms)))
      (dotimes (i reps)
        (incf checksum (run-once)))
      (let ((elapsed (- (now-ms) start)))
        (format t "result=~d time=~,1f ms (~d reps)~%" checksum elapsed reps)))))

(main)
