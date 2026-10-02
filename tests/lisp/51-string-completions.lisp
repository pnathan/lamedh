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
  (assert-equal (string-capitalize "hELLO world") "Hello World")
  (assert-equal (string-capitalize "don't-stop 4ever") "Don'T-Stop 4ever")
  (assert-equal (string-capitalize "") "")
  (assert-equal (string-reverse "hello") "olleh")
  (assert-equal (string-reverse "") ""))

;;; ---- Unicode case mapping and character classes (issue #519) -------------

(deftest str519-char-case-unicode
  (assert-equal (char-downcase "É") "é")
  (assert-equal (char-upcase "é") "É")
  (assert-equal (char-upcase "λ") "Λ")
  (assert-equal (char-downcase "Σ") "σ")
  (assert-equal (char-upcase (char-code "é")) "É")
  ;; One-to-one mapping only: no one-character uppercase for sharp s.
  (assert-equal (char-upcase "ß") "ß")
  ;; Uncased characters pass through.
  (assert-equal (char-upcase "漢") "漢")
  (assert-equal (char-downcase "7") "7"))

(deftest str519-string-case-unicode
  (assert-equal (string-upcase "straße") "STRASSE")
  (assert-equal (string-upcase "café") "CAFÉ")
  (assert-equal (string-downcase "CAFÉ") "café")
  (assert-equal (string-upcase "αβγ") "ΑΒΓ")
  ;; Full mapping: word-final capital sigma lowercases to final sigma.
  (assert-equal (string-downcase "ΟΔΟΣ") "οδος")
  (assert-equal (string-upcase "漢字 ok") "漢字 OK")
  (assert-equal (string-upcase "") "")
  (assert-equal (string-capitalize "élan vital") "Élan Vital")
  (assert-equal (string-capitalize "ÉTÉ été") "Été Été"))

(deftest str519-char-classes-unicode
  (assert-true  (alpha-p "é"))
  (assert-true  (alpha-p "Λ"))
  (assert-true  (alpha-p "漢"))
  (assert-true  (alphanumeric-p "é"))
  (assert-true  (alphanumeric-p "ß"))
  (assert-true  (alphanumeric-p "٣"))
  (assert-false (alphanumeric-p "—"))
  (assert-false (alpha-p "٣"))
  (assert-true  (char-upper-p "É"))
  (assert-false (char-upper-p "é"))
  (assert-true  (char-lower-p "ß"))
  (assert-true  (char-lower-p "σ"))
  (assert-false (char-upper-p "漢"))
  (assert-false (char-lower-p "漢"))
  ;; DIGIT-P stays ASCII: parsers rely on it.
  (assert-false (digit-p "٣")))

(deftest str519-ascii-unchanged
  ;; Every ASCII code point classifies and case-maps exactly as the old
  ;; A-Z / a-z / 0-9 range checks did.
  (assert-true
   (every (lambda (code)
            (let ((up (and (>= code 65) (<= code 90)))
                  (lo (and (>= code 97) (<= code 122)))
                  (dg (and (>= code 48) (<= code 57))))
              (and (eq (not (alpha-p code)) (not (or up lo)))
                   (eq (not (alphanumeric-p code)) (not (or up lo dg)))
                   (eq (not (char-upper-p code)) (not up))
                   (eq (not (char-lower-p code)) (not lo))
                   (equal (char-upcase code)
                          (code-char (if lo (- code 32) code)))
                   (equal (char-downcase code)
                          (code-char (if up (+ code 32) code))))))
          (iota 128)))
  (assert-equal (string-upcase "Hello, World! 123") "HELLO, WORLD! 123")
  (assert-equal (string-downcase "Hello, World! 123") "hello, world! 123"))

(deftest str519-kernel-primitives
  ;; Non-scalar code points (surrogates, beyond U+10FFFF) are in no class.
  (assert-false (char-alphabetic-p* 55296))
  (assert-false (char-alphabetic-p* 1114112))
  (assert-false (char-alphabetic-p* -1))
  (assert-nil (errorset '(char-alphabetic-p* "a")))
  (assert-nil (errorset '(string-upcase* 1))))
