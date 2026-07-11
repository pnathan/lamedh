;;; bank-conditions -- overdraft handling with restarts.
;;; Shows: the condition system as a protocol between low-level code
;;; that DETECTS (withdraw signals) and policy code that DECIDES.
;;; Lamedh deviation (documented in ch6): handler-bind handlers run
;;; POST-UNWIND, so the canonical shape is restarts established OUTSIDE
;;; the handler-bind -- policy owns both the restarts and the handler.
;;; Run: cargo run -- examples/bank-conditions/main.lisp

(def $accounts (make-hash-table))
(put! $accounts 'alice 100)
(put! $accounts 'bob 30)

(defun withdraw (who amount)
  "Detection only: succeed or signal. No policy here."
  (let ((balance (gethash $accounts who)))
    (if (> amount balance)
        (error (concat "overdraft: " (princ-to-string who)))
        (progn (put! $accounts who (- balance amount)) amount))))

(defun withdraw-with-policy (who amount policy)
  "POLICY is cap-at-balance or refuse-with-zero -- chosen by the CALLER,
not by withdraw. The canonical restarts-around-handler shape."
  (restart-case
      (handler-bind ((error (lambda (c) (invoke-restart policy))))
        (withdraw who amount))
    (cap-at-balance ()
      (let ((balance (gethash $accounts who)))
        (put! $accounts who 0)
        balance))
    (refuse-with-zero () 0)))

;; Policy A: cap at available funds.
(def $got-a (withdraw-with-policy 'bob 50 'cap-at-balance))
(format t "bob asked 50, got ~a (balance ~a)~%" $got-a (gethash $accounts 'bob))

;; Policy B: treat failure as zero cash dispensed.
(def $got-b (withdraw-with-policy 'bob 10 'refuse-with-zero))
(format t "bob asked 10 more, got ~a~%" $got-b)

;; No policy: an ordinary error propagates (caught here by errorset).
(def $unhandled (errorset '(withdraw 'alice 500)))

;; self-check: policies chose different restarts; balances consistent;
;; success path untouched; the bare error path still errors.
(if (and (= $got-a 30)
         (= (gethash $accounts 'bob) 0)
         (= $got-b 0)
         (= (withdraw-with-policy 'alice 40 'refuse-with-zero) 40)
         (= (gethash $accounts 'alice) 60)
         (null $unhandled))
    (print 'ok)
    (error "bank-conditions self-check failed"))
