;;; Help System Infrastructure
;;; Provides (help) function for interactive documentation

;; Global documentation database
(def HELP-DB (make-hash-table))

;; Categories for browsing
(def HELP-CATEGORIES nil)

;;; Documentation Entry Structure
;;; Each entry is an alist with keys:
;;;   :name       - Symbol name
;;;   :type       - 'function, 'special-form, 'macro, 'variable
;;;   :syntax     - String showing calling convention
;;;   :args       - List of (arg-name description) pairs
;;;   :returns    - What the function returns
;;;   :description - Full description
;;;   :examples   - List of (code result) pairs
;;;   :see-also   - List of related symbols
;;;   :category   - Category for browsing

(defun make-doc-entry (name type syntax description)
  "Create a basic documentation entry."
  (list (cons 'NAME name)
        (cons 'TYPE type)
        (cons 'SYNTAX syntax)
        (cons 'DESCRIPTION description)))

(defun doc-get (entry key)
  "Get a field from a documentation entry."
  (let ((pair (assoc key entry)))
    (if pair (cdr pair) nil)))

(defun doc-set (entry key value)
  "Add or update a field in a documentation entry."
  (cons (cons key value) entry))

;;; Registration Functions

(defun register-doc (name entry)
  "Register a documentation entry in the help database."
  (set-bang HELP-DB name entry)
  name)

(defun register-category (cat-name description symbols)
  "Register a category of related symbols."
  (setq HELP-CATEGORIES
        (cons (list cat-name description symbols)
              HELP-CATEGORIES)))

;;; Lookup Functions

(defun get-doc (name)
  "Get documentation for a symbol."
  (gethash HELP-DB name))

(defun list-categories ()
  "List all documentation categories."
  HELP-CATEGORIES)

(defun list-category (cat-name)
  "List symbols in a category."
  (let ((cat (assoc cat-name HELP-CATEGORIES)))
    (if cat (caddr cat) nil)))

;;; Display Functions

(defun help-print-line (s)
  "Print a line of help text."
  (princ s)
  (terpri))

(defun help-print-section (title content)
  "Print a titled section."
  (if content
      (progn
        (terpri)
        (princ title)
        (princ ":")
        (terpri)
        (princ "  ")
        (princ content)
        (terpri))
      nil))

(defun help-print-examples (examples)
  "Print examples section."
  (if examples
      (progn
        (terpri)
        (princ "Examples:")
        (terpri)
        (mapcar examples
                (lambda (ex)
                  (princ "  ")
                  (prin1 (car ex))
                  (terpri)
                  (princ "  => ")
                  (prin1 (cadr ex))
                  (terpri))))
      nil))

(defun help-print-see-also (syms)
  "Print see-also section."
  (if syms
      (progn
        (terpri)
        (princ "See also: ")
        (princ (car syms))
        (mapcar (cdr syms)
                (lambda (s)
                  (princ ", ")
                  (princ s)))
        (terpri))
      nil))

(defun display-doc (entry)
  "Display a documentation entry."
  (if entry
      (progn
        (terpri)
        (princ "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━")
        (terpri)
        ;; Name and type
        (princ (doc-get entry 'NAME))
        (princ "  [")
        (princ (doc-get entry 'TYPE))
        (princ "]")
        (terpri)
        (princ "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━")
        (terpri)
        ;; Syntax
        (if (doc-get entry 'SYNTAX)
            (progn
              (terpri)
              (princ "Syntax: ")
              (princ (doc-get entry 'SYNTAX))
              (terpri))
            nil)
        ;; Description
        (help-print-section "Description" (doc-get entry 'DESCRIPTION))
        ;; Arguments
        (let ((args (doc-get entry 'ARGS)))
          (if args
              (progn
                (terpri)
                (princ "Arguments:")
                (terpri)
                (mapcar args
                        (lambda (arg)
                          (princ "  ")
                          (princ (car arg))
                          (princ " - ")
                          (princ (cadr arg))
                          (terpri))))
              nil))
        ;; Returns
        (help-print-section "Returns" (doc-get entry 'RETURNS))
        ;; Examples
        (help-print-examples (doc-get entry 'EXAMPLES))
        ;; See also
        (help-print-see-also (doc-get entry 'SEE-ALSO))
        (terpri)
        t)
      nil))

;;; Main Help Function

(defun help (&rest args)
  "Interactive help system. Use (help 'symbol) for symbol help, (help 'CATEGORIES) to list categories, (help 'CATEGORY 'name) for category contents."
  (cond
    ;; No args - show overview
    ((null args)
     (help-overview))
    ;; Special commands first (before general symbol lookup)
    ((eq (car args) 'CATEGORIES)
     (help-list-categories))
    ((eq (car args) 'CATEGORY)
     (help-show-category (cadr args)))
    ((eq (car args) 'SEARCH)
     (help-search (cadr args)))
    ;; Single symbol - lookup (fallback)
    ((and (= (length args) 1)
          (symbolp (car args)))
     (help-symbol (car args)))
    ;; Unknown
    (t
     (princ "Unknown help command. Try (help) for usage.")
     (terpri))))

(defun help-overview ()
  "Display help system overview."
  (terpri)
  (princ "╔══════════════════════════════════════════╗")
  (terpri)
  (princ "║       LAMEDH HELP SYSTEM                 ║")
  (terpri)
  (princ "╚══════════════════════════════════════════╝")
  (terpri)
  (terpri)
  (princ "Usage:")
  (terpri)
  (princ "  (help 'symbol)        - Get help for a symbol")
  (terpri)
  (princ "  (help 'CATEGORIES)    - List all categories")
  (terpri)
  (princ "  (help 'CATEGORY 'cat) - Show category contents")
  (terpri)
  (princ "  (help 'SEARCH text)   - Search documentation")
  (terpri)
  (terpri)
  (princ "Quick reference:")
  (terpri)
  (princ "  (documentation 'sym) - Get docstring")
  (terpri)
  (princ "  (apropos text)       - Find matching symbols")
  (terpri)
  (terpri)
  (princ "Categories: ")
  (let ((cats (list-categories)))
    (if cats
        (progn
          (princ (caar cats))
          (mapcar (cdr cats)
                  (lambda (c)
                    (princ ", ")
                    (princ (car c)))))
        (princ "(none loaded)")))
  (terpri)
  (terpri)
  t)

(defun help-symbol (sym)
  "Display help for a specific symbol."
  (let ((entry (get-doc sym)))
    (if entry
        (display-doc entry)
        ;; Fall back to docstring
        (let ((docstr (documentation sym)))
          (if docstr
              (progn
                (terpri)
                (princ sym)
                (terpri)
                (princ "  ")
                (princ docstr)
                (terpri)
                t)
              (progn
                (princ "No documentation found for: ")
                (princ sym)
                (terpri)
                nil))))))

(defun help-list-categories ()
  "Display all categories."
  (terpri)
  (princ "Documentation Categories:")
  (terpri)
  (princ "─────────────────────────")
  (terpri)
  (mapcar (list-categories)
          (lambda (cat)
            (terpri)
            (princ (car cat))
            (princ " - ")
            (princ (cadr cat))
            (terpri)
            (princ "  Symbols: ")
            (princ (length (caddr cat)))
            (terpri)))
  (terpri)
  t)

(defun help-show-category (cat-name)
  "Show all symbols in a category."
  (let ((syms (list-category cat-name)))
    (if syms
        (progn
          (terpri)
          (princ "Category: ")
          (princ cat-name)
          (terpri)
          (princ "─────────────────────────")
          (terpri)
          (mapcar syms
                  (lambda (s)
                    (princ "  ")
                    (princ s)
                    (let ((entry (get-doc s)))
                      (if entry
                          (let ((desc (doc-get entry 'DESCRIPTION)))
                            (if desc
                                (progn
                                  (princ " - ")
                                  ;; Truncate long descriptions
                                  (princ (if (> (length (explode (intern desc))) 40)
                                             (concat (substring-proxy desc 0 37) "...")
                                             desc)))
                                nil))
                          nil))
                    (terpri)))
          (terpri)
          t)
        (progn
          (princ "Unknown category: ")
          (princ cat-name)
          (terpri)
          nil))))

;; SUBSTRING is now a real kernel primitive (issue #147); this remains as a
;; thin alias for the existing call sites.
(defun substring-proxy (s start end)
  "Deprecated alias for SUBSTRING; kept for existing call sites."
  (substring s start end))

(defun help-search (text)
  "Search documentation for text."
  (princ "Search not yet implemented. Try (help 'symbol)")
  (terpri))

;;; Apropos - find symbols matching a pattern

(defun apropos (pattern)
  "Find all documented symbols containing PATTERN in their name."
  (terpri)
  (princ "Symbols matching '")
  (princ pattern)
  (princ "':")
  (terpri)
  ;; For now just iterate through what we have
  (mapcar (keys HELP-DB)
          (lambda (name)
            (princ "  ")
            (princ name)
            (terpri)))
  (terpri))
