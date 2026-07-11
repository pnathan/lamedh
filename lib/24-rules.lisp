;;; The rulebook optimizer (build-order #4): optimization passes as DATA.
;;;
;;;   (defrule name pattern template)
;;;   (defrule name pattern template :when guard)
;;;
;;; A rule is a PAT-MATCH pattern (lib/23-match.lisp — ?x variables, ??xs
;;; segments, ?is/?and/?or/?not) plus an INSTANTIATE template, optionally
;;; guarded by a form evaluated with the pattern's variables bound. Rules
;;; live in *OPTIMIZER-RULES*, are applied bottom-up by APPLY-RULES, and —
;;; because this file loads after the pattern language — OPTIMIZE-FORM is
;;; redefined here to run the rulebook between the Lisp passes and the
;;; builtin constant folder.
;;;
;;; The point: an optimization becomes an inspectable, testable, *addable*
;;; datum. (LIST-RULES) shows the book; an agent — human or LLM — extends
;;; the compiler by stating an equation, not by editing an evaluator:
;;;
;;;   (defrule car-of-cons (car (cons ?a ?b)) ?a
;;;     :when (opt-pure-p ?b))           ; drop ?b only when effect-free
;;;
;;; Guards run at rewrite time with each pattern variable bound to its
;;; matched form as data — a guard reasons ABOUT code, so evaluating ?b
;;; inside a guard yields the matched subFORM itself (see the default
;;; rules: (opt-pure-p ?b) asks whether the code bound to ?b is pure).
;;; Termination: rules are retried at a node until none fires, with a
;;; per-node fire cap ($RULES-NODE-CAP) so a cyclic rulebook degrades to
;;; a no-op instead of hanging. QUOTE and QUASIQUOTE subtrees are data and
;;; are never rewritten.

;; The rulebook: a list of (name pattern template guard-form-or-nil defining-env),
;; applied in order by APPLY-RULES. Managed with DEFRULE / UNDEFRULE.
(def *optimizer-rules* nil)

(setq $rules-node-cap 64)

(defvau defrule (x e)
  "(DEFRULE name pattern template [:when guard]) — register (or replace)
an optimizer rewrite rule. PATTERN is a PAT-MATCH pattern; TEMPLATE is an
INSTANTIATE template over the same variables; GUARD, if given, is a form
evaluated (in the defining environment) with the pattern's variables
bound to the matched subforms — the rule fires only when it is truthy.
Returns the rule name."
  (let* ((name (car x))
         (pattern (car (cdr x)))
         (template (car (cdr (cdr x))))
         (rest (cdr (cdr (cdr x))))
         (guard (if (eq (car rest) ':when) (car (cdr rest)) nil)))
    (setq *optimizer-rules*
          (cons (list name pattern template guard e)
                ($rules-remove name *optimizer-rules*)))
    name))

(defun undefrule (name)
  "Remove the rule named NAME from the rulebook. Returns NAME."
  (setq *optimizer-rules* ($rules-remove name *optimizer-rules*))
  name)

(defun $rules-remove (name rules)
  (cond ((null rules) nil)
        ((eq (car (car rules)) name) ($rules-remove name (cdr rules)))
        (t (cons (car rules) ($rules-remove name (cdr rules))))))

(defun list-rules ()
  "The rulebook as (name pattern template) triples, application order."
  (mapcar (lambda (r) (list (car r) (car (cdr r)) (car (cdr (cdr r)))))
          *optimizer-rules*))

(defun $rules-guard-ok (guard bindings env)
  "Evaluate GUARD with each pattern variable bound to its matched form
(as data: bindings are quoted into the sealing lambda). NIL guard passes."
  (if (null guard)
      t
      (not (null (ignore-errors (eval ($guard-seal (list guard) bindings) env))))))

(defun $rules-fire (form)
  "Try each rule at FORM, first match wins; NIL when no rule fires."
  ($rules-fire-scan *optimizer-rules* form))

(defun $rules-fire-scan (rules form)
  (cond ((null rules) nil)
        (t (let* ((r (car rules))
                  (bindings (pat-match (car (cdr r)) form)))
             (cond ((match-fail-p bindings) ($rules-fire-scan (cdr rules) form))
                   (($rules-guard-ok (car (cdr (cdr (cdr r)))) bindings
                                     (car (cdr (cdr (cdr (cdr r))))))
                    (list (instantiate (car (cdr (cdr r))) bindings)))
                   (t ($rules-fire-scan (cdr rules) form)))))))

(defun $rules-settle (form fuel)
  "Re-fire rules at FORM until none applies or the per-node cap is hit."
  (if (< fuel 1)
      form
      (let ((fired ($rules-fire form)))
        (if (null fired)
            form
            ($rules-settle (car fired) (- fuel 1))))))

(defun apply-rules (form)
  "FORM rewritten bottom-up by the rulebook: children first, then rules
retried at each node to a bounded fixpoint. QUOTE/QUASIQUOTE subtrees are
data and pass through untouched."
  (cond
    ((not (consp form)) ($rules-settle form $rules-node-cap))
    ((or (eq (car form) 'quote) (eq (car form) 'quasiquote)) form)
    (t ($rules-settle
        (cons (apply-rules (car form)) ($rules-apply-cdr (cdr form)))
        $rules-node-cap))))

(defun $rules-apply-cdr (tail)
  (cond ((null tail) nil)
        ((consp tail)
         (cons (apply-rules (car tail)) ($rules-apply-cdr (cdr tail))))
        (t (apply-rules tail))))

;;; ------------------------------------------------------------------
;;; Hook into the optimizer pipeline: this redefinition upgrades
;;; lib/11-optimizer-vau.lisp's entry point now that the pattern language
;;; is loaded. Stage order: Lisp passes -> rulebook -> frame collapse ->
;;; builtin constant folder.

(defun optimize-form (form)
  "Run Lisp-level passes, the rulebook (APPLY-RULES), frame-collapse, then
the builtin constant-folder on FORM."
  (optimize (opt-collapse-frames (apply-rules (opt-pass form)))))

;;; ------------------------------------------------------------------
;;; The starter rulebook: small, safe, and each one a worked example of
;;; the idiom (structural identity + purity guard where dropping a form).
;;; OPT-PURE-P is lib/11's conservative form-purity check; guards receive
;;; matched FORMS, so they quote the variable to talk about the code.

(defrule car-of-cons (car (cons ?a ?b)) ?a
  :when (opt-pure-p ?b))

(defrule cdr-of-cons (cdr (cons ?a ?b)) ?b
  :when (opt-pure-p ?a))

(defrule append-nil (append ?x nil) ?x)

;;; REQUIRE-ABLE (issue #256): `(require 'rules)` on a with_prelude()
;;; environment loads exactly this file. with_stdlib() still loads it
;;; unconditionally, unchanged.
(provide 'rules)
