;; TEXT module: explicit String <-> UTF-8 Array<Char> boundary (issue #254,
;; epic #253). Exercises the qualified names directly and via IMPORT, plus
;; ASCII / multibyte / non-BMP / empty / invalid-UTF-8 coverage.

(import text)

(deftest text254-qualified-names-available
  (assert-true (module-p 'text))
  (assert-equal (module-exports 'text)
                '(string->utf8 utf8->string utf8->string-lossy)))

(deftest text254-ascii-roundtrip
  (let ((bytes (string->utf8 "hello")))
    (assert-true (arrayp bytes))
    (assert-equal (array-length* bytes) 5)
    (assert-equal (fetch bytes 0) (make-char 104)) ; 'h'
    (assert-equal (utf8->string bytes) "hello")))

(deftest text254-multibyte-roundtrip
  ;; "café" — the é is a 2-byte UTF-8 sequence, so the byte count exceeds
  ;; the 4 Unicode-scalar STRING-LENGTH*.
  (let ((s "café"))
    (assert-equal (string-length* s) 4)
    (assert-equal (array-length* (string->utf8 s)) 5)
    (assert-equal (utf8->string (string->utf8 s)) s)))

(deftest text254-non-bmp-roundtrip
  ;; U+1F389 PARTY POPPER: a non-BMP scalar, 4 UTF-8 bytes, one Unicode
  ;; scalar / one-character STRING per the epic's indexing rule.
  (let ((s (code-char 127881)))
    (assert-equal (string-length* s) 1)
    (assert-equal (array-length* (string->utf8 s)) 4)
    (assert-equal (utf8->string (string->utf8 s)) s)))

(deftest text254-cjk-roundtrip
  (let ((s "世界"))
    (assert-equal (string-length* s) 2)
    (assert-equal (array-length* (string->utf8 s)) 6)
    (assert-equal (utf8->string (string->utf8 s)) s)))

(deftest text254-empty-string
  (let ((bytes (string->utf8 "")))
    (assert-true (arrayp bytes))
    (assert-equal (array-length* bytes) 0)
    (assert-equal (utf8->string bytes) "")))

(deftest text254-invalid-utf8-strict-errors
  ;; A lone UTF-8 continuation byte (0x80) is never valid on its own.
  (let ((bad (list->array (list (make-char 128)))))
    (assert-nil (errorset (list 'utf8->string (list 'quote bad))))))

(deftest text254-invalid-utf8-lossy-substitutes
  (let ((bad (list->array (list (make-char 104) (make-char 128) (make-char 105)))))
    (assert-equal (utf8->string-lossy bad) "h�i")))

(deftest text254-qualified-alongside-unqualified
  ;; Arrays compare by identity (like other mutable containers here), so
  ;; compare contents via array->list rather than the freshly-built arrays.
  (assert-equal (array->list (text:string->utf8 "hi"))
                (array->list (string->utf8 "hi")))
  (assert-equal (text:utf8->string (text:string->utf8 "hi")) "hi"))
