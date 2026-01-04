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
    (cons 'SYNTAX "(mapcar list function)")
    (cons 'CATEGORY 'lists)
    (cons 'DESCRIPTION "Applies function to each element of list, returns list of results.")
    (cons 'EXAMPLES '(((mapcar '(1 2 3) (lambda (x) (* x 2))) (2 4 6))))
    (cons 'SEE-ALSO '(maplist apply))))

(register-doc 'maplist
  (list
    (cons 'NAME 'maplist)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(maplist list function)")
    (cons 'CATEGORY 'lists)
    (cons 'DESCRIPTION "Applies function to successive tails of list.")
    (cons 'EXAMPLES '(((maplist '(a b c) (lambda (x) (length x))) (3 2 1))))
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
    (cons 'DESCRIPTION "Loads and evaluates a Lisp source file.")
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

(register-doc 'defmacro
  (list
    (cons 'NAME 'defmacro)
    (cons 'TYPE 'special-form)
    (cons 'SYNTAX "(defmacro name (params...) body...)")
    (cons 'CATEGORY 'special-forms)
    (cons 'DESCRIPTION "Defines a macro that transforms code before evaluation.")
    (cons 'SEE-ALSO '(defun defexpr macroexpand))))

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
    (cons 'SEE-ALSO '(get set-bang keys))))

(register-doc 'get
  (list
    (cons 'NAME 'get)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(get hash-table key)")
    (cons 'CATEGORY 'hash-tables)
    (cons 'DESCRIPTION "Retrieves the value for key in hash-table.")
    (cons 'SEE-ALSO '(set-bang keys make-hash-table))))

(register-doc 'set-bang
  (list
    (cons 'NAME 'set-bang)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(set-bang hash-table key value)")
    (cons 'CATEGORY 'hash-tables)
    (cons 'DESCRIPTION "Sets the value for key in hash-table.")
    (cons 'SEE-ALSO '(get delete-key make-hash-table))))

(register-doc 'keys
  (list
    (cons 'NAME 'keys)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(keys hash-table)")
    (cons 'CATEGORY 'hash-tables)
    (cons 'DESCRIPTION "Returns a list of all keys in hash-table.")
    (cons 'SEE-ALSO '(get set-bang make-hash-table))))

;;; ============================================================
;;; BITWISE FUNCTIONS
;;; ============================================================

(register-doc 'logor
  (list
    (cons 'NAME 'logor)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(logor integer...)")
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
;;; REGISTER CATEGORIES
;;; ============================================================

(register-category 'arithmetic
  "Numeric operations"
  '(+ - * / remainder mod expt add1 sub1 abs max min random))

(register-category 'predicates
  "Type and value predicates"
  '(zerop plusp minusp evenp oddp < > = atom symbolp numberp fixp floatp
    stringp consp listp null not eq equal functionp boundp))

(register-category 'lists
  "List manipulation"
  '(car cdr cons list append reverse length nth last member assoc
    mapcar maplist subst pairlis))

(register-category 'strings
  "String operations"
  '(concat index explode implode gensym intern))

(register-category 'special-forms
  "Special forms and macros"
  '(quote if cond and or def setq let lambda defun defmacro progn prog))

(register-category 'io
  "Input/Output"
  '(print prin1 princ terpri read load-file))

(register-category 'errors
  "Error handling"
  '(error errorset))

(register-category 'meta
  "Metaprogramming"
  '(eval apply funcall help documentation))

(register-category 'plists
  "Property lists"
  '(getp putp plist remprop documentation))

(register-category 'hash-tables
  "Hash tables"
  '(make-hash-table get set-bang keys delete-key))

(register-category 'bitwise
  "Bitwise operations"
  '(logor logand logxor lognot leftshift ash))

;;; Done loading help data
(princ "Help system loaded. Type (help) for documentation.")
(terpri)
