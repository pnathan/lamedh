# Bitwise Functions

This chapter documents bitwise (logical) operations on integers in Lamedh.

---

## Overview

Bitwise functions operate on the binary representation of integers:

```lisp
(logand 5 3)        ; 0101 AND 0011 = 0001 = 1
(logor 5 3)         ; 0101 OR  0011 = 0111 = 7
(logxor 5 3)        ; 0101 XOR 0011 = 0110 = 6
```

---

## Logical Operations

### LOGOR

**Syntax:** `(logor integer...)`

Bitwise OR of all arguments.

```lisp
(logor 5 3)         ; => 7   (0101 | 0011 = 0111)
(logor 1 2 4)       ; => 7   (0001 | 0010 | 0100 = 0111)
(logor 0 0)         ; => 0
(logor)             ; => 0   (identity for OR)
```

**Arguments:**
- `integer...` - Zero or more integers

**Returns:** Bitwise OR of all arguments

---

### LOGAND

**Syntax:** `(logand integer...)`

Bitwise AND of all arguments.

```lisp
(logand 5 3)        ; => 1   (0101 & 0011 = 0001)
(logand 15 7 3)     ; => 3   (1111 & 0111 & 0011 = 0011)
(logand 5 0)        ; => 0
(logand)            ; => -1  (all bits set, identity for AND)
```

**Arguments:**
- `integer...` - Zero or more integers

**Returns:** Bitwise AND of all arguments

---

### LOGXOR

**Syntax:** `(logxor integer...)`

Bitwise XOR (exclusive or) of all arguments.

```lisp
(logxor 5 3)        ; => 6   (0101 ^ 0011 = 0110)
(logxor 7 7)        ; => 0   (any ^ itself = 0)
(logxor 5 3 6)      ; => 0   (associative)
(logxor)            ; => 0   (identity for XOR)
```

**Arguments:**
- `integer...` - Zero or more integers

**Returns:** Bitwise XOR of all arguments

---

### LOGNOT

**Syntax:** `(lognot integer)`

Bitwise complement (NOT). Flips all bits.

```lisp
(lognot 0)          ; => -1
(lognot -1)         ; => 0
(lognot 5)          ; => -6  (in two's complement)
```

**Arguments:**
- `integer` - An integer

**Returns:** Bitwise complement

**Note:** For a 64-bit integer, `(lognot x)` equals `(- (+ x 1))` or `-x - 1`.

---

## Shift Operations

### LEFTSHIFT

**Syntax:** `(leftshift n count)`

Shifts bits left or right.

```lisp
;; Left shift (positive count)
(leftshift 1 0)     ; => 1   (no shift)
(leftshift 1 1)     ; => 2   (0001 << 1 = 0010)
(leftshift 1 2)     ; => 4   (0001 << 2 = 0100)
(leftshift 1 3)     ; => 8   (0001 << 3 = 1000)
(leftshift 5 2)     ; => 20  (0101 << 2 = 10100)

;; Right shift (negative count)
(leftshift 8 -1)    ; => 4   (1000 >> 1 = 0100)
(leftshift 8 -2)    ; => 2   (1000 >> 2 = 0010)
(leftshift 8 -3)    ; => 1   (1000 >> 3 = 0001)
```

**Arguments:**
- `n` - Integer to shift
- `count` - Shift amount (positive = left, negative = right)

**Returns:** Shifted value

**Errors:** If shift amount >= 64 or <= -64

---

### ASH (Arithmetic Shift)

**Syntax:** `(ash integer count)`

Arithmetic shift. Preserves sign on right shift.

```lisp
(ash 8 1)           ; => 16  (left shift)
(ash 8 -1)          ; => 4   (right shift)
(ash -8 -1)         ; => -4  (sign preserved)
```

**Arguments:**
- `integer` - Integer to shift
- `count` - Shift amount

**Note:** For positive integers, same as LEFTSHIFT.

---

