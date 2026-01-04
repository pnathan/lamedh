;;; Documentation for builtin functions

;; List Operations
(putp 'list "docstring" "Constructs a list from its arguments.")
(putp 'last "docstring" "Returns the last cons cell of a list.")
(putp 'nth "docstring" "Returns the nth element of a list.")
(putp 'nthcdr "docstring" "Returns the list after n CDRs.")
(putp 'efface "docstring" "Removes first occurrence of item from list.")
(putp 'delete "docstring" "Alias for EFFACE.")

;; Numeric Functions
(putp 'mod "docstring" "Returns x modulo y.")
(putp 'plusp "docstring" "Returns T if number is positive.")
(putp 'evenp "docstring" "Returns T if integer is even.")
(putp 'oddp "docstring" "Returns T if integer is odd.")
(putp 'add1 "docstring" "Returns n + 1.")
(putp 'sub1 "docstring" "Returns n - 1.")
(putp 'random "docstring" "Returns random integer in [0, n).")

;; Type Predicates
(putp 'symbolp "docstring" "Returns T if argument is a symbol.")
(putp 'boundp "docstring" "Returns T if symbol has a value binding.")
(putp 'functionp "docstring" "Returns T if argument is a function.")
(putp 'macrop "docstring" "Returns T if argument is a macro.")

;; Function Operations
(putp 'funcall "docstring" "Calls function with given arguments.")
(putp 'macroexpand "docstring" "Expands a macro form.")

;; String/Symbol Functions
(putp 'explode "docstring" "Converts atom to list of character symbols.")
(putp 'implode "docstring" "Converts list of chars to interned symbol.")
(putp 'maknam "docstring" "Same as IMPLODE.")
(putp 'gensym "docstring" "Generates a unique uninterned symbol.")
(putp 'intern "docstring" "Interns a string as a symbol.")
(putp 'plist "docstring" "Returns the property list of a symbol.")

;; Bitwise Operations
(putp 'ash "docstring" "Arithmetic shift.")
(putp 'lognot "docstring" "Bitwise complement.")
(putp 'rot "docstring" "Rotate bits.")

;; PUT alias
(putp 'put "docstring" "Alias for PUTP.")
