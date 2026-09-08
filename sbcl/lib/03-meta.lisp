(defun documentation (sym)
  "Retrieves the docstring for a symbol. Checks the symbol plist first,
then falls back to the DESCRIPTION field of any registered help entry."
  (let ((plist-doc (GETP sym "docstring")))
    (if plist-doc
        plist-doc
        (if (boundp 'HELP-DB)
            (let ((entry (gethash HELP-DB sym)))
              (if entry
                  (let ((pair (assoc 'DESCRIPTION entry)))
                    (if pair (cdr pair) nil))
                  nil))
            nil))))
