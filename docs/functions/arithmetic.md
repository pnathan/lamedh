# Arithmetic Functions

This chapter documents all arithmetic and numeric functions in Lamedh.

---

## +  (PLUS)

**Syntax:** `(+ number...)`

Returns the sum of all arguments. With no arguments, returns 0.

```lisp
(+)             ; => 0
(+ 1)           ; => 1
(+ 1 2)         ; => 3
(+ 1 2 3 4 5)   ; => 15
(+ 1.5 2.5)     ; => 4.0
```

**Arguments:**
- `number...` - Zero or more numbers (integers or floats)

**Returns:** Sum of arguments (float if any argument is float)

**Errors:** If any argument is not a number

---

## -  (MINUS, DIFFERENCE)

**Syntax:** `(- number)` or `(- number number...)`

With one argument, returns negation. With multiple, subtracts rest from first.

```lisp
(- 5)           ; => -5 (negation)
(- 10 3)        ; => 7
(- 10 3 2)      ; => 5 (10 - 3 - 2)
(- 5.0 1.5)     ; => 3.5
```

**Arguments:**
- `number` - One or more numbers

**Returns:** Difference (float if any argument is float)

**Errors:** If no arguments or if any argument is not a number

---

## *  (TIMES)

**Syntax:** `(* number...)`

Returns the product of all arguments. With no arguments, returns 1.

```lisp
(*)             ; => 1
(* 2)           ; => 2
(* 2 3)         ; => 6
(* 2 3 4)       ; => 24
(* 2.0 3)       ; => 6.0
```

**Arguments:**
- `number...` - Zero or more numbers

**Returns:** Product of arguments

---

## /  (QUOTIENT, DIVIDE)

**Syntax:** `(/ dividend divisor)`

Returns the quotient of two numbers. Integer division truncates.

```lisp
(/ 10 2)        ; => 5
(/ 10 3)        ; => 3 (truncated)
(/ 10.0 3)      ; => 3.333...
(/ 7 2)         ; => 3
```

**Arguments:**
- `dividend` - Number to divide
- `divisor` - Number to divide by

**Returns:** Quotient

**Errors:**
- Division by zero
- Overflow (i64::MIN / -1)

---

## REMAINDER

**Syntax:** `(remainder dividend divisor)`

Returns the remainder of integer division.

```lisp
(remainder 10 3)   ; => 1
(remainder 10 5)   ; => 0
(remainder -10 3)  ; => -1
(remainder 10 -3)  ; => 1
```

**Arguments:**
- `dividend` - Number
- `divisor` - Number (non-zero)

**Returns:** Integer remainder

**Errors:** Division by zero

---

## MOD

**Syntax:** `(mod x y)`

Returns x modulo y. Unlike REMAINDER, result has same sign as divisor.

```lisp
(mod 10 3)      ; => 1
(mod -10 3)     ; => 2
(mod 10 -3)     ; => -2
```

**Arguments:**
- `x` - Number
- `y` - Number (non-zero)

**Returns:** Modulo result

---

## EXPT

**Syntax:** `(expt base power)`

Returns base raised to the power.

```lisp
(expt 2 10)     ; => 1024
(expt 2 0)      ; => 1
(expt 3 3)      ; => 27
(expt 2.0 0.5)  ; => 1.414... (square root)
```

**Arguments:**
- `base` - Number
- `power` - Integer (for integers) or any number (for floats)

**Returns:** base^power

**Errors:** Overflow for large results

---

## ADD1 (1+)

**Syntax:** `(add1 n)` or `(1+ n)`

Returns n + 1.

```lisp
(add1 5)        ; => 6
(1+ 10)         ; => 11
(add1 -1)       ; => 0
```

---

## SUB1 (1-)

**Syntax:** `(sub1 n)` or `(1- n)`

Returns n - 1.

```lisp
(sub1 5)        ; => 4
(1- 10)         ; => 9
(sub1 0)        ; => -1
```

---

## ABS

**Syntax:** `(abs n)`

Returns the absolute value of n. (Standard library function)

```lisp
(abs 5)         ; => 5
(abs -5)        ; => 5
(abs 0)         ; => 0
(abs -3.14)     ; => 3.14
```

---

## MAX

**Syntax:** `(max number number...)`

