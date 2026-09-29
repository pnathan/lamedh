;;;; printer.lisp -- render Lamedh values back to readable text.

(in-package #:lamedh-rt)

(defun float-repr (f)
  ;; Non-finite values (reachable through the trap-masked ARRAY-SUM/
  ;; ARRAY-DOT reductions) print as Rust's f64 Display does.
  (cond ((sb-ext:float-nan-p f) (return-from float-repr "NaN"))
        ((sb-ext:float-infinity-p f) (return-from float-repr (if (plusp f) "inf" "-inf"))))
  (let ((s (let ((*read-default-float-format* 'double-float)) (prin1-to-string f))))
    ;; CL prints 3.0d0 / 1.0d5; Lamedh wants 3.0 / 100000.0 (no exponent
    ;; marker for plain doubles, matching the reference reader/printer,
    ;; which has no separate single/double float syntax).
    (let ((pos (position #\d s)))
      (if pos (concatenate 'string (subseq s 0 pos) (subseq s (1+ pos))) s))))

(defun lprint-1 (v stream readably)
  (cond
    ;; The empty list prints as () everywhere, never NIL (KERNEL Part III).
    ((null v) (write-string "()" stream))
    ((eq v *t-sym*) (write-string "T" stream))
    ((symbolp v) (write-string (symbol-name v) stream))
    ((integerp v) (princ v stream))
    ((floatp v) (write-string (float-repr v) stream))
    ((characterp v)
     (if readably
         (format stream "'~A'" v)
         (write-char v stream)))
    ((stringp v)
     (if readably
         (progn (write-char #\" stream)
                (loop for c across v do
                  (case c
                    (#\" (write-string "\\\"" stream))
                    (#\\ (write-string "\\\\" stream))
                    (#\Newline (write-string "\\n" stream))
                    (#\Tab (write-string "\\t" stream))
                    (#\Return (write-string "\\r" stream))
                    (t (write-char c stream))))
                (write-char #\" stream))
         (write-string v stream)))
    ((consp v)
     (write-char #\( stream)
     (lprint-1 (car v) stream readably)
     (let ((rest (cdr v)))
       (loop
         (cond
           ((null rest) (return))
           ((consp rest) (write-char #\Space stream) (lprint-1 (car rest) stream readably) (setf rest (cdr rest)))
           (t (write-string " . " stream) (lprint-1 rest stream readably) (return)))))
     (write-char #\) stream))
    ((lamedh-struct-p v)
     (write-string "#S(" stream)
     (lprint-1 (lamedh-struct-type-name v) stream readably)
     (loop for f across (lamedh-struct-values v) do (write-char #\Space stream) (lprint-1 f stream readably))
     (write-char #\) stream))
    ;; Opaque tags match the reference printer (src/printer.rs).
    ((hash-table-p v) (write-string "<hash-table>" stream))
    ((simple-vector-p v) (format stream "<array:~D>" (length v)))
    ((typed-array-p v) (format stream "<typed-array:~A:~D>" (typed-array-elem-name v) (length v)))
    ((lambda-obj-p v) (format stream "#<LAMBDA~@[ ~A~]>" (lambda-obj-name v)))
    ((macro-obj-p v) (write-string "#<MACRO>" stream))
    ((fexpr-obj-p v) (write-string "#<FEXPR>" stream))
    ((vau-obj-p v) (write-string "#<VAU>" stream))
    ((lenv-p v) (write-string "#<ENVIRONMENT>" stream))
    ((lamedh-error-obj-p v) (format stream "#<ERROR ~A>" (lamedh-error-obj-message v)))
    ((functionp v) (format stream "#<BUILTIN ~A>" (or (nth-value 2 (function-lambda-expression v)) "?")))
    (t (princ v stream))))

(defun lprint-to-string (v &optional (readably t))
  (with-output-to-string (s) (lprint-1 v s readably)))

(defun lprint (v)
  "PRINT semantics: the READABLE representation, then a newline (matches
the reference implementation, not CL's leading-newline/trailing-space
framing); used by the (print ...) builtin."
  (format t "~A~%" (lprint-to-string v t))
  v)

(defun lprinc (v) (format t "~A" (lprint-to-string v nil)) v)
(defun lprin1 (v) (format t "~A" (lprint-to-string v t)) v)
