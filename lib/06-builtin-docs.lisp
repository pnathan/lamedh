;;; Documentation for builtin functions (putp "docstring" entries)
;;; These are picked up by (documentation 'sym) via GETP.

;;; ---- Arithmetic -----------------------------------------------------------

(putp '+ "docstring" "Sum of arguments; 0 with no args. Promotes to float if any arg is float.")
(putp '- "docstring" "Negate (1 arg) or subtract (2+ args).")
(putp '* "docstring" "Product of arguments; 1 with no args.")
(putp '/ "docstring" "Integer division (truncates toward zero); promotes to float if args are float.")
(putp 'plus  "docstring" "Alias for +.")
(putp 'times "docstring" "Alias for *.")
(putp 'quotient "docstring" "Alias for /.")
(putp 'remainder "docstring" "Integer remainder: (remainder x y) — same sign as dividend.")
(putp 'mod "docstring" "Modulo: (mod x y) — result has same sign as divisor.")
(putp 'expt "docstring" "Raise base to a power: (expt base exp).")
(putp 'add1 "docstring" "Return n + 1.  Alias: 1+")
(putp 'sub1 "docstring" "Return n - 1.  Alias: 1-")
(putp 'random "docstring" "Return a random non-negative integer less than n.")
(putp 'sqrt "docstring" "Square root (float): (sqrt x).")
(putp 'sin  "docstring" "Sine (radians, float).")
(putp 'cos  "docstring" "Cosine (radians, float).")
(putp 'tan  "docstring" "Tangent (radians, float).")
(putp 'log  "docstring" "Natural logarithm (float): (log x).")
(putp 'exp  "docstring" "e raised to a power (float): (exp x).")
(putp 'floor    "docstring" "Largest integer <= x: (floor x) -> integer.")
(putp 'ceiling  "docstring" "Smallest integer >= x: (ceiling x) -> integer.")
(putp 'round    "docstring" "Round to nearest integer (half-up): (round x) -> integer.")
(putp 'truncate "docstring" "Truncate toward zero: (truncate x) -> integer.")
(putp 'gcd   "docstring" "Greatest common divisor: (gcd a b).")
(putp 'lcm   "docstring" "Least common multiple: (lcm a b).")
(putp 'isqrt "docstring" "Integer square root (floor of sqrt): (isqrt n).")
(putp 'signum "docstring" "Sign of n: -1, 0, or 1 as a Number.")

;;; ---- Numeric predicates ---------------------------------------------------

(putp 'zerop  "docstring" "T if n is zero.")
(putp 'plusp  "docstring" "T if n is positive (> 0).")
(putp 'minusp "docstring" "T if n is negative (< 0).")
(putp 'onep   "docstring" "T if n equals 1.")
(putp 'evenp  "docstring" "T if integer n is even.")
(putp 'oddp   "docstring" "T if integer n is odd.")

;;; ---- Comparisons ----------------------------------------------------------

(putp '=       "docstring" "Numeric equality (cross-type: Char, Number, Float). (= a b)")
(putp '<       "docstring" "True if a < b (numeric).")
(putp '>       "docstring" "True if a > b (numeric).")
(putp 'lessp   "docstring" "Alias for <.")
(putp 'greaterp "docstring" "Alias for >.")

;;; ---- Type predicates ------------------------------------------------------

(putp 'atom      "docstring" "T if x is not a cons cell.")
(putp 'numberp   "docstring" "T if x is a Number or Float.")
(putp 'fixp      "docstring" "T if x is an integer Number (not Float, not Char).")
(putp 'floatp    "docstring" "T if x is a Float.")
(putp 'charp     "docstring" "T if x is a Char (byte 0-255 from a char literal). NIL for integers.")
(putp 'stringp   "docstring" "T if x is a String.")
(putp 'symbolp   "docstring" "T if x is a symbol (including NIL and T).")
(putp 'consp     "docstring" "T if x is a cons cell.")
(putp 'listp     "docstring" "T if x is a cons cell or NIL.")
(putp 'null      "docstring" "T if x is NIL.")
(putp 'boundp    "docstring" "T if symbol currently has a value binding.")
(putp 'functionp "docstring" "T if x is a callable (lambda, builtin, or fexpr).")
(putp 'macrop    "docstring" "T if x is a macro.")
(putp 'arrayp    "docstring" "T if x is an Array.")
(putp 'error-p   "docstring" "T if x is a first-class Error value (made by MAKE-ERROR or signalled by ERROR).")

;;; ---- Equality and identity ------------------------------------------------

(putp 'eq    "docstring" "Identity comparison (pointer equality). (eq a b)")
(putp 'equal "docstring" "Structural equality (deep). (equal a b)")
(putp 'not   "docstring" "T if x is NIL; NIL otherwise.")

;;; ---- Core list operations -------------------------------------------------

(putp 'car    "docstring" "Return the first element of a cons. NIL on NIL.")
(putp 'cdr    "docstring" "Return the rest of a cons. NIL on NIL.")
(putp 'cons   "docstring" "Create a new cons cell: (cons car cdr).")
(putp 'list   "docstring" "Create a list from arguments: (list 1 2 3) => (1 2 3).")
(putp 'last   "docstring" "Return the last cons cell of a list.")
(putp 'nth    "docstring" "Return the 0-indexed nth element of a list.")
(putp 'nthcdr "docstring" "Return the list after n CDRs.")
(putp 'efface "docstring" "Remove first occurrence of ITEM from LIST (non-destructive).")
(putp 'delete "docstring" "Alias for EFFACE.")
(putp 'rplaca "docstring" "Destructively replace the CAR of a cons: (rplaca cons new-car).")
(putp 'rplacd "docstring" "Destructively replace the CDR of a cons: (rplacd cons new-cdr).")
(putp 'subst  "docstring" "Replace all occurrences of OLD with NEW in TREE: (subst new old tree).")
(putp 'sublis "docstring" "Apply an alist of substitutions to TREE: (sublis alist tree).")
(putp 'assoc  "docstring" "Return first pair in ALIST whose car equals KEY: (assoc key alist).")
(putp 'pairlis "docstring" "Pair two lists into an alist: (pairlis keys values).")

;;; ---- Mapping and iteration ------------------------------------------------

(putp 'mapcar  "docstring" "Apply FN to each element of LIST; return list of results.")
(putp 'maplist "docstring" "Apply FN to successive tails of LIST; return list of results.")
(putp 'apply   "docstring" "Apply FUNCTION to ARG-LIST: (apply fn list).")
(putp 'funcall "docstring" "Call FUNCTION with explicit args: (funcall fn arg...).")

;;; ---- Evaluation and meta -------------------------------------------------

(putp 'eval       "docstring" "Evaluate an expression: (eval form).")
(putp 'evlis      "docstring" "Evaluate a list of expressions, return list of results.")
(putp 'evcon      "docstring" "Lisp 1.5 EVCON: evaluate clauses until one is true.")
(putp 'macroexpand "docstring" "Expand a macro call one step: (macroexpand form).")
(putp 'optimize   "docstring" "Run the source optimizer on a form: (optimize form).")

;;; ---- I/O -----------------------------------------------------------------

(putp 'print   "docstring" "Print object(s) in readable form followed by a newline.")
(putp 'prin1   "docstring" "Print object in readable form (strings quoted, chars as 'c').")
(putp 'princ   "docstring" "Print object without escape sequences (strings bare, chars as char).")
(putp 'terpri  "docstring" "Print a newline.")
(putp 'spaces  "docstring" "Print N space characters: (spaces n).")
(putp 'read    "docstring" "Read one S-expression from stdin.")
(putp 'load-file "docstring" "Load and evaluate a Lisp file: (load-file path).")

(putp 'prin1->string "docstring" "Return readable representation of VALUE as a string (like PRIN1 but to string).")
(putp 'princ->string "docstring" "Return display representation of VALUE as a string (like PRINC but to string).")

;;; ---- File I/O (require READ-FS / CREATE-FS capability) -------------------

(putp 'read-file         "docstring" "Read entire file as a string: (read-file path).")
(putp 'read-file-byte    "docstring" "Read one byte from file as integer: (read-file-byte path offset).")
(putp 'read-file-section "docstring" "Read bytes [start, end) from file as string: (read-file-section path start end).")
(putp 'write-file        "docstring" "Write string to file, overwriting: (write-file path content).")

;;; ---- File metadata (require READ-FS capability) --------------------------

(putp 'file-exists-p    "docstring" "T if path exists (any kind).")
(putp 'directory-p      "docstring" "T if path is a directory.")
(putp 'file-p           "docstring" "T if path is a regular file.")
(putp 'file-readable-p  "docstring" "T if path is readable by the current process.")
(putp 'file-writable-p  "docstring" "T if path is writable by the current process.")
(putp 'file-executable-p "docstring" "T if path is executable by the current process.")
(putp 'file-size        "docstring" "Return file size in bytes as a Number: (file-size path).")
(putp 'directory-files  "docstring" "Return list of entry names in directory: (directory-files path).")
(putp 'file-newer-p     "docstring" "T if file A is newer than file B: (file-newer-p a b).")

;;; ---- File mutation (require CREATE-FS capability) ------------------------

(putp 'chmod           "docstring" "Set file permissions: (chmod path mode-octal-integer).")
(putp 'create-directory "docstring" "Create a directory (and parents): (create-directory path).")
(putp 'delete-file     "docstring" "Delete a file: (delete-file path).")
(putp 'rename-file     "docstring" "Rename or move a file: (rename-file old new).")

;;; ---- Temp filesystem (require TEMP-FS capability) ------------------------

(putp 'make-temp-file      "docstring" "Create a temp file and return its path.")
(putp 'make-temp-directory "docstring" "Create a temp directory and return its path.")

;;; ---- Strings (kernel primitives) -----------------------------------------

(putp 'concat          "docstring" "Concatenate zero or more strings into one.")
(putp 'index           "docstring" "Return character at position N as a one-char string: (index str n).")
(putp 'string-length   "docstring" "Return number of characters in string: (string-length s).")
(putp 'substring       "docstring" "Return substring [start, end): (substring s start &optional end).")
(putp 'string->number  "docstring" "Parse a decimal string to a Number, or NIL if not numeric.")
(putp 'number->string  "docstring" "Convert a Number or Float to its decimal string representation.")

;;; ---- Char operations ------------------------------------------------------

(putp 'char-code "docstring"
  "Return integer code point of a Char literal or one-character string: (char-code c).")
(putp 'code-char "docstring"
  "Return a one-character string for code point n (0-255): (code-char n).")
(putp 'make-char "docstring"
  "Return a Char value for integer n (0-255): (make-char n). Distinct from Number.")

;;; ---- Symbol / string utilities -------------------------------------------

(putp 'explode "docstring" "Convert atom name to list of single-character symbols: (explode sym).")
(putp 'implode "docstring" "Convert list of char symbols to an interned symbol: (implode chars).")
(putp 'maknam  "docstring" "Alias for IMPLODE.")
(putp 'gensym  "docstring" "Generate a unique uninterned symbol.")
(putp 'intern  "docstring" "Intern a string as a symbol: (intern str).")

;;; ---- Property lists -------------------------------------------------------

(putp 'getp    "docstring" "Get property INDICATOR of SYMBOL: (getp sym indicator).")
;; (putp 'putp ...) is intentionally absent: calling putp with 'putp as the first
;; arg triggers a RefCell double-borrow (the match in eval_step holds an immutable
;; borrow on the function-head symbol for the entire dispatch block). See 99-help-data.lisp.
(putp 'get     "docstring" "Same as GETP.")
(putp 'put     "docstring" "Same as PUTP.")
(putp 'plist   "docstring" "Return the full property list of SYMBOL as an alist.")
(putp 'remprop "docstring" "Remove property INDICATOR from SYMBOL's plist: (remprop sym indicator).")
(putp 'deflist "docstring" "Associate each symbol in LIST with a value in VALS under INDICATOR.")

;;; ---- Hash tables ---------------------------------------------------------

(putp 'make-hash-table  "docstring" "Create a new empty hash table.")
(putp 'gethash          "docstring" "Look up KEY in hash table: (gethash table key). Returns NIL if absent.")
(putp 'set-bang         "docstring" "Set KEY to VALUE in hash table: (set-bang table key value).")
(putp 'delete-key       "docstring" "Remove KEY from hash table: (delete-key table key).")
(putp 'keys             "docstring" "Return list of all keys in hash table.")
(putp 'current-environment "docstring" "Return the currently active environment object.")

;;; ---- Bitwise operations ---------------------------------------------------

(putp 'logor     "docstring" "Bitwise OR of integer arguments.")
(putp 'logand    "docstring" "Bitwise AND of integer arguments.")
(putp 'logxor    "docstring" "Bitwise XOR of integer arguments.")
(putp 'lognot    "docstring" "Bitwise complement (NOT) of an integer.")
(putp 'leftshift "docstring" "Shift bits left (positive count) or right (negative): (leftshift n count).")
(putp 'ash       "docstring" "Arithmetic shift: (ash n count). Positive count = left shift.")
(putp 'rot       "docstring" "Rotate bits: (rot n count width).")

;;; ---- Sort ----------------------------------------------------------------

(putp 'sort "docstring"
  "Non-destructive stable sort: (sort list comparator). E.g. (sort lst #'lessp).")

;;; ---- Arrays ---------------------------------------------------------------

(putp 'array        "docstring" "Create an uninitialized array of N elements: (array n).")
(putp 'fetch        "docstring" "Return element I of array A: (fetch a i).")
(putp 'store        "docstring" "Set element I of array A to V (returns V): (store a i v).")
(putp 'array-length "docstring" "Return number of elements in array: (array-length a).")

;;; ---- First-class errors --------------------------------------------------

(putp 'make-error    "docstring"
  "Create an Error value: (make-error message &optional data). Does NOT signal.")
(putp 'error-message "docstring" "Return the message string of an Error value.")
(putp 'error-data    "docstring" "Return the data field of an Error value, or NIL if none.")
(putp 'error         "docstring"
  "Signal an error: (error message &optional data). Caught by ERRORSET/HANDLER-CASE.")
(putp 'errorset      "docstring"
  "Trap ordinary errors: (errorset '(form)). Returns (result) on success, NIL on error.")

;;; ---- Condition flags ------------------------------------------------------

(putp 'set-flag       "docstring" "Set a named condition flag to T: (set-flag name).")
(putp 'clear-flag     "docstring" "Clear a named condition flag: (clear-flag name).")
(putp 'flag-set-p     "docstring" "T if named condition flag is set: (flag-set-p name).")
(putp 'clear-all-flags "docstring" "Clear all condition flags.")

;;; ---- Capabilities --------------------------------------------------------

(putp 'feature-enabled-p "docstring"
  "T if a named capability is granted for this environment: (feature-enabled-p :shell).")
(putp 'features "docstring" "Return list of all currently granted capability keywords.")

;;; ---- Shell (require SHELL capability) ------------------------------------

(putp 'shell "docstring"
  "Run a shell command and return its stdout as a string: (shell command-string).")

;;; ---- First-class environments --------------------------------------------

(putp 'make-environment "docstring" "Create a new child environment of the current one.")
(putp 'the-environment  "docstring" "Return the compile-time environment (for use in lambdas).")

;;; ---- Extensions ----------------------------------------------------------

(putp 'extensionp        "docstring" "T if x is a foreign extension value.")
(putp 'extension-type-name "docstring" "Return the type-name string of an extension value.")
