;; Conformance cases: conditions, HANDLER-CASE, ERRORSET.
;; Message text is not portable: cases compare messages only where the spec fixes them.
;; Format: (case ID SPEC-REF SOURCE EXPECT). See conformance/README.md.

(case "error/make-error-message" "KERNEL.md Part VIII: make-error builds a two-field condition" "(error-message (make-error \"boom\" 7))" "\"boom\"")
(case "error/make-error-data" "KERNEL.md Part VIII: make-error data field" "(error-data (make-error \"boom\" 7))" "7")
(case "error/handler-case-catches-signal" "KERNEL.md Part VIII: HANDLER-CASE catches an (error ...) signal" "(handler-case (error \"boom\") (error (e) (error-message e)))" "\"boom\"")
(case "error/handler-case-data-field" "KERNEL.md Part VIII: (error message data) signals data" "(handler-case (error \"m\" 42) (error (e) (error-data e)))" "42")
(case "error/error-no-arguments-message" "KERNEL.md Part VIII: (error) signals message \"Error\"" "(handler-case (error) (error (e) (error-message e)))" "\"Error\"")
(case "error/errorset-failure-is-nil" "KERNEL.md Part VIII: ERRORSET returns NIL on failure" "(errorset '(car 5))" "()")
(case "error/errorset-nil-success-distinct" "KERNEL.md Part VIII: The ERRORSET wrapper distinguishes a NIL success" "(errorset nil)" "(())")
(case "error/throw-not-caught-by-handler" "KERNEL.md Part VIII: Non-local exits are invisible to HANDLER-CASE" "(handler-case (throw 'foo 1) (error (e) 'caught))" "ERROR")
(case "error/unbound-variable-is-caught" "KERNEL.md Part VIII: Native errors such as unbound variables are catchable" "(handler-case unbound-conformance-var (error (e) 'caught))" "CAUGHT")
