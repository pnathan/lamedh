;;; Set, alist, and hash-table helpers (issue #145, epic #141).
;;;
;;; List-as-set ops and alist helpers are non-mutating (they return new lists).
;;; Hash-table helpers build on MAKE-HASH-TABLE / GETHASH / SET-BANG / KEYS /
;;; DELETE-KEY-BANG. (Hash tables are mutable cells; the deferred-mutation
;;; decision in #114 concerns cons cells, not hash tables.)

;;; ---- list as set ---------------------------------------------------------

(defun adjoin (item lst)
  "Return LST with ITEM added to the front unless an EQUAL element is present."
  (if (member item lst) lst (cons item lst)))

(defun union (a b)
  "Set union of lists A and B (EQUAL), keeping A's elements first."
  (cond ((null a) b)
        ((member (car a) b) (union (cdr a) b))
        (t (cons (car a) (union (cdr a) b)))))

(defun intersection (a b)
  "Elements of A that also appear in B (EQUAL), in A's order."
  (cond ((null a) nil)
        ((member (car a) b) (cons (car a) (intersection (cdr a) b)))
        (t (intersection (cdr a) b))))

(defun set-difference (a b)
  "Elements of A that do not appear in B (EQUAL), in A's order."
  (cond ((null a) nil)
        ((member (car a) b) (set-difference (cdr a) b))
        (t (cons (car a) (set-difference (cdr a) b)))))

;;; ---- association lists ----------------------------------------------------

(defun rassoc (val alist)
  "Return the first cell of ALIST whose CDR is EQUAL to VAL, or NIL."
  (cond ((null alist) nil)
        ((equal (cdr (car alist)) val) (car alist))
        (t (rassoc val (cdr alist)))))

(defun alist-get (a b)
  "Return the value associated with a key in an alist (EQUAL), or NIL.
Accepts (alist-get alist key) or Elisp-style (alist-get key alist) —
whichever argument is the list is treated as the alist (issue #246).
When both are lists (a list-valued key), the historical (alist key)
order applies."
  (let* ((alist (if (listp a) a b))
         (key (if (listp a) b a))
         (cell (assoc key alist)))
    (if cell (cdr cell) nil)))

(defun alist-put (alist key val)
  "Return a new alist with KEY mapped to VAL (replacing any existing entry)."
  (cond ((null alist) (list (cons key val)))
        ((equal (car (car alist)) key)
         (cons (cons key val) (cdr alist)))
        (t (cons (car alist) (alist-put (cdr alist) key val)))))

;;; ---- hash tables ----------------------------------------------------------

(defun has-key-p (table key)
  "True if KEY is present in hash TABLE."
  (not (null (member key (keys table)))))

(defun gethash-or (table key default)
  "Return the value for KEY in TABLE, or DEFAULT if KEY is absent."
  (if (has-key-p table key) (gethash table key) default))

(defun hash-table-count* (table)
  "Number of entries in hash TABLE."
  (length (keys table)))

(defun maphash (a b)
  "Call (FN key value) for each entry of TABLE; return NIL.
Accepts (maphash table fn) or CL-style (maphash fn table) — the hash
table is recognised by type in either position (issue #246)."
  (let* ((table (if (hash-table-p a) a b))
         (fn (if (hash-table-p a) b a)))
    (mapc (lambda (k) (funcall fn k (gethash table k))) (keys table))
    nil))

(defun hash->alist (table)
  "Return TABLE's entries as an alist of (key . value)."
  (mapcar (lambda (k) (cons k (gethash table k))) (keys table)))

(defun alist->hash (alist)
  "Build a fresh hash table from an alist of (key . value)."
  (let ((table (make-hash-table)))
    (mapc (lambda (cell) (set-bang table (car cell) (cdr cell))) alist)
    table))

(defun clrhash (table)
  "Remove every entry from TABLE; return TABLE."
  (mapc (lambda (k) (remhash table k)) (keys table))
  table)

(defun copy-hash* (table)
  "A fresh hash table with TABLE's entries (shallow)."
  (let ((new (make-hash-table)))
    (mapc (lambda (k) (set-bang new k (gethash table k))) (keys table))
    new))
