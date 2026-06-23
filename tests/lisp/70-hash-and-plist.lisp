;; Hash table and property list (plist) coverage.
;; Distinct symbol names are used throughout to avoid cross-test interference.

(deftest hash-make
  ;; make-hash-table returns a usable hash table object
  (let ((h (make-hash-table)))
    (assert-true (not (null h)))))

(deftest hash-set-get
  (let ((h (make-hash-table)))
    (progn
      (set-bang h 'hkey 42)
      (assert-equal (get h 'hkey) 42))))

(deftest hash-keys
  (let ((h (make-hash-table)))
    (progn
      (set-bang h 'hkeyonly 99)
      (assert-equal (keys h) '(HKEYONLY)))))

(deftest hash-delete
  (let ((h (make-hash-table)))
    (progn
      (set-bang h 'hdel-key 7)
      (delete-key-bang h 'hdel-key)
      (assert-nil (get h 'hdel-key)))))

(deftest plist-putp-getp
  ;; putp/getp use string keys
  (progn
    (putp 'plist-test-sym-1 "mykey" 99)
    (assert-equal (getp 'plist-test-sym-1 "mykey") 99)))

(deftest plist-plist
  ;; plist returns the full key-value list for a symbol
  (progn
    (putp 'plist-test-sym-2 "pk" 55)
    (assert-equal (plist 'plist-test-sym-2) '("pk" 55))))

(deftest plist-remprop
  ;; remprop removes a plist entry; getp then returns nil
  (progn
    (putp 'plist-test-sym-3 "rk" 77)
    (remprop 'plist-test-sym-3 "rk")
    (assert-nil (getp 'plist-test-sym-3 "rk"))))

(deftest flag-set-clear
  ;; set-flag / flag-set-p / clear-flag
  (progn
    (set-flag 'flag-test-myflag)
    (assert-true (flag-set-p 'flag-test-myflag))
    (clear-flag 'flag-test-myflag)
    (assert-false (flag-set-p 'flag-test-myflag))))
