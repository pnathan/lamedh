;;; Help Data - Documentation for Lamedh functions and special forms
;;; This file populates the help database with documentation entries
;;;
;;; REQUIRE-ABLE (issue #256): HELP-DATA is one of the optional embedded
;;; modules -- it requires 'help-system first because REGISTER-DOC and
;;; REGISTER-CATEGORY (defined there) run immediately at this file's top
;;; level, not lazily inside a function body. `with_stdlib()` still loads
;;; this file (and help-system before it) unconditionally, unchanged.

(require 'help-system)

;;; ============================================================
;;; ARITHMETIC FUNCTIONS
;;; ============================================================

(register-doc '+
  (list
    (cons 'NAME '+)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(+ number...)")
    (cons 'CATEGORY 'arithmetic)
    (cons 'DESCRIPTION "Returns the sum of all arguments. With no arguments, returns 0.")
    (cons 'ARGS '((numbers "Zero or more numbers to add")))
    (cons 'RETURNS "Sum of arguments (float if any argument is float)")
    (cons 'EXAMPLES '(((+ 1 2 3) 6)
                       ((+ 1.5 2.5) 4.0)
                       ((+) 0)))
    (cons 'SEE-ALSO '(- * /))))

(register-doc '-
  (list
    (cons 'NAME '-)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(- number) or (- number number...)")
    (cons 'CATEGORY 'arithmetic)
    (cons 'DESCRIPTION "With one argument, returns negation. With multiple, subtracts rest from first.")
    (cons 'ARGS '((number "One or more numbers")))
    (cons 'RETURNS "Difference or negation")
    (cons 'EXAMPLES '(((- 5) -5)
                       ((- 10 3) 7)
                       ((- 10 3 2) 5)))
    (cons 'SEE-ALSO '(+ * /))))

(register-doc '*
  (list
    (cons 'NAME '*)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(* number...)")
    (cons 'CATEGORY 'arithmetic)
    (cons 'DESCRIPTION "Returns the product of all arguments. With no arguments, returns 1.")
    (cons 'ARGS '((numbers "Zero or more numbers to multiply")))
    (cons 'RETURNS "Product of arguments")
    (cons 'EXAMPLES '(((* 2 3 4) 24)
                       ((*) 1)))
    (cons 'SEE-ALSO '(+ - / expt))))

(register-doc '/
  (list
    (cons 'NAME '/)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(/ dividend divisor)")
    (cons 'CATEGORY 'arithmetic)
    (cons 'DESCRIPTION "Returns the quotient of two numbers. Integer division truncates toward zero.")
    (cons 'ARGS '((dividend "Number to divide")
                   (divisor "Number to divide by (non-zero)")))
    (cons 'RETURNS "Quotient")
    (cons 'EXAMPLES '(((/ 10 2) 5)
                       ((/ 10 3) 3)
                       ((/ 10.0 3) 3.333333)))
    (cons 'SEE-ALSO '(remainder mod * -))))

(register-doc 'remainder
  (list
    (cons 'NAME 'remainder)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(remainder dividend divisor)")
    (cons 'CATEGORY 'arithmetic)
    (cons 'DESCRIPTION "Returns the remainder of integer division.")
    (cons 'EXAMPLES '(((remainder 10 3) 1)
                       ((remainder -10 3) -1)))
    (cons 'SEE-ALSO '(mod /))))

(register-doc 'mod
  (list
    (cons 'NAME 'mod)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(mod x y)")
    (cons 'CATEGORY 'arithmetic)
    (cons 'DESCRIPTION "Returns x modulo y. Result has same sign as divisor.")
    (cons 'EXAMPLES '(((mod 10 3) 1)
                       ((mod -10 3) 2)))
    (cons 'SEE-ALSO '(remainder /))))

(register-doc 'expt
  (list
    (cons 'NAME 'expt)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(expt base power)")
    (cons 'CATEGORY 'arithmetic)
    (cons 'DESCRIPTION "Returns base raised to the power.")
    (cons 'EXAMPLES '(((expt 2 10) 1024)
                       ((expt 3 3) 27)))
    (cons 'SEE-ALSO '(* /))))

(register-doc 'add1
  (list
    (cons 'NAME 'add1)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(add1 n)")
    (cons 'CATEGORY 'arithmetic)
    (cons 'DESCRIPTION "Returns n + 1. Same as (1+ n).")
    (cons 'EXAMPLES '(((add1 5) 6)))
    (cons 'SEE-ALSO '(sub1 + -))))

(register-doc 'sub1
  (list
    (cons 'NAME 'sub1)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(sub1 n)")
    (cons 'CATEGORY 'arithmetic)
    (cons 'DESCRIPTION "Returns n - 1. Same as (1- n).")
    (cons 'EXAMPLES '(((sub1 5) 4)))
    (cons 'SEE-ALSO '(add1 + -))))

(register-doc 'abs
  (list
    (cons 'NAME 'abs)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(abs n)")
    (cons 'CATEGORY 'arithmetic)
    (cons 'DESCRIPTION "Returns the absolute value of n.")
    (cons 'EXAMPLES '(((abs 5) 5)
                       ((abs -5) 5)))
    (cons 'SEE-ALSO '(minusp))))

(register-doc 'max
  (list
    (cons 'NAME 'max)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(max number...)")
    (cons 'CATEGORY 'arithmetic)
    (cons 'DESCRIPTION "Returns the largest of its arguments.")
    (cons 'EXAMPLES '(((max 1 5 3) 5)
                       ((max -1 -5) -1)))
    (cons 'SEE-ALSO '(min))))

(register-doc 'min
  (list
    (cons 'NAME 'min)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(min number...)")
    (cons 'CATEGORY 'arithmetic)
    (cons 'DESCRIPTION "Returns the smallest of its arguments.")
    (cons 'EXAMPLES '(((min 1 5 3) 1)))
    (cons 'SEE-ALSO '(max))))

(register-doc 'random
  (list
    (cons 'NAME 'random)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(random n)")
    (cons 'CATEGORY 'arithmetic)
    (cons 'DESCRIPTION "Returns a random integer from 0 (inclusive) to n (exclusive).")
    (cons 'EXAMPLES '(((random 10) "0-9 randomly")))
    (cons 'SEE-ALSO '())))

;;; ============================================================
;;; NUMERIC PREDICATES
;;; ============================================================

(register-doc 'zerop
  (list
    (cons 'NAME 'zerop)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(zerop n)")
    (cons 'CATEGORY 'predicates)
    (cons 'DESCRIPTION "Returns T if n is zero.")
    (cons 'EXAMPLES '(((zerop 0) t)
                       ((zerop 1) nil)))
    (cons 'SEE-ALSO '(plusp minusp onep))))

(register-doc 'plusp
  (list
    (cons 'NAME 'plusp)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(plusp n)")
    (cons 'CATEGORY 'predicates)
    (cons 'DESCRIPTION "Returns T if n is positive (greater than zero).")
    (cons 'EXAMPLES '(((plusp 1) t)
                       ((plusp 0) nil)))
    (cons 'SEE-ALSO '(minusp zerop))))

(register-doc 'minusp
  (list
    (cons 'NAME 'minusp)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(minusp n)")
    (cons 'CATEGORY 'predicates)
    (cons 'DESCRIPTION "Returns T if n is negative (less than zero).")
    (cons 'EXAMPLES '(((minusp -1) t)
                       ((minusp 0) nil)))
    (cons 'SEE-ALSO '(plusp zerop abs))))

(register-doc 'evenp
  (list
    (cons 'NAME 'evenp)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(evenp n)")
    (cons 'CATEGORY 'predicates)
    (cons 'DESCRIPTION "Returns T if n is an even integer.")
    (cons 'EXAMPLES '(((evenp 2) t)
                       ((evenp 3) nil)))
    (cons 'SEE-ALSO '(oddp))))

(register-doc 'oddp
  (list
    (cons 'NAME 'oddp)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(oddp n)")
    (cons 'CATEGORY 'predicates)
    (cons 'DESCRIPTION "Returns T if n is an odd integer.")
    (cons 'EXAMPLES '(((oddp 3) t)
                       ((oddp 2) nil)))
    (cons 'SEE-ALSO '(evenp))))

;;; ============================================================
;;; COMPARISON FUNCTIONS
;;; ============================================================

(register-doc '<
  (list
    (cons 'NAME '<)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(< a b)")
    (cons 'CATEGORY 'predicates)
    (cons 'DESCRIPTION "Returns T if a is less than b.")
    (cons 'EXAMPLES '(((< 1 2) t)
                       ((< 2 1) nil)))
    (cons 'SEE-ALSO '(> = lessp greaterp))))

(register-doc '>
  (list
    (cons 'NAME '>)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(> a b)")
    (cons 'CATEGORY 'predicates)
    (cons 'DESCRIPTION "Returns T if a is greater than b.")
    (cons 'EXAMPLES '(((> 2 1) t)
                       ((> 1 2) nil)))
    (cons 'SEE-ALSO '(< = lessp greaterp))))

(register-doc '=
  (list
    (cons 'NAME '=)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(= a b)")
    (cons 'CATEGORY 'predicates)
    (cons 'DESCRIPTION "Returns T if a and b are numerically equal.")
    (cons 'EXAMPLES '(((= 1 1) t)
                       ((= 1 1.0) t)
                       ((= 1 2) nil)))
    (cons 'SEE-ALSO '(eq equal))))

;;; ============================================================
;;; TYPE PREDICATES
;;; ============================================================

(register-doc 'atom
  (list
    (cons 'NAME 'atom)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(atom x)")
    (cons 'CATEGORY 'predicates)
    (cons 'DESCRIPTION "Returns T if x is not a cons cell (i.e., is an atom).")
    (cons 'EXAMPLES '(((atom 'a) t)
                       ((atom 42) t)
                       ((atom '(a b)) nil)))
    (cons 'SEE-ALSO '(consp listp symbolp))))

(register-doc 'symbolp
  (list
    (cons 'NAME 'symbolp)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(symbolp x)")
    (cons 'CATEGORY 'predicates)
    (cons 'DESCRIPTION "Returns T if x is a symbol.")
    (cons 'EXAMPLES '(((symbolp 'foo) t)
                       ((symbolp nil) t)
                       ((symbolp 42) nil)))
    (cons 'SEE-ALSO '(atom numberp stringp))))

(register-doc 'numberp
  (list
    (cons 'NAME 'numberp)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(numberp x)")
    (cons 'CATEGORY 'predicates)
    (cons 'DESCRIPTION "Returns T if x is a number (integer or float).")
    (cons 'EXAMPLES '(((numberp 42) t)
                       ((numberp 3.14) t)
                       ((numberp 'a) nil)))
    (cons 'SEE-ALSO '(fixp floatp))))

(register-doc 'fixp
  (list
    (cons 'NAME 'fixp)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(fixp x)")
    (cons 'CATEGORY 'predicates)
    (cons 'DESCRIPTION "Returns T if x is a fixed-point (integer) number.")
    (cons 'EXAMPLES '(((fixp 42) t)
                       ((fixp 3.14) nil)))
    (cons 'SEE-ALSO '(floatp numberp))))

(register-doc 'floatp
  (list
    (cons 'NAME 'floatp)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(floatp x)")
    (cons 'CATEGORY 'predicates)
    (cons 'DESCRIPTION "Returns T if x is a floating-point number.")
    (cons 'EXAMPLES '(((floatp 3.14) t)
                       ((floatp 42) nil)))
    (cons 'SEE-ALSO '(fixp numberp))))

(register-doc 'stringp
  (list
    (cons 'NAME 'stringp)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(stringp x)")
    (cons 'CATEGORY 'predicates)
    (cons 'DESCRIPTION "Returns T if x is a string.")
    (cons 'EXAMPLES '(((stringp "hello") t)
                       ((stringp 'hello) nil)))
    (cons 'SEE-ALSO '(symbolp atom))))

(register-doc 'consp
  (list
    (cons 'NAME 'consp)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(consp x)")
    (cons 'CATEGORY 'predicates)
    (cons 'DESCRIPTION "Returns T if x is a cons cell.")
    (cons 'EXAMPLES '(((consp '(a b)) t)
                       ((consp nil) nil)))
    (cons 'SEE-ALSO '(atom listp null))))

(register-doc 'listp
  (list
    (cons 'NAME 'listp)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(listp x)")
    (cons 'CATEGORY 'predicates)
    (cons 'DESCRIPTION "Returns T if x is a list (cons or NIL).")
    (cons 'EXAMPLES '(((listp '(a b)) t)
                       ((listp nil) t)
                       ((listp 'a) nil)))
    (cons 'SEE-ALSO '(consp null atom))))

(register-doc 'null
  (list
    (cons 'NAME 'null)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(null x)")
    (cons 'CATEGORY 'predicates)
    (cons 'DESCRIPTION "Returns T if x is NIL.")
    (cons 'EXAMPLES '(((null nil) t)
                       ((null '()) t)
                       ((null '(a)) nil)))
    (cons 'SEE-ALSO '(not listp))))

(register-doc 'not
  (list
    (cons 'NAME 'not)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(not x)")
    (cons 'CATEGORY 'predicates)
    (cons 'DESCRIPTION "Returns T if x is NIL, NIL otherwise.")
    (cons 'EXAMPLES '(((not nil) t)
                       ((not t) nil)))
    (cons 'SEE-ALSO '(null and or))))

(register-doc 'eq
  (list
    (cons 'NAME 'eq)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(eq a b)")
    (cons 'CATEGORY 'predicates)
    (cons 'DESCRIPTION "Returns T if a and b are the same object (identity test).")
    (cons 'EXAMPLES '(((eq 'a 'a) t)
                       ((eq '(1) '(1)) nil)))
    (cons 'SEE-ALSO '(equal =))))

(register-doc 'equal
  (list
    (cons 'NAME 'equal)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(equal a b)")
    (cons 'CATEGORY 'predicates)
    (cons 'DESCRIPTION "Returns T if a and b are structurally equivalent (recursive comparison).")
    (cons 'EXAMPLES '(((equal '(a b) '(a b)) t)
                       ((equal "hi" "hi") t)))
    (cons 'SEE-ALSO '(eq =))))

(register-doc 'functionp
  (list
    (cons 'NAME 'functionp)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(functionp x)")
    (cons 'CATEGORY 'predicates)
    (cons 'DESCRIPTION "Returns T if x is a function (lambda, fexpr, or builtin).")
    (cons 'EXAMPLES '(((functionp (lambda (x) x)) t)))
    (cons 'SEE-ALSO '(macrop boundp))))

(register-doc 'boundp
  (list
    (cons 'NAME 'boundp)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(boundp symbol)")
    (cons 'CATEGORY 'predicates)
    (cons 'DESCRIPTION "Returns T if symbol has a value binding.")
    (cons 'EXAMPLES '(((boundp 'car) t)))
    (cons 'SEE-ALSO '(symbolp))))

;;; ============================================================
;;; LIST FUNCTIONS
;;; ============================================================

(register-doc 'car
  (list
    (cons 'NAME 'car)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(car list)")
    (cons 'CATEGORY 'lists)
    (cons 'DESCRIPTION "Returns the first element of a list (the car of a cons cell).")
    (cons 'ARGS '((list "A cons cell or NIL")))
    (cons 'RETURNS "First element, or NIL for empty list")
    (cons 'EXAMPLES '(((car '(a b c)) a)
                       ((car nil) nil)))
    (cons 'SEE-ALSO '(cdr cons cadr caddr))))

(register-doc 'cdr
  (list
    (cons 'NAME 'cdr)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(cdr list)")
    (cons 'CATEGORY 'lists)
    (cons 'DESCRIPTION "Returns the rest of a list (the cdr of a cons cell).")
    (cons 'ARGS '((list "A cons cell or NIL")))
    (cons 'RETURNS "Rest of list, or NIL")
    (cons 'EXAMPLES '(((cdr '(a b c)) (b c))
                       ((cdr '(a)) nil)))
    (cons 'SEE-ALSO '(car cons cddr))))

(register-doc 'cons
  (list
    (cons 'NAME 'cons)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(cons car cdr)")
    (cons 'CATEGORY 'lists)
    (cons 'DESCRIPTION "Creates a new cons cell with the given car and cdr.")
    (cons 'ARGS '((car "First element")
                   (cdr "Rest (usually a list)")))
    (cons 'RETURNS "New cons cell")
    (cons 'EXAMPLES '(((cons 'a '(b c)) (a b c))
                       ((cons 'a 'b) (a . b))))
    (cons 'SEE-ALSO '(car cdr list))))

(register-doc 'list
  (list
    (cons 'NAME 'list)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(list item...)")
    (cons 'CATEGORY 'lists)
    (cons 'DESCRIPTION "Creates a list from its arguments.")
    (cons 'EXAMPLES '(((list 1 2 3) (1 2 3))
                       ((list) nil)))
    (cons 'SEE-ALSO '(cons append))))

(register-doc 'append
  (list
    (cons 'NAME 'append)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(append list1 list2)")
    (cons 'CATEGORY 'lists)
    (cons 'DESCRIPTION "Concatenates two lists.")
    (cons 'EXAMPLES '(((append '(a b) '(c d)) (a b c d))))
    (cons 'SEE-ALSO '(cons list reverse))))

(register-doc 'reverse
  (list
    (cons 'NAME 'reverse)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(reverse list)")
    (cons 'CATEGORY 'lists)
    (cons 'DESCRIPTION "Returns a list with elements in reverse order.")
    (cons 'EXAMPLES '(((reverse '(a b c)) (c b a))))
    (cons 'SEE-ALSO '(append))))

(register-doc 'length
  (list
    (cons 'NAME 'length)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(length list)")
    (cons 'CATEGORY 'lists)
    (cons 'DESCRIPTION "Returns the number of elements in a list.")
    (cons 'EXAMPLES '(((length '(a b c)) 3)
                       ((length nil) 0)))
    (cons 'SEE-ALSO '(null))))

(register-doc 'nth
  (list
    (cons 'NAME 'nth)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(nth n list)")
    (cons 'CATEGORY 'lists)
    (cons 'DESCRIPTION "Returns the nth element of a list (0-indexed).")
    (cons 'EXAMPLES '(((nth 0 '(a b c)) a)
                       ((nth 2 '(a b c)) c)))
    (cons 'SEE-ALSO '(nthcdr car cadr))))

(register-doc 'last
  (list
    (cons 'NAME 'last)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(last list)")
    (cons 'CATEGORY 'lists)
    (cons 'DESCRIPTION "Returns the last cons cell of a list.")
    (cons 'EXAMPLES '(((last '(a b c)) (c))))
    (cons 'SEE-ALSO '(car cdr nth))))

(register-doc 'member
  (list
    (cons 'NAME 'member)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(member item list)")
    (cons 'CATEGORY 'lists)
    (cons 'DESCRIPTION "Searches for item in list using EQUAL. Returns tail starting at match.")
    (cons 'EXAMPLES '(((member 'b '(a b c)) (b c))
                       ((member 'x '(a b c)) nil)))
    (cons 'SEE-ALSO '(assoc equal))))

(register-doc 'assoc
  (list
    (cons 'NAME 'assoc)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(assoc key alist)")
    (cons 'CATEGORY 'lists)
    (cons 'DESCRIPTION "Searches an association list for a pair with matching key.")
    (cons 'EXAMPLES '(((assoc 'b '((a . 1) (b . 2))) (b . 2))))
    (cons 'SEE-ALSO '(member pairlis))))

(register-doc 'mapcar
  (list
    (cons 'NAME 'mapcar)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(mapcar function list)")
    (cons 'CATEGORY 'lists)
    (cons 'DESCRIPTION "Applies function to each element of list, returns list of results.")
    (cons 'EXAMPLES '(((mapcar (lambda (x) (* x 2)) '(1 2 3)) (2 4 6))))
    (cons 'SEE-ALSO '(maplist apply))))

(register-doc 'maplist
  (list
    (cons 'NAME 'maplist)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(maplist function list)")
    (cons 'CATEGORY 'lists)
    (cons 'DESCRIPTION "Applies function to successive tails of list.")
    (cons 'EXAMPLES '(((maplist (lambda (x) (length x)) '(a b c)) (3 2 1))))
    (cons 'SEE-ALSO '(mapcar))))

(register-doc 'subst
  (list
    (cons 'NAME 'subst)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(subst new old tree)")
    (cons 'CATEGORY 'lists)
    (cons 'DESCRIPTION "Replaces all occurrences of old with new in tree.")
    (cons 'EXAMPLES '(((subst 'x 'a '(a b a)) (x b x))))
    (cons 'SEE-ALSO '())))

;;; ============================================================
;;; I/O FUNCTIONS
;;; ============================================================

(register-doc 'print
  (list
    (cons 'NAME 'print)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(print object...)")
    (cons 'CATEGORY 'io)
    (cons 'DESCRIPTION "Prints objects to standard output.")
    (cons 'RETURNS "NIL")
    (cons 'SEE-ALSO '(prin1 princ terpri))))

(register-doc 'prin1
  (list
    (cons 'NAME 'prin1)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(prin1 object)")
    (cons 'CATEGORY 'io)
    (cons 'DESCRIPTION "Prints object in readable form (strings with quotes).")
    (cons 'RETURNS "The object printed")
    (cons 'EXAMPLES '(((prin1 hello) hello)))
    (cons 'SEE-ALSO '(princ print))))

(register-doc 'princ
  (list
    (cons 'NAME 'princ)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(princ object)")
    (cons 'CATEGORY 'io)
    (cons 'DESCRIPTION "Prints object without escaping (strings without quotes).")
    (cons 'RETURNS "The object printed")
    (cons 'SEE-ALSO '(prin1 print))))

(register-doc 'terpri
  (list
    (cons 'NAME 'terpri)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(terpri)")
    (cons 'CATEGORY 'io)
    (cons 'DESCRIPTION "Prints a newline character.")
    (cons 'RETURNS "NIL")
    (cons 'SEE-ALSO '(print princ))))

(register-doc 'read
  (list
    (cons 'NAME 'read)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(read)")
    (cons 'CATEGORY 'io)
    (cons 'DESCRIPTION "Reads one S-expression from standard input.")
    (cons 'RETURNS "Parsed S-expression")
    (cons 'SEE-ALSO '(eval load-file))))

(register-doc 'load-file
  (list
    (cons 'NAME 'load-file)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(load-file filename)")
    (cons 'CATEGORY 'io)
    (cons 'DESCRIPTION "Loads and evaluates a Lisp source file. A loaded source file may include another file with a top-level (include \"path.lisp\") directive; relative include paths resolve from the file that contains the include.")
    (cons 'ARGS '((filename "String path to file")))
    (cons 'RETURNS "T on success")
    (cons 'SEE-ALSO '(read eval))))

(register-doc 'format
  (list
    (cons 'NAME 'format)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(format dest ctrl &rest args)")
    (cons 'CATEGORY 'io)
    (cons 'DESCRIPTION "CL-style format string rendering (issue #150, lib/18-format.lisp). DEST nil returns the formatted string; t prints it to stdout and returns nil; a PORTS port writes the UTF-8 bytes to it and returns nil. Directives: ~a ~s ~d ~f ~x ~o ~b ~c ~% ~& ~~ ~{...~} ~^ -- an unrecognized directive, or a supported one with an unsupported numeric/colon/at-sign prefix, is a hard error rather than a silent pass-through. See docs/cl-divergences.md and lib/18-format.lisp's header for exact semantics.")
    (cons 'ARGS '((dest "NIL (string), T (stdout), or a PORTS port")
                  (ctrl "The control string")
                  (args "Zero or more arguments consumed by the control string's directives")))
    (cons 'RETURNS "The formatted string (DEST nil) or NIL (DEST t or a port)")
    (cons 'EXAMPLES '(((format nil "~a + ~a = ~a" 2 3 5) "2 + 3 = 5")
                       ((format nil "~,4f" 3.14159) "3.1416")
                       ((format nil "~{~a~^, ~}" (1 2 3)) "1, 2, 3")))
    (cons 'SEE-ALSO '(prin1-to-string princ-to-string ports:write-string!))))

(register-doc 'read-line
  (list
    (cons 'NAME 'read-line)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(read-line &optional port)")
    (cons 'CATEGORY 'io)
    (cons 'DESCRIPTION "Reads one line of text (bytes up to but excluding a trailing newline, decoded as UTF-8 lossy) from PORT, or from the process's standard input if PORT is not given (which requires the IO capability). Returns NIL only at true EOF. Thin sugar over PORTS:READ-LINE! (lib/18-format.lisp), lazily requiring the PORTS module on first use.")
    (cons 'ARGS '((port "Optional PORTS port; defaults to (ports:stdin)")))
    (cons 'RETURNS "A STRING, or NIL at true EOF")
    (cons 'SEE-ALSO '(ports:read-line! ports:stdin with-output-to-string))))

(register-doc 'with-output-to-string
  (list
    (cons 'NAME 'with-output-to-string)
    (cons 'TYPE 'macro)
    (cons 'SYNTAX "(with-output-to-string (var) body...)")
    (cons 'CATEGORY 'io)
    (cons 'DESCRIPTION "Binds VAR to a fresh in-memory output port for BODY's dynamic extent (write to it with ports:write-string!, ports:write-byte!/write-bytes!, or format with VAR as the destination) and returns everything written to it, decoded as UTF-8 (lossy), as a STRING. The port is always closed afterward; if BODY signals an error, that error propagates (no string is returned) and the port is still closed. Lazily requires the PORTS module on first use.")
    (cons 'ARGS '((binding "A one-element list (var)")
                  (body "Forms writing to VAR")))
    (cons 'RETURNS "The captured STRING")
    (cons 'EXAMPLES '(((with-output-to-string (s) (ports:write-string! s "hi")) "hi")))
    (cons 'SEE-ALSO '(read-line ports:open-output-bytes ports:output-contents))))

;;; ============================================================
;;; STRING FUNCTIONS
;;; ============================================================

(register-doc 'concat
  (list
    (cons 'NAME 'concat)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(concat string...)")
    (cons 'CATEGORY 'strings)
    (cons 'DESCRIPTION "Concatenates all string arguments.")
    (cons 'EXAMPLES '(((concat "Hello" " " "World") "Hello World")))
    (cons 'SEE-ALSO '(index explode))))

(register-doc 'index
  (list
    (cons 'NAME 'index)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(index string n)")
    (cons 'CATEGORY 'strings)
    (cons 'DESCRIPTION "Returns the character at position n (0-indexed) as a string.")
    (cons 'EXAMPLES '(((index "hello" 0) "h")
                       ((index "hello" 4) "o")))
    (cons 'SEE-ALSO '(concat explode))))

(register-doc 'explode
  (list
    (cons 'NAME 'explode)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(explode atom)")
    (cons 'CATEGORY 'strings)
    (cons 'DESCRIPTION "Converts an atom to a list of single-character symbols.")
    (cons 'EXAMPLES '(((explode 'hello) (h e l l o))))
    (cons 'SEE-ALSO '(implode intern))))

(register-doc 'implode
  (list
    (cons 'NAME 'implode)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(implode char-list)")
    (cons 'CATEGORY 'strings)
    (cons 'DESCRIPTION "Converts a list of character symbols to an interned symbol.")
    (cons 'EXAMPLES '(((implode '(h e l l o)) hello)))
    (cons 'SEE-ALSO '(explode intern gensym))))

(register-doc 'gensym
  (list
    (cons 'NAME 'gensym)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(gensym)")
    (cons 'CATEGORY 'strings)
    (cons 'DESCRIPTION "Generates a unique uninterned symbol.")
    (cons 'RETURNS "Unique symbol like G0001")
    (cons 'SEE-ALSO '(intern implode))))

(register-doc 'intern
  (list
    (cons 'NAME 'intern)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(intern string)")
    (cons 'CATEGORY 'strings)
    (cons 'DESCRIPTION "Interns a string as a symbol in the global symbol table.")
    (cons 'EXAMPLES '(((intern "HELLO") hello)))
    (cons 'SEE-ALSO '(implode gensym))))

;;; ============================================================
;;; SPECIAL FORMS
;;; ============================================================

(register-doc 'quote
  (list
    (cons 'NAME 'quote)
    (cons 'TYPE 'special-form)
    (cons 'SYNTAX "(quote expression) or 'expression")
    (cons 'CATEGORY 'special-forms)
    (cons 'DESCRIPTION "Prevents evaluation and returns expression as data.")
    (cons 'EXAMPLES '(((quote (+ 1 2)) (+ 1 2))
                       ('foo foo)))
    (cons 'SEE-ALSO '(quasiquote eval))))

(register-doc 'if
  (list
    (cons 'NAME 'if)
    (cons 'TYPE 'special-form)
    (cons 'SYNTAX "(if condition then-form else-form)")
    (cons 'CATEGORY 'special-forms)
    (cons 'DESCRIPTION "Evaluates condition; if non-NIL, evaluates then-form, otherwise else-form.")
    (cons 'EXAMPLES '(((if t "yes" "no") "yes")
                       ((if nil "yes" "no") "no")))
    (cons 'SEE-ALSO '(cond and or))))

(register-doc 'cond
  (list
    (cons 'NAME 'cond)
    (cons 'TYPE 'special-form)
    (cons 'SYNTAX "(cond (test form...)...)")
    (cons 'CATEGORY 'special-forms)
    (cons 'DESCRIPTION "Multi-way conditional. Evaluates tests until one is true, then evaluates its forms.")
    (cons 'EXAMPLES '(((cond ((= 1 2) "a") (t "b")) "b")))
    (cons 'SEE-ALSO '(if and or))))

(register-doc 'and
  (list
    (cons 'NAME 'and)
    (cons 'TYPE 'special-form)
    (cons 'SYNTAX "(and form...)")
    (cons 'CATEGORY 'special-forms)
    (cons 'DESCRIPTION "Short-circuit AND. Returns first NIL or last value.")
    (cons 'EXAMPLES '(((and t t t) t)
                       ((and t nil t) nil)
                       ((and 1 2 3) 3)))
    (cons 'SEE-ALSO '(or not if))))

(register-doc 'or
  (list
    (cons 'NAME 'or)
    (cons 'TYPE 'special-form)
    (cons 'SYNTAX "(or form...)")
    (cons 'CATEGORY 'special-forms)
    (cons 'DESCRIPTION "Short-circuit OR. Returns first non-NIL value or NIL.")
    (cons 'EXAMPLES '(((or nil nil t) t)
                       ((or 1 2 3) 1)))
    (cons 'SEE-ALSO '(and not if))))

(register-doc 'def
  (list
    (cons 'NAME 'def)
    (cons 'TYPE 'special-form)
    (cons 'SYNTAX "(def symbol value &optional docstring)")
    (cons 'CATEGORY 'special-forms)
    (cons 'DESCRIPTION "Binds symbol to value in the current environment.")
    (cons 'EXAMPLES '(((def x 42) x)))
    (cons 'SEE-ALSO '(setq let defun))))

(register-doc 'setq
  (list
    (cons 'NAME 'setq)
    (cons 'TYPE 'special-form)
    (cons 'SYNTAX "(setq symbol value)")
    (cons 'CATEGORY 'special-forms)
    (cons 'DESCRIPTION "Assigns a new value to an existing variable.")
    (cons 'SEE-ALSO '(def let))))

(register-doc 'let
  (list
    (cons 'NAME 'let)
    (cons 'TYPE 'special-form)
    (cons 'SYNTAX "(let ((var val)...) body...)")
    (cons 'CATEGORY 'special-forms)
    (cons 'DESCRIPTION "Creates local variable bindings for the duration of body.")
    (cons 'EXAMPLES '(((let ((x 1) (y 2)) (+ x y)) 3)))
    (cons 'SEE-ALSO '(def lambda prog))))

(register-doc 'lambda
  (list
    (cons 'NAME 'lambda)
    (cons 'TYPE 'special-form)
    (cons 'SYNTAX "(lambda (params...) body...)")
    (cons 'CATEGORY 'special-forms)
    (cons 'DESCRIPTION "Creates an anonymous function (closure).")
    (cons 'EXAMPLES '((((lambda (x) (* x x)) 5) 25)))
    (cons 'SEE-ALSO '(defun function apply))))

(register-doc 'defun
  (list
    (cons 'NAME 'defun)
    (cons 'TYPE 'macro)
    (cons 'SYNTAX "(defun name (params...) &optional docstring body...)")
    (cons 'CATEGORY 'special-forms)
    (cons 'DESCRIPTION "Defines a named function with optional docstring.")
    (cons 'SEE-ALSO '(lambda def defmacro))))

(register-doc 'defun*
  (list
    (cons 'NAME 'defun*)
    (cons 'TYPE 'vau)
    (cons 'SYNTAX "(defun* name [docstring] params... [return-type] body...)")
    (cons 'CATEGORY 'special-forms)
    (cons 'DESCRIPTION "Recommended default function definition form. Tries HM type inference automatically and compiles a native typed edition when the body is a fully-inferable typed island; otherwise falls back transparently to an ordinary lambda. Params may be classic ((a b)), flat bare (a b), or typed ((x int64)); an optional bare type keyword after the params pins the return type, and any unspecified type is inferred. Emits a note on stderr when types were inferred and compiled.")
    (cons 'EXAMPLES '(((defun* sq (x) (* x x)) sq)
                      ((defun* add (x int64) (y int64) (+ x y)) add)))
    (cons 'SEE-ALSO '(defun defun-typed defun-typed-opt check-type lambda))))

(register-doc 'defmacro
  (list
    (cons 'NAME 'defmacro)
    (cons 'TYPE 'special-form)
    (cons 'SYNTAX "(defmacro name (params...) body...)")
    (cons 'CATEGORY 'special-forms)
    (cons 'DESCRIPTION "Defines a macro that transforms code before evaluation.")
    (cons 'SEE-ALSO '(defun defexpr macroexpand))))

(register-doc 'macro
  (list
    (cons 'NAME 'macro)
    (cons 'TYPE 'special-form)
    (cons 'SYNTAX "(macro (params...) body...)")
    (cons 'CATEGORY 'special-forms)
    (cons 'DESCRIPTION "Anonymous macro constructor: evaluates to a macro VALUE (the macro counterpart of LAMBDA). Because operator dispatch resolves the head symbol through the lexical environment, a name locally bound to a macro value is used as an operator in that scope. Backs MACROLET.")
    (cons 'EXAMPLES '(((let ((sq (macro (x) (list '* x x)))) (sq 6)) 36)))
    (cons 'SEE-ALSO '(lambda fexpr vau defmacro macrolet))))

(register-doc 'fexpr
  (list
    (cons 'NAME 'fexpr)
    (cons 'TYPE 'special-form)
    (cons 'SYNTAX "(fexpr (params...) body...)")
    (cons 'CATEGORY 'special-forms)
    (cons 'DESCRIPTION "Anonymous fexpr constructor: evaluates to a fexpr VALUE whose operands reach the body unevaluated (the fexpr counterpart of LAMBDA). Backs FEXPRLET.")
    (cons 'EXAMPLES '(((let ((q (fexpr (a) (car a)))) (q (+ 1 2))) (+ 1 2))))
    (cons 'SEE-ALSO '(lambda macro vau defexpr fexprlet))))

(register-doc 'flet
  (list
    (cons 'NAME 'flet)
    (cons 'TYPE 'macro)
    (cons 'SYNTAX "(flet ((name (params...) body...) ...) body...)")
    (cons 'CATEGORY 'special-forms)
    (cons 'DESCRIPTION "Locally bind named functions (non-recursive) for the extent of the body. Parallel LET semantics: clauses do not see one another. A local binding shadows a global operator of the same name only within the body.")
    (cons 'EXAMPLES '(((flet ((sq (x) (* x x))) (sq 7)) 49)))
    (cons 'SEE-ALSO '(let lambda macrolet fexprlet vaulet))))

(register-doc 'macrolet
  (list
    (cons 'NAME 'macrolet)
    (cons 'TYPE 'macro)
    (cons 'SYNTAX "(macrolet ((name (params...) body...) ...) body...)")
    (cons 'CATEGORY 'special-forms)
    (cons 'DESCRIPTION "Locally bind macros for the extent of the body. Each clause is expanded at call sites like a DEFMACRO definition. Parallel LET semantics: clauses do not see one another.")
    (cons 'EXAMPLES '(((macrolet ((twice (e) (list 'progn e e))) (twice 1)) 1)))
    (cons 'SEE-ALSO '(macro defmacro flet fexprlet vaulet))))

(register-doc 'fexprlet
  (list
    (cons 'NAME 'fexprlet)
    (cons 'TYPE 'macro)
    (cons 'SYNTAX "(fexprlet ((name (params...) body...) ...) body...)")
    (cons 'CATEGORY 'special-forms)
    (cons 'DESCRIPTION "Locally bind fexprs (unevaluated-argument operatives) for the extent of the body. Operands reach the body unevaluated, as with DEFEXPR. Parallel LET semantics.")
    (cons 'EXAMPLES '(((fexprlet ((q (a) (car a))) (q (+ 1 2))) (+ 1 2))))
    (cons 'SEE-ALSO '(fexpr defexpr flet macrolet vaulet))))

(register-doc 'vaulet
  (list
    (cons 'NAME 'vaulet)
    (cons 'TYPE 'macro)
    (cons 'SYNTAX "(vaulet ((name (operands env) body...) ...) body...)")
    (cons 'CATEGORY 'special-forms)
    (cons 'DESCRIPTION "Locally bind vau operatives for the extent of the body. Each clause's OPERANDS receives the unevaluated operand list and ENV the caller's environment, as with VAU. Parallel LET semantics.")
    (cons 'SEE-ALSO '(vau $vau flet macrolet fexprlet))))

(register-doc 'check-type
  (list
    (cons 'NAME 'check-type)
    (cons 'TYPE 'special-form)
    (cons 'SYNTAX "(check-type name-or-expression)")
    (cons 'CATEGORY 'meta)
    (cons 'DESCRIPTION "Pure type-checking pass that never compiles. Given a function name (check-type f) or (check-type 'f) it reports that function's stored or inferred signature. Given any other expression it elaborates it in checker mode and returns the inferred type as a string: (check-type 10) is \"int64\", (check-type (+ 10 1)) is \"int64\", (check-type (+ 10 1.0)) is a type error, (check-type (array 5)) is \"(forall (a) (array a))\". Returns a string; makes no binding change and generates no code.")
    (cons 'EXAMPLES '(((check-type 10) "int64")
                      ((check-type (+ 10 1)) "int64")))
    (cons 'SEE-ALSO '(defun* defun-typed defun-typed-opt disassemble))))

(register-doc 'progn
  (list
    (cons 'NAME 'progn)
    (cons 'TYPE 'special-form)
    (cons 'SYNTAX "(progn form...)")
    (cons 'CATEGORY 'special-forms)
    (cons 'DESCRIPTION "Evaluates forms in sequence, returns last value.")
    (cons 'EXAMPLES '(((progn (+ 1 2) (* 3 4)) 12)))
    (cons 'SEE-ALSO '(prog let))))

(register-doc 'prog
  (list
    (cons 'NAME 'prog)
    (cons 'TYPE 'special-form)
    (cons 'SYNTAX "(prog (vars...) statements...)")
    (cons 'CATEGORY 'special-forms)
    (cons 'DESCRIPTION "Imperative block with local variables and labels for GO/RETURN.")
    (cons 'SEE-ALSO '(go return progn let))))

;;; ============================================================
;;; ERROR HANDLING
;;; ============================================================

(register-doc 'error
  (list
    (cons 'NAME 'error)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(error message)")
    (cons 'CATEGORY 'errors)
    (cons 'DESCRIPTION "Raises an error with the given message.")
    (cons 'SEE-ALSO '(errorset))))

(register-doc 'errorset
  (list
    (cons 'NAME 'errorset)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(errorset form)")
    (cons 'CATEGORY 'errors)
    (cons 'DESCRIPTION "Evaluates form, catching errors. Returns (result) on success, NIL on error.")
    (cons 'EXAMPLES '(((errorset '(+ 1 2)) (3))
                       ((errorset '(/ 1 0)) nil)))
    (cons 'SEE-ALSO '(error))))

;;; ============================================================
;;; METAPROGRAMMING
;;; ============================================================

(register-doc 'eval
  (list
    (cons 'NAME 'eval)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(eval expression)")
    (cons 'CATEGORY 'meta)
    (cons 'DESCRIPTION "Evaluates an expression.")
    (cons 'EXAMPLES '(((eval '(+ 1 2)) 3)))
    (cons 'SEE-ALSO '(apply funcall quote))))

(register-doc 'apply
  (list
    (cons 'NAME 'apply)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(apply function args)")
    (cons 'CATEGORY 'meta)
    (cons 'DESCRIPTION "Applies function to a list of arguments.")
    (cons 'EXAMPLES '(((apply '+ '(1 2 3)) 6)))
    (cons 'SEE-ALSO '(eval funcall mapcar))))

(register-doc 'funcall
  (list
    (cons 'NAME 'funcall)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(funcall function arg...)")
    (cons 'CATEGORY 'meta)
    (cons 'DESCRIPTION "Calls function with the given arguments.")
    (cons 'EXAMPLES '(((funcall '+ 1 2 3) 6)))
    (cons 'SEE-ALSO '(apply eval))))

;;; ============================================================
;;; PROPERTY LISTS
;;; ============================================================

(register-doc 'getp
  (list
    (cons 'NAME 'getp)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(getp symbol indicator)")
    (cons 'CATEGORY 'plists)
    (cons 'DESCRIPTION "Retrieves a property from a symbol's property list.")
    (cons 'SEE-ALSO '(putp remprop plist))))

(register-doc 'putp
  (list
    (cons 'NAME 'putp)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(putp symbol indicator value)")
    (cons 'CATEGORY 'plists)
    (cons 'DESCRIPTION "Sets a property on a symbol's property list.")
    (cons 'SEE-ALSO '(getp remprop plist))))

(register-doc 'plist
  (list
    (cons 'NAME 'plist)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(plist symbol)")
    (cons 'CATEGORY 'plists)
    (cons 'DESCRIPTION "Returns the entire property list of a symbol.")
    (cons 'SEE-ALSO '(getp putp))))

(register-doc 'documentation
  (list
    (cons 'NAME 'documentation)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(documentation symbol)")
    (cons 'CATEGORY 'plists)
    (cons 'DESCRIPTION "Returns the docstring for a symbol.")
    (cons 'SEE-ALSO '(getp help))))

;;; ============================================================
;;; HASH TABLES
;;; ============================================================

(register-doc 'make-hash-table
  (list
    (cons 'NAME 'make-hash-table)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(make-hash-table)")
    (cons 'CATEGORY 'hash-tables)
    (cons 'DESCRIPTION "Creates and returns a new empty hash table.")
    (cons 'SEE-ALSO '(gethash set-bang sethash keys))))

(register-doc 'get
  (list
    (cons 'NAME 'get)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(get symbol indicator)")
    (cons 'CATEGORY 'plists)
    (cons 'DESCRIPTION "Retrieves a property from symbol's property list. Classic Lisp 1.5 name for GETP. Returns NIL if the indicator is not found.")
    (cons 'EXAMPLES '(((get 'foo 'docstring) nil)))
    (cons 'SEE-ALSO '(getp putp plist remprop))))

(register-doc 'set-bang
  (list
    (cons 'NAME 'set-bang)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(set-bang hash-table key value)")
    (cons 'CATEGORY 'hash-tables)
    (cons 'DESCRIPTION "Sets the value for key in hash-table. SETHASH is accepted as a compatibility alias.")
    (cons 'SEE-ALSO '(gethash sethash remhash make-hash-table))))

(register-doc 'sethash
  (list
    (cons 'NAME 'sethash)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(sethash hash-table key value)")
    (cons 'CATEGORY 'hash-tables)
    (cons 'DESCRIPTION "Compatibility alias for SET-BANG. Sets the value for key in hash-table and returns T.")
    (cons 'EXAMPLES '(((let ((h (make-hash-table))) (sethash h 'x 42) (gethash h 'x)) 42)))
    (cons 'SEE-ALSO '(set-bang gethash delete-key make-hash-table))))

(register-doc 'keys
  (list
    (cons 'NAME 'keys)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(keys hash-table)")
    (cons 'CATEGORY 'hash-tables)
    (cons 'DESCRIPTION "Returns a list of all keys in hash-table.")
    (cons 'SEE-ALSO '(gethash set-bang make-hash-table))))

;;; ============================================================
;;; BITWISE FUNCTIONS
;;; ============================================================

(register-doc 'logior
  (list
    (cons 'NAME 'logior)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(logior integer...)")
    (cons 'CATEGORY 'bitwise)
    (cons 'DESCRIPTION "Bitwise OR of all arguments.")
    (cons 'EXAMPLES '(((logor 5 3) 7)))
    (cons 'SEE-ALSO '(logand logxor lognot))))

(register-doc 'logand
  (list
    (cons 'NAME 'logand)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(logand integer...)")
    (cons 'CATEGORY 'bitwise)
    (cons 'DESCRIPTION "Bitwise AND of all arguments.")
    (cons 'EXAMPLES '(((logand 5 3) 1)))
    (cons 'SEE-ALSO '(logor logxor lognot))))

(register-doc 'logxor
  (list
    (cons 'NAME 'logxor)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(logxor integer...)")
    (cons 'CATEGORY 'bitwise)
    (cons 'DESCRIPTION "Bitwise XOR of all arguments.")
    (cons 'EXAMPLES '(((logxor 5 3) 6)))
    (cons 'SEE-ALSO '(logor logand lognot))))

(register-doc 'lognot
  (list
    (cons 'NAME 'lognot)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(lognot integer)")
    (cons 'CATEGORY 'bitwise)
    (cons 'DESCRIPTION "Bitwise complement (NOT).")
    (cons 'EXAMPLES '(((lognot 0) -1)))
    (cons 'SEE-ALSO '(logor logand logxor))))

(register-doc 'leftshift
  (list
    (cons 'NAME 'leftshift)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(leftshift n count)")
    (cons 'CATEGORY 'bitwise)
    (cons 'DESCRIPTION "Shifts bits left (positive count) or right (negative count).")
    (cons 'EXAMPLES '(((leftshift 1 3) 8)
                       ((leftshift 8 -2) 2)))
    (cons 'SEE-ALSO '(ash logor logand))))

;;; ============================================================
;;; BITWISE (CONTINUED) — orphan entries
;;; ============================================================

(register-doc 'ash
  (list
    (cons 'NAME 'ash)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(ash n count)")
    (cons 'CATEGORY 'bitwise)
    (cons 'DESCRIPTION "Arithmetic shift. Shifts n left by count bits (right when count is negative).
Left shifts of 64 or more bits return 0 and set the OVERFLOW flag.
Right shifts of 64 or more bits return 0 or -1 (sign extension).
Both n and count must be integers.")
    (cons 'EXAMPLES '(((ash 1 4) 16)
                       ((ash 16 -2) 4)
                       ((ash -1 -1) -1)))
    (cons 'SEE-ALSO '(leftshift rot logor logand lognot logxor))))

(register-doc 'rot
  (list
    (cons 'NAME 'rot)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(rot n count)")
    (cons 'CATEGORY 'bitwise)
    (cons 'DESCRIPTION "Rotate bits of n left by count positions (64-bit rotation).
count is reduced modulo 64, so (rot n 64) equals (rot n 0).
Both n and count must be integers.")
    (cons 'EXAMPLES '(((rot 1 1) 2)
                       ((rot 1 63) "most-significant bit set")))
    (cons 'SEE-ALSO '(ash logor logand lognot))))

;;; ============================================================
;;; SYMBOL / STRING (CONTINUED) — orphan entries
;;; ============================================================

(register-doc 'maknam
  (list
    (cons 'NAME 'maknam)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(maknam char-list)")
    (cons 'CATEGORY 'strings)
    (cons 'DESCRIPTION "Converts a list of character symbols or strings to an interned symbol.
Identical to IMPLODE. Lisp 1.5 name for the same operation.")
    (cons 'EXAMPLES '(((maknam '(f o o)) foo)))
    (cons 'SEE-ALSO '(implode explode intern gensym))))

(register-doc 'macrop
  (list
    (cons 'NAME 'macrop)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(macrop x)")
    (cons 'CATEGORY 'predicates)
    (cons 'DESCRIPTION "Returns T if x is a macro object, NIL otherwise.")
    (cons 'EXAMPLES '(((defmacro m (x) x) m)
                       ((macrop (macro-function 'm)) t)))
    (cons 'SEE-ALSO '(functionp symbolp defmacro))))

(register-doc 'macroexpand
  (list
    (cons 'NAME 'macroexpand)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(macroexpand form)")
    (cons 'CATEGORY 'meta)
    (cons 'DESCRIPTION "Expands a macro call one level. If form is a list whose car names a macro,
returns the fully expanded form. If form is not a macro call, returns it unchanged.
Useful for debugging macro definitions.")
    (cons 'EXAMPLES '(((defmacro inc (x) `(+ ,x 1)) inc)
                       ((macroexpand '(inc 5)) (+ 5 1))))
    (cons 'SEE-ALSO '(defmacro macrop evlis))))

(register-doc 'put
  (list
    (cons 'NAME 'put)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(put symbol indicator value)")
    (cons 'CATEGORY 'plists)
    (cons 'DESCRIPTION "Alias for PUTP. Stores value under indicator on symbol's property list.
The classic Lisp 1.5 spelling.")
    (cons 'EXAMPLES '(((put 'foo 'color 'red) red)
                       ((getp 'foo 'color) red)))
    (cons 'SEE-ALSO '(putp getp plist remprop))))

;;; ============================================================
;;; HELP SYSTEM SELF-DOCUMENTATION
;;; ============================================================

(register-doc 'help
  (list
    (cons 'NAME 'help)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(help) or (help 'symbol) or (help :categories)")
    (cons 'CATEGORY 'meta)
    (cons 'DESCRIPTION "Interactive help system. Use (help) for overview, (help 'symbol) for specific help.")
    (cons 'SEE-ALSO '(documentation apropos))))

;;; ============================================================
;;; LISP 1.5 ARITHMETIC ALIASES
;;; ============================================================

(register-doc 'plus
  (list
    (cons 'NAME 'plus)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(plus number...)")
    (cons 'CATEGORY 'arithmetic)
    (cons 'DESCRIPTION "Classic Lisp 1.5 name for +. Returns the sum of all arguments.")
    (cons 'EXAMPLES '(((plus 1 2 3) 6)))
    (cons 'SEE-ALSO '(+ - times difference quotient))))

(register-doc 'difference
  (list
    (cons 'NAME 'difference)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(difference number number...)")
    (cons 'CATEGORY 'arithmetic)
    (cons 'DESCRIPTION "Classic Lisp 1.5 name for -. With one argument returns negation; with more, subtracts rest from first.")
    (cons 'EXAMPLES '(((difference 10 3) 7)))
    (cons 'SEE-ALSO '(- plus times quotient))))

(register-doc 'times
  (list
    (cons 'NAME 'times)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(times number...)")
    (cons 'CATEGORY 'arithmetic)
    (cons 'DESCRIPTION "Classic Lisp 1.5 name for *. Returns the product of all arguments.")
    (cons 'EXAMPLES '(((times 2 3 4) 24)))
    (cons 'SEE-ALSO '(* plus difference quotient))))

(register-doc 'quotient
  (list
    (cons 'NAME 'quotient)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(quotient dividend divisor)")
    (cons 'CATEGORY 'arithmetic)
    (cons 'DESCRIPTION "Classic Lisp 1.5 name for /. Returns the quotient; integer division truncates toward zero.")
    (cons 'EXAMPLES '(((quotient 10 3) 3)))
    (cons 'SEE-ALSO '(/ plus difference times remainder))))

(register-doc 'lessp
  (list
    (cons 'NAME 'lessp)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(lessp a b)")
    (cons 'CATEGORY 'predicates)
    (cons 'DESCRIPTION "Classic Lisp 1.5 name for <. Returns T if a is strictly less than b.")
    (cons 'EXAMPLES '(((lessp 1 2) t)
                       ((lessp 2 1) nil)))
    (cons 'SEE-ALSO '(< greaterp = float-lessp))))

(register-doc 'greaterp
  (list
    (cons 'NAME 'greaterp)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(greaterp a b)")
    (cons 'CATEGORY 'predicates)
    (cons 'DESCRIPTION "Classic Lisp 1.5 name for >. Returns T if a is strictly greater than b.")
    (cons 'EXAMPLES '(((greaterp 2 1) t)
                       ((greaterp 1 2) nil)))
    (cons 'SEE-ALSO '(> lessp = float-greaterp))))

(register-doc 'equal-number
  (list
    (cons 'NAME 'equal-number)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(equal-number a b)")
    (cons 'CATEGORY 'predicates)
    (cons 'DESCRIPTION "Alias for =. Returns T if a and b are numerically equal. Accepts both integers and floats.")
    (cons 'EXAMPLES '(((equal-number 1 1) t)
                       ((equal-number 1 1.0) t)))
    (cons 'SEE-ALSO '(= lessp greaterp))))

;;; ============================================================
;;; NUMERIC INCREMENT/DECREMENT ALIASES
;;; ============================================================

(register-doc '1+
  (list
    (cons 'NAME '1+)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(1+ n)")
    (cons 'CATEGORY 'arithmetic)
    (cons 'DESCRIPTION "Returns n + 1. Common Lisp-style alias for ADD1.")
    (cons 'EXAMPLES '(((1+ 5) 6)
                       ((1+ -1) 0)))
    (cons 'SEE-ALSO '(1- add1 sub1))))

(register-doc '1-
  (list
    (cons 'NAME '1-)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(1- n)")
    (cons 'CATEGORY 'arithmetic)
    (cons 'DESCRIPTION "Returns n - 1. Common Lisp-style alias for SUB1.")
    (cons 'EXAMPLES '(((1- 5) 4)
                       ((1- 1) 0)))
    (cons 'SEE-ALSO '(1+ sub1 add1))))

;;; ============================================================
;;; MATH LIBRARY (TRANSCENDENTALS AND ROUNDING)
;;; ============================================================

(register-doc 'sqrt
  (list
    (cons 'NAME 'sqrt)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(sqrt n)")
    (cons 'CATEGORY 'arithmetic)
    (cons 'DESCRIPTION "Returns the square root of n as a float. For integer square roots use ISQRT.")
    (cons 'EXAMPLES '(((sqrt 4) 2.0)
                       ((sqrt 2) 1.4142135)))
    (cons 'SEE-ALSO '(isqrt expt sin cos))))

(register-doc 'sin
  (list
    (cons 'NAME 'sin)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(sin radians)")
    (cons 'CATEGORY 'arithmetic)
    (cons 'DESCRIPTION "Returns the sine of an angle given in radians, as a float.")
    (cons 'EXAMPLES '(((sin 0) 0.0)
                       ((sin 3.14159) 0.0)))
    (cons 'SEE-ALSO '(cos tan sqrt))))

(register-doc 'cos
  (list
    (cons 'NAME 'cos)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(cos radians)")
    (cons 'CATEGORY 'arithmetic)
    (cons 'DESCRIPTION "Returns the cosine of an angle given in radians, as a float.")
    (cons 'EXAMPLES '(((cos 0) 1.0)))
    (cons 'SEE-ALSO '(sin tan sqrt))))

(register-doc 'tan
  (list
    (cons 'NAME 'tan)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(tan radians)")
    (cons 'CATEGORY 'arithmetic)
    (cons 'DESCRIPTION "Returns the tangent of an angle given in radians, as a float.")
    (cons 'EXAMPLES '(((tan 0) 0.0)))
    (cons 'SEE-ALSO '(sin cos))))

(register-doc 'log
  (list
    (cons 'NAME 'log)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(log x) or (log x base)")
    (cons 'CATEGORY 'arithmetic)
    (cons 'DESCRIPTION "With one argument returns the natural logarithm (ln) of x. With two arguments returns the logarithm of x in the given base.")
    (cons 'EXAMPLES '(((log 1) 0.0)
                       ((log 8 2) 3.0)))
    (cons 'SEE-ALSO '(exp sqrt expt))))

(register-doc 'exp
  (list
    (cons 'NAME 'exp)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(exp n)")
    (cons 'CATEGORY 'arithmetic)
    (cons 'DESCRIPTION "Returns e (Euler's number) raised to the power n, as a float.")
    (cons 'EXAMPLES '(((exp 1) 2.71828)
                       ((exp 0) 1.0)))
    (cons 'SEE-ALSO '(log expt))))

(register-doc 'floor
  (list
    (cons 'NAME 'floor)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(floor n)")
    (cons 'CATEGORY 'arithmetic)
    (cons 'DESCRIPTION "Returns the largest integer not greater than n (rounds toward negative infinity). Returns an integer even when given a float.")
    (cons 'EXAMPLES '(((floor 3.7) 3)
                       ((floor -3.7) -4)))
    (cons 'SEE-ALSO '(ceiling round truncate))))

(register-doc 'ceiling
  (list
    (cons 'NAME 'ceiling)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(ceiling n)")
    (cons 'CATEGORY 'arithmetic)
    (cons 'DESCRIPTION "Returns the smallest integer not less than n (rounds toward positive infinity). Returns an integer even when given a float.")
    (cons 'EXAMPLES '(((ceiling 3.2) 4)
                       ((ceiling -3.7) -3)))
    (cons 'SEE-ALSO '(floor round truncate))))

(register-doc 'round
  (list
    (cons 'NAME 'round)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(round n)")
    (cons 'CATEGORY 'arithmetic)
    (cons 'DESCRIPTION "Returns n rounded to the nearest integer. Ties round half away from zero (e.g. 0.5 rounds to 1, -0.5 rounds to -1). Returns an integer.")
    (cons 'EXAMPLES '(((round 3.5) 4)
                       ((round 3.4) 3)
                       ((round -3.5) -4)))
    (cons 'SEE-ALSO '(floor ceiling truncate))))

(register-doc 'truncate
  (list
    (cons 'NAME 'truncate)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(truncate n)")
    (cons 'CATEGORY 'arithmetic)
    (cons 'DESCRIPTION "Returns n truncated toward zero (drops the fractional part). Returns an integer. Equivalent to (floor n) for positive n and (ceiling n) for negative n.")
    (cons 'EXAMPLES '(((truncate 3.7) 3)
                       ((truncate -3.7) -3)))
    (cons 'SEE-ALSO '(floor ceiling round))))

(register-doc 'gcd
  (list
    (cons 'NAME 'gcd)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(gcd a b)")
    (cons 'CATEGORY 'arithmetic)
    (cons 'DESCRIPTION "Returns the greatest common divisor of integers a and b. Both arguments must be integers; sign is ignored.")
    (cons 'EXAMPLES '(((gcd 12 8) 4)
                       ((gcd 7 5) 1)))
    (cons 'SEE-ALSO '(lcm mod remainder))))

(register-doc 'lcm
  (list
    (cons 'NAME 'lcm)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(lcm a b)")
    (cons 'CATEGORY 'arithmetic)
    (cons 'DESCRIPTION "Returns the least common multiple of integers a and b. Returns 0 if either argument is 0. Both arguments must be integers.")
    (cons 'EXAMPLES '(((lcm 4 6) 12)
                       ((lcm 7 3) 21)))
    (cons 'SEE-ALSO '(gcd mod))))

(register-doc 'isqrt
  (list
    (cons 'NAME 'isqrt)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(isqrt n)")
    (cons 'CATEGORY 'arithmetic)
    (cons 'DESCRIPTION "Returns the integer square root of n (the largest integer k such that k*k <= n). Requires a non-negative integer argument. Use SQRT for floating-point results.")
    (cons 'EXAMPLES '(((isqrt 16) 4)
                       ((isqrt 17) 4)
                       ((isqrt 9) 3)))
    (cons 'SEE-ALSO '(sqrt gcd))))

(register-doc 'signum
  (list
    (cons 'NAME 'signum)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(signum n)")
    (cons 'CATEGORY 'arithmetic)
    (cons 'DESCRIPTION "Returns the sign of n: -1 for negative, 0 for zero, 1 for positive. Works on both integers (returns an integer) and floats (returns a float).")
    (cons 'EXAMPLES '(((signum 42) 1)
                       ((signum -7) -1)
                       ((signum 0) 0)))
    (cons 'SEE-ALSO '(abs plusp minusp zerop))))

;;; ============================================================
;;; FLOAT COMPARISON FUNCTIONS
;;; ============================================================

(register-doc 'float-equal
  (list
    (cons 'NAME 'float-equal)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(float-equal a b)")
    (cons 'CATEGORY 'predicates)
    (cons 'DESCRIPTION "Returns T if a and b are exactly bit-equal as floating-point values. Unlike =, this correctly distinguishes -0.0 from 0.0. Accepts both floats and integers (integers are widened to float before comparison).")
    (cons 'EXAMPLES '(((float-equal 1.0 1.0) t)
                       ((float-equal 0.0 -0.0) nil)))
    (cons 'SEE-ALSO '(= float-lessp float-greaterp))))

(register-doc 'float-lessp
  (list
    (cons 'NAME 'float-lessp)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(float-lessp a b)")
    (cons 'CATEGORY 'predicates)
    (cons 'DESCRIPTION "Returns T if a is strictly less than b in floating-point comparison. Accepts floats and integers. Use < for general numeric comparison.")
    (cons 'EXAMPLES '(((float-lessp 1.0 2.0) t)
                       ((float-lessp 2.0 1.0) nil)))
    (cons 'SEE-ALSO '(< float-greaterp float-equal))))

(register-doc 'float-greaterp
  (list
    (cons 'NAME 'float-greaterp)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(float-greaterp a b)")
    (cons 'CATEGORY 'predicates)
    (cons 'DESCRIPTION "Returns T if a is strictly greater than b in floating-point comparison. Accepts floats and integers. Use > for general numeric comparison.")
    (cons 'EXAMPLES '(((float-greaterp 2.0 1.0) t)
                       ((float-greaterp 1.0 2.0) nil)))
    (cons 'SEE-ALSO '(> float-lessp float-equal))))

;;; ============================================================
;;; STRING PRIMITIVES
;;; ============================================================

(register-doc 'string-length*
  (list
    (cons 'NAME 'string-length*)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(string-length* s)")
    (cons 'CATEGORY 'strings)
    (cons 'DESCRIPTION "Returns the number of Unicode characters in string s (not bytes). This is the kernel primitive; the Lisp layer builds higher-level string operations on top of it.")
    (cons 'EXAMPLES '(((string-length* "hello") 5)
                       ((string-length* "") 0)))
    (cons 'SEE-ALSO '(substring index concat))))

(register-doc 'substring
  (list
    (cons 'NAME 'substring)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(substring s start) or (substring s start end)")
    (cons 'CATEGORY 'strings)
    (cons 'DESCRIPTION "Returns a substring of s from character index start (inclusive, 0-based) to end (exclusive). End defaults to the length of s. Indices are clamped to valid bounds. Characters are counted by Unicode code point, not bytes.")
    (cons 'EXAMPLES '(((substring "hello" 1 3) "el")
                       ((substring "hello" 2) "llo")))
    (cons 'SEE-ALSO '(string-length* index concat))))

(register-doc 'char-code
  (list
    (cons 'NAME 'char-code)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(char-code c)")
    (cons 'CATEGORY 'strings)
    (cons 'DESCRIPTION "Returns the integer code point of c, where c is a Char value (from a literal like 'a') or a one-character string. Signals an error on an empty string.")
    (cons 'EXAMPLES '(((char-code "A") 65)
                       ((char-code 'a') 97)
                       ((char-code " ") 32)))
    (cons 'SEE-ALSO '(code-char make-char charp string-length*))))

(register-doc 'code-char
  (list
    (cons 'NAME 'code-char)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(code-char n)")
    (cons 'CATEGORY 'strings)
    (cons 'DESCRIPTION "Returns a one-character string containing the character at code point n. The inverse of CHAR-CODE. Signals an error if n is not a valid code point. (Use MAKE-CHAR to build a Char value instead of a string.)")
    (cons 'EXAMPLES '(((code-char 65) "A")
                       ((code-char 97) "a")))
    (cons 'SEE-ALSO '(char-code make-char string-length*))))

(register-doc 'charp
  (list
    (cons 'NAME 'charp)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(charp x)")
    (cons 'CATEGORY 'predicates)
    (cons 'DESCRIPTION "Returns T if x is a Char value (produced by a char literal like 'a'). NIL for integers, strings, and all other types. Distinct from FIXP, which is NIL for chars.")
    (cons 'EXAMPLES '(((charp 'a') t)
                       ((charp 97) nil)
                       ((charp "a") nil)))
    (cons 'SEE-ALSO '(make-char char-code code-char fixp))))

(register-doc 'make-char
  (list
    (cons 'NAME 'make-char)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(make-char n)")
    (cons 'CATEGORY 'strings)
    (cons 'DESCRIPTION "Returns a Char value for integer code point n (0-255). The Char-producing complement of CODE-CHAR, which returns a one-character string. Inverse of CHAR-CODE on Char inputs.")
    (cons 'EXAMPLES '(((make-char 65) 'A')
                       ((charp (make-char 65)) t)))
    (cons 'SEE-ALSO '(charp char-code code-char))))

(register-doc 'string->number
  (list
    (cons 'NAME 'string->number)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(string->number s)")
    (cons 'CATEGORY 'strings)
    (cons 'DESCRIPTION "Parses string s as a number. Tries integer first, then float. Returns the parsed number on success, or NIL if the string cannot be parsed as a number. Leading and trailing whitespace is ignored.")
    (cons 'EXAMPLES '(((string->number "42") 42)
                       ((string->number "3.14") 3.14)
                       ((string->number "abc") nil)))
    (cons 'SEE-ALSO '(number->string read))))

(register-doc 'number->string
  (list
    (cons 'NAME 'number->string)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(number->string n)")
    (cons 'CATEGORY 'strings)
    (cons 'DESCRIPTION "Converts number n to its decimal string representation. Integers produce digit strings; floats produce Rust's default float formatting.")
    (cons 'EXAMPLES '(((number->string 42) "42")
                       ((number->string 3.14) "3.14")))
    (cons 'SEE-ALSO '(string->number prin1-to-string concat))))

(register-doc 'prin1-to-string
  (list
    (cons 'NAME 'prin1-to-string)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(prin1-to-string object)")
    (cons 'CATEGORY 'strings)
    (cons 'DESCRIPTION "Returns the readable printed representation of object as a string, exactly as PRIN1 would print it to stdout. Strings are wrapped in double quotes; symbols print uppercased; cons cells print as S-expressions.")
    (cons 'EXAMPLES '(((prin1-to-string "hello") "\"hello\"")
                       ((prin1-to-string '(1 2)) "(1 2)")))
    (cons 'SEE-ALSO '(princ-to-string prin1 number->string))))

(register-doc 'princ-to-string
  (list
    (cons 'NAME 'princ-to-string)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(princ-to-string object)")
    (cons 'CATEGORY 'strings)
    (cons 'DESCRIPTION "Returns the human-readable printed representation of object as a string, exactly as PRINC would print it to stdout. Top-level strings are returned without surrounding quotes; everything else uses the same format as PRIN1-TO-STRING.")
    (cons 'EXAMPLES '(((princ-to-string "hello") "hello")
                       ((princ-to-string 42) "42")))
    (cons 'SEE-ALSO '(prin1-to-string princ number->string))))

;;; ============================================================
;;; STRING API COMPLETIONS (issue #254, epic #253)
;;; ============================================================

(register-doc 'make-string
  (list
    (cons 'NAME 'make-string)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(make-string n) or (make-string n char)")
    (cons 'CATEGORY 'strings)
    (cons 'DESCRIPTION "Returns a fresh string of length n, every character char (a one-character string or code point; default space). Signals an error if n is negative.")
    (cons 'EXAMPLES '(((make-string 3) "   ")
                       ((make-string 3 "x") "xxx")))
    (cons 'SEE-ALSO '(string-repeat string-pad-left string-pad-right))))

(register-doc 'string-empty-p
  (list
    (cons 'NAME 'string-empty-p)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(string-empty-p s)")
    (cons 'CATEGORY 'strings)
    (cons 'DESCRIPTION "True if s has length zero.")
    (cons 'EXAMPLES '(((string-empty-p "") t)
                       ((string-empty-p "a") nil)))
    (cons 'SEE-ALSO '(string-length*))))

(register-doc 'string-concat
  (list
    (cons 'NAME 'string-concat)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(string-concat &rest strs)")
    (cons 'CATEGORY 'strings)
    (cons 'DESCRIPTION "Concatenates zero or more strings. A named alias for CONCAT.")
    (cons 'EXAMPLES '(((string-concat "a" "b" "c") "abc")
                       ((string-concat) "")))
    (cons 'SEE-ALSO '(concat))))

(register-doc 'char-at
  (list
    (cons 'NAME 'char-at)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(char-at s i)")
    (cons 'CATEGORY 'strings)
    (cons 'DESCRIPTION "One-character access: the character at index i in s, as a one-character string. Unlike SUBSTRING, an out-of-range i signals a clear error naming i and s's length instead of clamping.")
    (cons 'EXAMPLES '(((char-at "hello" 0) "h")
                       ((char-at "hello" 4) "o")))
    (cons 'SEE-ALSO '(substring string-length*))))

(register-doc 'string<
  (list
    (cons 'NAME 'string<)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(string< a b)")
    (cons 'CATEGORY 'strings)
    (cons 'DESCRIPTION "True if string a is lexicographically (by code point) before string b. Case-sensitive. Same ordering as STRING-LESSP, under CL's name for the case-sensitive comparison.")
    (cons 'EXAMPLES '(((string< "abc" "abd") t)))
    (cons 'SEE-ALSO '(string> string<= string>= string-lessp string-ci<))))

(register-doc 'string>
  (list
    (cons 'NAME 'string>)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(string> a b)")
    (cons 'CATEGORY 'strings)
    (cons 'DESCRIPTION "True if string a is lexicographically (by code point) after string b. Case-sensitive.")
    (cons 'EXAMPLES '(((string> "abd" "abc") t)))
    (cons 'SEE-ALSO '(string< string<= string>= string-ci>))))

(register-doc 'string<=
  (list
    (cons 'NAME 'string<=)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(string<= a b)")
    (cons 'CATEGORY 'strings)
    (cons 'DESCRIPTION "Non-strict case-sensitive ordering: true unless a comes lexicographically after b.")
    (cons 'EXAMPLES '(((string<= "abc" "abc") t)))
    (cons 'SEE-ALSO '(string< string> string>= string-ci<=))))

(register-doc 'string>=
  (list
    (cons 'NAME 'string>=)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(string>= a b)")
    (cons 'CATEGORY 'strings)
    (cons 'DESCRIPTION "Non-strict case-sensitive ordering: true unless a comes lexicographically before b.")
    (cons 'EXAMPLES '(((string>= "abc" "abc") t)))
    (cons 'SEE-ALSO '(string< string> string<= string-ci>=))))

(register-doc 'string-ne
  (list
    (cons 'NAME 'string-ne)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(string-ne a b)")
    (cons 'CATEGORY 'strings)
    (cons 'DESCRIPTION "True if strings a and b do NOT have the same contents. Case-sensitive. Named STRING-NE rather than CL's STRING/=: the reader does not treat `/` as a symbol constituent, so `string/=` cannot be written as one token.")
    (cons 'EXAMPLES '(((string-ne "a" "b") t)
                       ((string-ne "a" "a") nil)))
    (cons 'SEE-ALSO '(string= string-ci-ne))))

(register-doc 'string-ci=
  (list
    (cons 'NAME 'string-ci=)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(string-ci= a b)")
    (cons 'CATEGORY 'strings)
    (cons 'DESCRIPTION "True if a and b have the same contents under Unicode default case folding (via STRING-CASEFOLD*: locale-independent, not ASCII-only). Named with a `-ci` infix rather than CL's STRING-EQUAL, because STRING-LESSP already has case-sensitive semantics here.")
    (cons 'EXAMPLES '(((string-ci= "ABC" "abc") t)))
    (cons 'SEE-ALSO '(string= string-ci-ne string-ci< string-ci>))))

(register-doc 'string-ci-ne
  (list
    (cons 'NAME 'string-ci-ne)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(string-ci-ne a b)")
    (cons 'CATEGORY 'strings)
    (cons 'DESCRIPTION "True if a and b do NOT have the same contents under Unicode case folding.")
    (cons 'EXAMPLES '(((string-ci-ne "ABC" "xyz") t)))
    (cons 'SEE-ALSO '(string-ci= string-ne))))

(register-doc 'string-ci<
  (list
    (cons 'NAME 'string-ci<)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(string-ci< a b)")
    (cons 'CATEGORY 'strings)
    (cons 'DESCRIPTION "True if a is lexicographically before b under Unicode case folding.")
    (cons 'EXAMPLES '(((string-ci< "abc" "ABD") t)))
    (cons 'SEE-ALSO '(string-ci> string-ci<= string-ci>= string<))))

(register-doc 'string-ci>
  (list
    (cons 'NAME 'string-ci>)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(string-ci> a b)")
    (cons 'CATEGORY 'strings)
    (cons 'DESCRIPTION "True if a is lexicographically after b under Unicode case folding.")
    (cons 'EXAMPLES '(((string-ci> "ABD" "abc") t)))
    (cons 'SEE-ALSO '(string-ci< string-ci<= string-ci>=))))

(register-doc 'string-ci<=
  (list
    (cons 'NAME 'string-ci<=)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(string-ci<= a b)")
    (cons 'CATEGORY 'strings)
    (cons 'DESCRIPTION "Non-strict case-insensitive ordering: true unless a comes after b under Unicode case folding.")
    (cons 'EXAMPLES '(((string-ci<= "ABC" "abc") t)))
    (cons 'SEE-ALSO '(string-ci< string-ci> string-ci>=))))

(register-doc 'string-ci>=
  (list
    (cons 'NAME 'string-ci>=)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(string-ci>= a b)")
    (cons 'CATEGORY 'strings)
    (cons 'DESCRIPTION "Non-strict case-insensitive ordering: true unless a comes before b under Unicode case folding.")
    (cons 'EXAMPLES '(((string-ci>= "ABC" "abc") t)))
    (cons 'SEE-ALSO '(string-ci< string-ci> string-ci<=))))

(register-doc 'string-last-index-of
  (list
    (cons 'NAME 'string-last-index-of)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(string-last-index-of s sub)")
    (cons 'CATEGORY 'strings)
    (cons 'DESCRIPTION "Returns the index of the LAST (rightmost) occurrence of non-empty sub in s, or NIL if sub does not occur (or is empty).")
    (cons 'EXAMPLES '(((string-last-index-of "abcabc" "bc") 4)))
    (cons 'SEE-ALSO '(string-index-of string-count))))

(register-doc 'string-count
  (list
    (cons 'NAME 'string-count)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(string-count s sub)")
    (cons 'CATEGORY 'strings)
    (cons 'DESCRIPTION "Counts non-overlapping occurrences of non-empty sub in s; 0 if sub is empty or does not occur.")
    (cons 'EXAMPLES '(((string-count "abcabcabc" "abc") 3)
                       ((string-count "aaaa" "aa") 2)))
    (cons 'SEE-ALSO '(string-index-of string-last-index-of))))

(register-doc 'string-replace-first
  (list
    (cons 'NAME 'string-replace-first)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(string-replace-first s old new)")
    (cons 'CATEGORY 'strings)
    (cons 'DESCRIPTION "Replaces only the first (non-empty) occurrence of old in s with new.")
    (cons 'EXAMPLES '(((string-replace-first "aaa" "a" "b") "baa")))
    (cons 'SEE-ALSO '(string-replace string-replace-all))))

(register-doc 'string-replace-all
  (list
    (cons 'NAME 'string-replace-all)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(string-replace-all s old new)")
    (cons 'CATEGORY 'strings)
    (cons 'DESCRIPTION "Replaces every (non-empty) occurrence of old in s with new. Alias for STRING-REPLACE, named to pair explicitly with STRING-REPLACE-FIRST.")
    (cons 'EXAMPLES '(((string-replace-all "aaa" "a" "b") "bbb")))
    (cons 'SEE-ALSO '(string-replace string-replace-first))))

(register-doc 'string-trim-left
  (list
    (cons 'NAME 'string-trim-left)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(string-trim-left s)")
    (cons 'CATEGORY 'strings)
    (cons 'DESCRIPTION "Removes leading whitespace from s.")
    (cons 'EXAMPLES '(((string-trim-left "  hi  ") "hi  ")))
    (cons 'SEE-ALSO '(string-trim-right string-trim))))

(register-doc 'string-trim-right
  (list
    (cons 'NAME 'string-trim-right)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(string-trim-right s)")
    (cons 'CATEGORY 'strings)
    (cons 'DESCRIPTION "Removes trailing whitespace from s.")
    (cons 'EXAMPLES '(((string-trim-right "  hi  ") "  hi")))
    (cons 'SEE-ALSO '(string-trim-left string-trim))))

(register-doc 'string-capitalize
  (list
    (cons 'NAME 'string-capitalize)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(string-capitalize s)")
    (cons 'CATEGORY 'strings)
    (cons 'DESCRIPTION "Returns s with its first character uppercased (ASCII) and the rest lowercased.")
    (cons 'EXAMPLES '(((string-capitalize "hELLO world") "Hello World")))
    (cons 'SEE-ALSO '(string-upcase string-downcase))))

(register-doc 'string-reverse
  (list
    (cons 'NAME 'string-reverse)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(string-reverse s)")
    (cons 'CATEGORY 'strings)
    (cons 'DESCRIPTION "Reverses s. A named entry point onto the generic REVERSE (which already works on strings).")
    (cons 'EXAMPLES '(((string-reverse "hello") "olleh")))
    (cons 'SEE-ALSO '(reverse))))

;;; ============================================================
;;; TEXT MODULE: UTF-8 <-> Array<Char> (issue #254, epic #253)
;;; ============================================================

(register-doc 'text:string->utf8
  (list
    (cons 'NAME 'text:string->utf8)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(text:string->utf8 s)")
    (cons 'CATEGORY 'text)
    (cons 'DESCRIPTION "Returns the exact UTF-8 bytes of string s as a fresh Array<Char> (an array whose every element is a Char byte 0-255). Never fails: every Lisp STRING is valid Unicode. Call qualified, or (import text) first to use STRING->UTF8 unqualified.")
    (cons 'EXAMPLES '(((array-length* (text:string->utf8 "hi")) 2)))
    (cons 'SEE-ALSO '(text:utf8->string text:utf8->string-lossy))))

(register-doc 'text:utf8->string
  (list
    (cons 'NAME 'text:utf8->string)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(text:utf8->string bytes)")
    (cons 'CATEGORY 'text)
    (cons 'DESCRIPTION "Decodes bytes (an Array<Char>) as UTF-8 and returns the resulting STRING. Strict: signals a descriptive error naming the offending byte offset if bytes is not well-formed UTF-8; use UTF8->STRING-LOSSY for replacement-character decoding instead.")
    (cons 'EXAMPLES '(((text:utf8->string (text:string->utf8 "hi")) "hi")))
    (cons 'SEE-ALSO '(text:string->utf8 text:utf8->string-lossy))))

(register-doc 'text:utf8->string-lossy
  (list
    (cons 'NAME 'text:utf8->string-lossy)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(text:utf8->string-lossy bytes)")
    (cons 'CATEGORY 'text)
    (cons 'DESCRIPTION "Decodes bytes (an Array<Char>) as UTF-8, substituting the Unicode replacement character (U+FFFD) for any invalid byte sequence instead of signalling an error.")
    (cons 'EXAMPLES '(((text:utf8->string-lossy (text:string->utf8 "hi")) "hi")))
    (cons 'SEE-ALSO '(text:string->utf8 text:utf8->string))))

;;; ============================================================
;;; PORTS MODULE: binary I/O (issue #255, epic #253)
;;; ============================================================

(register-doc 'ports:open-input
  (list
    (cons 'NAME 'ports:open-input)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(ports:open-input path)")
    (cons 'CATEGORY 'ports)
    (cons 'DESCRIPTION "Opens path as a binary input port. Requires the READ-FS capability.")
    (cons 'SEE-ALSO '(ports:open-output ports:open-append ports:with-open-port))))

(register-doc 'ports:open-output
  (list
    (cons 'NAME 'ports:open-output)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(ports:open-output path)")
    (cons 'CATEGORY 'ports)
    (cons 'DESCRIPTION "Opens path as a binary output port, truncating any existing contents (creating the file if needed). Requires the CREATE-FS capability.")
    (cons 'SEE-ALSO '(ports:open-input ports:open-append))))

(register-doc 'ports:open-append
  (list
    (cons 'NAME 'ports:open-append)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(ports:open-append path)")
    (cons 'CATEGORY 'ports)
    (cons 'DESCRIPTION "Opens path as a binary output port positioned at end-of-file, preserving existing contents. Requires the CREATE-FS capability.")
    (cons 'SEE-ALSO '(ports:open-output))))

(register-doc 'ports:open-input-bytes
  (list
    (cons 'NAME 'ports:open-input-bytes)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(ports:open-input-bytes bytes)")
    (cons 'CATEGORY 'ports)
    (cons 'DESCRIPTION "Opens a binary input port reading from a private copy of bytes (an Array<Char>). No capability required.")
    (cons 'EXAMPLES '(((ports:read-byte! (ports:open-input-bytes (list->array (list 65)))) 65)))
    (cons 'SEE-ALSO '(ports:open-output-bytes ports:output-contents))))

(register-doc 'ports:open-output-bytes
  (list
    (cons 'NAME 'ports:open-output-bytes)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(ports:open-output-bytes)")
    (cons 'CATEGORY 'ports)
    (cons 'DESCRIPTION "Opens a binary output port that accumulates written bytes in memory; read them back with ports:output-contents. No capability required; not seekable.")
    (cons 'SEE-ALSO '(ports:open-input-bytes ports:output-contents))))

(register-doc 'ports:output-contents
  (list
    (cons 'NAME 'ports:output-contents)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(ports:output-contents port)")
    (cons 'CATEGORY 'ports)
    (cons 'DESCRIPTION "Returns the bytes written so far to an open-output-bytes port, as a fresh Array<Char>.")
    (cons 'SEE-ALSO '(ports:open-output-bytes))))

(register-doc 'ports:stdin
  (list
    (cons 'NAME 'ports:stdin)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(ports:stdin)")
    (cons 'CATEGORY 'ports)
    (cons 'DESCRIPTION "The process's standard input as a binary input port. Requires the IO capability.")
    (cons 'SEE-ALSO '(ports:stdout ports:stderr))))

(register-doc 'ports:stdout
  (list
    (cons 'NAME 'ports:stdout)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(ports:stdout)")
    (cons 'CATEGORY 'ports)
    (cons 'DESCRIPTION "The process's standard output as a binary output port. No capability required.")
    (cons 'SEE-ALSO '(ports:stdin ports:stderr))))

(register-doc 'ports:stderr
  (list
    (cons 'NAME 'ports:stderr)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(ports:stderr)")
    (cons 'CATEGORY 'ports)
    (cons 'DESCRIPTION "The process's standard error as a binary output port. No capability required.")
    (cons 'SEE-ALSO '(ports:stdin ports:stdout))))

(register-doc 'ports:read-byte!
  (list
    (cons 'NAME 'ports:read-byte!)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(ports:read-byte! port)")
    (cons 'CATEGORY 'ports)
    (cons 'DESCRIPTION "Reads one byte from port as an integer 0-255, or NIL at EOF.")
    (cons 'SEE-ALSO '(ports:read-bytes! ports:write-byte!))))

(register-doc 'ports:read-bytes!
  (list
    (cons 'NAME 'ports:read-bytes!)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(ports:read-bytes! port n)")
    (cons 'CATEGORY 'ports)
    (cons 'DESCRIPTION "Reads up to n bytes from port into a fresh Array<Char>. May be shorter than n (including empty) at EOF or on a partial read; never NIL.")
    (cons 'SEE-ALSO '(ports:read-byte! ports:read-all-bytes!))))

(register-doc 'ports:write-byte!
  (list
    (cons 'NAME 'ports:write-byte!)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(ports:write-byte! port byte)")
    (cons 'CATEGORY 'ports)
    (cons 'DESCRIPTION "Writes one byte (a Char or integer 0-255) to port.")
    (cons 'SEE-ALSO '(ports:write-bytes! ports:read-byte!))))

(register-doc 'ports:write-bytes!
  (list
    (cons 'NAME 'ports:write-bytes!)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(ports:write-bytes! port bytes)")
    (cons 'CATEGORY 'ports)
    (cons 'DESCRIPTION "Writes bytes (an Array<Char>) to port; returns the number of bytes actually written (may be less than the length of bytes on a partial write).")
    (cons 'SEE-ALSO '(ports:write-byte! ports:read-bytes!))))

(register-doc 'ports:flush!
  (list
    (cons 'NAME 'ports:flush!)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(ports:flush! port)")
    (cons 'CATEGORY 'ports)
    (cons 'DESCRIPTION "Flushes any buffered writes on port.")
    (cons 'SEE-ALSO '(ports:write-bytes! ports:close!))))

(register-doc 'ports:close!
  (list
    (cons 'NAME 'ports:close!)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(ports:close! port)")
    (cons 'CATEGORY 'ports)
    (cons 'DESCRIPTION "Closes port. Idempotent: closing an already-closed port is a silent no-op, never an error.")
    (cons 'SEE-ALSO '(ports:with-open-port ports:open-p))))

(register-doc 'ports:with-open-port
  (list
    (cons 'NAME 'ports:with-open-port)
    (cons 'TYPE 'macro)
    (cons 'SYNTAX "(ports:with-open-port (var port-expr) body...)")
    (cons 'CATEGORY 'ports)
    (cons 'DESCRIPTION "Binds var to the value of port-expr (a port) for body's dynamic extent, unconditionally closing it afterward: normal return, an ordinary error, THROW, RETURN-FROM, or GO unwinding all run the close, via UNWIND-PROTECT. Double-close is a no-op, so body may close var itself without error.")
    (cons 'EXAMPLES '(((ports:with-open-port (p (ports:open-input-bytes (list->array (list 1 2)))) (ports:read-byte! p)) 1)))
    (cons 'SEE-ALSO '(ports:close! ports:open-input ports:open-output))))

(register-doc 'ports:port-p
  (list
    (cons 'NAME 'ports:port-p)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(ports:port-p v)")
    (cons 'CATEGORY 'ports)
    (cons 'DESCRIPTION "T if v is a port (open or closed) of any kind.")
    (cons 'SEE-ALSO '(ports:open-p ports:input-p ports:output-p))))

(register-doc 'ports:open-p
  (list
    (cons 'NAME 'ports:open-p)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(ports:open-p port)")
    (cons 'CATEGORY 'ports)
    (cons 'DESCRIPTION "T if port has not been closed.")
    (cons 'SEE-ALSO '(ports:close! ports:port-p))))

(register-doc 'ports:input-p
  (list
    (cons 'NAME 'ports:input-p)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(ports:input-p port)")
    (cons 'CATEGORY 'ports)
    (cons 'DESCRIPTION "T if port supports reading.")
    (cons 'SEE-ALSO '(ports:output-p ports:port-p))))

(register-doc 'ports:output-p
  (list
    (cons 'NAME 'ports:output-p)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(ports:output-p port)")
    (cons 'CATEGORY 'ports)
    (cons 'DESCRIPTION "T if port supports writing.")
    (cons 'SEE-ALSO '(ports:input-p ports:port-p))))

(register-doc 'ports:name
  (list
    (cons 'NAME 'ports:name)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(ports:name port)")
    (cons 'CATEGORY 'ports)
    (cons 'DESCRIPTION "port's diagnostic name (e.g. a file path, or \"<stdin>\").")
    (cons 'SEE-ALSO '(ports:kind))))

(register-doc 'ports:kind
  (list
    (cons 'NAME 'ports:kind)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(ports:kind port)")
    (cons 'CATEGORY 'ports)
    (cons 'DESCRIPTION "port's diagnostic resource kind, as a symbol: FILE, MEMORY, STDIN, STDOUT, or STDERR (or a host-registered kind for an embedder-wrapped port).")
    (cons 'SEE-ALSO '(ports:name))))

(register-doc 'ports:seekable-p
  (list
    (cons 'NAME 'ports:seekable-p)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(ports:seekable-p port)")
    (cons 'CATEGORY 'ports)
    (cons 'DESCRIPTION "T if port supports ports:position/ports:seek!. Files and byte-array input ports are seekable; byte-array output ports and the standard streams are not.")
    (cons 'SEE-ALSO '(ports:position ports:seek!))))

(register-doc 'ports:position
  (list
    (cons 'NAME 'ports:position)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(ports:position port)")
    (cons 'CATEGORY 'ports)
    (cons 'DESCRIPTION "The current byte offset in a seekable port. Signals an error on a non-seekable port. Qualified-only: deliberately not bound unqualified by (import ports), because the Prelude's flat (position item lst) list helper would be shadowed.")
    (cons 'SEE-ALSO '(ports:seek! ports:seekable-p))))

(register-doc 'ports:seek!
  (list
    (cons 'NAME 'ports:seek!)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(ports:seek! port offset)")
    (cons 'CATEGORY 'ports)
    (cons 'DESCRIPTION "Moves a seekable port to absolute byte offset from the start; returns the new position. Signals an error on a non-seekable port.")
    (cons 'SEE-ALSO '(ports:position ports:seekable-p))))

(register-doc 'ports:read-line!
  (list
    (cons 'NAME 'ports:read-line!)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(ports:read-line! port)")
    (cons 'CATEGORY 'ports)
    (cons 'DESCRIPTION "Reads one line of text from port: bytes up to but excluding a trailing newline, decoded as UTF-8 (lossy). Returns NIL only at true EOF; a final line with no trailing newline is still returned once.")
    (cons 'SEE-ALSO '(ports:read-string! ports:write-string!))))

(register-doc 'ports:read-string!
  (list
    (cons 'NAME 'ports:read-string!)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(ports:read-string! port n)")
    (cons 'CATEGORY 'ports)
    (cons 'DESCRIPTION "Reads up to n bytes from port and decodes them as UTF-8 (lossy), returning a STRING.")
    (cons 'SEE-ALSO '(ports:read-line! ports:write-string!))))

(register-doc 'ports:write-string!
  (list
    (cons 'NAME 'ports:write-string!)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(ports:write-string! port s)")
    (cons 'CATEGORY 'ports)
    (cons 'DESCRIPTION "Writes string s to port as its exact UTF-8 bytes. Returns the number of bytes written.")
    (cons 'SEE-ALSO '(ports:read-string! ports:read-line!))))

(register-doc 'ports:read-all-bytes!
  (list
    (cons 'NAME 'ports:read-all-bytes!)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(ports:read-all-bytes! port)")
    (cons 'CATEGORY 'ports)
    (cons 'DESCRIPTION "Reads port to EOF, returning every remaining byte as a fresh Array<Char>.")
    (cons 'SEE-ALSO '(ports:read-bytes!))))

;;; ============================================================
;;; BASE64 MODULE: Base64 encode/decode (issue #257, epic #253)
;;; ============================================================

(register-doc 'base64:encode
  (list
    (cons 'NAME 'base64:encode)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(base64:encode bytes &key (alphabet ':standard) (pad t))")
    (cons 'CATEGORY 'base64)
    (cons 'DESCRIPTION "Encodes bytes (an Array<Char>, elements Char or integer 0-255) as a Base64 ASCII String. :alphabet is :standard (RFC 4648 \"+/\") or :url (RFC 4648 \"-_\"); :pad (default T) controls trailing \"=\" padding.")
    (cons 'EXAMPLES '(((base64:encode (text:string->utf8 "foo")) "Zm9v")))
    (cons 'SEE-ALSO '(base64:decode hex:encode))))

(register-doc 'base64:decode
  (list
    (cons 'NAME 'base64:decode)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(base64:decode s &key (alphabet ':standard) (pad t))")
    (cons 'CATEGORY 'base64)
    (cons 'DESCRIPTION "Decodes s (a Base64 ASCII String, per :alphabet/:pad) into a fresh Array<Char> of the exact original bytes. Strict: invalid characters, misplaced/wrong-count padding, or a length inconsistent with the padding policy are named errors.")
    (cons 'EXAMPLES '(((array->list (base64:decode "Zm9v")) (102 111 111))))
    (cons 'SEE-ALSO '(base64:encode hex:decode))))

;;; ============================================================
;;; HEX MODULE: hexadecimal encode/decode (issue #257, epic #253)
;;; ============================================================

(register-doc 'hex:encode
  (list
    (cons 'NAME 'hex:encode)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(hex:encode bytes &key (case ':lower))")
    (cons 'CATEGORY 'hex)
    (cons 'DESCRIPTION "Encodes bytes (an Array<Char>, elements Char or integer 0-255) as a hexadecimal ASCII String, two digits per byte. :case is :lower (default) or :upper.")
    (cons 'EXAMPLES '(((hex:encode (text:string->utf8 "AB")) "4142")))
    (cons 'SEE-ALSO '(hex:decode base64:encode))))

(register-doc 'hex:decode
  (list
    (cons 'NAME 'hex:decode)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(hex:decode s)")
    (cons 'CATEGORY 'hex)
    (cons 'DESCRIPTION "Decodes s (a hexadecimal ASCII String, case-insensitive) into a fresh Array<Char> of the exact original bytes. Strict: an odd-length input or a non-hex-digit character is a named error.")
    (cons 'EXAMPLES '(((array->list (hex:decode "4142")) (65 66))))
    (cons 'SEE-ALSO '(hex:encode base64:decode))))

;;; ============================================================
;;; URL MODULE: percent-encoding, URL, and query-string parse/build
;;; (issue #257, epic #253)
;;; ============================================================

(register-doc 'url:encode-path-segment
  (list
    (cons 'NAME 'url:encode-path-segment)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(url:encode-path-segment s)")
    (cons 'CATEGORY 'url)
    (cons 'DESCRIPTION "Percent-encodes s for use as one URL path segment: unreserved characters plus sub-delims and \":\"/\"@\" stay literal; every other byte (including \"/\") is percent-encoded.")
    (cons 'EXAMPLES '(((url:encode-path-segment "a b") "a%20b")))
    (cons 'SEE-ALSO '(url:encode-query-component url:decode))))

(register-doc 'url:encode-query-component
  (list
    (cons 'NAME 'url:encode-query-component)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(url:encode-query-component s)")
    (cons 'CATEGORY 'url)
    (cons 'DESCRIPTION "Percent-encodes s for use as a query-string key or value: only unreserved characters stay literal; everything else (including \"&\"/\"=\"/\"+\") is percent-encoded.")
    (cons 'EXAMPLES '(((url:encode-query-component "a&b") "a%26b")))
    (cons 'SEE-ALSO '(url:encode-path-segment url:decode url:build-query))))

(register-doc 'url:decode
  (list
    (cons 'NAME 'url:decode)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(url:decode s &key (lossy nil))")
    (cons 'CATEGORY 'url)
    (cons 'DESCRIPTION "Percent-decodes s (produced by either encoder — decoding is context-free) back into the original Unicode STRING. Malformed \"%XX\" escapes are always errors; invalid UTF-8 after decoding is a strict error unless :lossy is T (U+FFFD substitution).")
    (cons 'EXAMPLES '(((url:decode "a%20b") "a b")))
    (cons 'SEE-ALSO '(url:encode-path-segment url:encode-query-component url:decode-path-segment url:decode-query-component))))

(register-doc 'url:decode-path-segment
  (list
    (cons 'NAME 'url:decode-path-segment)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(url:decode-path-segment s &key (lossy nil))")
    (cons 'CATEGORY 'url)
    (cons 'DESCRIPTION "Alias for url:decode: percent-decoding is context-free, so this is identical to url:decode-query-component; provided so url:encode-path-segment has a same-named inverse.")
    (cons 'SEE-ALSO '(url:decode url:encode-path-segment))))

(register-doc 'url:decode-query-component
  (list
    (cons 'NAME 'url:decode-query-component)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(url:decode-query-component s &key (lossy nil))")
    (cons 'CATEGORY 'url)
    (cons 'DESCRIPTION "Alias for url:decode; see url:decode-path-segment.")
    (cons 'SEE-ALSO '(url:decode url:encode-query-component))))

(register-doc 'url:parse-query
  (list
    (cons 'NAME 'url:parse-query)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(url:parse-query s)")
    (cons 'CATEGORY 'url)
    (cons 'DESCRIPTION "Parses query string s (without a leading \"?\") into a list of (key . value) conses, decoded via url:decode, in the string's original order. Repeated keys are preserved as repeated conses, never collapsed.")
    (cons 'EXAMPLES '(((url:parse-query "a=1&b=2") (("a" . "1") ("b" . "2")))))
    (cons 'SEE-ALSO '(url:build-query url:decode))))

(register-doc 'url:build-query
  (list
    (cons 'NAME 'url:build-query)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(url:build-query pairs)")
    (cons 'CATEGORY 'url)
    (cons 'DESCRIPTION "Builds a query string (without a leading \"?\") from pairs, a list of (key . value) conses, in the given order — the inverse of url:parse-query. Each key/value is percent-encoded via url:encode-query-component.")
    (cons 'SEE-ALSO '(url:parse-query url:encode-query-component))))

(register-doc 'url:parse
  (list
    (cons 'NAME 'url:parse)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(url:parse s)")
    (cons 'CATEGORY 'url)
    (cons 'DESCRIPTION "Parses URL string s into an alist with keys SCHEME, USERINFO, HOST, PORT, PATH, QUERY, FRAGMENT. All are NIL when absent except PATH (always a string). PATH/QUERY/FRAGMENT/USERINFO are raw — still percent-encoded exactly as they appeared, never auto-decoded. No regular expressions are used.")
    (cons 'SEE-ALSO '(url:build url:scheme url:host url:port url:path url:query url:fragment url:userinfo))))

(register-doc 'url:build
  (list
    (cons 'NAME 'url:build)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(url:build u)")
    (cons 'CATEGORY 'url)
    (cons 'DESCRIPTION "Builds a URL string from an alist u shaped like url:parse's result — the inverse of url:parse.")
    (cons 'EXAMPLES '(((url:build (url:parse "https://example.com/a?x=1")) "https://example.com/a?x=1")))
    (cons 'SEE-ALSO '(url:parse))))

(register-doc 'url:scheme
  (list (cons 'NAME 'url:scheme) (cons 'TYPE 'function) (cons 'SYNTAX "(url:scheme u)")
        (cons 'CATEGORY 'url) (cons 'DESCRIPTION "The SCHEME field of a url:parse alist, or NIL.")
        (cons 'SEE-ALSO '(url:parse))))

(register-doc 'url:userinfo
  (list (cons 'NAME 'url:userinfo) (cons 'TYPE 'function) (cons 'SYNTAX "(url:userinfo u)")
        (cons 'CATEGORY 'url) (cons 'DESCRIPTION "The USERINFO field of a url:parse alist, or NIL.")
        (cons 'SEE-ALSO '(url:parse))))

(register-doc 'url:host
  (list (cons 'NAME 'url:host) (cons 'TYPE 'function) (cons 'SYNTAX "(url:host u)")
        (cons 'CATEGORY 'url) (cons 'DESCRIPTION "The HOST field of a url:parse alist (a bracketed IPv6 literal is kept as one unit), or NIL.")
        (cons 'SEE-ALSO '(url:parse))))

(register-doc 'url:port
  (list (cons 'NAME 'url:port) (cons 'TYPE 'function) (cons 'SYNTAX "(url:port u)")
        (cons 'CATEGORY 'url) (cons 'DESCRIPTION "The PORT field of a url:parse alist (a Number), or NIL.")
        (cons 'SEE-ALSO '(url:parse))))

(register-doc 'url:path
  (list (cons 'NAME 'url:path) (cons 'TYPE 'function) (cons 'SYNTAX "(url:path u)")
        (cons 'CATEGORY 'url) (cons 'DESCRIPTION "The PATH field of a url:parse alist (always a String, raw/still-encoded, possibly \"\").")
        (cons 'SEE-ALSO '(url:parse))))

(register-doc 'url:query
  (list (cons 'NAME 'url:query) (cons 'TYPE 'function) (cons 'SYNTAX "(url:query u)")
        (cons 'CATEGORY 'url) (cons 'DESCRIPTION "The QUERY field of a url:parse alist (raw text after \"?\", no leading delimiter), or NIL.")
        (cons 'SEE-ALSO '(url:parse url:parse-query))))

(register-doc 'url:fragment
  (list (cons 'NAME 'url:fragment) (cons 'TYPE 'function) (cons 'SYNTAX "(url:fragment u)")
        (cons 'CATEGORY 'url) (cons 'DESCRIPTION "The FRAGMENT field of a url:parse alist (raw text after \"#\", no leading delimiter), or NIL.")
        (cons 'SEE-ALSO '(url:parse))))

;;; ============================================================
;;; JSON MODULE: JSON parse/stringify (issue #257, epic #253)
;;; ============================================================

(register-doc 'json:parse
  (list
    (cons 'NAME 'json:parse)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(json:parse s &key (max-depth 512) (on-integer-overflow ':error))")
    (cons 'CATEGORY 'json)
    (cons 'DESCRIPTION "Parses JSON text s into a Lamedh value: object -> hash table (String keys, last-key-wins), array -> Array (not a list), string -> String, true -> T, false -> NIL, null -> the keyword :NULL (never NIL). Integer literals in i64 range are exact Numbers; out-of-range literals error unless :on-integer-overflow is :float. Every other number is a Float. Strict: rejects trailing garbage, unescaped control characters, leading zeros, and unpaired \\u surrogate escapes, with line/column-located errors. :max-depth bounds nesting so deep input is a clean error, not a stack overflow.")
    (cons 'EXAMPLES '(((array->list (json:parse "[1,2,3]")) (1 2 3))
                       ((json:parse "null") :NULL)))
    (cons 'SEE-ALSO '(json:stringify json:null-p))))

(register-doc 'json:stringify
  (list
    (cons 'NAME 'json:stringify)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(json:stringify v &key (pretty nil) (indent 2))")
    (cons 'CATEGORY 'json)
    (cons 'DESCRIPTION "Serializes Lamedh value v to a JSON text String — the exact inverse of json:parse's mapping. :pretty (default NIL) produces multi-line, :indent-space-per-level indented output; compact output otherwise. A Float is always written with a \".\" so it round-trips back as a Float, never an integer. Signals an error for a NaN/infinite Float or a value outside the mapping.")
    (cons 'EXAMPLES '(((json:stringify (list->array (list 1 2))) "[1,2]")))
    (cons 'SEE-ALSO '(json:parse))))

(register-doc 'json:null-p
  (list
    (cons 'NAME 'json:null-p)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(json:null-p v)")
    (cons 'CATEGORY 'json)
    (cons 'DESCRIPTION "T if v is the JSON null marker :NULL that json:parse produces for a JSON null literal (never NIL, so it is distinguishable from false and from an empty array).")
    (cons 'EXAMPLES '(((json:null-p (json:parse "null")) T)
                       ((json:null-p (json:parse "false")) ())))
    (cons 'SEE-ALSO '(json:parse))))

;;; ============================================================
;;; MIME MODULE: headers and Content-Type (issue #257, epic #253)
;;; ============================================================

(register-doc 'mime:header-name=
  (list
    (cons 'NAME 'mime:header-name=)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(mime:header-name= a b)")
    (cons 'CATEGORY 'mime)
    (cons 'DESCRIPTION "Case-insensitive header-name equality (Unicode default case fold; agrees with ASCII case-insensitive comparison for HTTP header names).")
    (cons 'SEE-ALSO '(mime:headers-get))))

(register-doc 'mime:headers-get
  (list
    (cons 'NAME 'mime:headers-get)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(mime:headers-get headers name)")
    (cons 'CATEGORY 'mime)
    (cons 'DESCRIPTION "The value of the first header in headers (a list of (name . value) conses) whose name matches name case-insensitively, or NIL.")
    (cons 'SEE-ALSO '(mime:headers-get-all mime:headers-add))))

(register-doc 'mime:headers-get-all
  (list
    (cons 'NAME 'mime:headers-get-all)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(mime:headers-get-all headers name)")
    (cons 'CATEGORY 'mime)
    (cons 'DESCRIPTION "Every value in headers whose name matches name case-insensitively, in original order — the multi-value accessor (e.g. every Set-Cookie value; never collapsed into one).")
    (cons 'SEE-ALSO '(mime:headers-get mime:headers-add))))

(register-doc 'mime:headers-add
  (list
    (cons 'NAME 'mime:headers-add)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(mime:headers-add headers name value)")
    (cons 'CATEGORY 'mime)
    (cons 'DESCRIPTION "Returns a fresh headers list with (name . value) appended after headers. Never removes or collapses an existing entry of the same name — use for multi-value headers like Set-Cookie.")
    (cons 'SEE-ALSO '(mime:headers-set mime:headers-get-all))))

(register-doc 'mime:headers-set
  (list
    (cons 'NAME 'mime:headers-set)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(mime:headers-set headers name value)")
    (cons 'CATEGORY 'mime)
    (cons 'DESCRIPTION "Returns a fresh headers list with every existing entry matching name (case-insensitive) removed and (name . value) appended once. Use only for headers that must be singular (e.g. Content-Type).")
    (cons 'SEE-ALSO '(mime:headers-add mime:headers-remove))))

(register-doc 'mime:headers-remove
  (list
    (cons 'NAME 'mime:headers-remove)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(mime:headers-remove headers name)")
    (cons 'CATEGORY 'mime)
    (cons 'DESCRIPTION "Returns a fresh headers list with every entry matching name (case-insensitive) removed.")
    (cons 'SEE-ALSO '(mime:headers-set))))

(register-doc 'mime:headers-names
  (list
    (cons 'NAME 'mime:headers-names)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(mime:headers-names headers)")
    (cons 'CATEGORY 'mime)
    (cons 'DESCRIPTION "The distinct header names in headers, each spelled the way it was first given, in first-seen order.")
    (cons 'SEE-ALSO '(mime:headers-get))))

(register-doc 'mime:parse-content-type
  (list
    (cons 'NAME 'mime:parse-content-type)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(mime:parse-content-type s)")
    (cons 'CATEGORY 'mime)
    (cons 'DESCRIPTION "Parses a Content-Type header value s into an alist (TYPE . type-string) (SUBTYPE . subtype-string) (PARAMETERS . ((name . value)...)), parameters in order with quoted-string values already unescaped.")
    (cons 'EXAMPLES '(((cdr (assoc 'type (mime:parse-content-type "text/html"))) "text")))
    (cons 'SEE-ALSO '(mime:build-content-type mime:content-type-parameter))))

(register-doc 'mime:build-content-type
  (list
    (cons 'NAME 'mime:build-content-type)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(mime:build-content-type type subtype &optional parameters)")
    (cons 'CATEGORY 'mime)
    (cons 'DESCRIPTION "Builds a Content-Type header value from type, subtype, and an optional PARAMETERS list of (name . value) conses. A parameter value is written as a bare token when possible, else a quoted-string with \"\\\" and '\"' escaped.")
    (cons 'SEE-ALSO '(mime:parse-content-type))))

(register-doc 'mime:content-type-parameter
  (list
    (cons 'NAME 'mime:content-type-parameter)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(mime:content-type-parameter ct name)")
    (cons 'CATEGORY 'mime)
    (cons 'DESCRIPTION "Case-insensitive lookup of parameter name's value in ct (as returned by mime:parse-content-type), or NIL if absent.")
    (cons 'SEE-ALSO '(mime:parse-content-type))))

;;; ============================================================
;;; ADDITIONAL LIST OPERATIONS
;;; ============================================================

(register-doc 'rplaca
  (list
    (cons 'NAME 'rplaca)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(rplaca cons new-car)")
    (cons 'CATEGORY 'lists)
    (cons 'DESCRIPTION "Destructively replaces the CAR of a cons cell with new-car. Returns the modified cons cell. This is a mutating operation — use with care as it modifies shared structure. Classic Lisp 1.5 primitive.")
    (cons 'EXAMPLES '(((let ((x (cons 1 2))) (rplaca x 99) x) (99 . 2))))
    (cons 'SEE-ALSO '(rplacd car cons))))

(register-doc 'rplacd
  (list
    (cons 'NAME 'rplacd)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(rplacd cons new-cdr)")
    (cons 'CATEGORY 'lists)
    (cons 'DESCRIPTION "Destructively replaces the CDR of a cons cell with new-cdr. Returns the modified cons cell. This is a mutating operation — use with care as it can create circular structure. Classic Lisp 1.5 primitive.")
    (cons 'EXAMPLES '(((let ((x (cons 1 2))) (rplacd x 99) x) (1 . 99))))
    (cons 'SEE-ALSO '(rplaca cdr cons))))

(register-doc 'sublis
  (list
    (cons 'NAME 'sublis)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(sublis alist tree)")
    (cons 'CATEGORY 'lists)
    (cons 'DESCRIPTION "Substitutes values from an association list into a tree. For each leaf in tree that matches a key in alist (by EQUAL), replaces it with the corresponding value. Returns a new tree; does not modify the original. Classic Lisp 1.5 primitive.")
    (cons 'EXAMPLES '(((sublis '((a . 1) (b . 2)) '(a b c)) (1 2 c))))
    (cons 'SEE-ALSO '(subst assoc))))

(register-doc 'nthcdr
  (list
    (cons 'NAME 'nthcdr)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(nthcdr n list)")
    (cons 'CATEGORY 'lists)
    (cons 'DESCRIPTION "Returns the list after n applications of CDR. (nthcdr 0 list) returns list unchanged; (nthcdr 1 list) is CDR. Returns NIL if n exceeds the list length.")
    (cons 'EXAMPLES '(((nthcdr 2 '(a b c d)) (c d))
                       ((nthcdr 0 '(a b)) (a b))))
    (cons 'SEE-ALSO '(nth cdr))))

(register-doc 'efface
  (list
    (cons 'NAME 'efface)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(efface item list)")
    (cons 'CATEGORY 'lists)
    (cons 'DESCRIPTION "Returns a new list with the first occurrence of item (tested by EQUAL) removed. If item does not appear, returns the list unchanged. DELETE is an alias.")
    (cons 'EXAMPLES '(((efface 'b '(a b c b)) (a c b))
                       ((efface 'x '(a b c)) (a b c))))
    (cons 'SEE-ALSO '(delete member subst))))

;;; ============================================================
;;; ADDITIONAL I/O
;;; ============================================================

(register-doc 'spaces
  (list
    (cons 'NAME 'spaces)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(spaces n)")
    (cons 'CATEGORY 'io)
    (cons 'DESCRIPTION "Prints n space characters to standard output without a trailing newline. Lisp 1.5 I/O primitive for column-aligned output.")
    (cons 'EXAMPLES '(((spaces 3) "   ")))
    (cons 'SEE-ALSO '(terpri print princ))))

;;; ============================================================
;;; PROPERTY LIST OPERATIONS
;;; ============================================================

(register-doc 'remprop
  (list
    (cons 'NAME 'remprop)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(remprop symbol indicator)")
    (cons 'CATEGORY 'plists)
    (cons 'DESCRIPTION "Removes the property named indicator from symbol's property list. Returns T if the property was present and removed; returns NIL if it was not found. The indicator may be a symbol or string.")
    (cons 'EXAMPLES '(((putp 'x 'color 'red) red)
                       ((remprop 'x 'color) t)))
    (cons 'SEE-ALSO '(putp getp plist deflist))))

(register-doc 'deflist
  (list
    (cons 'NAME 'deflist)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(deflist pairs indicator)")
    (cons 'CATEGORY 'plists)
    (cons 'DESCRIPTION "Bulk property setter: for each pair (symbol value) in pairs, sets the property named indicator on symbol to value. A compact Lisp 1.5 idiom for initializing a property across many symbols at once.")
    (cons 'EXAMPLES '(((deflist '((x 1) (y 2) (z 3)) 'index) t)))
    (cons 'SEE-ALSO '(putp getp plist remprop))))

;;; ============================================================
;;; HASH TABLE OPERATIONS (EXTENDED)
;;; ============================================================

(register-doc 'gethash
  (list
    (cons 'NAME 'gethash)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(gethash hash-table key)")
    (cons 'CATEGORY 'hash-tables)
    (cons 'DESCRIPTION "Retrieves the value associated with key in hash-table. Returns NIL if the key is not present. Keys are compared by structural equality (like EQUAL). Use GET for property list lookup.")
    (cons 'EXAMPLES '(((let ((h (make-hash-table))) (set-bang h 'x 42) (gethash h 'x)) 42)))
    (cons 'SEE-ALSO '(set-bang sethash keys delete-key delete-key-bang make-hash-table get))))

(register-doc 'delete-key
  (list
    (cons 'NAME 'delete-key)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(delete-key hash-table key)")
    (cons 'CATEGORY 'hash-tables)
    (cons 'DESCRIPTION "Compatibility alias for DELETE-KEY-BANG. Destructively removes key and its associated value from hash-table. Returns T regardless of whether the key was present.")
    (cons 'EXAMPLES '(((let ((h (make-hash-table))) (set-bang h 'x 1) (delete-key h 'x) (gethash h 'x)) nil)))
    (cons 'SEE-ALSO '(delete-key-bang set-bang gethash keys make-hash-table))))

(register-doc 'delete-key-bang
  (list
    (cons 'NAME 'delete-key-bang)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(delete-key-bang hash-table key)")
    (cons 'CATEGORY 'hash-tables)
    (cons 'DESCRIPTION "Destructively removes key and its associated value from hash-table. Returns T regardless of whether the key was present. The bang suffix signals mutation in place.")
    (cons 'EXAMPLES '(((let ((h (make-hash-table))) (set-bang h 'x 1) (delete-key-bang h 'x) (gethash h 'x)) nil)))
    (cons 'SEE-ALSO '(delete-key set-bang gethash keys make-hash-table))))

;;; ============================================================
;;; ARRAYS (LISP 1.5 APPENDIX A)
;;; ============================================================

(register-doc 'array
  (list
    (cons 'NAME 'array)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(array n)")
    (cons 'CATEGORY 'arrays)
    (cons 'DESCRIPTION "Creates and returns a new mutable array of n elements, all initialised to NIL. Lisp 1.5 Appendix A name; MAKE-ARRAY is the longer alias. Arrays are random-access containers with O(1) indexed get/set. Use FETCH/STORE to access elements, ARRAY-LENGTH* to query the size.")
    (cons 'EXAMPLES '(((let ((a (array 3))) (store a 0 'x) (fetch a 0)) x)))
    (cons 'SEE-ALSO '(make-array fetch store array-length* arrayp))))

(register-doc 'make-array
  (list
    (cons 'NAME 'make-array)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(make-array n)")
    (cons 'CATEGORY 'arrays)
    (cons 'DESCRIPTION "Alias for ARRAY. Creates a mutable array of n NIL-initialised elements. See ARRAY for full documentation.")
    (cons 'EXAMPLES '(((make-array 5) "an array of 5 NILs")))
    (cons 'SEE-ALSO '(array fetch store array-length* arrayp))))

(register-doc 'fetch
  (list
    (cons 'NAME 'fetch)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(fetch array index)")
    (cons 'CATEGORY 'arrays)
    (cons 'DESCRIPTION "Returns the element of array at 0-based integer index. Signals an error if index is out of bounds. Lisp 1.5 Appendix A name; ARRAY-FETCH* is the longer alias, AREF the Common-Lisp-style one.")
    (cons 'EXAMPLES '(((let ((a (array 3))) (store a 1 'hello) (fetch a 1)) hello)))
    (cons 'SEE-ALSO '(array-fetch* aref store array array-length*))))

(register-doc 'array-fetch*
  (list
    (cons 'NAME 'array-fetch*)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(array-fetch* array index)")
    (cons 'CATEGORY 'arrays)
    (cons 'DESCRIPTION "Alias for FETCH. Returns the element of array at 0-based index. See FETCH for full documentation.")
    (cons 'SEE-ALSO '(fetch store array-store* array-length*))))

(register-doc 'aref
  (list
    (cons 'NAME 'aref)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(aref array index)")
    (cons 'CATEGORY 'arrays)
    (cons 'DESCRIPTION "Common-Lisp-style alias for FETCH. Returns the element of array at 0-based index. See FETCH for full documentation.")
    (cons 'SEE-ALSO '(fetch array-fetch* aset store array-length*))))

(register-doc 'store
  (list
    (cons 'NAME 'store)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(store array index value)")
    (cons 'CATEGORY 'arrays)
    (cons 'DESCRIPTION "Destructively sets the element of array at 0-based index to value. Returns the stored value. Signals an error if index is out of bounds. Lisp 1.5 Appendix A name; ARRAY-STORE* is the longer alias, ASET the Common-Lisp-style one. Mutation is in-place: all references to the same array see the change, including inside a defun-typed body (issue #216). Two scoped exceptions: an array nested inside another array or a struct does not write back through the outer object (only top-level flat arrays of scalars do); and passing the same array as two distinct arguments to one defun-typed call is last-writer-wins in argument order, not simultaneous true aliasing.")
    (cons 'EXAMPLES '(((let ((a (array 3))) (store a 0 99) (fetch a 0)) 99)))
    (cons 'SEE-ALSO '(array-store* aset fetch array array-length*))))

(register-doc 'array-store*
  (list
    (cons 'NAME 'array-store*)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(array-store* array index value)")
    (cons 'CATEGORY 'arrays)
    (cons 'DESCRIPTION "Alias for STORE. Destructively sets the element at index. See STORE for full documentation.")
    (cons 'SEE-ALSO '(store fetch array-fetch* array-length*))))

(register-doc 'aset
  (list
    (cons 'NAME 'aset)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(aset array index value)")
    (cons 'CATEGORY 'arrays)
    (cons 'DESCRIPTION "Common-Lisp-style alias for STORE. Destructively sets the element at index. See STORE for full documentation.")
    (cons 'SEE-ALSO '(store array-store* aref fetch array-length*))))

(register-doc 'array-length*
  (list
    (cons 'NAME 'array-length*)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(array-length* array)")
    (cons 'CATEGORY 'arrays)
    (cons 'DESCRIPTION "Returns the number of elements in array as an integer. The valid index range is 0 to (array-length* array) - 1.")
    (cons 'EXAMPLES '(((array-length* (array 5)) 5)
                       ((array-length* (array 0)) 0)))
    (cons 'SEE-ALSO '(array fetch store arrayp))))

(register-doc 'arrayp
  (list
    (cons 'NAME 'arrayp)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(arrayp x)")
    (cons 'CATEGORY 'arrays)
    (cons 'DESCRIPTION "Returns T if x is an array (created with ARRAY or MAKE-ARRAY); returns NIL otherwise. DEFSTRUCT instances are also arrays internally.")
    (cons 'EXAMPLES '(((arrayp (array 3)) t)
                       ((arrayp '(1 2 3)) nil)))
    (cons 'SEE-ALSO '(array array-length* extension-p))))

;;; ============================================================
;;; TYPE PREDICATES (EXTENDED)
;;; ============================================================

(register-doc 'extension-p
  (list
    (cons 'NAME 'extension-p)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(extension-p x)")
    (cons 'CATEGORY 'predicates)
    (cons 'DESCRIPTION "Returns T if x is an opaque extension value — a host-language object that was injected into the Lisp environment from Rust via the embedder API. Extension values have no direct Lisp representation but carry a type name accessible via EXTENSION-TYPE.")
    (cons 'SEE-ALSO '(extension-type arrayp functionp))))

(register-doc 'extension-type
  (list
    (cons 'NAME 'extension-type)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(extension-type x)")
    (cons 'CATEGORY 'predicates)
    (cons 'DESCRIPTION "Returns the type name string of extension value x (e.g. \"MyRustType\"). Signals an error if x is not an extension value. Use EXTENSION-P first to check. Useful for dispatching on host-provided objects.")
    (cons 'SEE-ALSO '(extension-p))))

;;; ============================================================
;;; SORTING
;;; ============================================================

(register-doc 'sort
  (list
    (cons 'NAME 'sort)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(sort list comparator)")
    (cons 'CATEGORY 'lists)
    (cons 'DESCRIPTION "Returns a new list with the same elements as list, sorted according to comparator. The comparator must be a two-argument predicate that returns T (or non-NIL) when its first argument should come before its second — i.e. a strict less-than. The sort is stable. Does not modify the original list.")
    (cons 'EXAMPLES '(((sort '(3 1 4 1 5 9 2 6) '<) (1 1 2 3 4 5 6 9))
                       ((sort '("banana" "apple" "cherry") 'string<) ("apple" "banana" "cherry"))))
    (cons 'SEE-ALSO '(mapcar filter reverse))))

;;; ============================================================
;;; FIRST-CLASS ERROR/CONDITION VALUES
;;; ============================================================

(register-doc 'make-error
  (list
    (cons 'NAME 'make-error)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(make-error message) or (make-error message data)")
    (cons 'CATEGORY 'errors)
    (cons 'DESCRIPTION "Creates an error condition value with the given message string and optional data (any Lisp value). Error values are first-class: they can be stored, passed around, and inspected without being signalled. Use ERROR to signal an error that terminates the current computation. Use HANDLER-CASE or ERRORSET to catch signalled errors.")
    (cons 'EXAMPLES '(((let ((e (make-error "oops"))) (error-message e)) "oops")
                       ((let ((e (make-error "oops" '(1 2)))) (error-data e)) (1 2))))
    (cons 'SEE-ALSO '(error error-p error-message error-data errorset))))

(register-doc 'error-p
  (list
    (cons 'NAME 'error-p)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(error-p x)")
    (cons 'CATEGORY 'errors)
    (cons 'DESCRIPTION "Returns T if x is an error condition value (created with MAKE-ERROR or captured by ERRORSET). Returns NIL for any other value including ordinary NIL. Useful for dispatching on values that might be errors.")
    (cons 'EXAMPLES '(((error-p (make-error "oops")) t)
                       ((error-p 42) nil)))
    (cons 'SEE-ALSO '(make-error error-message error-data errorset))))

(register-doc 'error-message
  (list
    (cons 'NAME 'error-message)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(error-message error-val)")
    (cons 'CATEGORY 'errors)
    (cons 'DESCRIPTION "Extracts the message string from an error condition value. Signals an error if the argument is not an error value. Use ERROR-P to test first.")
    (cons 'EXAMPLES '(((error-message (make-error "bad thing")) "bad thing")))
    (cons 'SEE-ALSO '(error-p error-data make-error))))

(register-doc 'error-data
  (list
    (cons 'NAME 'error-data)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(error-data error-val)")
    (cons 'CATEGORY 'errors)
    (cons 'DESCRIPTION "Extracts the associated data from an error condition value. Returns NIL if no data was attached (i.e. MAKE-ERROR was called with only a message). Signals an error if the argument is not an error value.")
    (cons 'EXAMPLES '(((error-data (make-error "x" '(a b c))) (a b c))
                       ((error-data (make-error "x")) nil)))
    (cons 'SEE-ALSO '(error-p error-message make-error))))

;;; ============================================================
;;; METAPROGRAMMING (EXTENDED)
;;; ============================================================

(register-doc 'evlis
  (list
    (cons 'NAME 'evlis)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(evlis list) or (evlis list environment)")
    (cons 'CATEGORY 'meta)
    (cons 'DESCRIPTION "Evaluates each element of list in order and returns a new list of results. With one argument, uses the current environment. With two arguments, evaluates in the given environment object. This is the classic Lisp 1.5 primitive for evaluating argument lists; it is exposed for metaprogramming — most code uses MAPCAR or ordinary function calls instead.")
    (cons 'EXAMPLES '(((evlis '((+ 1 2) (* 3 4))) (3 12))))
    (cons 'SEE-ALSO '(eval evcon apply mapcar the-environment))))

(register-doc 'evcon
  (list
    (cons 'NAME 'evcon)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(evcon clauses) or (evcon clauses environment)")
    (cons 'CATEGORY 'meta)
    (cons 'DESCRIPTION "Classic Lisp 1.5 evaluator for COND-style clauses. Walks the list of (test value) pairs, evaluates each test in turn, and returns the evaluated value of the first clause whose test is non-NIL. Returns NIL if no test passes. With two arguments, evaluates in the given environment object. Exposed for metaprogramming; prefer COND in ordinary code.")
    (cons 'EXAMPLES '(((evcon '(((= 1 2) "no") ((= 1 1) "yes"))) "yes")))
    (cons 'SEE-ALSO '(cond eval evlis the-environment))))

(register-doc 'optimize
  (list
    (cons 'NAME 'optimize)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(optimize form)")
    (cons 'CATEGORY 'meta)
    (cons 'DESCRIPTION "Runs the source-level optimizer on form and returns the optimized Lisp expression without evaluating it. The optimizer performs constant folding, dead binding elimination, and other algebraic simplifications. The result is a structurally equivalent but potentially faster form. Used by the REPL and compiler pipeline; also useful for inspecting optimizer output during development.")
    (cons 'EXAMPLES '(((optimize '(+ 1 2)) 3)
                       ((optimize '(let ((x 1)) x)) 1)))
    (cons 'SEE-ALSO '(eval macroexpand defun-typed-opt))))

(register-doc 'defun-typed-opt
  (list
    (cons 'NAME 'defun-typed-opt)
    (cons 'TYPE 'vau)
    (cons 'SYNTAX "(defun-typed-opt (name return-type) ((arg type) ...) body...)")
    (cons 'CATEGORY 'meta)
    (cons 'DESCRIPTION "Optimizer-to-compiler bridge for typed functions. Receives a DEFUN-TYPED-shaped definition as source, runs the Lisp/vau source optimizer over it, then evaluates the optimized DEFUN-TYPED form so the normal HM checker and native compiler install the typed edition. Use this when you want explicit source optimization before typed compilation without making every DEFUN-TYPED globally auto-optimized.")
    (cons 'EXAMPLES '(((defun-typed-opt (inc int64) ((x int64)) (+ x 0)) inc)))
    (cons 'SEE-ALSO '(optimize defun-typed check-type disassemble))))

;;; ============================================================
;;; SPECIAL FORMS: FEXPR AND VAU OPERATIVES
;;; ============================================================
;;;
;;; Background: In most modern Lisps a function's arguments are evaluated
;;; before being passed to it (applicative order).  Two mechanisms let you
;;; receive *unevaluated* arguments instead:
;;;
;;;   DEFEXPR / anonymous FUNCTION (fexpr) — classical Lisp 1.5 style.
;;;   VAU / $VAU (operative) — John Shutt's Kernel-language style.
;;;
;;; Both are unusual in contemporary practice but were central to early
;;; Lisp design and remain powerful tools for building new control structures
;;; without macros.

(register-doc 'defexpr
  (list
    (cons 'NAME 'defexpr)
    (cons 'TYPE 'special-form)
    (cons 'SYNTAX "(defexpr name (param...) [docstring] body)")
    (cons 'CATEGORY 'special-forms)
    (cons 'DESCRIPTION
      "Defines a named FEXPR (\"functional expression\") — a function-like object that receives its arguments UNEVALUATED as raw list structure instead of as computed values.

A fexpr is the classic Lisp 1.5 mechanism for user-defined special forms. When a fexpr is called the evaluator does NOT evaluate the operands before passing them in; the body of the fexpr receives the literal source forms and may choose to evaluate them (with EVAL), ignore them, or inspect/transform them.

With a single parameter the entire unevaluated operand list is bound to that parameter as a Lisp list:
  (defexpr my-and (args) (cond ((null args) t) ((null (cdr args)) (eval (car args))) ...))
  (my-and (< x 5) (> x 0))  ; args = ((< x 5) (> x 0)) -- not evaluated yet

With multiple parameters each unevaluated operand is bound to the corresponding parameter individually.

Fexprs are powerful but compose poorly: because the evaluator cannot see past a fexpr call, optimisations and macro-expanders that need to walk the code tree are blocked.  Modern usage (post-1970s) generally prefers DEFMACRO for compile-time code transformation and LAMBDA for runtime abstraction.  Use fexprs when you genuinely need access to both the unevaluated source and the current environment at call time — for example, to implement a custom binding form or a quoting operator.

See also VAU/$VAU for the Kernel-language operative, which makes the caller's environment explicit.")
    (cons 'EXAMPLES
      '(((defexpr my-quote (x) (car x))
         (my-quote foo))
        ((defexpr verbose-if (test then else)
           (if (eval test) (eval then) (eval else)))
         (verbose-if (> 3 2) (print "yes") (print "no")))))
    (cons 'SEE-ALSO '(vau defmacro lambda funcall eval))))

(register-doc 'vau
  (list
    (cons 'NAME 'vau)
    (cons 'TYPE 'special-form)
    (cons 'SYNTAX "(vau (operands-param env-param) body...)")
    (cons 'CATEGORY 'special-forms)
    (cons 'DESCRIPTION
      "Creates an anonymous VAU operative (also written $VAU following Kernel convention).  A vau operative is similar to a fexpr — it receives arguments UNEVALUATED — but it also receives the CALLER'S ENVIRONMENT as an explicit first-class value, giving the operative complete reflective access.

The parameter list must contain exactly two symbols:
  operands-param — bound to the unevaluated operand list (a Lisp list of the literal source forms)
  env-param      — bound to the caller's environment as a first-class Environment object

Inside the body you can call (eval form env-param) to evaluate any form in the caller's scope, inspect bindings via environment operations, or build derived control structures.

VAU operatives originate in John Shutt's Kernel language (dissertation, 2010).  The key insight is that the combination of (1) receiving operands unevaluated and (2) having the caller's environment as an explicit object is strictly more general than either macros or fexprs alone.  From VAU you can *derive* both LAMBDA (wrap in an evaluating shell) and DEFMACRO (evaluate operands, produce code, evaluate result in caller's env).  This makes VAU the minimal kernel for a reflective Lisp.

Unlike DEFEXPR fexprs, vau operatives do not capture a dynamic parent environment for argument evaluation — the caller's environment is passed explicitly, making the data flow transparent to analysis tools.

In Lamedh the $VAU alias is also recognised (the dollar sign is idiomatic Kernel notation for operatives that receive unevaluated operands).")
    (cons 'EXAMPLES
      '(((def $my-if
            ($vau (test then else) e
              (if (eval test e) (eval then e) (eval else e))))
         ($my-if (> 3 2) 'yes 'no))
        ((def $seq
            ($vau (forms) e
              (if (null forms) nil
                  (if (null (cdr forms)) (eval (car forms) e)
                      (progn (eval (car forms) e)
                             (eval (cons '$seq (cdr forms)) e))))))
         ($seq (print "a") (print "b")))))
    (cons 'SEE-ALSO '(defexpr defmacro lambda eval the-environment make-environment))))

;;; ============================================================
;;; CONDITION FLAGS
;;; ============================================================

(register-doc 'set-flag
  (list
    (cons 'NAME 'set-flag)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(set-flag name)")
    (cons 'CATEGORY 'flags)
    (cons 'DESCRIPTION "Sets the global condition flag named name (a symbol or string) to true. Condition flags are global boolean signals used to communicate exceptional conditions such as arithmetic overflow. The built-in flag \"OVERFLOW\" is set by some arithmetic operations when overflow is detected. Custom flags can be set and tested by application code.")
    (cons 'EXAMPLES '(((set-flag 'done) t)
                       ((flag-set-p 'done) t)))
    (cons 'SEE-ALSO '(clear-flag flag-set-p clear-all-flags))))

(register-doc 'clear-flag
  (list
    (cons 'NAME 'clear-flag)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(clear-flag name)")
    (cons 'CATEGORY 'flags)
    (cons 'DESCRIPTION "Clears the global condition flag named name (a symbol or string), setting it to false. Has no effect if the flag was not set. See SET-FLAG for an overview of condition flags.")
    (cons 'EXAMPLES '(((set-flag 'x) t)
                       ((clear-flag 'x) t)
                       ((flag-set-p 'x) nil)))
    (cons 'SEE-ALSO '(set-flag flag-set-p clear-all-flags))))

(register-doc 'flag-set-p
  (list
    (cons 'NAME 'flag-set-p)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(flag-set-p name)")
    (cons 'CATEGORY 'flags)
    (cons 'DESCRIPTION "Returns T if the global condition flag named name is currently set; returns NIL otherwise. The name may be a symbol or a string. Flags default to unset (false) until explicitly set with SET-FLAG.")
    (cons 'EXAMPLES '(((flag-set-p 'overflow) nil)
                       ((set-flag 'overflow) t)
                       ((flag-set-p 'overflow) t)))
    (cons 'SEE-ALSO '(set-flag clear-flag clear-all-flags))))

(register-doc 'clear-all-flags
  (list
    (cons 'NAME 'clear-all-flags)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(clear-all-flags)")
    (cons 'CATEGORY 'flags)
    (cons 'DESCRIPTION "Clears all global condition flags at once. Takes no arguments. Useful at the start of a test suite or computation to ensure a clean flag state.")
    (cons 'SEE-ALSO '(clear-flag set-flag flag-set-p))))

;;; ============================================================
;;; CAPABILITIES AND SHELL
;;; ============================================================

(register-doc 'feature-enabled-p
  (list
    (cons 'NAME 'feature-enabled-p)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(feature-enabled-p name)")
    (cons 'CATEGORY 'capabilities)
    (cons 'DESCRIPTION "Returns T if the capability (feature) named name is currently enabled; returns NIL otherwise. Capability names are case-insensitive. Available capabilities: SHELL (subprocess execution), READ-FS (filesystem reads), CREATE-FS (filesystem mutation), TEMP-FS (temp file creation), IO (stdin reads). All capabilities are OFF by default in every environment; they must be granted by the host via Rust API or the --capability CLI flag.")
    (cons 'EXAMPLES '(((feature-enabled-p 'shell) nil)
                       ((feature-enabled-p "READ-FS") nil)))
    (cons 'SEE-ALSO '(features shell read-file write-file))))

(register-doc 'features
  (list
    (cons 'NAME 'features)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(features)")
    (cons 'CATEGORY 'capabilities)
    (cons 'DESCRIPTION "Returns a sorted list of strings naming all currently-enabled capabilities. An empty list means no capabilities have been granted. Lisp code cannot grant capabilities to itself; this function is read-only introspection.")
    (cons 'EXAMPLES '(((features) nil)))
    (cons 'SEE-ALSO '(feature-enabled-p shell read-file))))

(register-doc 'shell
  (list
    (cons 'NAME 'shell)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(shell command) or (shell program arg...)")
    (cons 'CATEGORY 'capabilities)
    (cons 'DESCRIPTION "Runs a shell command and returns a list (exit-code stdout stderr) as three values. With a single string argument the command is passed to \"sh -c\"; with multiple arguments the first is the program and the rest are arguments passed directly (no shell expansion). Requires the SHELL capability to be enabled.

The return value is always a proper three-element list:
  (0)   exit code as an integer (-1 if the process exited without a code)
  (1)   stdout as a string
  (2)   stderr as a string

Use the helpers in lib/07-shell.lisp (SHELL-EXIT-CODE, SHELL-STDOUT, SHELL-STDERR, SHELL-OK-P, SH) for more ergonomic access to these values.

Grant the capability: --capability SHELL on the CLI, or (env.enable_feature \"SHELL\") from Rust host code.")
    (cons 'EXAMPLES '(((shell "echo hello") (0 "hello\n" ""))
                       ((shell "ls" "/tmp") (0 "..." ""))))
    (cons 'SEE-ALSO '(feature-enabled-p features))))

;;; ============================================================
;;; MODULE LOADING (REQUIRE / PROVIDE, issue #256)
;;; ============================================================

(register-doc 'require
  (list
    (cons 'NAME 'require)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(require 'name)")
    (cons 'CATEGORY 'modules)
    (cons 'DESCRIPTION "Loads module NAME (a symbol or string) at most once in this environment; returns NAME's canonical (uppercase) symbol. A second REQUIRE of an already-loaded module is a no-op -- it never re-evaluates the source. NAME resolves through a per-environment registry, in order: (1) sources a host registered directly (Rust: env.register_module); (2) sources embedded in the binary (the numbered optional library files -- SHELL, TESTING, CONDENSATION, TEXT, ...); (3) -- only under the READ-FS capability -- files under host-configured disk search paths. A REQUIRE for a module already mid-load (directly or transitively) is a hard cycle error naming the full chain. A module whose source signals an error, or which finishes without calling (PROVIDE 'NAME), is NOT marked loaded -- whatever top-level definitions it already ran are not rolled back. See docs/manual/10-modules.md section 10.7 for the full story, and lib/06-require.lisp for the implementation.")
    (cons 'EXAMPLES '(((require 'shell) SHELL)
                       ((require 'shell) SHELL)))
    (cons 'SEE-ALSO '(provide require-reload loaded-modules module-state module-info defmodule))))

(register-doc 'provide
  (list
    (cons 'NAME 'provide)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(provide 'name) or (provide 'name exports)")
    (cons 'CATEGORY 'modules)
    (cons 'DESCRIPTION "Called from within a module's own source (as loaded by REQUIRE) to mark NAME complete; conventionally the module's last top-level form. REQUIRE signals an error if a module's source finishes evaluating without a matching PROVIDE. The optional EXPORTS argument is a list of symbol names this module claims to define -- metadata only, not enforcement (Lamedh has no reader-level privacy or namespaces); REQUIRE warns if a declared export ends up unbound, and warns (or, with *REQUIRE-STRICT-EXPORTS* bound to T, errors) if a declared export was already claimed by a different module.")
    (cons 'EXAMPLES '(((provide 'my-app) MY-APP)))
    (cons 'SEE-ALSO '(require require-reload))))

(register-doc 'require-reload
  (list
    (cons 'NAME 'require-reload)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(require-reload 'name)")
    (cons 'CATEGORY 'modules)
    (cons 'DESCRIPTION "Development/debugging operation: forces NAME to be re-resolved and re-evaluated via REQUIRE's normal procedure even though it is already loaded. Ordinary REQUIRE never does this implicitly -- use REQUIRE-RELOAD when iterating on a registered or disk module's source without restarting the interpreter. Errors if NAME is currently mid-load.")
    (cons 'SEE-ALSO '(require provide))))

(register-doc 'loaded-modules
  (list
    (cons 'NAME 'loaded-modules)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(loaded-modules)")
    (cons 'CATEGORY 'modules)
    (cons 'DESCRIPTION "Returns all module names currently REQUIRE-loaded in this environment, in no particular order.")
    (cons 'SEE-ALSO '(require module-state module-info))))

(register-doc 'module-state
  (list
    (cons 'NAME 'module-state)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(module-state 'name)")
    (cons 'CATEGORY 'modules)
    (cons 'DESCRIPTION "Returns 'REQUIRE-LOADED, 'REQUIRE-LOADING, 'REQUIRE-UNLOADED, or NIL if NAME has never been REQUIREd, PROVIDEd, or registered in this environment.")
    (cons 'SEE-ALSO '(require loaded-modules module-info))))

(register-doc 'module-info
  (list
    (cons 'NAME 'module-info)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(module-info 'name)")
    (cons 'CATEGORY 'modules)
    (cons 'DESCRIPTION "Returns an alist of diagnostic metadata REQUIRE tracks for NAME: STATE, SOURCE (an origin string such as \"registered\", \"embedded\", or \"disk:<path>\"), DEPS (names REQUIREd while NAME itself was loading), EXPORTS, and ERROR (the last load failure's message, or NIL).")
    (cons 'SEE-ALSO '(require module-state loaded-modules))))

;;; ============================================================
;;; FIRST-CLASS ENVIRONMENTS
;;; ============================================================

(register-doc 'the-environment
  (list
    (cons 'NAME 'the-environment)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(the-environment)")
    (cons 'CATEGORY 'environments)
    (cons 'DESCRIPTION "Returns the current lexical environment as a first-class Environment object. This is a live reference — any bindings established after the call are visible through the returned object. The environment can be passed to EVAL, EVLIS, EVCON, or MAKE-ENVIRONMENT as a parent to evaluate code in a specific scope. Primarily used by VAU operatives (via their env-param) and by metaprogramming utilities.")
    (cons 'SEE-ALSO '(make-environment current-environment eval evlis vau))))

(register-doc 'make-environment
  (list
    (cons 'NAME 'make-environment)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(make-environment) or (make-environment parent-env)")
    (cons 'CATEGORY 'environments)
    (cons 'DESCRIPTION "Creates a new first-class environment. With no arguments creates a fresh root environment pre-populated with all builtins (equivalent to a clean Lamedh session before loading the standard library). With one argument — an Environment object — creates a child environment that inherits all bindings from parent-env while new definitions are isolated to the child. Useful for sandboxing, module systems, and eval-in-context patterns.")
    (cons 'EXAMPLES '(((let ((e (make-environment (the-environment))))
                          (eval '(def x 42) e)
                          (eval 'x e))
                       42)))
    (cons 'SEE-ALSO '(the-environment current-environment eval))))

(register-doc 'current-environment
  (list
    (cons 'NAME 'current-environment)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(current-environment)")
    (cons 'CATEGORY 'environments)
    (cons 'DESCRIPTION "Returns a snapshot of all currently visible bindings as a hash table (symbol → value). Unlike THE-ENVIRONMENT, this returns a new, frozen hash table rather than a live environment object. Useful for inspection, debugging, and serialisation. The keys are symbols; the values are the current binding values at call time.")
    (cons 'SEE-ALSO '(the-environment make-environment keys))))

;;; ============================================================
;;; FILE SYSTEM I/O
;;; ============================================================

(register-doc 'read-file
  (list
    (cons 'NAME 'read-file)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(read-file path)")
    (cons 'CATEGORY 'filesystem)
    (cons 'DESCRIPTION "Reads the entire contents of the file at path as a UTF-8 string. Signals an error if the file does not exist, cannot be read, or is not valid UTF-8. Requires the READ-FS capability.")
    (cons 'EXAMPLES '(((read-file "/etc/hostname") "myhost\n")))
    (cons 'SEE-ALSO '(write-file read-file-byte read-file-section file-exists-p feature-enabled-p))))

(register-doc 'read-file-byte
  (list
    (cons 'NAME 'read-file-byte)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(read-file-byte path offset)")
    (cons 'CATEGORY 'filesystem)
    (cons 'DESCRIPTION "Reads a single byte at byte offset from the file at path. Returns the byte value as an integer (0–255), or NIL if offset is at or past the end of the file. Requires the READ-FS capability. Useful for binary file inspection; for text use READ-FILE or READ-FILE-SECTION.")
    (cons 'EXAMPLES '(((read-file-byte "/bin/true" 0) 127)))
    (cons 'SEE-ALSO '(read-file read-file-section feature-enabled-p))))

(register-doc 'read-file-section
  (list
    (cons 'NAME 'read-file-section)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(read-file-section path offset len)")
    (cons 'CATEGORY 'filesystem)
    (cons 'DESCRIPTION "Reads up to len bytes starting at byte offset from the file at path and decodes them as UTF-8 STRICTLY: invalid bytes signal an error (naming the offending byte offset) rather than being silently coerced. Returns a shorter string if fewer than len bytes are available. Use READ-FILE-SECTION-LOSSY for replacement-character decoding, or READ-FILE-SECTION-BYTES for the raw bytes. Requires the READ-FS capability.")
    (cons 'SEE-ALSO '(read-file-section-lossy read-file-section-bytes read-file read-file-byte write-file feature-enabled-p))))

(register-doc 'read-file-section-lossy
  (list
    (cons 'NAME 'read-file-section-lossy)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(read-file-section-lossy path offset len)")
    (cons 'CATEGORY 'filesystem)
    (cons 'DESCRIPTION "Like READ-FILE-SECTION but decodes UTF-8 lossily: invalid bytes become the U+FFFD replacement character instead of signaling an error. The explicit opt-in to lossy decoding, mirroring TEXT:UTF8->STRING-LOSSY. Requires the READ-FS capability.")
    (cons 'SEE-ALSO '(read-file-section read-file-section-bytes feature-enabled-p))))

(register-doc 'read-file-section-bytes
  (list
    (cons 'NAME 'read-file-section-bytes)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(read-file-section-bytes path offset len)")
    (cons 'CATEGORY 'filesystem)
    (cons 'DESCRIPTION "Reads up to len bytes starting at byte offset from the file at path and returns them as an Array of bytes (Array<Char>), with no text decoding. Cross the text boundary yourself with TEXT:UTF8->STRING / TEXT:UTF8->STRING-LOSSY, or feed the bytes to a codec (BASE64, HEX, JSON, ...). Returns a shorter array if fewer than len bytes are available. Requires the READ-FS capability.")
    (cons 'SEE-ALSO '(read-file-section read-file-section-lossy feature-enabled-p))))

(register-doc 'write-file
  (list
    (cons 'NAME 'write-file)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(write-file path content)")
    (cons 'CATEGORY 'filesystem)
    (cons 'DESCRIPTION "Writes the string content to the file at path, replacing any existing content. Creates the file if it does not exist. Returns T on success; signals an error on failure. Requires the CREATE-FS capability. For appending or streaming writes, use the SHELL capability with shell tools.")
    (cons 'EXAMPLES '(((write-file "/tmp/hello.txt" "hello world\n") t)))
    (cons 'SEE-ALSO '(read-file make-temp-file feature-enabled-p))))

(register-doc 'read-sexpr-file
  (list
    (cons 'NAME 'read-sexpr-file)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(read-sexpr-file path)")
    (cons 'CATEGORY 'filesystem)
    (cons 'DESCRIPTION "Reads path's full text (requires READ-FS) and parses it into a list of every top-level s-expression it contains, via READ-STRING. The inverse of WRITE-SEXPR-FILE (issue #150, lib/18-format.lisp).")
    (cons 'SEE-ALSO '(write-sexpr-file read-file read-string))))

(register-doc 'write-sexpr-file
  (list
    (cons 'NAME 'write-sexpr-file)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(write-sexpr-file path forms)")
    (cons 'CATEGORY 'filesystem)
    (cons 'DESCRIPTION "Writes forms (a list of s-expressions) to path (requires CREATE-FS), one per line in readable (PRIN1) form; the inverse of READ-SEXPR-FILE (issue #150, lib/18-format.lisp).")
    (cons 'SEE-ALSO '(read-sexpr-file write-file prin1-to-string))))

(register-doc 'file-exists-p
  (list
    (cons 'NAME 'file-exists-p)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(file-exists-p path)")
    (cons 'CATEGORY 'filesystem)
    (cons 'DESCRIPTION "Returns T if something (file, directory, symlink, etc.) exists at path; returns NIL otherwise. Requires the READ-FS capability.")
    (cons 'SEE-ALSO '(file-p directory-p file-readable-p feature-enabled-p))))

(register-doc 'directory-p
  (list
    (cons 'NAME 'directory-p)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(directory-p path)")
    (cons 'CATEGORY 'filesystem)
    (cons 'DESCRIPTION "Returns T if path names an existing directory; returns NIL otherwise. Requires the READ-FS capability.")
    (cons 'SEE-ALSO '(file-p file-exists-p directory-files feature-enabled-p))))

(register-doc 'file-p
  (list
    (cons 'NAME 'file-p)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(file-p path)")
    (cons 'CATEGORY 'filesystem)
    (cons 'DESCRIPTION "Returns T if path names an existing regular file (not a directory or special file); returns NIL otherwise. Requires the READ-FS capability.")
    (cons 'SEE-ALSO '(directory-p file-exists-p file-readable-p feature-enabled-p))))

(register-doc 'file-readable-p
  (list
    (cons 'NAME 'file-readable-p)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(file-readable-p path)")
    (cons 'CATEGORY 'filesystem)
    (cons 'DESCRIPTION "Returns T if the file at path can be opened for reading by the current process; returns NIL otherwise. Requires the READ-FS capability. Implemented by attempting to open the file.")
    (cons 'SEE-ALSO '(file-writable-p file-executable-p file-exists-p feature-enabled-p))))

(register-doc 'file-writable-p
  (list
    (cons 'NAME 'file-writable-p)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(file-writable-p path)")
    (cons 'CATEGORY 'filesystem)
    (cons 'DESCRIPTION "Returns T if the file at path exists and is not marked read-only; returns NIL otherwise. Requires the READ-FS capability. Checks the filesystem metadata permissions; does not attempt to open the file.")
    (cons 'SEE-ALSO '(file-readable-p file-executable-p feature-enabled-p))))

(register-doc 'file-executable-p
  (list
    (cons 'NAME 'file-executable-p)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(file-executable-p path)")
    (cons 'CATEGORY 'filesystem)
    (cons 'DESCRIPTION "Returns T if the file at path has at least one executable permission bit set (Unix execute bit); returns NIL otherwise or on non-Unix platforms. Requires the READ-FS capability.")
    (cons 'SEE-ALSO '(file-readable-p file-writable-p feature-enabled-p))))

(register-doc 'file-size
  (list
    (cons 'NAME 'file-size)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(file-size path)")
    (cons 'CATEGORY 'filesystem)
    (cons 'DESCRIPTION "Returns the size of the file at path in bytes as an integer. Signals an error if the file does not exist or cannot be accessed. Requires the READ-FS capability.")
    (cons 'EXAMPLES '(((file-size "/etc/hostname") 8)))
    (cons 'SEE-ALSO '(file-exists-p read-file feature-enabled-p))))

(register-doc 'directory-files
  (list
    (cons 'NAME 'directory-files)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(directory-files path)")
    (cons 'CATEGORY 'filesystem)
    (cons 'DESCRIPTION "Returns a sorted list of filename strings (not full paths) for all entries in the directory at path. Includes both files and subdirectories; does not recurse. Signals an error if path is not a readable directory. Requires the READ-FS capability.")
    (cons 'EXAMPLES '(((directory-files "/tmp") ("file1.txt" "subdir"))))
    (cons 'SEE-ALSO '(directory-p file-exists-p feature-enabled-p))))

(register-doc 'file-newer-p
  (list
    (cons 'NAME 'file-newer-p)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(file-newer-p path1 path2)")
    (cons 'CATEGORY 'filesystem)
    (cons 'DESCRIPTION "Returns T if path1's modification time is strictly later than path2's modification time; returns NIL otherwise. Both files must exist. Requires the READ-FS capability. Useful for incremental build-like logic.")
    (cons 'SEE-ALSO '(file-exists-p file-size feature-enabled-p))))

(register-doc 'chmod
  (list
    (cons 'NAME 'chmod)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(chmod path mode)")
    (cons 'CATEGORY 'filesystem)
    (cons 'DESCRIPTION "Changes the permissions of the file at path to mode. Mode may be an integer (the raw Unix mode value) or an octal string like \"755\". Returns T on success; signals an error on failure. Only supported on Unix; signals an error on Windows. Requires the CREATE-FS capability.")
    (cons 'EXAMPLES '(((chmod "/tmp/myscript.sh" "755") t)))
    (cons 'SEE-ALSO '(file-executable-p write-file feature-enabled-p))))

(register-doc 'create-directory
  (list
    (cons 'NAME 'create-directory)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(create-directory path)")
    (cons 'CATEGORY 'filesystem)
    (cons 'DESCRIPTION "Creates the directory at path and all intermediate directories as needed (like mkdir -p). Returns T on success; signals an error on failure. Requires the CREATE-FS capability.")
    (cons 'EXAMPLES '(((create-directory "/tmp/new/subdir") t)))
    (cons 'SEE-ALSO '(directory-p delete-file feature-enabled-p))))

(register-doc 'delete-file
  (list
    (cons 'NAME 'delete-file)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(delete-file path)")
    (cons 'CATEGORY 'filesystem)
    (cons 'DESCRIPTION "Deletes the regular file at path. Signals an error if the file does not exist or is a directory. Returns T on success. Requires the CREATE-FS capability. To remove directories, use shell tools via SHELL.")
    (cons 'EXAMPLES '(((delete-file "/tmp/old.txt") t)))
    (cons 'SEE-ALSO '(rename-file write-file file-exists-p feature-enabled-p))))

(register-doc 'rename-file
  (list
    (cons 'NAME 'rename-file)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(rename-file from to)")
    (cons 'CATEGORY 'filesystem)
    (cons 'DESCRIPTION "Renames (or moves) the file or directory at from to to. On the same filesystem this is atomic; across filesystems it may copy-then-delete. Returns T on success; signals an error on failure. Requires the CREATE-FS capability.")
    (cons 'EXAMPLES '(((rename-file "/tmp/old.txt" "/tmp/new.txt") t)))
    (cons 'SEE-ALSO '(delete-file write-file feature-enabled-p))))

(register-doc 'make-temp-file
  (list
    (cons 'NAME 'make-temp-file)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(make-temp-file) or (make-temp-file prefix)")
    (cons 'CATEGORY 'filesystem)
    (cons 'DESCRIPTION "Creates a new empty temporary file and returns its path as a string. The optional prefix string is prepended to the filename. The file is created atomically in the system temp directory. The caller is responsible for deleting the file when done. Requires the TEMP-FS capability.")
    (cons 'EXAMPLES '(((make-temp-file "myapp-") "/tmp/myapp-abc123")))
    (cons 'SEE-ALSO '(make-temp-directory write-file delete-file feature-enabled-p))))

(register-doc 'make-temp-directory
  (list
    (cons 'NAME 'make-temp-directory)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(make-temp-directory) or (make-temp-directory prefix)")
    (cons 'CATEGORY 'filesystem)
    (cons 'DESCRIPTION "Creates a new empty temporary directory and returns its path as a string. The optional prefix string is prepended to the directory name. The directory is created in the system temp directory. The caller is responsible for deleting the directory and its contents when done. Requires the TEMP-FS capability.")
    (cons 'EXAMPLES '(((make-temp-directory "work-") "/tmp/work-abc123")))
    (cons 'SEE-ALSO '(make-temp-file create-directory delete-file feature-enabled-p))))

;;; ============================================================
;;; NETWORKING (NET / TCP / UDP modules, issue #258)
;;; ============================================================

(register-doc 'net:resolve
  (list
    (cons 'NAME 'net:resolve)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(net:resolve host) or (net:resolve host port)")
    (cons 'CATEGORY 'net)
    (cons 'DESCRIPTION "Resolves host (and optional service port, default 0) to an ordered list of NET:ADDRESS records via the system resolver. Requires the NET-DNS capability. Signals a structured error (:CATEGORY :DNS) on failure.")
    (cons 'SEE-ALSO '(net:address net:local-addr net:peer-addr tcp:connect))))

(register-doc 'net:local-addr
  (list
    (cons 'NAME 'net:local-addr)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(net:local-addr resource)")
    (cons 'CATEGORY 'net)
    (cons 'DESCRIPTION "The local NET:ADDRESS a connected TCP port or a TCP/UDP network handle is bound to. No capability required.")
    (cons 'SEE-ALSO '(net:peer-addr tcp:listen udp:bind))))

(register-doc 'net:peer-addr
  (list
    (cons 'NAME 'net:peer-addr)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(net:peer-addr port)")
    (cons 'CATEGORY 'net)
    (cons 'DESCRIPTION "The remote NET:ADDRESS a connected TCP port is connected to. No capability required.")
    (cons 'SEE-ALSO '(net:local-addr tcp:connect tcp:accept))))

(register-doc 'net:address->string
  (list
    (cons 'NAME 'net:address->string)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(net:address->string addr)")
    (cons 'CATEGORY 'net)
    (cons 'DESCRIPTION "Formats addr as \"ip:port\", bracketing an IPv6 host (e.g. \"[::1]:8080\").")
    (cons 'EXAMPLES '(((net:address->string (net:make-address ':ipv4 "127.0.0.1" 80)) "127.0.0.1:80")))
    (cons 'SEE-ALSO '(net:address net:resolve))))

(register-doc 'net:address
  (list
    (cons 'NAME 'net:address)
    (cons 'TYPE 'record)
    (cons 'SYNTAX "(net:make-address family ip port)")
    (cons 'CATEGORY 'net)
    (cons 'DESCRIPTION "A DEFRECORD with fields FAMILY (:ipv4 or :ipv6), IP (a string, never bracketed), and PORT (an integer 0-65535). Accessors: net:address-family, net:address-ip, net:address-port. First-class, printable address data -- the kernel never hands Lisp a raw platform socket-address struct.")
    (cons 'SEE-ALSO '(net:resolve net:address->string net:local-addr net:peer-addr))))

(register-doc 'tcp:connect
  (list
    (cons 'NAME 'tcp:connect)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(tcp:connect host port) or (tcp:connect host port timeout-ms)")
    (cons 'CATEGORY 'tcp)
    (cons 'DESCRIPTION "Connects to host:port over TCP, returning a duplex binary PORTS port -- every PORTS operation (read-byte!, write-bytes!, close!, ...) works on it unchanged. Requires the NET-CONNECT capability. TIMEOUT-MS, if given, bounds the connect attempt; NIL (default) blocks without a timeout.")
    (cons 'SEE-ALSO '(tcp:listen tcp:accept ports:read-bytes! ports:write-bytes!))))

(register-doc 'tcp:listen
  (list
    (cons 'NAME 'tcp:listen)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(tcp:listen host port) or (tcp:listen host port backlog)")
    (cons 'CATEGORY 'tcp)
    (cons 'DESCRIPTION "Binds and listens on host:port for inbound TCP connections, returning a listener handle. Requires the NET-LISTEN capability. BACKLOG (default 128) is accepted for API completeness but is currently advisory only.")
    (cons 'SEE-ALSO '(tcp:accept tcp:close-listener! tcp:listener-p))))

(register-doc 'tcp:accept
  (list
    (cons 'NAME 'tcp:accept)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(tcp:accept listener)")
    (cons 'CATEGORY 'tcp)
    (cons 'DESCRIPTION "Blocks until an inbound connection arrives on listener, returning (cons port peer-address) -- port is a duplex PORTS port, peer-address a NET:ADDRESS. Rejects use after tcp:close-listener! with a :CLOSED error.")
    (cons 'SEE-ALSO '(tcp:listen net:peer-addr))))

(register-doc 'tcp:shutdown!
  (list
    (cons 'NAME 'tcp:shutdown!)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(tcp:shutdown! port how)")
    (cons 'CATEGORY 'tcp)
    (cons 'DESCRIPTION "Shuts down port's read half, write half, or both (HOW: :read, :write, or :both) without closing it.")
    (cons 'SEE-ALSO '(tcp:connect ports:close!))))

(register-doc 'tcp:set-read-timeout!
  (list
    (cons 'NAME 'tcp:set-read-timeout!)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(tcp:set-read-timeout! port ms)")
    (cons 'CATEGORY 'tcp)
    (cons 'DESCRIPTION "Sets port's read timeout in milliseconds; NIL blocks without a timeout (the default). A timed-out read signals a structured :TIMEOUT error.")
    (cons 'SEE-ALSO '(tcp:set-write-timeout! ports:read-bytes!))))

(register-doc 'tcp:set-write-timeout!
  (list
    (cons 'NAME 'tcp:set-write-timeout!)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(tcp:set-write-timeout! port ms)")
    (cons 'CATEGORY 'tcp)
    (cons 'DESCRIPTION "Sets port's write timeout in milliseconds; NIL blocks without a timeout (the default).")
    (cons 'SEE-ALSO '(tcp:set-read-timeout! ports:write-bytes!))))

(register-doc 'tcp:close-listener!
  (list
    (cons 'NAME 'tcp:close-listener!)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(tcp:close-listener! listener)")
    (cons 'CATEGORY 'tcp)
    (cons 'DESCRIPTION "Closes listener. Idempotent, like ports:close!. Every subsequent tcp:accept on this listener errors immediately with a :CLOSED error.")
    (cons 'SEE-ALSO '(tcp:listen tcp:listener-open-p))))

(register-doc 'tcp:listener-p
  (list
    (cons 'NAME 'tcp:listener-p)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(tcp:listener-p x)")
    (cons 'CATEGORY 'tcp)
    (cons 'DESCRIPTION "T if x is a TCP listener handle (as returned by tcp:listen).")
    (cons 'SEE-ALSO '(tcp:listen udp:socket-p))))

(register-doc 'tcp:listener-open-p
  (list
    (cons 'NAME 'tcp:listener-open-p)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(tcp:listener-open-p listener)")
    (cons 'CATEGORY 'tcp)
    (cons 'DESCRIPTION "T unless listener has been closed.")
    (cons 'SEE-ALSO '(tcp:close-listener!))))

(register-doc 'udp:bind
  (list
    (cons 'NAME 'udp:bind)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(udp:bind host port)")
    (cons 'CATEGORY 'udp)
    (cons 'DESCRIPTION "Binds a UDP socket to host:port (port 0 for an OS-assigned ephemeral port). Requires the NET-LISTEN capability -- a bound socket receives datagrams from any sender, matching \"binding for inbound traffic\".")
    (cons 'SEE-ALSO '(udp:connect! udp:send-to udp:receive-from))))

(register-doc 'udp:connect!
  (list
    (cons 'NAME 'udp:connect!)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(udp:connect! socket host port)")
    (cons 'CATEGORY 'udp)
    (cons 'DESCRIPTION "Sets socket's default peer to host:port so udp:send/udp:receive-from can be used without repeating the address. Requires the NET-CONNECT capability.")
    (cons 'SEE-ALSO '(udp:send udp:bind))))

(register-doc 'udp:send-to
  (list
    (cons 'NAME 'udp:send-to)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(udp:send-to socket host port bytes)")
    (cons 'CATEGORY 'udp)
    (cons 'DESCRIPTION "Sends bytes (an Array<Char>) as one datagram to host:port, returning the number of bytes sent. Requires the NET-CONNECT capability.")
    (cons 'SEE-ALSO '(udp:send udp:receive-from))))

(register-doc 'udp:send
  (list
    (cons 'NAME 'udp:send)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(udp:send socket bytes)")
    (cons 'CATEGORY 'udp)
    (cons 'DESCRIPTION "Sends bytes as one datagram to socket's connected peer (see udp:connect!).")
    (cons 'SEE-ALSO '(udp:connect! udp:send-to))))

(register-doc 'udp:receive-from
  (list
    (cons 'NAME 'udp:receive-from)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(udp:receive-from socket maxlen)")
    (cons 'CATEGORY 'udp)
    (cons 'DESCRIPTION "Blocks for one datagram of at most maxlen bytes, returning (list bytes peer-address possibly-truncated-p). Datagram boundaries are preserved. possibly-truncated-p is T exactly when the received length equals maxlen, since plain std::net exposes no MSG_TRUNC indicator.")
    (cons 'SEE-ALSO '(udp:bind udp:send-to))))

(register-doc 'udp:close!
  (list
    (cons 'NAME 'udp:close!)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(udp:close! socket)")
    (cons 'CATEGORY 'udp)
    (cons 'DESCRIPTION "Closes socket. Idempotent; every subsequent send/receive on socket errors immediately with a :CLOSED error.")
    (cons 'SEE-ALSO '(udp:bind udp:socket-open-p))))

(register-doc 'udp:socket-p
  (list
    (cons 'NAME 'udp:socket-p)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(udp:socket-p x)")
    (cons 'CATEGORY 'udp)
    (cons 'DESCRIPTION "T if x is a UDP socket handle (as returned by udp:bind).")
    (cons 'SEE-ALSO '(udp:bind tcp:listener-p))))

(register-doc 'udp:socket-open-p
  (list
    (cons 'NAME 'udp:socket-open-p)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(udp:socket-open-p socket)")
    (cons 'CATEGORY 'udp)
    (cons 'DESCRIPTION "T unless socket has been closed.")
    (cons 'SEE-ALSO '(udp:close!))))

(register-doc 'udp:set-timeout!
  (list
    (cons 'NAME 'udp:set-timeout!)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(udp:set-timeout! socket ms)")
    (cons 'CATEGORY 'udp)
    (cons 'DESCRIPTION "Sets socket's read and write timeout in milliseconds together; NIL blocks without a timeout (the default). A timed-out receive-from/send/send-to signals a structured :TIMEOUT error.")
    (cons 'SEE-ALSO '(udp:receive-from tcp:set-read-timeout!))))

(register-doc 'tls:available-p
  (list
    (cons 'NAME 'tls:available-p)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(tls:available-p)")
    (cons 'CATEGORY 'tls)
    (cons 'DESCRIPTION "T if this build of lamedh was compiled with the net-tls cargo feature (rustls); NIL otherwise. Every tls:* name is bound either way -- with the feature off, every other tls:* operation signals a structured :TLS-UNAVAILABLE error instead of doing any work.")
    (cons 'SEE-ALSO '(tls:connect tls:wrap-client))))

(register-doc 'tls:connect
  (list
    (cons 'NAME 'tls:connect)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(tls:connect host port &key connect-timeout-ms handshake-timeout-ms alpn extra-roots)")
    (cons 'CATEGORY 'tls)
    (cons 'DESCRIPTION "tcp:connect + tls:wrap-client sugar: connects to host:port then TLS-wraps the result, :hostname defaulting to host (used for SNI and certificate verification). Verification is on by default against the default (webpki-roots) root store plus :extra-roots (a list of PEM sources: String paths, READ-FS-gated, or Array<Char> bytes). :handshake-timeout-ms bounds the handshake via the underlying TCP port's read/write timeouts. Returns an ordinary PORTS port.")
    (cons 'SEE-ALSO '(tls:wrap-client tls:connect-insecure! tls:alpn-protocol tls:peer-certificates))))

(register-doc 'tls:connect-insecure!
  (list
    (cons 'NAME 'tls:connect-insecure!)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(tls:connect-insecure! host port &key connect-timeout-ms handshake-timeout-ms alpn)")
    (cons 'CATEGORY 'tls)
    (cons 'DESCRIPTION "Like tls:connect, but skips certificate-chain verification entirely -- the only Lisp-facing way to do so. ALWAYS signals a structured :POLICY-DENIED error unless the embedding host has separately opted in via Environment::set_allow_insecure_tls (Rust-only, default false) -- Lisp code alone can never disable verification.")
    (cons 'SEE-ALSO '(tls:connect tls:wrap-client-insecure!))))

(register-doc 'tls:wrap-client
  (list
    (cons 'NAME 'tls:wrap-client)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(tls:wrap-client port &key hostname alpn extra-roots)")
    (cons 'CATEGORY 'tls)
    (cons 'DESCRIPTION "Wraps port -- an already-connected tcp:connect PORT -- as a TLS client, performing the handshake now (blocking). Consumes port (it becomes CLOSED); returns a new PORTS port. :hostname is required (SNI + certificate verification). See tls:connect for the connect+wrap sugar.")
    (cons 'SEE-ALSO '(tls:connect tls:wrap-client-insecure! tls:wrap-server))))

(register-doc 'tls:wrap-client-insecure!
  (list
    (cons 'NAME 'tls:wrap-client-insecure!)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(tls:wrap-client-insecure! port &key hostname alpn)")
    (cons 'CATEGORY 'tls)
    (cons 'DESCRIPTION "Like tls:wrap-client, but skips certificate verification -- denied by default (:POLICY-DENIED) unless the host opted in via Environment::set_allow_insecure_tls. See tls:connect-insecure! for the connect+wrap sugar.")
    (cons 'SEE-ALSO '(tls:wrap-client tls:connect-insecure!))))

(register-doc 'tls:wrap-server
  (list
    (cons 'NAME 'tls:wrap-server)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(tls:wrap-server port cert key &key alpn)")
    (cons 'CATEGORY 'tls)
    (cons 'DESCRIPTION "Wraps port -- an already-accepted tcp:accept PORT -- as a TLS server, performing the handshake now (blocking). Consumes port (it becomes CLOSED); returns a new PORTS port. cert/key are each a PEM source (String path, READ-FS-gated, or Array<Char> bytes); cert may be a full chain, leaf first. No client-certificate authentication is requested.")
    (cons 'SEE-ALSO '(tls:wrap-client tcp:accept))))

(register-doc 'tls:alpn-protocol
  (list
    (cons 'NAME 'tls:alpn-protocol)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(tls:alpn-protocol port)")
    (cons 'CATEGORY 'tls)
    (cons 'DESCRIPTION "The ALPN protocol negotiated on TLS port (a String), or NIL if none was negotiated.")
    (cons 'SEE-ALSO '(tls:connect tls:wrap-server))))

(register-doc 'tls:peer-certificates
  (list
    (cons 'NAME 'tls:peer-certificates)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(tls:peer-certificates port)")
    (cons 'CATEGORY 'tls)
    (cons 'DESCRIPTION "The peer's certificate chain on TLS port, leaf first, as a list of Array<Char> raw DER bytes -- or NIL if none presented. No X.509 parser is part of this dependency ruling, so these are opaque DER bytes, not parsed fields; see tls:peer-certificate-summary for structural (unparsed) data.")
    (cons 'SEE-ALSO '(tls:peer-certificate-summary))))

(register-doc 'tls:peer-certificate-summary
  (list
    (cons 'NAME 'tls:peer-certificate-summary)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(tls:peer-certificate-summary port)")
    (cons 'CATEGORY 'tls)
    (cons 'DESCRIPTION "A structural summary alist for TLS port's peer certificate chain: ((:count . N) (:leaf-der-length . M) (:leaf-der . bytes)), or NIL if none presented. No parsed subject/issuer/expiry fields -- see tls:peer-certificates for the full raw DER chain.")
    (cons 'SEE-ALSO '(tls:peer-certificates))))

(register-doc 'tls:sni-hostname
  (list
    (cons 'NAME 'tls:sni-hostname)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(tls:sni-hostname port)")
    (cons 'CATEGORY 'tls)
    (cons 'DESCRIPTION "The SNI hostname a TLS client offered when connecting to server-side port, or NIL if none was sent (or port is a client-side TLS port).")
    (cons 'SEE-ALSO '(tls:wrap-server))))

(register-doc 'http:request
  (list
    (cons 'NAME 'http:request)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(http:request method url &key headers body connect-timeout-ms read-timeout-ms overall-timeout-ms max-redirects follow-redirects max-line-bytes max-header-count extra-roots)")
    (cons 'CATEGORY 'http)
    (cons 'DESCRIPTION "Performs an HTTP/1.1 request. http:// always; https:// too when the net-tls cargo feature is compiled in (:extra-roots forwards to tls:connect for a private/throwaway CA) -- otherwise https:// is a structured :HTTPS-UNSUPPORTED error naming the net-tls feature (issue #365). Requires NET-CONNECT (via tcp:connect; HTTP adds no capability of its own). :HEADERS is an ordered (name . value) list (repeats preserved); :BODY is NIL, a String, an Array<Char>, or a readable PORTS port (streamed via chunked transfer-encoding). Follows 301/302/303/307/308 redirects by default, hop-capped, stripping credentials cross-origin, never crossing schemes silently. Returns a response alist whose :BODY is an UNREAD framing-aware body stream.")
    (cons 'SEE-ALSO '(http:get http:post http:response-status http:response-body http:collect-string))))

(register-doc 'http:get
  (list
    (cons 'NAME 'http:get)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(http:get url &key headers connect-timeout-ms read-timeout-ms overall-timeout-ms max-redirects follow-redirects)")
    (cons 'CATEGORY 'http)
    (cons 'DESCRIPTION "Ergonomic (http:request \"GET\" url ...). See http:request for every keyword and the capability/scheme rules.")
    (cons 'SEE-ALSO '(http:request http:post http:collect-string))))

(register-doc 'http:post
  (list
    (cons 'NAME 'http:post)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(http:post url &key body headers connect-timeout-ms read-timeout-ms overall-timeout-ms max-redirects follow-redirects)")
    (cons 'CATEGORY 'http)
    (cons 'DESCRIPTION "Ergonomic (http:request \"POST\" url :body body ...). See http:request for every keyword.")
    (cons 'SEE-ALSO '(http:request http:get))))

(register-doc 'http:response-status
  (list
    (cons 'NAME 'http:response-status)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(http:response-status response)")
    (cons 'CATEGORY 'http)
    (cons 'DESCRIPTION "The integer status code of a client response alist. Companions: http:response-reason, http:response-version, http:response-headers, http:response-header (case-insensitive first-match lookup), http:response-body (the unread body stream).")
    (cons 'SEE-ALSO '(http:request http:response-body mime:headers-get))))

(register-doc 'http:response-body
  (list
    (cons 'NAME 'http:response-body)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(http:response-body response)")
    (cons 'CATEGORY 'http)
    (cons 'DESCRIPTION "The response's UNREAD body stream: framing-aware (Content-Length exact / chunked / read-to-close / no body for HEAD, 1xx, 204, 304). Read incrementally with http:stream-read!, or collect bounded with http:collect-bytes / http:collect-string / http:collect-json.")
    (cons 'SEE-ALSO '(http:stream-read! http:collect-bytes http:collect-string http:collect-json))))

(register-doc 'http:stream-read!
  (list
    (cons 'NAME 'http:stream-read!)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(http:stream-read! stream n)")
    (cons 'CATEGORY 'http)
    (cons 'DESCRIPTION "Reads up to N bytes from an HTTP body stream, honoring its message framing -- never reads past this message's body. Returns a fresh Array<Char>, possibly shorter than N, empty exactly at the logical end of the body (mirrors ports:read-bytes!). Companions: http:stream-eof-p, http:stream-read-all!, http:stream-close! (closes the client connection; a no-op for a server request body).")
    (cons 'SEE-ALSO '(http:stream-eof-p http:stream-close! http:collect-bytes ports:read-bytes!))))

(register-doc 'http:collect-string
  (list
    (cons 'NAME 'http:collect-string)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(http:collect-string stream &key max-bytes lossy)")
    (cons 'CATEGORY 'http)
    (cons 'DESCRIPTION "Collects an HTTP body stream to its end (bounded: default 10 MiB, error past :MAX-BYTES -- never unbounded buffering) and decodes it as UTF-8 into a String (:LOSSY t for replacement characters instead of a strict decode error). Companions: http:collect-bytes (raw Array<Char>), http:collect-json (parses via json:parse).")
    (cons 'SEE-ALSO '(http:collect-bytes http:collect-json http:stream-read! text:utf8->string))))

(register-doc 'http:serve
  (list
    (cons 'NAME 'http:serve)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(http:serve listener handler &key read-timeout-ms max-line-bytes max-header-count max-body-bytes on-error max-requests stop-p)")
    (cons 'CATEGORY 'http)
    (cons 'DESCRIPTION "Serves HTTP/1.1 on a tcp:listen listener (NET-LISTEN gates the listen; HTTP adds no capability). HANDLER: request alist -> response alist (see http:respond). Serial keep-alive: one connection served fully before the next accept (concurrency is issue #140's scope). Request line/header/body limits enforced (oversize body -> 413 without running the handler); an uncaught handler error becomes a generic 500 that never leaks the condition to the peer (:ON-ERROR receives it host-side). :STOP-P is consulted between connections; :MAX-REQUESTS bounds the connection count. http:serve-one! accepts and serves exactly one connection.")
    (cons 'SEE-ALSO '(http:serve-one! http:respond tcp:listen))))

(register-doc 'http:respond
  (list
    (cons 'NAME 'http:respond)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(http:respond status &key headers body reason)")
    (cons 'CATEGORY 'http)
    (cons 'DESCRIPTION "Builds a response alist for an http:serve handler: STATUS integer, :HEADERS an ordered (name . value) list, :BODY NIL/String/Array<Char>/readable PORTS port (a port streams out chunked; Content-Length is set automatically otherwise), :REASON defaulting to http:default-reason. The handler-side request accessors are http:request-method, -target, -path, -query, -headers, -header, -body (a streaming body, Content-Length and chunked framing both), -version, and -peer-addr.")
    (cons 'SEE-ALSO '(http:serve http:default-reason http:request-body))))

;;; ============================================================
;;; OS INTEGRATION (OS / OS-LINUX modules, issue #260)
;;; ============================================================

(register-doc 'os:args
  (list
    (cons 'NAME 'os:args)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(os:args)")
    (cons 'CATEGORY 'os)
    (cons 'DESCRIPTION "The process's raw argv (including argv[0]) as a list of strings. Requires OS-ENV. Distinct from *ARGV*, which the CLI binds to only the arguments after a script path.")
    (cons 'SEE-ALSO '(os:executable-path))))

(register-doc 'os:executable-path
  (list
    (cons 'NAME 'os:executable-path)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(os:executable-path)")
    (cons 'CATEGORY 'os)
    (cons 'DESCRIPTION "The absolute path to the currently running executable. Requires OS-ENV.")
    (cons 'SEE-ALSO '(os:args))))

(register-doc 'os:cwd
  (list
    (cons 'NAME 'os:cwd)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(os:cwd)")
    (cons 'CATEGORY 'os)
    (cons 'DESCRIPTION "The process's current working directory, as a string. Requires OS-ENV.")
    (cons 'SEE-ALSO '(os:chdir!))))

(register-doc 'os:chdir!
  (list
    (cons 'NAME 'os:chdir!)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(os:chdir! path)")
    (cons 'CATEGORY 'os)
    (cons 'DESCRIPTION "Changes the process's current working directory to path (process-wide; affects every subsequent relative-path filesystem operation). Requires OS-ENV-WRITE.")
    (cons 'SEE-ALSO '(os:cwd))))

(register-doc 'os:env-get
  (list
    (cons 'NAME 'os:env-get)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(os:env-get name)")
    (cons 'CATEGORY 'os)
    (cons 'DESCRIPTION "The value of environment variable name, or NIL if unset. Requires OS-ENV.")
    (cons 'SEE-ALSO '(os:env-list os:env-set! os:env-unset!))))

(register-doc 'os:env-list
  (list
    (cons 'NAME 'os:env-list)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(os:env-list)")
    (cons 'CATEGORY 'os)
    (cons 'DESCRIPTION "Every environment variable as an alist of (name . value) strings, sorted by name. Requires OS-ENV.")
    (cons 'SEE-ALSO '(os:env-get))))

(register-doc 'os:env-set!
  (list
    (cons 'NAME 'os:env-set!)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(os:env-set! name value)")
    (cons 'CATEGORY 'os)
    (cons 'DESCRIPTION "Sets environment variable name to value (process-wide). Requires OS-ENV-WRITE.")
    (cons 'SEE-ALSO '(os:env-get os:env-unset!))))

(register-doc 'os:env-unset!
  (list
    (cons 'NAME 'os:env-unset!)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(os:env-unset! name)")
    (cons 'CATEGORY 'os)
    (cons 'DESCRIPTION "Removes environment variable name (process-wide); a no-op if already unset. Requires OS-ENV-WRITE.")
    (cons 'SEE-ALSO '(os:env-set!))))

(register-doc 'os:pid
  (list
    (cons 'NAME 'os:pid)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(os:pid)")
    (cons 'CATEGORY 'os)
    (cons 'DESCRIPTION "This process's OS process ID. Requires OS-ENV.")
    (cons 'SEE-ALSO '(os:ppid os:process-id))))

(register-doc 'os:ppid
  (list
    (cons 'NAME 'os:ppid)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(os:ppid)")
    (cons 'CATEGORY 'os)
    (cons 'DESCRIPTION "This process's parent process ID. Requires OS-ENV. Linux-only (std has no portable getppid()); signals a structured :UNSUPPORTED-PLATFORM error elsewhere.")
    (cons 'SEE-ALSO '(os:pid))))

(register-doc 'os:hostname
  (list
    (cons 'NAME 'os:hostname)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(os:hostname)")
    (cons 'CATEGORY 'os)
    (cons 'DESCRIPTION "This host's hostname. Requires OS-ENV. Linux-only; signals a structured :UNSUPPORTED-PLATFORM error elsewhere.")
    (cons 'SEE-ALSO '(os:pid))))

(register-doc 'os:now
  (list
    (cons 'NAME 'os:now)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(os:now)")
    (cons 'CATEGORY 'os)
    (cons 'DESCRIPTION "Current wall-clock time since the Unix epoch, as (CONS seconds nanoseconds). No capability required.")
    (cons 'SEE-ALSO '(os:now-unix os:monotonic-nanos))))

(register-doc 'os:now-unix
  (list
    (cons 'NAME 'os:now-unix)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(os:now-unix)")
    (cons 'CATEGORY 'os)
    (cons 'DESCRIPTION "Current wall-clock time since the Unix epoch as a single float number of seconds. No capability required.")
    (cons 'SEE-ALSO '(os:now))))

(register-doc 'os:monotonic-nanos
  (list
    (cons 'NAME 'os:monotonic-nanos)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(os:monotonic-nanos)")
    (cons 'CATEGORY 'os)
    (cons 'DESCRIPTION "Nanoseconds elapsed since an arbitrary, process-local, never-goes-backward reference point -- not comparable across processes or with os:now. No capability required.")
    (cons 'SEE-ALSO '(os:elapsed-seconds os:now))))

(register-doc 'os:elapsed-seconds
  (list
    (cons 'NAME 'os:elapsed-seconds)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(os:elapsed-seconds start-nanos)")
    (cons 'CATEGORY 'os)
    (cons 'DESCRIPTION "Seconds elapsed since start-nanos (a prior os:monotonic-nanos reading), as a float. No capability required.")
    (cons 'SEE-ALSO '(os:monotonic-nanos))))

(register-doc 'os:sleep
  (list
    (cons 'NAME 'os:sleep)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(os:sleep ms)")
    (cons 'CATEGORY 'os)
    (cons 'DESCRIPTION "Blocks the calling thread for ms milliseconds. Always sleeps for at least the requested duration; no EINTR/short-sleep behavior is observable. No capability required.")
    (cons 'SEE-ALSO '(os:sleep-seconds))))

(register-doc 'os:sleep-seconds
  (list
    (cons 'NAME 'os:sleep-seconds)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(os:sleep-seconds secs)")
    (cons 'CATEGORY 'os)
    (cons 'DESCRIPTION "Blocks the calling thread for secs seconds (a float or integer). See os:sleep.")
    (cons 'SEE-ALSO '(os:sleep))))

(register-doc 'os:make-prng
  (list
    (cons 'NAME 'os:make-prng)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(os:make-prng seed)")
    (cons 'CATEGORY 'os)
    (cons 'DESCRIPTION "A fresh deterministic PRNG state seeded with seed (any integer). Distinct from the global time-seeded (random n): explicitly seeded and reproducible. No capability required.")
    (cons 'SEE-ALSO '(os:prng-next os:random-double random))))

(register-doc 'os:prng-next
  (list
    (cons 'NAME 'os:prng-next)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(os:prng-next state)")
    (cons 'CATEGORY 'os)
    (cons 'DESCRIPTION "Advances PRNG state (from os:make-prng or a prior os:prng-next) one SplitMix64 step. Returns (CONS new-state value), value a non-negative integer in [0, 2^63). Purely functional -- never mutates state in place. Deterministic: the same state always yields the same result.")
    (cons 'EXAMPLES '(((cdr (os:prng-next (os:make-prng 42))) 6839728766377637706)))
    (cons 'SEE-ALSO '(os:make-prng os:random-double))))

(register-doc 'os:random-double
  (list
    (cons 'NAME 'os:random-double)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(os:random-double state)")
    (cons 'CATEGORY 'os)
    (cons 'DESCRIPTION "Like os:prng-next, but value is a float in [0.0, 1.0).")
    (cons 'SEE-ALSO '(os:prng-next))))

(register-doc 'os:random-bytes
  (list
    (cons 'NAME 'os:random-bytes)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(os:random-bytes n)")
    (cons 'CATEGORY 'os)
    (cons 'DESCRIPTION "N cryptographically secure random bytes from the OS entropy source (/dev/urandom on Linux) as a fresh Array<Char>. No capability required -- a read-only entropy source, not application data. Distinct from os:make-prng/os:prng-next's deterministic, explicitly-seeded generator.")
    (cons 'SEE-ALSO '(os:make-prng))))

(register-doc 'os:spawn
  (list
    (cons 'NAME 'os:spawn)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(os:spawn program &optional argv &key (inherit-env t) env cwd stdin stdout stderr)")
    (cons 'CATEGORY 'os)
    (cons 'DESCRIPTION "Spawns program (a path, never run through a shell -- argv is passed through literally, no interpolation) with argv (a list of strings). Requires OS-PROCESS. :INHERIT-ENV/:ENV control the child's environment; :CWD its working directory; :STDIN/:STDOUT/:STDERR are each NIL/:INHERIT/:NULL/:PIPE. Returns ((:handle . child) (:stdin . port-or-nil) (:stdout . port-or-nil) (:stderr . port-or-nil)).")
    (cons 'SEE-ALSO '(os:process-handle os:process-wait! os:process-kill! shell))))

(register-doc 'os:process-handle
  (list
    (cons 'NAME 'os:process-handle)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(os:process-handle process)")
    (cons 'CATEGORY 'os)
    (cons 'DESCRIPTION "The OS:CHILD handle inside an os:spawn result alist. Companions: os:process-stdin, os:process-stdout, os:process-stderr (the stdio ports requested as :pipe, or NIL).")
    (cons 'SEE-ALSO '(os:spawn os:process-wait!))))

(register-doc 'os:process-wait!
  (list
    (cons 'NAME 'os:process-wait!)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(os:process-wait! handle)")
    (cons 'CATEGORY 'os)
    (cons 'DESCRIPTION "Blocks until the child behind handle exits, then reaps it. Returns an exit-status alist: ((:exit-code . n-or-nil) (:signal . n-or-nil) (:success . t-or-nil)). Idempotent: a second call after reaping returns the cached status. No further capability required once handle exists (OS-PROCESS covers continued use).")
    (cons 'SEE-ALSO '(os:process-try-wait! os:exit-code os:exit-success-p))))

(register-doc 'os:process-try-wait!
  (list
    (cons 'NAME 'os:process-try-wait!)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(os:process-try-wait! handle)")
    (cons 'CATEGORY 'os)
    (cons 'DESCRIPTION "Non-blocking poll of handle: NIL if still running, else the same exit-status alist os:process-wait! returns (reaping the child).")
    (cons 'SEE-ALSO '(os:process-wait!))))

(register-doc 'os:process-id
  (list
    (cons 'NAME 'os:process-id)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(os:process-id handle)")
    (cons 'CATEGORY 'os)
    (cons 'DESCRIPTION "handle's OS PID. Retained (not NIL) even after the process has been reaped, for diagnostics/logging.")
    (cons 'SEE-ALSO '(os:pid os:process-alive-p))))

(register-doc 'os:process-alive-p
  (list
    (cons 'NAME 'os:process-alive-p)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(os:process-alive-p handle)")
    (cons 'CATEGORY 'os)
    (cons 'DESCRIPTION "T unless handle has been reaped (by os:process-wait!/os:process-try-wait! or the Drop backstop).")
    (cons 'SEE-ALSO '(os:process-wait!))))

(register-doc 'os:process-kill!
  (list
    (cons 'NAME 'os:process-kill!)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(os:process-kill! handle)")
    (cons 'CATEGORY 'os)
    (cons 'DESCRIPTION "Sends SIGKILL to the child behind handle (hard, unignorable). Does NOT reap it -- call os:process-wait!/os:process-try-wait! afterward. Signals a :CLOSED error if already reaped.")
    (cons 'SEE-ALSO '(os:process-terminate! os:process-wait!))))

(register-doc 'os:process-terminate!
  (list
    (cons 'NAME 'os:process-terminate!)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(os:process-terminate! handle)")
    (cons 'CATEGORY 'os)
    (cons 'DESCRIPTION "Sends SIGTERM to the child behind handle (graceful; the child may ignore or handle it). Does NOT reap it. Signals a :CLOSED error if already reaped.")
    (cons 'SEE-ALSO '(os:process-kill! os:signal!))))

(register-doc 'os:process-p
  (list
    (cons 'NAME 'os:process-p)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(os:process-p x)")
    (cons 'CATEGORY 'os)
    (cons 'DESCRIPTION "T if x is an OS:CHILD handle (as returned by os:process-handle).")
    (cons 'SEE-ALSO '(os:spawn))))

(register-doc 'os:exit-code
  (list
    (cons 'NAME 'os:exit-code)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(os:exit-code status)")
    (cons 'CATEGORY 'os)
    (cons 'DESCRIPTION "The :exit-code field of an exit-status alist, or NIL if the process was terminated by a signal. Companions: os:exit-signal, os:exit-success-p.")
    (cons 'SEE-ALSO '(os:process-wait! os:exit-success-p))))

(register-doc 'os:signal!
  (list
    (cons 'NAME 'os:signal!)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(os:signal! pid signal-name)")
    (cons 'CATEGORY 'os)
    (cons 'DESCRIPTION "Sends signal-name (a typed name, e.g. :term, :kill, :hup, :int, :usr1, :usr2, :quit, :cont, :stop, :chld, :pipe, :alrm -- never a raw number) to pid (an arbitrary integer PID not necessarily owned as a handle). Requires OS-SIGNAL. Prefer os:process-kill!/os:process-terminate! for a handle you already hold -- those need only OS-PROCESS.")
    (cons 'SEE-ALSO '(os:process-kill! os:process-terminate!))))

(register-doc 'os-linux:stat
  (list
    (cons 'NAME 'os-linux:stat)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(os-linux:stat path)")
    (cons 'CATEGORY 'os-linux)
    (cons 'DESCRIPTION "path's metadata, following a trailing symlink (like stat(2)), as a typed alist: :size :mode :uid :gid :nlink :ino :dev :mtime :atime :ctime :is-dir :is-file :is-symlink -- never a raw C struct. Requires READ-FS.")
    (cons 'SEE-ALSO '(os-linux:lstat os-linux:stat-size os-linux:readlink))))

(register-doc 'os-linux:lstat
  (list
    (cons 'NAME 'os-linux:lstat)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(os-linux:lstat path)")
    (cons 'CATEGORY 'os-linux)
    (cons 'DESCRIPTION "Like os-linux:stat, but does not follow a trailing symlink (like lstat(2)) -- if path itself is a symlink, describes the symlink, and :is-symlink is T.")
    (cons 'SEE-ALSO '(os-linux:stat os-linux:readlink))))

(register-doc 'os-linux:readlink
  (list
    (cons 'NAME 'os-linux:readlink)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(os-linux:readlink path)")
    (cons 'CATEGORY 'os-linux)
    (cons 'DESCRIPTION "The target path points to, as a string, if path is a symlink. Requires READ-FS. Signals a structured :INVALID-ARGUMENT error if path is not a symlink.")
    (cons 'SEE-ALSO '(os-linux:lstat))))

;;; ============================================================
;;; REGISTER CATEGORIES
;;; ============================================================

(register-category 'arithmetic
  "Numeric operations"
  '(+ - * / remainder mod expt add1 sub1 abs max min random
    plus difference times quotient lessp greaterp equal-number
    1+ 1-
    sqrt sin cos tan log exp floor ceiling round truncate
    gcd lcm isqrt signum
    float-equal float-lessp float-greaterp))

(register-category 'predicates
  "Type and value predicates"
  '(zerop plusp minusp evenp oddp < > = atom symbolp numberp fixp floatp
    charp stringp consp listp null not eq equal functionp boundp macrop
    arrayp extension-p error-p))

(register-category 'lists
  "List manipulation"
  '(car cdr cons list append reverse length nth last member assoc
    mapcar maplist subst pairlis nthcdr efface delete
    rplaca rplacd sublis sort))

(register-category 'strings
  "String operations"
  '(concat index explode implode gensym intern maknam
    string-length* substring char-code code-char make-char
    string->number number->string prin1-to-string princ-to-string
    make-string string-empty-p string-concat char-at
    string< string> string<= string>= string-ne
    string-ci= string-ci-ne string-ci< string-ci> string-ci<= string-ci>=
    string-last-index-of string-count
    string-replace-first string-replace-all
    string-trim-left string-trim-right
    string-capitalize string-reverse))

(register-category 'text
  "Explicit String <-> UTF-8 Array<Char> boundary (TEXT module, lib/30-text.lisp)"
  '(text:string->utf8 text:utf8->string text:utf8->string-lossy))

(register-category 'ports
  "Synchronous binary I/O ports (PORTS module, lib/31-ports.lisp)"
  '(ports:open-input ports:open-output ports:open-append
    ports:open-input-bytes ports:open-output-bytes ports:output-contents
    ports:stdin ports:stdout ports:stderr
    ports:read-byte! ports:read-bytes! ports:write-byte! ports:write-bytes!
    ports:flush! ports:close! ports:open-p ports:input-p ports:output-p
    ports:seekable-p ports:position ports:seek! ports:port-p
    ports:name ports:kind
    ports:read-line! ports:read-string! ports:write-string!
    ports:read-all-bytes! ports:with-open-port))

(register-category 'base64
  "Base64 encode/decode over Array<Char> bytes (BASE64 module, lib/32-base64.lisp, issue #257)"
  '(base64:encode base64:decode))

(register-category 'hex
  "Hexadecimal encode/decode over Array<Char> bytes (HEX module, lib/33-hex.lisp, issue #257)"
  '(hex:encode hex:decode))

(register-category 'url
  "URL parse/build, percent-encoding, and query-string parse/build (URL module, lib/34-url.lisp, issue #257)"
  '(url:encode-path-segment url:encode-query-component
    url:decode url:decode-path-segment url:decode-query-component
    url:parse-query url:build-query
    url:parse url:build
    url:scheme url:userinfo url:host url:port url:path url:query url:fragment))

(register-category 'json
  "JSON parse/stringify (JSON module, lib/35-json.lisp, issue #257)"
  '(json:parse json:stringify json:null-p))

(register-category 'mime
  "Case-insensitive multi-value headers and Content-Type parse/build (MIME module, lib/36-mime.lisp, issue #257)"
  '(mime:header-name= mime:headers-get mime:headers-get-all mime:headers-add
    mime:headers-set mime:headers-remove mime:headers-names
    mime:parse-content-type mime:build-content-type mime:content-type-parameter))

(register-category 'net
  "Addresses and DNS resolution (NET module, lib/37-net.lisp, issue #258); NET-DNS capability"
  '(net:address net:address-p net:address-family net:address-ip net:address-port
    net:make-address net:address->string net:resolve net:local-addr net:peer-addr))

(register-category 'tcp
  "TCP connect/bind/listen/accept over binary ports (TCP module, lib/38-tcp.lisp, issue #258); NET-CONNECT/NET-LISTEN capabilities"
  '(tcp:connect tcp:listen tcp:accept tcp:shutdown!
    tcp:set-read-timeout! tcp:set-write-timeout! tcp:close-listener!
    tcp:listener-p tcp:listener-open-p tcp:local-addr tcp:peer-addr))

(register-category 'udp
  "UDP bind/send-to/receive-from datagram sockets (UDP module, lib/39-udp.lisp, issue #258); NET-CONNECT/NET-LISTEN capabilities"
  '(udp:bind udp:connect! udp:send-to udp:send udp:receive-from
    udp:close! udp:socket-p udp:socket-open-p udp:local-addr udp:set-timeout!))

(register-category 'tls
  "TLS client/server wrap of a connected TCP port (TLS module, lib/43-tls.lisp, issue #365); off-by-default net-tls cargo feature, rustls/ring"
  '(tls:available-p tls:connect tls:connect-insecure!
    tls:wrap-client tls:wrap-client-insecure! tls:wrap-server
    tls:alpn-protocol tls:peer-certificates tls:peer-certificate-summary
    tls:sni-hostname))

(register-category 'http
  "HTTP/1.1 client and server (http:// always, https:// with the net-tls cargo feature -- issue #365)"
  '(http:request http:get http:post
    http:response-status http:response-reason http:response-version
    http:response-headers http:response-header http:response-body
    http:request-method http:request-target http:request-path
    http:request-query http:request-headers http:request-header
    http:request-body http:request-version http:request-peer-addr
    http:request-url
    http:stream-read! http:stream-read-all! http:stream-eof-p
    http:stream-close!
    http:collect-bytes http:collect-string http:collect-json
    http:serve http:serve-one! http:respond http:default-reason))

(register-category 'os
  "Process identity/environment, time, randomness, and process spawn/control (OS module, lib/41-os.lisp, issue #260); OS-ENV/OS-ENV-WRITE/OS-PROCESS/OS-SIGNAL capabilities"
  '(os:args os:executable-path os:cwd os:chdir!
    os:env-get os:env-list os:env-set! os:env-unset!
    os:pid os:ppid os:hostname
    os:now os:now-unix os:monotonic-nanos os:elapsed-seconds
    os:sleep os:sleep-seconds
    os:make-prng os:prng-next os:random-double os:random-bytes
    os:spawn os:process-handle os:process-stdin os:process-stdout
    os:process-stderr os:process-wait! os:process-try-wait! os:process-id
    os:process-alive-p os:process-kill! os:process-terminate! os:process-p
    os:exit-code os:exit-signal os:exit-success-p os:signal!))

(register-category 'os-linux
  "Typed Linux file metadata and symlinks (OS-LINUX module, lib/42-os-linux.lisp, issue #260); READ-FS capability"
  '(os-linux:stat os-linux:lstat os-linux:readlink
    os-linux:stat-size os-linux:stat-mode os-linux:stat-uid os-linux:stat-gid
    os-linux:stat-nlink os-linux:stat-ino os-linux:stat-dev
    os-linux:stat-mtime os-linux:stat-atime os-linux:stat-ctime
    os-linux:stat-directory-p os-linux:stat-file-p os-linux:stat-symlink-p))

(register-category 'special-forms
  "Special forms and macros"
  '(quote if cond and or def setq let lambda defun defun* defmacro progn prog
    block return-from catch throw unwind-protect while for
    label define defexpr vau $vau
    macro fexpr flet macrolet fexprlet vaulet))

(register-category 'io
  "Input/Output"
  '(print prin1 princ terpri read load-file spaces
    format read-line with-output-to-string))

(register-category 'errors
  "Error handling"
  '(error errorset make-error error-p error-message error-data handler-case))

(register-category 'meta
  "Metaprogramming"
  '(eval apply funcall help documentation evlis evcon optimize defun-typed-opt macroexpand))

(register-category 'plists
  "Property lists"
  '(getp putp plist remprop documentation get deflist))

(register-category 'hash-tables
  "Hash tables"
  '(make-hash-table gethash set-bang sethash keys delete-key delete-key-bang))

(register-category 'bitwise
  "Bitwise operations"
  '(logor logand logxor lognot leftshift ash rot))

(register-category 'arrays
  "Mutable random-access arrays (Lisp 1.5 Appendix A)"
  '(array make-array fetch array-fetch* store array-store* array-length* arrayp))

(register-category 'filesystem
  "File system I/O (requires READ-FS / CREATE-FS / TEMP-FS capability)"
  '(read-file read-file-byte read-file-section read-file-section-lossy
    read-file-section-bytes write-file
    read-sexpr-file write-sexpr-file
    file-exists-p directory-p file-p file-readable-p file-writable-p
    file-executable-p file-size directory-files file-newer-p
    chmod create-directory delete-file rename-file
    make-temp-file make-temp-directory))

(register-category 'capabilities
  "Capability feature flags and shell access"
  '(feature-enabled-p features shell))

(register-category 'modules
  "REQUIRE/PROVIDE load-once library loading (issue #256)"
  '(require provide require-reload loaded-modules module-state module-info))

(register-category 'environments
  "First-class environment objects"
  '(the-environment make-environment current-environment))

(register-category 'flags
  "Global condition/signal flags"
  '(set-flag clear-flag flag-set-p clear-all-flags))

;;; ============================================================
;;; INTROSPECTION
;;; ============================================================

(register-doc 'describe
  (list
    (cons 'NAME 'describe)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(describe 'sym)")
    (cons 'CATEGORY 'introspection)
    (cons 'DESCRIPTION "Print a brief summary of what a symbol (or value) is: its kind, parameters/arity, value, any typed (JIT) signature and compiled status, and its docstring.")
    (cons 'ARGS '((sym "A (usually quoted) symbol, or any value")))
    (cons 'RETURNS "T (the summary is printed to stdout)")
    (cons 'EXAMPLES '(((describe '+) T)
                       ((describe 'car) T)))
    (cons 'SEE-ALSO '(see-source disassemble documentation help))))

(register-doc 'see-source
  (list
    (cons 'NAME 'see-source)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(see-source 'sym) or (see-source 'sym t)")
    (cons 'CATEGORY 'introspection)
    (cons 'DESCRIPTION "Reconstruct the source form the evaluator registered for an operative (lambda, fexpr, macro, vau). With no second argument it returns the form; with a non-NIL second argument it prints the form as an indented tree and returns T.")
    (cons 'ARGS '((sym "A (usually quoted) symbol bound to an operative, or the operative value itself")
                   (tree "Optional: when non-NIL, render an indented tree to stdout")))
    (cons 'RETURNS "The reconstructed source form, or T in tree mode")
    (cons 'EXAMPLES '(((see-source 'cube) (LAMBDA (X) (* X (* X X))))))
    (cons 'SEE-ALSO '(describe disassemble macroexpand))))

(register-doc 'disassemble
  (list
    (cons 'NAME 'disassemble)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(disassemble 'sym)")
    (cons 'CATEGORY 'introspection)
    (cons 'DESCRIPTION "Print the typed-core pseudo-assembly of a jotted (defun-typed) function: the typed IR lowered to a flat register/label instruction listing. Reports clearly when the symbol has no typed edition.")
    (cons 'ARGS '((sym "A quoted symbol naming a typed (defun-typed) function")))
    (cons 'RETURNS "T (the listing is printed to stdout)")
    (cons 'EXAMPLES '(((disassemble 'fact) T)))
    (cons 'SEE-ALSO '(describe see-source defun-typed))))

(register-category 'introspection
  "Inspecting registered definitions and compiled code"
  '(describe see-source disassemble documentation))

;;; Done loading help data. Keep stdlib loading silent so CLI -s output is
;;; machine-readable and benchmark harnesses can parse stdout directly.

;;; Registered as a module for introspection (issue #56). This is a DATA
;;; module -- it populates the help database via REGISTER-DOC and defines
;;; no public functions of its own -- so its export set is empty; the
;;; DEFMODULE records that `help-data` is a module.
(require 'modules)
(defmodule help-data
  (:export))
(provide 'help-data)
