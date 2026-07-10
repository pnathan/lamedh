;;; Help Data - Documentation for Lamedh functions and special forms
;;; This file populates the help database with documentation entries

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
    (cons 'SEE-ALSO '(gethash sethash delete-key make-hash-table))))

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

(register-doc 'string-length
  (list
    (cons 'NAME 'string-length)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(string-length s)")
    (cons 'CATEGORY 'strings)
    (cons 'DESCRIPTION "Returns the number of Unicode characters in string s (not bytes). This is the kernel primitive; the Lisp layer builds higher-level string operations on top of it.")
    (cons 'EXAMPLES '(((string-length "hello") 5)
                       ((string-length "") 0)))
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
    (cons 'SEE-ALSO '(string-length index concat))))

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
    (cons 'SEE-ALSO '(code-char make-char charp string-length))))

(register-doc 'code-char
  (list
    (cons 'NAME 'code-char)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(code-char n)")
    (cons 'CATEGORY 'strings)
    (cons 'DESCRIPTION "Returns a one-character string containing the character at code point n. The inverse of CHAR-CODE. Signals an error if n is not a valid code point. (Use MAKE-CHAR to build a Char value instead of a string.)")
    (cons 'EXAMPLES '(((code-char 65) "A")
                       ((code-char 97) "a")))
    (cons 'SEE-ALSO '(char-code make-char string-length))))

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
    (cons 'DESCRIPTION "Creates and returns a new mutable array of n elements, all initialised to NIL. Lisp 1.5 Appendix A name; MAKE-ARRAY is the longer alias. Arrays are random-access containers with O(1) indexed get/set. Use FETCH/STORE to access elements, ARRAY-LENGTH to query the size.")
    (cons 'EXAMPLES '(((let ((a (array 3))) (store a 0 'x) (fetch a 0)) x)))
    (cons 'SEE-ALSO '(make-array fetch store array-length arrayp))))

(register-doc 'make-array
  (list
    (cons 'NAME 'make-array)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(make-array n)")
    (cons 'CATEGORY 'arrays)
    (cons 'DESCRIPTION "Alias for ARRAY. Creates a mutable array of n NIL-initialised elements. See ARRAY for full documentation.")
    (cons 'EXAMPLES '(((make-array 5) "an array of 5 NILs")))
    (cons 'SEE-ALSO '(array fetch store array-length arrayp))))

(register-doc 'fetch
  (list
    (cons 'NAME 'fetch)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(fetch array index)")
    (cons 'CATEGORY 'arrays)
    (cons 'DESCRIPTION "Returns the element of array at 0-based integer index. Signals an error if index is out of bounds. Lisp 1.5 Appendix A name; ARRAY-FETCH is the longer alias, AREF the Common-Lisp-style one.")
    (cons 'EXAMPLES '(((let ((a (array 3))) (store a 1 'hello) (fetch a 1)) hello)))
    (cons 'SEE-ALSO '(array-fetch aref store array array-length))))

(register-doc 'array-fetch
  (list
    (cons 'NAME 'array-fetch)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(array-fetch array index)")
    (cons 'CATEGORY 'arrays)
    (cons 'DESCRIPTION "Alias for FETCH. Returns the element of array at 0-based index. See FETCH for full documentation.")
    (cons 'SEE-ALSO '(fetch store array-store array-length))))

(register-doc 'aref
  (list
    (cons 'NAME 'aref)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(aref array index)")
    (cons 'CATEGORY 'arrays)
    (cons 'DESCRIPTION "Common-Lisp-style alias for FETCH. Returns the element of array at 0-based index. See FETCH for full documentation.")
    (cons 'SEE-ALSO '(fetch array-fetch aset store array-length))))

(register-doc 'store
  (list
    (cons 'NAME 'store)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(store array index value)")
    (cons 'CATEGORY 'arrays)
    (cons 'DESCRIPTION "Destructively sets the element of array at 0-based index to value. Returns the stored value. Signals an error if index is out of bounds. Lisp 1.5 Appendix A name; ARRAY-STORE is the longer alias, ASET the Common-Lisp-style one. Mutation is in-place: all references to the same array see the change, including inside a defun-typed body (issue #216). Two scoped exceptions: an array nested inside another array or a struct does not write back through the outer object (only top-level flat arrays of scalars do); and passing the same array as two distinct arguments to one defun-typed call is last-writer-wins in argument order, not simultaneous true aliasing.")
    (cons 'EXAMPLES '(((let ((a (array 3))) (store a 0 99) (fetch a 0)) 99)))
    (cons 'SEE-ALSO '(array-store aset fetch array array-length))))

(register-doc 'array-store
  (list
    (cons 'NAME 'array-store)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(array-store array index value)")
    (cons 'CATEGORY 'arrays)
    (cons 'DESCRIPTION "Alias for STORE. Destructively sets the element at index. See STORE for full documentation.")
    (cons 'SEE-ALSO '(store fetch array-fetch array-length))))

(register-doc 'aset
  (list
    (cons 'NAME 'aset)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(aset array index value)")
    (cons 'CATEGORY 'arrays)
    (cons 'DESCRIPTION "Common-Lisp-style alias for STORE. Destructively sets the element at index. See STORE for full documentation.")
    (cons 'SEE-ALSO '(store array-store aref fetch array-length))))

(register-doc 'array-length
  (list
    (cons 'NAME 'array-length)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(array-length array)")
    (cons 'CATEGORY 'arrays)
    (cons 'DESCRIPTION "Returns the number of elements in array as an integer. The valid index range is 0 to (array-length array) - 1.")
    (cons 'EXAMPLES '(((array-length (array 5)) 5)
                       ((array-length (array 0)) 0)))
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
    (cons 'SEE-ALSO '(array array-length extension-p))))

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
    (cons 'DESCRIPTION "Reads up to len bytes starting at byte offset from the file at path. Returns the bytes as a string (lossily decoded from UTF-8; non-UTF-8 bytes become replacement characters). Returns a shorter string if fewer than len bytes are available. Requires the READ-FS capability.")
    (cons 'SEE-ALSO '(read-file read-file-byte write-file feature-enabled-p))))

(register-doc 'write-file
  (list
    (cons 'NAME 'write-file)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(write-file path content)")
    (cons 'CATEGORY 'filesystem)
    (cons 'DESCRIPTION "Writes the string content to the file at path, replacing any existing content. Creates the file if it does not exist. Returns T on success; signals an error on failure. Requires the CREATE-FS capability. For appending or streaming writes, use the SHELL capability with shell tools.")
    (cons 'EXAMPLES '(((write-file "/tmp/hello.txt" "hello world\n") t)))
    (cons 'SEE-ALSO '(read-file make-temp-file feature-enabled-p))))

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
    string-length substring char-code code-char make-char
    string->number number->string prin1-to-string princ-to-string))

(register-category 'special-forms
  "Special forms and macros"
  '(quote if cond and or def setq let lambda defun defun* defmacro progn prog
    block return-from catch throw unwind-protect while for
    label define defexpr vau $vau
    macro fexpr flet macrolet fexprlet vaulet))

(register-category 'io
  "Input/Output"
  '(print prin1 princ terpri read load-file spaces))

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
  '(array make-array fetch array-fetch store array-store array-length arrayp))

(register-category 'filesystem
  "File system I/O (requires READ-FS / CREATE-FS / TEMP-FS capability)"
  '(read-file read-file-byte read-file-section write-file
    file-exists-p directory-p file-p file-readable-p file-writable-p
    file-executable-p file-size directory-files file-newer-p
    chmod create-directory delete-file rename-file
    make-temp-file make-temp-directory))

(register-category 'capabilities
  "Capability feature flags and shell access"
  '(feature-enabled-p features shell))

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