### ROT (Rotate)

**Syntax:** `(rot integer count &optional width)`

Rotates bits. Bits shifted out one end come in the other.

```lisp
(rot 1 1)           ; Rotate left by 1
(rot 128 -1)        ; Rotate right by 1
```

**Note:** Default width is 64 bits.

---

## Common Patterns

### Testing a Bit

```lisp
(defun bit-set-p (n bit)
  "Test if BIT is set in N."
  (not (zerop (logand n (leftshift 1 bit)))))

(bit-set-p 5 0)     ; => T   (bit 0 set in 0101)
(bit-set-p 5 1)     ; => NIL (bit 1 not set)
(bit-set-p 5 2)     ; => T   (bit 2 set)
```

### Setting a Bit

```lisp
(defun set-bit (n bit)
  "Set BIT in N."
  (logor n (leftshift 1 bit)))

(set-bit 4 0)       ; => 5   (0100 | 0001 = 0101)
```

### Clearing a Bit

```lisp
(defun clear-bit (n bit)
  "Clear BIT in N."
  (logand n (lognot (leftshift 1 bit))))

(clear-bit 5 2)     ; => 1   (0101 & 1011 = 0001)
```

### Toggling a Bit

```lisp
(defun toggle-bit (n bit)
  "Toggle BIT in N."
  (logxor n (leftshift 1 bit)))

(toggle-bit 5 0)    ; => 4   (0101 ^ 0001 = 0100)
(toggle-bit 4 0)    ; => 5   (0100 ^ 0001 = 0101)
```

### Extracting Bits

```lisp
(defun extract-bits (n start count)
  "Extract COUNT bits from N starting at START."
  (logand (leftshift n (- start))
          (- (leftshift 1 count) 1)))

(extract-bits 255 4 4)  ; => 15 (upper nibble of 11111111)
```

---

## Bit Counting

```lisp
(defun popcount (n)
  "Count set bits in N (for non-negative N)."
  (if (zerop n)
      0
      (+ (logand n 1)
         (popcount (leftshift n -1)))))

(popcount 5)        ; => 2   (0101 has 2 bits)
(popcount 7)        ; => 3   (0111 has 3 bits)
(popcount 255)      ; => 8
```

---

## Flags and Masks

```lisp
;; Define flag positions
(def FLAG-READ    (leftshift 1 0))  ; 1
(def FLAG-WRITE   (leftshift 1 1))  ; 2
(def FLAG-EXECUTE (leftshift 1 2))  ; 4

;; Combine flags
(def RWX (logor FLAG-READ FLAG-WRITE FLAG-EXECUTE))  ; 7

;; Check flags
(defun has-flag (flags flag)
  (not (zerop (logand flags flag))))

(has-flag RWX FLAG-READ)     ; => T
(has-flag FLAG-READ FLAG-WRITE)  ; => NIL
```

---

## Truth Tables

### LOGAND
```
A B | A AND B
0 0 |    0
0 1 |    0
1 0 |    0
1 1 |    1
```

### LOGOR
```
A B | A OR B
0 0 |    0
0 1 |    1
1 0 |    1
1 1 |    1
```

### LOGXOR
```
A B | A XOR B
0 0 |    0
0 1 |    1
1 0 |    1
1 1 |    0
```

### LOGNOT
```
A | NOT A
0 |   1
1 |   0
```

---

## Integer Representation

Lamedh uses 64-bit signed integers (two's complement):

- Range: -9,223,372,036,854,775,808 to 9,223,372,036,854,775,807
- Negative numbers have high bit set
- `(lognot 0)` = -1 (all bits set)

```lisp
;; Two's complement examples
(lognot 0)          ; => -1
(lognot -1)         ; => 0
(logand -1 255)     ; => 255 (mask lower 8 bits)
```

---

**See Also:**
- [Arithmetic Functions](arithmetic.md)
- [Data Types - Numbers](../data_types.md#32-numbers)
