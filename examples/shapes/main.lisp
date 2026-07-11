;;; shapes -- the OO chestnut, the condensation way.
;;; Shows: defrecord with invariants and derived equality, a user-defined
;;; typed protocol (area) with per-brand instances, row polymorphism
;;; (any record with a name field), and static misuse as a checker error.
;;; Run: cargo run -- examples/shapes/main.lisp

;; :invariant defines the VALIDATE-CIRCLE judgment (construction stays
;; total; validation is an explicit tier).
(defrecord circle (name symbol) (radius float64)
  (:invariant (> radius 0.0))
  (:derive equality))
(defrecord rect (name symbol) (w float64) (h float64)
  (:invariant (and (> w 0.0) (> h 0.0)))
  (:derive equality))
(defrecord triangle (name symbol) (base float64) (height float64))

(defprotocol area "Surface area of a shape.")
(definstance area ((c circle)) float64
  (* 3.14159265 (* (circle-radius c) (circle-radius c))))
(definstance area ((r rect)) float64 (* (rect-w r) (rect-h r)))
(definstance area ((tr triangle)) float64
  (* 0.5 (* (triangle-base tr) (triangle-height tr))))

;; Row polymorphism: works on ANY record with a name field.
(defun shape-label (s)
  (concat (princ-to-string (record-ref s 'name))
          ": "
          (princ-to-string (area s))))

(def $shapes
  (list (make-circle 'wheel 1.0)
        (make-rect 'door 2.0 1.0)
        (make-triangle 'sail 3.0 4.0)))

(for-each $shapes (lambda (s) (format t "~a~%" (shape-label s))))

(def $total (reduce (lambda (acc s) (+ acc (area s))) $shapes 0.0))
(format t "total area: ~a~%" $total)

;; self-check: dispatch per brand; the invariant lives in the VALIDATOR
;; (make- constructs, validate-NAME judges -- values stay checkable
;; data); derived equality is structural; misuse is a STATIC error.
(if (and (< (abs (- (area (make-circle 'c 1.0)) 3.14159265)) 0.0001)
         (= (area (make-rect 'r 2.0 3.0)) 6.0)
         (= (area (make-triangle 'tr 3.0 4.0)) 6.0)
         (validate-circle (make-circle 'c 1.0))
         (not (validate-circle (make-circle 'bad -1.0)))
         (equal (make-rect 'a 1.0 2.0) (make-rect 'a 1.0 2.0))
         (contains-p (check-type (area 42)) "no `AREA` instance"))
    (print 'ok)
    (error "shapes self-check failed"))
