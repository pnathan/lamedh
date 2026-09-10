;;; 47-typed-island.lisp -- the typed-island FRONT END: freeze, gate, settle,
;;; hand off, validate (issue #451 follow-on).
;;;
;;; ---- what this is ---------------------------------------------------------
;;;
;;; lib/46-hm-check.lisp ports the reference host's type checker: its checking
;;; mode (section 7) and its codegen mode, the compileable-type GATE (section
;;; 7b). This file is the FRONT END built on that gate. Given a set of function
;;; names it produces a TYPED ISLAND: the largest subset that compiles AS A
;;; GROUP -- every member monomorphic over the compileable sub-lattice
;;; (int64 float64 bool char (array T) struct), every call inside it resolved
;;; to another member or to a function the kernel already has typed, every
;;; body operative-free. That set, with each member's resolved signature and
;;; frozen body, is a MANIFEST a kernel can compile without doing any
;;; inference of its own.
;;;
;;; Three kernels exist or are emerging: the reference Rust host with its
;;; Cranelift back end (`src/jit/`), the SBCL port (`sbcl/`) and the x86-64
;;; native compiler (`lamedh-asm/`). Only the first types anything today. This
;;; front end is the shared piece: the DECISION of what is compileable, made
;;; once, portably, in Lamedh; each kernel receives a manifest of pinned
;;; signatures and bodies and owns only lowering. On the reference host that
;;; hand-off is `declare-typed` + `defun-typed` (ISLAND-INSTALL!), and the
;;; host's own verdict on each member is read back and compared, so the
;;; portable front end and the native gate VALIDATE each other on every call.
;;;
;;; ---- the pipeline ---------------------------------------------------------
;;;
;;;   names
;;;     | ISLAND-SOURCE      the live plain lambda, or the source the kernel
;;;     |                    compiled (see the honesty note there)
;;;     | ISLAND-FREEZE      global macros expanded to a fixpoint, once (the
;;;     |                    typed-region "freeze": phase separation, so what
;;;     |                    is typed is what will run and no later DEFMACRO
;;;     |                    can reach into it)
;;;     | TYPED-ISLAND       the greatest stable group that compiles together
;;;     v                    (HM-COMPILE-GROUP, shrink to clean, grow by pins)
;;;   island = ((members . ((NAME SIG LAMBDA ARROW) ...)) (rejected . ((NAME . why) ...)))
;;;     | ISLAND-OPTIMIZE    OPTIMIZE-FORM on every body; re-gated under the
;;;     |                    island's own signatures as pins, a member whose
;;;     |                    type the optimizer changed keeps its original body
;;;     |                    and is reported -- the optimizer is validated, not
;;;     |                    trusted
;;;     | ISLAND-INSTALL!    hand-off to the host kernel, with read-back
;;;     v
;;;   ((NAME AGREE sig) | (NAME DISAGREE island-sig kernel-sig) | (NAME KERNEL-REJECTED msg) ...)
;;;
;;; ---- why a GROUP, and why "stable" ----------------------------------------
;;;
;;; The kernel's own per-definition `jit-optimize` compiles one function at a
;;; time in definition order, so two mutually recursive helpers, or a helper
;;; whose parameter type is only pinned by its callers, never compile natively
;;; on their own. Handed as a group with signatures declared first they do --
;;; that is exactly what `declare-typed` + `defun-typed` exist for. So the
;;; island is computed as a group: every member registered before any body is
;;; elaborated (HM-COMPILE-GROUP). Stability means two things the manifest
;;; guarantees the kernel: the set is CLOSED (no member calls anything outside
;;; the set that the kernel has not already typed) and CONSISTENT (every
;;; signature in the manifest was produced in one clean round in which every
;;; participant compiled -- see the contamination note in HM-COMPILE-GROUP and
;;; the settle/grow protocol in TYPED-ISLAND).
;;;
;;; ---- honesty --------------------------------------------------------------
;;;
;;; A name with no visible source is rejected as such, never guessed. A member
;;; the gate blocks is listed under REJECTED with the kernel's own blocker
;;; wording. ISLAND-INSTALL! reports what the kernel actually said about every
;;; member; it never reports AGREE from the island's side alone.

;;; ==========================================================================
;;; 1. Source: what will be typed is what will run.
;;; ==========================================================================

(defun island-typed-plist-source (name)
  "(PARAMS . BODY) for a NAME whose live value is the host's typed membrane.

HM-LAMBDA-SOURCE reads only the live value, because a `source-form` plist
entry can outlive the binding it described (a `def`/`setq` rebinding never
clears it). Here the plist is trusted in exactly ONE situation, and it is the
situation in which it cannot be stale: the host reports the name TYPED (its
live value is the kernel's membrane for THIS name) and the entry was written
by the very act that installed that membrane -- `jit-optimize`, `defun*` and
`defun-typed` all record the form as they bind. Three shapes are recognised:
`(lambda (p...) body...)` (jit-optimize), `(defun* name (p-or-(p ty) ...)
body...)` and `(defun-typed (name ret) ((p ty) ...) body...)`; any type
annotation is DROPPED, the gate re-derives it. Anything else is NIL."
  (if (not (and (boundp 'see-type)
                (eq (car (handler-case (see-type name) (error (e) nil))) 'typed)))
      nil
      (let ((form (handler-case (see-source name) (error (e) nil))))
        (cond
          ((and (consp form) (eq (car form) 'lambda) (consp (cdr form)))
           (island-plain-source (cadr form) (hm-progn-body (cddr form))))
          ((and (consp form) (eq (car form) 'defun*)
                (consp (cdr form)) (consp (cddr form)))
           (island-plain-source (island-strip-annotations (caddr form))
                                (cdddr form)))
          ((and (consp form) (eq (car form) 'defun-typed)
                (consp (cdr form)) (consp (cddr form)))
           (island-plain-source (island-strip-annotations (caddr form))
                                (cdddr form)))
          (t nil)))))

(defun island-strip-annotations (params)
  "A `((p ty) ...)` or mixed parameter list as bare names; a non-list stays
as it is (and then fails ISLAND-PLAIN-SOURCE's shape test)."
  (if (consp params)
      (mapcar (lambda (p) (if (consp p) (car p) p)) params)
      params))

(defun island-annotated-source-p (name)
  "Was the source the kernel compiled for NAME ANNOTATED -- a `defun-typed`,
or a `defun*` with at least one typed parameter? An annotated member's
signature was pinned by its author, not inferred, so the honest portable
re-derivation of it runs under the same pins (see the parity sweep in
tests/test_typed_island.rs)."
  (let ((form (handler-case (see-source name) (error (e) nil))))
    (cond
      ((not (consp form)) nil)
      ((eq (car form) 'defun-typed) t)
      ((eq (car form) 'defun*)
       (and (consp (cddr form)) (consp (caddr form))
            (if (exists #'consp (caddr form)) t nil)))
      (t nil))))

(defun island-plain-source (params body)
  "(PARAMS . BODY) when PARAMS is a flat list of bare symbols and BODY is
non-empty; NIL otherwise (variadic and keyword lambda lists are outside the
monomorphic island)."
  (if (and (every #'symbolp params)
           (not (exists (lambda (m) (member m params)) '(&rest &optional &key)))
           body)
      (cons params body)
      nil))

(defun island-source (name)
  "(PARAMS . BODY) for NAME, or NIL: the live plain lambda first, then the
source the kernel compiled (ISLAND-TYPED-PLIST-SOURCE)."
  (let ((live (hm-lambda-source name)))
    (if live live (island-typed-plist-source name))))

;;; ==========================================================================
;;; 2. Freeze: expand global macros to a fixpoint, once.
;;; ==========================================================================
;;;
;;; The gate types applicative code. A body written with `when`, `incf`,
;;; `dotimes` is applicative only after expansion, and the kernel's own
;;; codegen path never expands (a macro head is "call to unknown function" to
;;; it). Freezing runs every GLOBAL macro to its fixpoint before typing, and
;;; the frozen residue -- not the live macro table -- is what enters the
;;; manifest. That is the typed-region freeze (docs/typed-region-design.md
;;; section 4): a DEFMACRO after the freeze cannot reach code that no longer
;;; mentions it. Local operator bindings (MACROLET-style) are not expanded
;;; here and, being operatives to the gate, block the member -- correctly.
;;;
;;; The walker expands a head that is currently bound to a macro and recurses
;;; into every other cons, skipping QUOTE/QUASIQUOTE data. It does not know
;;; binding forms, so a LOCAL VARIABLE named like a global macro would be
;;; expanded at its use sites; such a body types wrongly and is REJECTED by
;;; the gate rather than mistyped into the island.

(def $island-freeze-fuel 256)

(defun island-macro-head-p (head)
  (and (symbolp head) head (boundp head)
       (macrop (handler-case (eval head) (error (e) nil)))))

(defun island-freeze (form)
  "FORM with every global macro call expanded, recursively, to a fixpoint."
  (island-freeze-fuelled form $island-freeze-fuel))

(defun island-freeze-fuelled (form fuel)
  (cond
    ((not (consp form)) form)
    ((member (car form) '(quote quasiquote)) form)
    ((island-macro-head-p (car form))
     (if (<= fuel 0)
         (error (concat "island-freeze: macro expansion of `"
                        (princ-to-string (car form)) "` does not terminate"))
         (island-freeze-fuelled (macroexpand form) (- fuel 1))))
    (t (island-freeze-list form fuel))))

(defun island-freeze-list (forms fuel)
  (cond
    ((null forms) nil)
    ((consp forms) (cons (island-freeze-fuelled (car forms) fuel)
                         (island-freeze-list (cdr forms) fuel)))
    (t forms)))

;;; ==========================================================================
;;; 3. The island: discover in one state, then verify in a clean one.
;;; ==========================================================================
;;;
;;; Two rounds of a different character.
;;;
;;; DISCOVERY (ISLAND-DISCOVER) runs the group in ONE codegen state, every
;;; member registered under a provisional arrow, and elaborates every member
;;; repeatedly -- pass after pass, until the set that compiles stops growing.
;;; Passes matter because the gate resolves eagerly: `(defun addp (a b) (+ a
;;; b))` blocks on its own (`+` cannot pick a kind), but a caller `(addp x
;;; 1.5)` elaborated in the same state pins A and B to FLOAT64, and on the
;;; next pass ADDP compiles. A member that fails leaves its partial bindings
;;; behind on purpose: that is how a caller's knowledge reaches a callee.
;;; The set of compiling members grows monotonically (a member whose
;;; signature has resolved has nothing left that a later binding could
;;; contradict), so this terminates in at most one pass per member.
;;;
;;; VERIFICATION (ISLAND-SETTLE) then re-runs the discovered members from a
;;; FRESH state with the discovered signatures as pins -- the portable
;;; `declare-typed` -- and shrinks the set until a round passes with no
;;; failure. Only that round's signatures enter the manifest, so no rejected
;;; member's bindings can have shaped them, and the set is closed: a member
;;; that called a rejected peer sees "call to unknown function" here and
;;; drops, and its own callers drop after it. This is exactly the protocol
;;; the kernel runs on hand-off (declare every member, define each), so what
;;; passes here is what the kernel accepts.

(defun island-collect (names)
  "(MEMBERS . REJECTED): MEMBERS are (NAME PARAMS . FROZEN-BODY) for every
name with a visible source, REJECTED the (NAME . reason) pairs for the rest."
  (let ((members nil) (rejected nil))
    (mapc (lambda (n)
            (let ((src (island-source n)))
              (if (null src)
                  (setq rejected (cons (cons n "no visible plain-lambda source") rejected))
                  (setq members
                        (cons (cons n (cons (car src)
                                            (mapcar #'island-freeze (cdr src))))
                              members)))))
          names)
    (cons (reverse members) (reverse rejected))))

(defun island-verdict-ok-p (v)
  (eq (car (cdr v)) 'compileable))

(defun island-verdict-arrow (v)
  (cadr (cdr v)))

(defun island-verdict-reason (v)
  (cadr (cdr v)))

(defun island-pins-of (verdicts)
  "NAME -> arrow for every COMPILEABLE verdict."
  (mapcar (lambda (v) (cons (car v) (island-verdict-arrow v)))
          (filter #'island-verdict-ok-p verdicts)))

(defun island-reasons-of (verdicts)
  "NAME -> reason for every BLOCKED verdict."
  (mapcar (lambda (v) (cons (car v) (island-verdict-reason v)))
          (filter (lambda (v) (not (island-verdict-ok-p v))) verdicts)))

(defun island-discover (members)
  "(PINS . REASONS) after elaborating MEMBERS in one shared codegen state until
the compiling set stops growing. PINS are the resolved arrows of the members
that compiled; REASONS the last pass's blocker for each of the rest."
  (let* ((state (hm-codegen-state))
         (reg (gethash state 'registry)))
    (mapc (lambda (m)
            (sethash reg (car m) (hm-provisional-arrow state (cadr m))))
          members)
    (island-discover-passes state reg members -1 (+ (length members) 1))))

(defun island-discover-passes (state reg members previous fuel)
  (let* ((verdicts (mapcar (lambda (m)
                             (cons (car m)
                                   (hm-compile-one state (gethash reg (car m))
                                                   (cadr m) (cddr m))))
                           members))
         (pins (island-pins-of verdicts)))
    (if (or (= (length pins) previous) (<= fuel 0))
        (cons pins (island-reasons-of verdicts))
        (island-discover-passes state reg members (length pins) (- fuel 1)))))

(defun island-restrict-pins (pins members)
  (filter (lambda (p) (assoc (car p) members)) pins))

(defun island-settle (members pins)
  "Shrink MEMBERS to a set that compiles as a whole in ONE round from a fresh
state under PINS, re-running after every removal. Returns (VERDICTS .
REJECTED): VERDICTS the clean round's compileable verdicts, REJECTED the
(NAME . reason) pairs of everything removed, each with the blocker from the
round it fell in."
  (let* ((verdicts (hm-compile-group members pins))
         (bad (island-reasons-of verdicts)))
    (if (null bad)
        (cons verdicts nil)
        (let* ((keep (filter (lambda (m) (not (assoc (car m) bad))) members))
               (inner (island-settle keep (island-restrict-pins pins keep))))
          (cons (car inner) (append bad (cdr inner)))))))

(defun typed-island (names)
  "The typed island of NAMES: the greatest stable subset that compiles as a
group under the codegen gate, as a manifest

  ((members . ((NAME SIG LAMBDA ARROW) ...))
   (rejected . ((NAME . \"why\") ...)))

SIG is the rendered monomorphic signature `(-> (T...) R)` exactly as the host's
SEE-TYPE reports a TYPED function; LAMBDA is `(lambda PARAMS . FROZEN-BODY)`,
the code the kernel is to compile; ARROW is SIG in the checker's internal
representation (the pin ISLAND-OPTIMIZE and ISLAND-INSTALL! reuse). Members
keep the order of NAMES."
  (let* ((collected (island-collect names))
         (members (car collected))
         (unsourced (cdr collected))
         (found (island-discover members))
         (pins (car found))
         (candidates (filter (lambda (m) (assoc (car m) pins)) members))
         (settled (island-settle candidates pins))
         (verdicts (car settled))
         (rejected (append (cdr settled) (cdr found))))
    (list (cons 'members
                (mapcar (lambda (m)
                          (let ((arrow (island-verdict-arrow (assoc (car m) verdicts))))
                            (list (car m)
                                  (hm-render-ty arrow nil)
                                  (cons 'lambda (cons (cadr m) (cddr m)))
                                  arrow)))
                        (filter (lambda (m) (assoc (car m) verdicts)) members)))
          (cons 'rejected
                (append (mapcar (lambda (m) (assoc (car m) rejected))
                                (filter (lambda (m) (assoc (car m) rejected)) members))
                        unsourced)))))

;;; ---- accessors ------------------------------------------------------------

(defun island-members (island) (cdr (assoc 'members island)))
(defun island-rejected (island) (cdr (assoc 'rejected island)))
(defun island-regressions (island) (cdr (assoc 'regressions island)))
(defun island-member-names (island) (mapcar #'car (island-members island)))
(defun island-member (island name) (assoc name (island-members island)))
(defun island-signature (island name)
  "NAME's rendered signature in ISLAND, or NIL when it is not a member."
  (let ((m (island-member island name))) (if m (cadr m) nil)))
(defun island-member-lambda (island name)
  (let ((m (island-member island name))) (if m (caddr m) nil)))
(defun island-member-arrow (m) (nth 3 m))
(defun island-rejection (island name)
  "Why NAME is not a member of ISLAND, or NIL when it is."
  (let ((r (assoc name (island-rejected island)))) (if r (cdr r) nil)))

;;; ==========================================================================
;;; 4. Optimize, then re-gate: the optimizer is validated, not trusted.
;;; ==========================================================================

(defun island-group-of (members)
  "Island MEMBERS as HM-COMPILE-GROUP input: (NAME PARAMS . BODY)."
  (mapcar (lambda (m) (cons (car m) (cdr (caddr m)))) members))

(defun island-pins (island)
  (mapcar (lambda (m) (cons (car m) (island-member-arrow m))) (island-members island)))

(defun island-optimize (island)
  "ISLAND with every member body run through OPTIMIZE-FORM (the compiler
pipeline hook: Lisp passes, the rulebook, frame collapse, constant folding) and
then RE-GATED as a group under the island's own signatures as pins. A member
whose optimized body still compiles under its pinned signature takes the
optimized body; one whose optimized body does not -- which is how a changed
type shows up under a pin: as a mismatch against the signature -- or on which
the optimizer signalled, KEEPS its original body and is listed under
REGRESSIONS as (NAME . reason). The member set and every signature are
unchanged by construction."
  (let* ((members (island-members island))
         (pins (island-pins island))
         (optimized
          (mapcar (lambda (m)
                    (handler-case
                        (let ((lam (optimize-form (caddr m))))
                          (if (and (consp lam) (eq (car lam) 'lambda) (consp (cdr lam))
                                   (cddr lam))
                              (list (car m) (cadr m) lam (island-member-arrow m))
                              (list (car m) (cadr m) (caddr m) (island-member-arrow m)
                                    "optimizer did not return a lambda")))
                      (error (e)
                        (list (car m) (cadr m) (caddr m) (island-member-arrow m)
                              (concat "optimizer signalled: " (error-message e))))))
                  members))
         (verdicts (hm-compile-group (island-group-of optimized) pins))
         (regressions nil)
         (final
          (mapcar (lambda (o m)
                    (let* ((why (nth 4 o))
                           (v (assoc (car o) verdicts)))
                      (cond
                        (why (progn (setq regressions (cons (cons (car o) why) regressions))
                                    m))
                        ((not (island-verdict-ok-p v))
                         (progn (setq regressions
                                      (cons (cons (car o)
                                                  (concat "optimized body no longer compiles: "
                                                          (island-verdict-reason v)))
                                            regressions))
                                m))
                        (t (list (car o) (cadr o) (caddr o) (island-member-arrow m))))))
                  optimized members)))
    (list (cons 'members final)
          (cons 'rejected (island-rejected island))
          (cons 'regressions (reverse regressions)))))

;;; ==========================================================================
;;; 5. Hand-off to the kernel, with read-back.
;;; ==========================================================================

(defun island-typed-params (m)
  "((p T) ...) for member M, in the annotation grammar `defun-typed` reads."
  (mapcar (lambda (p ty) (list p (hm-render-ty ty nil)))
          (cadr (caddr m))
          (cadr (island-member-arrow m))))

(defun island-return-type (m)
  (hm-render-ty (caddr (island-member-arrow m)) nil))

(defun island-declare-typed-form (m)
  "`(declare-typed (NAME RET) ((p T) ...))` for member M."
  (list 'declare-typed (list (car m) (island-return-type m)) (island-typed-params m)))

(defun island-defun-typed-form (m)
  "`(defun-typed (NAME RET) ((p T) ...) body...)` for member M."
  (cons 'defun-typed
        (cons (list (car m) (island-return-type m))
              (cons (island-typed-params m)
                    (cddr (caddr m))))))

(defun island-forms (island)
  "The hand-off as data: every member's `declare-typed` form followed by every
member's `defun-typed` form -- the kernel protocol (declare all, define each)
spelled out so a host with no `eval`-time kernel can consume it offline."
  (append (mapcar #'island-declare-typed-form (island-members island))
          (mapcar #'island-defun-typed-form (island-members island))))

(defun island-kernel-p ()
  "Does this host have a typed kernel to hand an island to?"
  (and (boundp 'see-type) (boundp 'signature) t))

(defun island-install-one! (m e)
  (handler-case
      (progn
        (eval (island-defun-typed-form m) e)
        (if (boundp 'see-type)
            (let ((native (see-type (car m))))
              (cond
                ((not (eq (car native) 'typed))
                 (list (car m) 'kernel-silent native))
                ((equal (cadr native) (cadr m))
                 (list (car m) 'agree (cadr m)))
                (t (list (car m) 'disagree (cadr m) (cadr native)))))
            (list (car m) 'installed (cadr m))))
    (error (err) (list (car m) 'kernel-rejected (error-message err)))))

(defun island-install-in! (island e)
  (let ((members (island-members island)))
    (mapc (lambda (m)
            (handler-case (eval (island-declare-typed-form m) e)
              (error (err) nil)))
          members)
    (mapcar (lambda (m) (island-install-one! m e)) members)))

(defvau island-install! (x e)
  "Hand ISLAND to the host kernel and read its verdict back, member by member.

  (island-install! island)

Every member is first forward-declared with its island signature
(`declare-typed`, so mutual recursion and caller-pinned helpers resolve), then
defined with it (`defun-typed`); the definitions land in the CALLER's
environment, like `edit!`. Returns one entry per member:

  (NAME AGREE sig)                       the kernel compiled it and reports
                                         the island's signature
  (NAME DISAGREE island-sig kernel-sig)  the kernel compiled it under a
                                         different signature
  (NAME KERNEL-REJECTED \"msg\")           the kernel refused the definition
  (NAME KERNEL-SILENT verdict)           installed, but SEE-TYPE does not
                                         report it TYPED
  (NAME INSTALLED sig)                   a host with no SEE-TYPE to read back

AGREE on every member is the two gates -- portable front end and native
kernel -- validating each other on this island. Note the hand-off REBINDS
each member to the kernel's typed entry: a call with arguments outside the
signature is then a membrane error, not a fall-back to the dynamic closure."
  (island-install-in! (eval (car x) e) e))

(defun island-agreement (report)
  "(AGREED . DISPUTED) from an ISLAND-INSTALL! report: the names the kernel
agreed on, and every other entry verbatim."
  (cons (mapcar #'car (filter (lambda (r) (eq (cadr r) 'agree)) report))
        (filter (lambda (r) (not (eq (cadr r) 'agree))) report)))

;;; ==========================================================================
;;; 6. Reporting.
;;; ==========================================================================

(defun island-summary (island)
  "((members . n) (rejected . m) [(regressions . k)])."
  (append (list (cons 'members (length (island-members island)))
                (cons 'rejected (length (island-rejected island))))
          (if (assoc 'regressions island)
              (list (cons 'regressions (length (island-regressions island))))
              nil)))

;;; Registered as a module for introspection (issue #56); the surface stays
;;; FLAT like the checker's.
(require 'modules)
(defmodule typed-island
  (:export typed-island island-members island-rejected island-regressions
           island-member-names island-member island-signature
           island-member-lambda island-rejection island-optimize
           island-forms island-install! island-agreement island-summary
           island-source island-annotated-source-p island-freeze
           island-kernel-p))
(provide 'typed-island)