Returns the largest argument. (Standard library function)

```lisp
(max 1 2 3)     ; => 3
(max 5)         ; => 5
(max -1 -2 -3)  ; => -1
```

**Errors:** If called with no arguments

---

## MIN

**Syntax:** `(min number number...)`

Returns the smallest argument. (Standard library function)

```lisp
(min 1 2 3)     ; => 1
(min 5)         ; => 5
(min -1 -2 -3)  ; => -3
```

**Errors:** If called with no arguments

---

## RANDOM

**Syntax:** `(random n)`

Returns a random integer from 0 (inclusive) to n (exclusive).

```lisp
(random 10)     ; => 0-9 randomly
(random 100)    ; => 0-99 randomly
```

**Arguments:**
- `n` - Positive integer upper bound

**Returns:** Random integer in [0, n)

---

## Comparison Functions

### <  (LESSP)

**Syntax:** `(< a b)`

Returns T if a is less than b.

```lisp
(< 1 2)         ; => T
(< 2 1)         ; => NIL
(< 1.0 2.0)     ; => T
```

### >  (GREATERP)

**Syntax:** `(> a b)`

Returns T if a is greater than b.

```lisp
(> 2 1)         ; => T
(> 1 2)         ; => NIL
```

### =

**Syntax:** `(= a b)`

Returns T if a equals b numerically.

```lisp
(= 1 1)         ; => T
(= 1 2)         ; => NIL
(= 1 1.0)       ; => T (numeric equality)
```

---

## Numeric Predicates

### ZEROP

**Syntax:** `(zerop n)`

Returns T if n is zero.

```lisp
(zerop 0)       ; => T
(zerop 1)       ; => NIL
(zerop 0.0)     ; => T
```

### PLUSP

**Syntax:** `(plusp n)`

Returns T if n is positive (greater than zero).

```lisp
(plusp 1)       ; => T
(plusp 0)       ; => NIL
(plusp -1)      ; => NIL
```

### MINUSP

**Syntax:** `(minusp n)`

Returns T if n is negative (less than zero). (Standard library function)

```lisp
(minusp -1)     ; => T
(minusp 0)      ; => NIL
(minusp 1)      ; => NIL
```

### ONEP

**Syntax:** `(onep n)`

Returns T if n equals 1. (Standard library function)

```lisp
(onep 1)        ; => T
(onep 2)        ; => NIL
```

### EVENP

**Syntax:** `(evenp n)`

Returns T if n is an even integer.

```lisp
(evenp 2)       ; => T
(evenp 3)       ; => NIL
(evenp 0)       ; => T
```

### ODDP

**Syntax:** `(oddp n)`

Returns T if n is an odd integer.

```lisp
(oddp 3)        ; => T
(oddp 2)        ; => NIL
```

---

## Type Predicates

### NUMBERP

**Syntax:** `(numberp x)`

Returns T if x is a number (integer or float).

```lisp
(numberp 42)    ; => T
(numberp 3.14)  ; => T
(numberp "42")  ; => NIL
```

### FIXP

**Syntax:** `(fixp x)`

Returns T if x is a fixed-point (integer) number.

```lisp
(fixp 42)       ; => T
(fixp 3.14)     ; => NIL
```

### FLOATP

**Syntax:** `(floatp x)`

Returns T if x is a floating-point number.

```lisp
(floatp 3.14)   ; => T
(floatp 42)     ; => NIL
```

---

## Float Comparison

### FLOAT-EQUAL

**Syntax:** `(float-equal a b)`

Returns T if floats a and b are equal.

```lisp
(float-equal 1.0 1.0)   ; => T
(float-equal 1.0 1.1)   ; => NIL
```

### FLOAT-LESSP

**Syntax:** `(float-lessp a b)`

Returns T if float a is less than float b.

### FLOAT-GREATERP

**Syntax:** `(float-greaterp a b)`

Returns T if float a is greater than float b.

---

## Mixed Arithmetic

When integers and floats are mixed, the result is a float:

```lisp
(+ 1 2.0)       ; => 3.0
(* 3 1.5)       ; => 4.5
(/ 5 2.0)       ; => 2.5
```

---

**See Also:**
- [Bitwise Functions](bitwise.md) for bit manipulation
- [Predicates](predicates.md) for type checking
