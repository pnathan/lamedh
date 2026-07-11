;; String API completions (issue #254, epic #253): construction/access,
;; the full comparison family, and the remaining search/transformation ops.

;;; ---- construction and access -----------------------------------------

(deftest str254-make-string
  (assert-equal (make-string 5) "     ")
  (assert-equal (make-string 3 "x") "xxx")
  (assert-equal (make-string 0) "")
  (assert-equal (make-string 3 (char-code "z")) "zzz")
  (assert-nil (errorset '(make-string -1))))

(deftest str254-empty-p
  (assert-true  (string-empty-p ""))
  (assert-false (string-empty-p "a")))

(deftest str254-concat
  (assert-equal (string-concat "a" "b" "c") "abc")
  (assert-equal (string-concat) ""))

(deftest str254-char-at
  (assert-equal (char-at "hello" 0) "h")
  (assert-equal (char-at "hello" 4) "o")
  (assert-nil (errorset '(char-at "hello" 5)))
  (assert-nil (errorset '(char-at "hello" -1))))

;;; ---- comparison ---------------------------------------------------------

(deftest str254-case-sensitive-family
  (assert-true  (string-ne "a" "b"))
  (assert-false (string-ne "a" "a"))
  (assert-true  (string< "abc" "abd"))
  (assert-false (string< "abd" "abc"))
  (assert-true  (string> "abd" "abc"))
  (assert-true  (string<= "abc" "abc"))
  (assert-true  (string<= "abc" "abd"))
  (assert-false (string<= "abd" "abc"))
  (assert-true  (string>= "abc" "abc"))
  (assert-true  (string>= "abd" "abc"))
  (assert-false (string>= "abc" "abd"))
  ;; STRING< agrees with the pre-existing STRING-LESSP.
  (assert-equal (string< "Zebra" "apple") (string-lessp "Zebra" "apple")))

(deftest str254-case-insensitive-family
  (assert-true  (string-ci= "ABC" "abc"))
  (assert-false (string-ci= "ABC" "abd"))
  (assert-true  (string-ci-ne "ABC" "abd"))
  (assert-false (string-ci-ne "ABC" "abc"))
  (assert-true  (string-ci< "abc" "ABD"))
  (assert-true  (string-ci> "ABD" "abc"))
  (assert-true  (string-ci<= "ABC" "abc"))
  (assert-true  (string-ci>= "ABC" "abc"))
  ;; Unicode-aware, not ASCII-only: non-ASCII letters case-fold too.
  (assert-true (string-ci= "MÜNCHEN" "münchen"))
  (assert-true (string-ci= "ΣΊΓΜΑ" "σίγμα")))

;;; ---- search and transformation ------------------------------------------

(deftest str254-last-index-of
  (assert-equal (string-last-index-of "abcabc" "bc") 4)
  (assert-equal (string-last-index-of "abcabc" "z") nil)
  (assert-equal (string-last-index-of "abc" "") nil))

(deftest str254-count
  (assert-equal (string-count "abcabcabc" "abc") 3)
  (assert-equal (string-count "aaaa" "aa") 2)
  (assert-equal (string-count "abc" "z") 0)
  (assert-equal (string-count "abc" "") 0))

(deftest str254-replace-first-vs-all
  (assert-equal (string-replace-first "aaa" "a" "b") "baa")
  (assert-equal (string-replace-all "aaa" "a" "b") "bbb")
  (assert-equal (string-replace "aaa" "a" "b") (string-replace-all "aaa" "a" "b")))

(deftest str254-split-empty-fields
  (assert-equal (string-split ",a,,b," ",") '("" "a" "" "b" ""))
  (assert-equal (string-split "abc" ",") '("abc")))

(deftest str254-trim-sides
  (assert-equal (string-trim-left "  hi  ") "hi  ")
  (assert-equal (string-trim-right "  hi  ") "  hi")
  (assert-equal (string-trim "  hi  ") "hi")
  (assert-equal (string-trim-left "hi") "hi")
  (assert-equal (string-trim-right "") ""))

(deftest str254-capitalize-reverse
  (assert-equal (string-capitalize "hELLO world") "Hello world")
  (assert-equal (string-capitalize "") "")
  (assert-equal (string-reverse "hello") "olleh")
  (assert-equal (string-reverse "") ""))
