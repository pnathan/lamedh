;;; Formatting and string I/O (issue #150, epic #141).
;;;
;;; FORMAT implements a useful subset of Common Lisp format directives on top of
;;; the string primitives (#147), the PRIN1-TO-STRING / PRINC-TO-STRING kernel
;;; builtins, and (only when a port destination or READ-LINE/WITH-OUTPUT-TO-STRING
;;; is actually used) the PORTS module (#255, lib/31-ports.lisp). This file stays
;;; flat/Prelude (18-format.lisp loads before PORTS, which is optional) by lazily
;;; `(require 'ports)` only inside the few forms that need it, not at load time.
;;;
;;; Supported directives:
;;;
;;;   ~a / ~A     human (princ) rendering of the next argument
;;;   ~s / ~S     readable (prin1) rendering of the next argument
;;;   ~d / ~D     the next argument rendered as a decimal datum
;;;   ~f / ~F     fixed-point float; ~<n>f (e.g. ~4f) rounds/pads to exactly
;;;               n digits after the decimal point. Bare ~f (no digit count)
;;;               uses the argument's default rendering (an integer gets a
;;;               trailing ".0"); Lisp's float printer never emits
;;;               scientific notation (see src/printer.rs), so this is
;;;               always fixed-point, just not digit-count-controlled.
;;;   ~x / ~X     the next argument (an integer) rendered in hexadecimal
;;;   ~o / ~O     the next argument (an integer) rendered in octal
;;;   ~b / ~B     the next argument (an integer) rendered in binary
;;;   ~c / ~C     the next argument, a one-character string or a CHAR,
;;;               rendered as the bare character (no quoting)
;;;   ~%          newline
;;;   ~&          a newline, but only if the output built so far by *this*
;;;               FORMAT call does not already end in one (CL "fresh-line").
;;;               NOTE: this only sees this call's own accumulated output --
;;;               it has no memory of a destination's prior output, since
;;;               nothing in this language tracks a stream's column state.
;;;   ~~          a literal tilde
;;;   ~{...~}     iteration: the next argument must be a list; the control
;;;               directives between ~{ and ~} run repeatedly, consuming
;;;               successive elements of that list, until it is exhausted.
;;;               Not supported: nesting another ~{ inside the body, and the
;;;               ~:{ (list-of-lists) / ~@{ (rest-of-args) reader forms.
;;;   ~^          inside a ~{...~} body (or at top level), stop processing
;;;               the rest of the control string immediately if there are no
;;;               more arguments to consume -- the common "~a~^, " idiom for
;;;               separator-joining a list without a trailing separator.
;;;               Only the plain, no-parameter form is supported.
;;;
;;; No other directive is recognized. Per the epic's honesty-over-convenience
;;; default (docs/cl-divergences.md), an unrecognized directive -- including
;;; any of the above written with an unsupported numeric/colon/at-sign
;;; prefix, e.g. ~3a or ~:d -- is a hard error naming the offending
;;; directive, not a silent pass-through: a typo should not degrade
;;; gracefully into wrong output.
;;;
;;; Destination: NIL returns the formatted string; T prints it to stdout and
;;; returns NIL; a PORTS port (#255) writes the UTF-8 bytes to it (lazily
;;; requiring 'ports) and returns NIL. Anything else is an error.
;;;
;;; READ-LINE and WITH-OUTPUT-TO-STRING (this ticket's other deferred items)
;;; are thin sugar over PORTS, defined below; each lazily requires 'ports on
;;; first use so an environment that never touches I/O never pays for it.
;;; READ-SEXPR-FILE / WRITE-SEXPR-FILE (the ticket's third deferred item, an
;;; s-expression file round-trip) sit on top of the existing whole-file
;;; READ-FILE/WRITE-FILE kernel builtins and the existing READ-STRING
;;; builtin (parses every top-level form out of a string) -- no new kernel
;;; surface needed for either.
;;;
;;; STACK SAFETY (#361): FORMAT-BUILD walks the control string by tail
;;; recursion so arbitrarily long control strings do not grow the Rust
;;; stack. The ~{...~} iteration helper ($FORMAT-ITERATE) is also tail
;;; recursive over the iteration list; it makes one *non-tail* call into
;;; FORMAT-BUILD per element (bounded stack use per call, popped before the
;;; next iteration), so an iteration body must itself be a short control
;;; string, but the iteration list length is unbounded. Keep this invariant
;;; if you touch either function.

;; ---- directive helpers ------------------------------------------------------

(defun $format-digit-run (ctrl j n)
  "Index just past the run of ASCII digit characters in CTRL starting at
index J (equal to J itself if CTRL does not start a digit run there)."
  (if (>= j n)
      j
      (let ((code (char-code (substring ctrl j (+ j 1)))))
        (if (and (>= code 48) (<= code 57))
            ($format-digit-run ctrl (+ j 1) n)
            j))))

(defun $format-radix-digit (d)
  "One-character string for digit D (0-15): decimal digits render normally,
10-15 render as uppercase A-F."
  (if (< d 10) (princ-to-string d) (code-char (+ 55 d))))

(defun $format-radix-acc (n base acc)
  (if (< n base)
      (concat ($format-radix-digit n) acc)
      ($format-radix-acc (/ n base) base (concat ($format-radix-digit (mod n base)) acc))))

(defun $format-radix (x base)
  "Render integer X in BASE (2, 8, or 16) for ~b/~o/~x. Errors on a
non-integer argument."
  (if (not (fixp x))
      (error (concat "FORMAT: ~b/~o/~x require an integer argument, got "
                      (prin1-to-string x)))
      (if (< x 0)
          (concat "-" ($format-radix-acc (- x) base ""))
          ($format-radix-acc x base ""))))

(defun $format-fixed-digits (x dig)
  "Fixed-point rendering of number X to exactly DIG digits after the
decimal point (DIG a non-negative integer), rounding half away from zero."
  (let* ((neg (< x 0))
         (ax (if neg (- x) x))
         (scale (expt 10 dig))
         (scaled (round (* ax scale)))
         (whole (/ scaled scale))
         (frac (mod scaled scale)))
    (concat (if neg "-" "")
            (princ-to-string whole)
            (if (> dig 0) (concat "." (string-pad-left (princ-to-string frac) dig "0")) ""))))

(defun $format-fixed (x dig)
  "Render number X for ~f. DIG is the ~<n>f digit count, or NIL for bare
~f: an integer prints with an added \".0\"; a float prints via its default
(always fixed-point -- Lisp's float printer never emits scientific
notation, see printer.rs) rendering."
  (cond
    ((not (numberp x))
     (error (concat "FORMAT: ~f requires a number, got " (prin1-to-string x))))
    ((not (null dig)) ($format-fixed-digits x dig))
    ((floatp x) (princ-to-string x))
    (t (concat (princ-to-string x) ".0"))))

(defun $format-char (c)
  "Render C for ~c: a one-character string prints as itself; a CHAR (byte)
value prints as its one-character string. Errors otherwise."
  (cond
    ((and (stringp c) (= (string-length* c) 1)) c)
    ((charp c) (code-char (char-code c)))
    (t (error (concat "FORMAT: ~c requires a one-character string or a CHAR, got "
                       (prin1-to-string c))))))

(defun $format-find-close (ctrl j n)
  "Index of the ~} that closes a ~{ whose body starts at J. Does not
support nesting another ~{ inside the body."
  (cond
    ((>= (+ j 1) n) (error "FORMAT: ~{ without a matching ~}"))
    ((and (equal (substring ctrl j (+ j 1)) "~")
          (equal (substring ctrl (+ j 1) (+ j 2)) "}"))
     j)
    (t ($format-find-close ctrl (+ j 1) n))))

(defun $format-iterate (body lst acc)
  "Run control string BODY repeatedly against successive elements of LST
(via FORMAT-BUILD), accumulating output onto ACC, until LST is exhausted or
a pass makes no progress (guards against an infinite loop when BODY
consumes no arguments -- e.g. `~{no-directives-here~}`)."
  (if (null lst)
      acc
      (let* ((result (format-build body lst 0 (string-length* body) ""))
             (piece (car result))
             (remaining (cdr result)))
        (if (equal remaining lst)
            (concat acc piece)
            ($format-iterate body remaining (concat acc piece))))))

;; ---- the control-string walker ----------------------------------------------

(defun format-build (ctrl args i n acc)
  "Walk control string CTRL from index I (of N) consuming ARGS, accumulating
onto ACC. Returns (CONS output remaining-args) -- remaining-args lets
$FORMAT-ITERATE know how much of an iteration list a ~{...~} pass consumed."
  (if (>= i n)
      (cons acc args)
      (let ((c (substring ctrl i (+ i 1))))
        (if (not (equal c "~"))
            (format-build ctrl args (+ i 1) n (concat acc c))
            (let* ((j (+ i 1))
                   (digits-end ($format-digit-run ctrl j n))
                   (has-digits (> digits-end j))
                   (numarg (if has-digits (string->number (substring ctrl j digits-end)) nil))
                   (d (substring ctrl digits-end (+ digits-end 1)))
                   (next (+ digits-end 1)))
              (cond
                ((and has-digits (not (or (equal d "f") (equal d "F"))))
                 (error (concat "FORMAT: unsupported numeric prefix in ~"
                                 (substring ctrl j digits-end) d)))
                ((equal d "%")
                 (format-build ctrl args next n (concat acc (code-char 10))))
                ((equal d "&")
                 (format-build ctrl args next n
                               (if (or (= (string-length* acc) 0)
                                       (equal (substring acc (- (string-length* acc) 1) (string-length* acc))
                                              (code-char 10)))
                                   acc
                                   (concat acc (code-char 10)))))
                ((equal d "~")
                 (format-build ctrl args next n (concat acc "~")))
                ((equal d "^")
                 (if (null args) (cons acc args) (format-build ctrl args next n acc)))
                ((or (equal d "a") (equal d "A"))
                 (format-build ctrl (cdr args) next n
                               (concat acc (princ-to-string (car args)))))
                ((or (equal d "s") (equal d "S"))
                 (format-build ctrl (cdr args) next n
                               (concat acc (prin1-to-string (car args)))))
                ((or (equal d "d") (equal d "D"))
                 (format-build ctrl (cdr args) next n
                               (concat acc (princ-to-string (car args)))))
                ((or (equal d "f") (equal d "F"))
                 (format-build ctrl (cdr args) next n
                               (concat acc ($format-fixed (car args) numarg))))
                ((or (equal d "x") (equal d "X"))
                 (format-build ctrl (cdr args) next n
                               (concat acc ($format-radix (car args) 16))))
                ((or (equal d "o") (equal d "O"))
                 (format-build ctrl (cdr args) next n
                               (concat acc ($format-radix (car args) 8))))
                ((or (equal d "b") (equal d "B"))
                 (format-build ctrl (cdr args) next n
                               (concat acc ($format-radix (car args) 2))))
                ((or (equal d "c") (equal d "C"))
                 (format-build ctrl (cdr args) next n
                               (concat acc ($format-char (car args)))))
                ((equal d "{")
                 (let* ((body-start next)
                        (close-idx ($format-find-close ctrl body-start n))
                        (body (substring ctrl body-start close-idx))
                        (iter-list (car args)))
                   (if (not (listp iter-list))
                       (error (concat "FORMAT: ~{ requires a list argument, got "
                                       (prin1-to-string iter-list)))
                       (format-build ctrl (cdr args) (+ close-idx 2) n
                                     (concat acc ($format-iterate body iter-list ""))))))
                (t (error (concat "FORMAT: unknown directive ~" d
                                    " (unrecognized directives are an error, not a pass-through -- "
                                    "see lib/18-format.lisp)")))))))))

;; ---- FORMAT itself -----------------------------------------------------------

(defun format (dest ctrl &rest args)
  "Format CTRL with ARGS. DEST NIL returns the string; DEST T prints it to
stdout and returns NIL; a PORTS port destination writes the UTF-8 bytes to
it and returns NIL. See lib/18-format.lisp's header for the directive set."
  (let ((out (car (format-build ctrl args 0 (string-length* ctrl) ""))))
    (cond
      ((null dest) out)
      ((eq dest t) (progn (princ out) nil))
      ((port-p* dest) (progn (require 'ports) (ports:write-string! dest out) nil))
      (t (error (concat "FORMAT: destination must be NIL, T, or a port, got "
                          (prin1-to-string dest)))))))

;; ---- READ-LINE and WITH-OUTPUT-TO-STRING (#150) -----------------------------

(defun read-line (&optional port)
  "Read one line of text (bytes up to but excluding a trailing newline,
decoded as UTF-8 lossy) from PORT, or from the process's standard input if
PORT is not given (which requires the IO capability; an explicit PORT needs
whatever capability opening it required, already spent). Returns NIL only
at true EOF -- a final line with no trailing newline is still returned
once. Thin sugar over PORTS:READ-LINE! (#255), lazily requiring the PORTS
module on first use."
  (require 'ports)
  (ports:read-line! (if (null port) (ports:stdin) port)))

(defmacro with-output-to-string (binding &rest body)
  "(WITH-OUTPUT-TO-STRING (var) body...) -- bind VAR to a fresh in-memory
output port for BODY's dynamic extent (write to it with PORTS:WRITE-STRING!,
PORTS:WRITE-BYTE!/WRITE-BYTES!, or FORMAT with VAR as the destination) and
return everything written to it, decoded as UTF-8 (lossy), as a STRING. The
port is always closed afterward. If BODY signals an error, that error
propagates (no string is returned) and the port is still closed. Lazily
requires the PORTS module (#255) on first use."
  (progn
    (require 'ports)
    (let ((var (car binding)))
      (list 'let (list (list var '(ports:open-output-bytes)))
            (list 'unwind-protect
                  (list 'progn
                        (cons 'progn body)
                        (list 'text:utf8->string-lossy (list 'ports:output-contents var)))
                  (list 'ports:close! var))))))

;; ---- s-expression file round-trip (#150) -------------------------------------

(defun read-sexpr-file (path)
  "Read PATH's full text (requires READ-FS) and parse it into a list of
every top-level s-expression it contains."
  (read-string (read-file path)))

(defun write-sexpr-file (path forms)
  "Write FORMS (a list of s-expressions) to PATH (requires CREATE-FS), one
per line in readable (PRIN1) form; the inverse of READ-SEXPR-FILE."
  (write-file path (concat (string-join (mapcar #'prin1-to-string forms) (code-char 10))
                            (code-char 10))))
