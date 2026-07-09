;;; Lisp 1.5 Appendix A functions not yet in the core

;;; Table Building
;;; PAIR[x;y] -> ((a1 . b1) (a2 . b2) ...)
(defun pair (x y)
  "Zip two lists X and Y into a dotted-pair association list."
  (cond ((or (null x) (null y)) nil)
        (t (cons (cons (car x) (car y))
                 (pair (cdr x) (cdr y))))))

;;; ATTRIB[x;e] - change last element of x to point to e (destructive)
;;; Value is e (the second argument).
(defun attrib (x e)
  "Destructively concatenate X and E by changing the last cons of X to point to E."
  (cond ((null (cdr x)) (rplacd x e) e)
        (t (attrib (cdr x) e))))

;;; PROP[x;y;u] - search flat plist x for indicator y
;;; If found, return the rest of the list beginning at the value after y.
;;; Otherwise call u[] (function of no args).
(defun prop (x y u)
  "Search flat plist X for indicator Y; return cdr of match, or call U with no args."
  (cond ((null x) (funcall u))
        ((eq (car x) y) (cdr x))
        (t (prop (cdr x) y u))))

;;; FLAG[l;ind] - set indicator ind on plist of each symbol in list l
(defun flag (l ind)
  "Put indicator IND (with value T) on the plist of each symbol in L."
  (cond ((null l) nil)
        (t (putp (car l) ind t)
           (flag (cdr l) ind)
           nil)))

;;; REMFLAG[l;ind] - remove indicator ind from plist of each symbol in list l
(defun remflag (l ind)
  "Remove indicator IND from the plist of each symbol in L."
  (cond ((null l) nil)
        (t (remprop (car l) ind)
           (remflag (cdr l) ind)
           nil)))

;;; MAP[x;f] - apply f to each successive tail of x; return NIL
;;; Like MAPLIST but for side effects only.
(defun map (x f)
  "Apply F to each successive tail of X for side effects; return NIL."
  (cond ((null x) nil)
        (t (funcall f x)
           (map (cdr x) f))))

;;; SEARCH[x;p;f;u] - search list x for element where p[element] is true
;;; If found, return f[element]; else return u[x] where x is the remainder.
(defun search (x p f u)
  "Search X for element satisfying P; if found return F applied to element, else U applied to tail."
  (cond ((null x) (funcall u x))
        ((funcall p (car x)) (funcall f (car x)))
        (t (search (cdr x) p f u))))

;;; RECIP[x] - floating-point reciprocal
(defun recip (x)
  "Return 1/X as a float."
  (/ 1.0 x))

;;; SELECT[q;(q1 e1);...;en]
;;; FEXPR: evaluate each qi left to right; when qi = q return ei.
;;; If none match, return the final default expression en.
(defexpr select (args)
  (let ((q (eval (car args)))
        (clauses (cdr args)))
    (prog (rest)
          (setq rest clauses)
      loop
          (cond ((null rest) (return nil))
                ((null (cdr rest))
                 ;; last element is default
                 (return (eval (car rest))))
                (t
                 (cond ((equal (eval (caar rest)) q)
                        (return (eval (cadar rest)))))
                 (setq rest (cdr rest))
                 (go loop))))))

;;; TRACE and UNTRACE moved to lib/26-instrument.lisp as a REAL
;;; instrumentation facility (0.3: the Lisp 1.5 flag-only stubs are gone).
