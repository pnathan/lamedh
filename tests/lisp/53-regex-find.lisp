;; REGEX:FIND with a START offset (lib/44-regex.lisp): the search begins at
;; the character index START but the match still sees the whole string, so
;; anchors and word boundaries keep their meaning relative to S, not to START.

(require 'regex)

(deftest regex-find-start-keeps-anchor-meaning
  ;; ^ means the start of S, not of the search window.
  (assert-nil   (regex:find (regex:compile "^h") "xhh" 1))
  (assert-equal (regex:find (regex:compile "^h") "hxh" 0) '("h" 0 1))
  (assert-nil   (regex:find (regex:compile "^h") "hxh" 2))
  ;; \b looks at the character before START.
  (assert-equal (regex:find (regex:compile "\\bfoo") "afoo foo" 1) '("foo" 5 8))
  ;; \B: START 1 is inside the word "foo", so "oo" there is not at a boundary.
  (assert-equal (regex:find (regex:compile "\\Boo") "foo foo" 1) '("oo" 1 3))
  (assert-nil   (regex:find (regex:compile "\\Bfoo") "foo foo" 4)))

(deftest regex-find-start-positions-are-character-indices
  (assert-equal (regex:find (regex:compile "l") "héllo" 3) '("l" 3 4))
  (assert-equal (regex:find (regex:compile "o") "日本語o" 2) '("o" 3 4)))

(deftest regex-find-start-bounds
  (assert-equal (regex:find (regex:compile "a+") "xaay" 2) '("a" 2 3))
  (assert-nil   (regex:find (regex:compile "a+") "xaay" 3))
  (assert-nil   (regex:find (regex:compile "a+") "xaay" 99))
  (assert-equal (regex:find (regex:compile "$") "ab" 2) '("" 2 2)))

(deftest regex-find-stepping-agrees-with-find-all
  ;; Walking REGEX:FIND forward from each match's end must visit the same
  ;; matches REGEX:FIND-ALL reports.
  (let ((re (regex:compile "\\bfoo"))
        (s "afoo foo foo"))
    (assert-equal (regex:find-all re s) '(("foo" 5 8) ("foo" 9 12)))
    (assert-equal (regex:find re s 0) '("foo" 5 8))
    (assert-equal (regex:find re s 8) '("foo" 9 12))
    (assert-nil   (regex:find re s 12))))
