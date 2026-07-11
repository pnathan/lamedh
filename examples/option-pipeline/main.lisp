;;; option-pipeline -- config lookup without nil ambiguity.
;;; Shows: Option end to end -- a store where () is a legitimate VALUE,
;;; chained fallbacks, unwrap-or defaults, and mapping inside the option.
;;; Run: cargo run -- examples/option-pipeline/main.lisp

(def $cli (make-hash-table))
(def $env-cfg (make-hash-table))
(def $defaults (make-hash-table))

(put! $cli 'verbose ())               ; explicitly set to NIL on the CLI!
(put! $env-cfg 'port 8080)
(put! $defaults 'port 80)
(put! $defaults 'verbose t)
(put! $defaults 'workers 4)

(defun lookup (table key)
  "Presence-honest read: (some v) even when v is ()."
  (if (has-key-p table key) (some (gethash table key)) (none)))

(defun or-else (opt thunk)
  "OPT if present, else the option THUNK produces."
  (variant-case opt
    (some (v) opt)
    (none () (funcall thunk))))

(defun config (key)
  "CLI beats env beats defaults."
  (or-else (lookup $cli key)
           (lambda () (or-else (lookup $env-cfg key)
                               (lambda () (lookup $defaults key))))))

(for-each '(port verbose workers missing)
  (lambda (k)
    (variant-case (config k)
      (some (v) (format t "~a = ~a~%" k v))
      (none () (format t "~a is unset~%" k)))))

;; self-check: the () set on the CLI WINS over the default t -- the
;; whole point of Option over nil-punning; unwrap-or gives defaults;
;; option-map transforms in place.
(if (and (equal (config 'port) (some 8080))
         (equal (config 'verbose) (some ()))   ; present, value nil
         (equal (config 'missing) (none))
         (= (unwrap-or (config 'port) 0) 8080)
         (= (unwrap-or (config 'missing) 0) 0)
         (equal (option-map #'1+ (config 'port)) (some 8081)))
    (print 'ok)
    (error "option-pipeline self-check failed"))
