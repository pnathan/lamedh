;;; Benchmark: native make-hash-table/gethash/sethash vs. the pure-Lamedh
;;; LHT (lib/45-hashtable.lisp), for issue #458.
;;;
;;; Run (release build recommended -- the interpreter is a tree-walker, and
;;; debug-mode timings for either table dwarf the difference this measures):
;;;
;;;   cargo build --release
;;;   ./target/release/lamedh -i lib/45-hashtable.lisp benchmarks/hashtable/bench.lisp
;;;
;;; Key distribution: N distinct interned symbols "KEY-0" .. "KEY-{N-1}",
;;; inserted in order, then looked up in a different (reversed) order so the
;;; lookup pass isn't just replaying the insert pass's access pattern.

(def n 3000)

(defun make-keys (i n acc)
  (if (>= i n) (reverse acc)
      (make-keys (+ i 1) n (cons (intern (concat "KEY-" (number->string i))) acc))))

(def bench-keys (make-keys 0 n nil))
(def bench-keys-rev (reverse bench-keys))

(defun native-insert-all (h ks)
  (if (null ks) nil
      (progn (sethash h (car ks) (car ks)) (native-insert-all h (cdr ks)))))

(defun native-lookup-all (h ks acc)
  (if (null ks) acc
      (native-lookup-all h (cdr ks) (+ acc (if (gethash h (car ks)) 1 0)))))

(defun lht-insert-all (h ks)
  (if (null ks) nil
      (progn (lht-put! h (car ks) (car ks)) (lht-insert-all h (cdr ks)))))

(defun lht-lookup-all (h ks acc)
  (if (null ks) acc
      (lht-lookup-all h (cdr ks) (+ acc (if (lht-get h (car ks)) 1 0)))))

(defun report (label t0 t1 n)
  (let ((elapsed (- t1 t0)))
    (princ label) (princ ": ") (princ (princ-to-string elapsed)) (princ "s total, ")
    (princ (princ-to-string (/ (* elapsed 1000000.0) n))) (princ " us/op")
    (terpri)))

(princ "N = ") (princ (princ-to-string n)) (terpri)
(princ "---- native make-hash-table ----") (terpri)

(def h-native (make-hash-table))
(def t0 (os:now-unix))
(native-insert-all h-native bench-keys)
(def t1 (os:now-unix))
(report "native insert" t0 t1 n)

(def t2 (os:now-unix))
(def native-hits (native-lookup-all h-native bench-keys-rev 0))
(def t3 (os:now-unix))
(report "native lookup" t2 t3 n)
(princ "native hits: ") (princ (princ-to-string native-hits)) (terpri)

(princ "---- pure-Lamedh LHT ----") (terpri)

(def h-lht (make-lht))
(def t4 (os:now-unix))
(lht-insert-all h-lht bench-keys)
(def t5 (os:now-unix))
(report "lht insert" t4 t5 n)

(def t6 (os:now-unix))
(def lht-hits (lht-lookup-all h-lht bench-keys-rev 0))
(def t7 (os:now-unix))
(report "lht lookup" t6 t7 n)
(princ "lht hits: ") (princ (princ-to-string lht-hits)) (terpri)

(princ "---- ratios (lht / native) ----") (terpri)
(princ "insert: ") (princ (princ-to-string (/ (- t5 t4) (- t1 t0)))) (princ "x") (terpri)
(princ "lookup: ") (princ (princ-to-string (/ (- t7 t6) (- t3 t2)))) (princ "x") (terpri)
