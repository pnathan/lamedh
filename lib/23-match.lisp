;;; Structural pattern language: one matcher, three surfaces.
;;;
;;;   MATCH               — pattern-matching control form with guards
;;;   DESTRUCTURING-BIND  — single-pattern binding form
;;;   SGREP               — structural search over code-as-data (issue #171)
;;;   REWRITE             — structural transformation with templates
;;;
;;; Everything is built on PAT-MATCH, a classic s-expression matcher.
;;; Patterns are ordinary data:
;;;
;;;   ?X          pattern variable — binds the datum; a repeated occurrence
;;;               must be EQUAL to its first binding (unification-lite)
;;;   ?_          wildcard — matches anything, binds nothing
;;;   ??XS        segment variable — inside a list pattern, matches zero or
;;;               more consecutive elements (shortest match first, with
;;;               backtracking); binds the matched sublist. ??_ is the
;;;               non-binding segment wildcard.
;;;   (?IS ?X P)  predicate pattern — matches when (P datum) is truthy; P is
;;;               a globally-resolvable function name or a (LAMBDA ...) form.
;;;               ?X may be ?_ to test without binding.
;;;   (?AND P...) all subpatterns must match (bindings accumulate)
;;;   (?OR P...)  first matching subpattern wins
;;;   (?NOT P)    matches when P does not (no bindings escape)
;;;   (QUOTE X)   literal escape — matches a datum EQUAL to X, so '?X
;;;               matches the symbol ?X itself
;;;   any atom    literal — matches by EQUAL
;;;   (P . PS)    cons pattern — matches cons structure, including dotted
;;;               tails: (?H . ?T) is CAR/CDR destructuring
;;;
;;; PAT-MATCH returns an alist of (VAR . VALUE) bindings — NIL for a match
;;; that bound nothing — or the sentinel $MATCH-FAIL. Test with MATCH-FAIL-P.

;;; ------------------------------------------------------------------
;;; Variable taxonomy.

(defun $match-name-prefix-p (sym prefix)
  "T when SYM's print name starts with PREFIX."
  (let* ((s (princ-to-string sym))
         (n (string-length prefix)))
    (and (>= (string-length s) n)
         (equal (substring s 0 n) prefix))))

(defun $match-var-p (p)
  "An element pattern variable: a symbol starting with ? (but not ??)."
  (and (symbolp p)
       ($match-name-prefix-p p "?")
       (not ($match-name-prefix-p p "??"))))

(defun $match-segment-var-p (p)
  "A segment pattern variable: a symbol starting with ??."
  (and (symbolp p) ($match-name-prefix-p p "??")))

(defun $match-wildcard-p (p)
  (and (symbolp p) (or (eq p '?_) (eq p '??_))))

(defun match-fail-p (x)
  "T when X is the PAT-MATCH failure sentinel."
  (eq x '$match-fail))

;;; ------------------------------------------------------------------
;;; The matcher core. BINDINGS is an alist; $MATCH-FAIL propagates.

(defun $match-extend (var datum bindings)
  "Bind VAR to DATUM, or check consistency against an existing binding."
  (if ($match-wildcard-p var)
      bindings
      (let ((existing (assoc var bindings)))
        (cond ((null existing) (cons (cons var datum) bindings))
              ((equal (cdr existing) datum) bindings)
              (t '$match-fail)))))

(defun $match-apply-pred (pred datum)
  "Apply predicate designator PRED (global function name or LAMBDA form)."
  (ignore-errors (funcall (eval pred) datum)))

(defun pat-match (pattern datum &rest opt)
  "Match PATTERN against DATUM. Returns a bindings alist (NIL when nothing
was bound) or the sentinel $MATCH-FAIL — test with MATCH-FAIL-P. An
optional third argument supplies initial bindings."
  (let ((bindings (if opt (car opt) nil)))
    ($match-run pattern datum bindings)))

(defun $match-run (pattern datum bindings)
  (cond
    ((match-fail-p bindings) '$match-fail)
    ;; Element variable / wildcard.
    (($match-var-p pattern) ($match-extend pattern datum bindings))
    ;; Bare segment variable outside list context: match whole datum.
    (($match-segment-var-p pattern)
     (if (listp datum)
         ($match-extend pattern datum bindings)
         '$match-fail))
    ;; Atomic literal.
    ((not (consp pattern))
     (if (equal pattern datum) bindings '$match-fail))
    ;; Special pattern operators.
    ((eq (car pattern) 'quote)
     (if (equal (car (cdr pattern)) datum) bindings '$match-fail))
    ((eq (car pattern) '?is)
     (let ((var (car (cdr pattern)))
           (pred (car (cdr (cdr pattern)))))
       (if ($match-apply-pred pred datum)
           ($match-extend var datum bindings)
           '$match-fail)))
    ((eq (car pattern) '?and)
     ($match-and (cdr pattern) datum bindings))
    ((eq (car pattern) '?or)
     ($match-or (cdr pattern) datum bindings))
    ((eq (car pattern) '?not)
     (if (match-fail-p ($match-run (car (cdr pattern)) datum bindings))
         bindings
         '$match-fail))
    ;; Segment variable heading a list pattern.
    (($match-segment-var-p (car pattern))
     ($match-segment (car pattern) (cdr pattern) datum bindings))
    ;; Ordinary cons pattern (covers dotted tails).
    ((consp datum)
     ($match-run (cdr pattern) (cdr datum)
                 ($match-run (car pattern) (car datum) bindings)))
    (t '$match-fail)))

(defun $match-and (patterns datum bindings)
  (cond ((match-fail-p bindings) '$match-fail)
        ((null patterns) bindings)
        (t ($match-and (cdr patterns) datum
                       ($match-run (car patterns) datum bindings)))))

(defun $match-or (patterns datum bindings)
  (cond ((null patterns) '$match-fail)
        (t (let ((r ($match-run (car patterns) datum bindings)))
             (if (match-fail-p r)
                 ($match-or (cdr patterns) datum bindings)
                 r)))))

(defun $match-take (n lst)
  (if (or (< n 1) (null lst))
      nil
      (cons (car lst) ($match-take (- n 1) (cdr lst)))))

(defun $match-segment (var rest-pattern datum bindings)
  "Match segment VAR followed by REST-PATTERN against list DATUM: try spans
of increasing length until the remainder matches (backtracking)."
  (let ((existing (and (not ($match-wildcard-p var)) (assoc var bindings))))
    (if existing
        ;; Pre-bound segment: the datum must start with exactly that span.
        (let* ((span (cdr existing))
               (n (length span)))
          (if (equal span ($match-take n datum))
              ($match-run rest-pattern (nthcdr n datum) bindings)
              '$match-fail))
        ($match-segment-try var rest-pattern datum bindings 0))))

(defun $match-segment-try (var rest-pattern datum bindings n)
  (let ((tail (nthcdr n datum)))
    (let ((r ($match-run rest-pattern tail
                         ($match-extend var ($match-take n datum) bindings))))
      (cond ((not (match-fail-p r)) r)
            ;; No longer span available to try.
            ((null tail) '$match-fail)
            (t ($match-segment-try var rest-pattern datum bindings (+ n 1)))))))

;;; ------------------------------------------------------------------
;;; Binding collection and body sealing (reuses $GUARD-SEAL from
;;; lib/22-guard.lisp: an immediately-applied lambda whose parameters are
;;; the pattern variables).

(defun $match-body-env-form (body bindings)
  "Body FORMS sealed so each bound pattern variable is a lexical binding."
  (if (null bindings)
      (cons 'progn body)
      ($guard-seal body bindings)))

;;; ------------------------------------------------------------------
;;; MATCH — the control form.
;;;
;;;   (match EXPR
;;;     (PATTERN body...)
;;;     (PATTERN :when GUARD body...)
;;;     (?_ default...))
;;;
;;; EXPR is evaluated once in the caller's environment; clauses are tried
;;; in order. Pattern variables are lexically bound in the guard and body.
;;; With no matching clause, MATCH returns NIL.

(defvau match (x e)
  "(MATCH expr (pattern [:when guard] body...)...) — evaluate EXPR, try each
clause's PATTERN in order, and evaluate the first matching clause's body
with the pattern's variables lexically bound. A clause may carry a :WHEN
guard, evaluated under the same bindings; a falsy guard moves on to the
next clause. Use ?_ as the final pattern for a default clause. Returns NIL
when no clause matches."
  (let ((datum (eval (car x) e)))
    ($match-clauses (cdr x) datum e)))

(defun $match-clauses (clauses datum e)
  (cond
    ((null clauses) nil)
    (t (let* ((clause (car clauses))
              (bindings (pat-match (car clause) datum)))
         (if (match-fail-p bindings)
             ($match-clauses (cdr clauses) datum e)
             (let* ((guardedp (eq (car (cdr clause)) ':when))
                    (guard (and guardedp (car (cdr (cdr clause)))))
                    (body (if guardedp
                              (cdr (cdr (cdr clause)))
                              (cdr clause))))
               (if (and guardedp
                        (null (eval ($match-body-env-form (list guard) bindings) e)))
                   ($match-clauses (cdr clauses) datum e)
                   (eval ($match-body-env-form body bindings) e))))))))

(defvau destructuring-bind (x e)
  "(DESTRUCTURING-BIND pattern expr body...) — match PATTERN against the
value of EXPR and evaluate BODY with the pattern's variables bound.
Signals an error when the pattern does not match."
  (let* ((pattern (car x))
         (datum (eval (car (cdr x)) e))
         (body (cdr (cdr x)))
         (bindings (pat-match pattern datum)))
    (if (match-fail-p bindings)
        (error (concat "destructuring-bind: "
                       (prin1-to-string datum)
                       " does not match pattern "
                       (prin1-to-string pattern)))
        (eval ($match-body-env-form body bindings) e))))

;;; ------------------------------------------------------------------
;;; SGREP — structural search over any s-expression (code is data).
;;;
;;; Returns a list of matches, each (SUBFORM . BINDINGS), visiting the form
;;; itself and, recursively, every element of every proper-or-dotted list
;;; (elements, not tails — code subforms).

(defun sgrep (pattern form)
  "All subforms of FORM matching PATTERN, as a list of (subform . bindings)
pairs, in depth-first source order. See PAT-MATCH for pattern syntax."
  (reverse ($sgrep-walk pattern form nil)))

(defun $sgrep-walk (pattern form acc)
  (let* ((r (pat-match pattern form))
         (acc2 (if (match-fail-p r) acc (cons (cons form r) acc))))
    ($sgrep-descend pattern form acc2)))

(defun $sgrep-descend (pattern form acc)
  (cond ((not (consp form)) acc)
        (t (let ((acc2 ($sgrep-walk pattern (car form) acc))
                 (tail (cdr form)))
             (cond ((consp tail) ($sgrep-descend pattern tail acc2))
                   ((null tail) acc2)
                   ;; Dotted tail: visit the atom.
                   (t ($sgrep-walk pattern tail acc2)))))))

(defun sgrep-fn (pattern name)
  "SGREP over the source of the function bound to symbol NAME (via
SEE-SOURCE) — search a definition the way you would grep a file."
  (sgrep pattern (see-source name)))

;;; ------------------------------------------------------------------
;;; REWRITE — structural transformation.
;;;
;;; (rewrite PATTERN TEMPLATE FORM): every subform matching PATTERN is
;;; replaced by TEMPLATE instantiated with the match's bindings (?X inserts
;;; the binding; ??XS splices it). Top-down, single pass: replacements are
;;; not re-visited, unmatched conses are rewritten element-wise.

(defun instantiate (template bindings)
  "TEMPLATE with every bound pattern variable replaced by its binding;
segment variables (??XS) splice their sublists into the enclosing list."
  (cond
    ((and ($match-var-p template) (assoc template bindings))
     (cdr (assoc template bindings)))
    ((not (consp template)) template)
    ((eq (car template) 'quote) template)
    (t ($match-instantiate-list template bindings))))

(defun $match-instantiate-list (template bindings)
  (cond
    ((null template) nil)
    ((not (consp template)) (instantiate template bindings))
    (($match-segment-var-p (car template))
     (let ((b (assoc (car template) bindings)))
       (append (if b (cdr b) nil)
               ($match-instantiate-list (cdr template) bindings))))
    (t (cons (instantiate (car template) bindings)
             ($match-instantiate-list (cdr template) bindings)))))

(defun rewrite (pattern template form)
  "FORM with every PATTERN-matching subform replaced by TEMPLATE
instantiated with that match's bindings. Bottom-up (innermost-first),
single pass: children are rewritten before their parent is matched, so
nested matches inside a binding are already transformed when they are
carried into the template — but an instantiated replacement is not
re-searched at its own node, so a template may mention the pattern's own
shape without looping."
  (let* ((rebuilt (if (consp form)
                      (cons (rewrite pattern template (car form))
                            ($rewrite-cdr pattern template (cdr form)))
                      form))
         (r (pat-match pattern rebuilt)))
    (if (match-fail-p r)
        rebuilt
        (instantiate template r))))

(defun $rewrite-cdr (pattern template tail)
  ;; Rewrite the elements of a list tail without matching the tail itself
  ;; as a whole (subFORMS are elements, matching SGREP's notion).
  (cond ((null tail) nil)
        ((consp tail)
         (cons (rewrite pattern template (car tail))
               ($rewrite-cdr pattern template (cdr tail))))
        (t (rewrite pattern template tail))))

;;; ------------------------------------------------------------------
;;; Positioned structural search (issue #171 phase 2a): SGREP over whole
;;; source texts and files, each hit carrying the 1-based line/column of
;;; its top-level form. (Sub-form positions await reader spans on values —
;;; phase 2b; the top-level anchor is the jump-to-definition 80%.)
;;;
;;; Hit shape: (LINE COL SUBFORM BINDINGS) — destructures nicely with the
;;; pattern language itself:
;;;   (match hit ((?line ?col ?form ?bs) ...))

(defun sgrep-source (pattern src)
  "All PATTERN matches in the source text SRC, as (line col subform
bindings) hits in file order. Line/column anchor the enclosing top-level
form (1-based)."
  ($sgrep-positioned pattern (read-all-positioned src)))

(defun $sgrep-positioned (pattern triples)
  (cond ((null triples) nil)
        (t (let* ((triple (car triples))
                  (form (car triple))
                  (line (car (cdr triple)))
                  (col (car (cdr (cdr triple)))))
             (append
              (mapcar (lambda (hit)
                        (list line col (car hit) (cdr hit)))
                      (sgrep pattern form))
              ($sgrep-positioned pattern (cdr triples)))))))

(defun sgrep-file (pattern path)
  "SGREP-SOURCE over the file at PATH (requires the READ-FS capability),
returning (line col subform bindings) hits."
  (sgrep-source pattern (read-file path)))
