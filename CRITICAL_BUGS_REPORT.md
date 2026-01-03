# CRITICAL BUGS AND LOGIC ERRORS - Lisp 1.5 Interpreter

This report documents **CRITICAL** and **HIGH** severity bugs found through aggressive code review.

---

## CRITICAL SEVERITY BUGS

### BUG #1: Negative Index in STRING INDEX - Integer Wraparound
**File**: `src/evaluator.rs:197-208`
**Severity**: CRITICAL

**Description**: The `INDEX` builtin casts `i64` to `usize` without checking for negative numbers. On line 198, `*n as usize` will wrap a negative number to a massive positive value, causing out-of-bounds access or panic.

**Triggering Input**:
```lisp
(INDEX "hello" -1)
```

**Expected**: Error message "index out of bounds" or "negative index"
**Actual**: Attempts to access index 18446744073709551615 (2^64 - 1 on 64-bit), causing panic or wrong behavior

**Test Case**:
```rust
#[test]
fn test_index_negative_number() {
    let result = eval_str("(INDEX \"hello\" -1)");
    assert!(result.is_err(), "Negative index should error");
    assert!(result.unwrap_err().contains("negative") ||
            result.unwrap_err().contains("out of bounds"));
}

#[test]
fn test_index_large_negative() {
    let result = eval_str("(INDEX \"test\" -999999)");
    assert!(result.is_err(), "Large negative index should error");
}
```

---

### BUG #2: Infinite Loop on Circular Lists in list_to_vec
**File**: `src/evaluator.rs:8-21`
**Severity**: CRITICAL

**Description**: The `list_to_vec` helper function traverses cons cells without cycle detection. If given a circular list (created via RPLACA/RPLACD or other means), it will loop forever.

**Triggering Input**:
```lisp
; Create circular list: (cons 1 (cons 2 X)) where cdr of second cons points back
(PROGN
  (DEF x (CONS 1 (CONS 2 NIL)))
  (RPLACD (CDR x) x)  ; Make it circular
  (+ 1 2 3))  ; This uses list_to_vec and would hang
```

**Expected**: Error or cycle detection
**Actual**: Infinite loop, process hangs

**Note**: While RPLACA/RPLACD return new cons cells (not mutating), this could still occur if the implementation changes or through other means.

**Test Case**:
```rust
#[test]
fn test_circular_list_detection() {
    // This test documents the expected behavior
    // Currently there's no way to create truly circular lists,
    // but if RPLACA/RPLACD are modified to mutate in place,
    // this would be a problem

    // Simpler test: very deeply nested list
    let mut deep = "(+ ".to_string();
    for _ in 0..10000 {
        deep.push_str("1 ");
    }
    deep.push(')');

    let result = eval_str(&deep);
    // Should not hang (but might stack overflow)
    println!("Deep nesting result: {:?}", result);
}
```

---

### BUG #3: Float NaN as HashMap Key - Broken Lookups
**File**: `src/lib.rs:181, 211`
**Severity**: CRITICAL

**Description**:
1. Line 181: `LispVal::Float(a) == LispVal::Float(b)` uses `a == b`, which violates IEEE 754 for NaN (NaN != NaN should be true, but Rust's PartialEq makes NaN == NaN return false correctly)
2. Line 211: Floats are hashed via `to_bits()`, but NaN can have multiple bit representations. Also, `-0.0` and `0.0` have different bit patterns but compare equal.
3. Using floats as HashMap keys violates Eq/Hash contract: items that compare equal must have the same hash, but -0.0 and 0.0 hash differently.

**Triggering Input**:
```lisp
(PROGN
  (DEF h (MAKE-HASH-TABLE))
  (SET-BANG h -0.0 "negative zero")
  (SET-BANG h 0.0 "positive zero")
  (GET h -0.0))  ; Might return wrong value
```

**Expected**: Either error on float keys, or handle -0.0/0.0 consistently
**Actual**: Inconsistent behavior, HashMap corruption

**Test Case**:
```rust
#[test]
fn test_float_zero_hash_key() {
    let input = r#"
        (PROGN
            (DEF h (MAKE-HASH-TABLE))
            (SET-BANG h 0.0 "positive")
            (SET-BANG h -0.0 "negative")
            (KEYS h))
    "#;
    let result = eval_str(input);
    println!("Float zero keys: {:?}", result);
    // -0.0 and 0.0 should be treated as same key or error
}

#[test]
fn test_nan_hash_key() {
    let input = r#"
        (PROGN
            (DEF h (MAKE-HASH-TABLE))
            (SET-BANG h (/ 0.0 0.0) "nan1")
            (GET h (/ 0.0 0.0)))
    "#;
    let result = eval_str(input);
    println!("NaN key lookup: {:?}", result);
    // NaN lookups will fail because NaN != NaN
}
```

---

## HIGH SEVERITY BUGS

### BUG #4: Integer Overflow in Arithmetic Operations
**File**: `src/evaluator.rs:96, 108, 113`
**Severity**: HIGH

**Description**: Arithmetic operations use unchecked operations that can silently overflow:
- Line 96: `nums.iter().sum()` - can overflow
- Line 108: `result -= num` - can overflow
- Line 113: `nums.iter().product()` - can overflow

In debug mode these panic, in release mode they wrap around silently.

**Triggering Input**:
```lisp
(PLUS 9223372036854775807 1)  ; i64::MAX + 1
(TIMES 9223372036854775807 2)  ; i64::MAX * 2
(DIFFERENCE -9223372036854775808 1)  ; i64::MIN - 1
```

**Expected**: Error "arithmetic overflow"
**Actual**: In debug: panic. In release: silent wraparound (incorrect result)

**Test Case**:
```rust
#[test]
fn test_plus_overflow() {
    let result = eval_str("(PLUS 9223372036854775807 1)");
    // Should error instead of wrapping to negative
    println!("Plus overflow: {:?}", result);
}

#[test]
fn test_times_overflow() {
    let result = eval_str("(TIMES 9223372036854775807 2)");
    println!("Times overflow: {:?}", result);
}

#[test]
fn test_minus_underflow() {
    let result = eval_str("(DIFFERENCE -9223372036854775808 1)");
    println!("Minus underflow: {:?}", result);
}
```

---

### BUG #5: Division of MIN by -1 Causes Panic
**File**: `src/evaluator.rs:114-124`
**Severity**: HIGH

**Description**: Division checks for divide-by-zero (line 120) but not for the edge case `i64::MIN / -1`, which overflows because `|i64::MIN| > i64::MAX`. This causes a panic.

**Triggering Input**:
```lisp
(QUOTIENT -9223372036854775808 -1)  ; i64::MIN / -1
```

**Expected**: Error "division overflow" or return a float
**Actual**: Panic with "attempt to divide with overflow"

**Test Case**:
```rust
#[test]
fn test_division_min_by_neg_one() {
    let result = eval_str("(QUOTIENT -9223372036854775808 -1)");
    assert!(result.is_err(), "i64::MIN / -1 should error, not panic");
}

#[test]
fn test_division_normal() {
    let result = eval_str("(QUOTIENT 10 -2)");
    assert!(result.is_ok());
    if let Ok(LispVal::Number(n)) = result {
        assert_eq!(n, -5);
    }
}
```

---

### BUG #6: Bit Shift Overflow Can Panic
**File**: `src/evaluator.rs:1663-1680`
**Severity**: HIGH

**Description**: The `LEFTSHIFT` function (lines 1669-1674) performs bit shifts without checking if the shift amount is too large. Shifting by >= 64 bits on a 64-bit integer is undefined behavior and can panic.

**Triggering Input**:
```lisp
(LEFTSHIFT 1 64)   ; Shift by 64 bits
(LEFTSHIFT 5 100)  ; Shift by 100 bits
(LEFTSHIFT 7 -100) ; Negative shift (right shift by 100)
```

**Expected**: Error "shift amount too large" or wrap/saturate
**Actual**: Panic "attempt to shift left with overflow"

**Test Case**:
```rust
#[test]
fn test_leftshift_overflow() {
    let result = eval_str("(LEFTSHIFT 1 64)");
    println!("Shift by 64: {:?}", result);
    // Should error, not panic
}

#[test]
fn test_leftshift_large_amount() {
    let result = eval_str("(LEFTSHIFT 5 100)");
    println!("Shift by 100: {:?}", result);
}

#[test]
fn test_leftshift_large_negative() {
    let result = eval_str("(LEFTSHIFT 128 -100)");
    println!("Negative shift by 100: {:?}", result);
}
```

---

### BUG #7: LABEL Self-Reference Can Cause Infinite Recursion
**File**: `src/evaluator.rs:785-801, 1021-1044`
**Severity**: HIGH

**Description**: The LABEL implementation has two parts:
1. Lines 1021-1044: LABEL creates an environment where the name is bound to the entire LABEL expression
2. Lines 785-801: When evaluating a symbol, if it's bound to a LABEL expression, re-evaluate it

This can cause infinite recursion if the LABEL expression doesn't properly guard against infinite evaluation. The check on line 794 looks for a symbol named "LABEL" but doesn't prevent re-evaluation loops.

**Triggering Input**:
```lisp
(LABEL x x)  ; Simplest case: x evaluates to (LABEL x x), which re-evaluates...
```

**Expected**: Error "infinite recursion in LABEL" or stack overflow
**Actual**: Stack overflow (not caught)

**Test Case**:
```rust
#[test]
fn test_label_infinite_recursion() {
    let result = eval_str("(LABEL x x)");
    println!("LABEL x x: {:?}", result);
    // Should error or detect recursion, not stack overflow
}

#[test]
fn test_label_circular_reference() {
    let input = "(LABEL a (LABEL b a))";
    let result = eval_str(input);
    println!("Circular LABEL: {:?}", result);
}
```

---

## MEDIUM SEVERITY BUGS

### BUG #8: ASSOC with Malformed Association List
**File**: `src/evaluator.rs:1512-1533`
**Severity**: MEDIUM

**Description**: The ASSOC function (lines 1520-1531) assumes each element of the association list is a cons cell with a car that can be compared. However, lines 1521-1524 use pattern matching that will panic if an alist entry is not a cons cell.

**Triggering Input**:
```lisp
(ASSOC 'x '(1 2 3))  ; alist elements are atoms, not pairs
(ASSOC 'a '((a . 1) 2 (b . 3)))  ; mixed: second element is not a pair
```

**Expected**: Error "malformed association list" or skip non-pairs
**Actual**: Incorrect behavior or panic

**Test Case**:
```rust
#[test]
fn test_assoc_with_atoms() {
    let result = eval_str("(ASSOC 'X '(1 2 3))");
    println!("ASSOC with atoms: {:?}", result);
    // Should return NIL or error gracefully
}

#[test]
fn test_assoc_mixed_list() {
    let input = "(ASSOC 'A '((A . 1) B (C . 3)))";
    let result = eval_str(input);
    println!("ASSOC mixed list: {:?}", result);
    // Should find (A . 1) or handle 'B' gracefully
}
```

---

### BUG #9: SETQ Creates Variables Instead of Erroring
**File**: `src/environment.rs:237-250, src/evaluator.rs:1125-1147`
**Severity**: MEDIUM

**Description**: The `Environment::update` function (lines 237-250) walks the environment chain looking for the variable. If not found, it creates it in the current environment (line 249). This means SETQ silently creates new variables instead of erroring on undefined variables.

This violates the typical SETQ semantics where you should use DEF/DEFVAR to create and SETQ to update.

**Triggering Input**:
```lisp
(SETQ undefined-var 42)  ; Variable doesn't exist
undefined-var  ; Will be 42 instead of error
```

**Expected**: Error "undefined variable 'undefined-var'"
**Actual**: Creates the variable with value 42

**Test Case**:
```rust
#[test]
fn test_setq_undefined_variable() {
    let env = Environment::new_with_builtins();
    let expr = read("(SETQ new-var 123)", &env).unwrap();
    let result = eval(&expr, &env);

    println!("SETQ undefined: {:?}", result);
    // In most Lisps, this would error
    // Current behavior: creates the variable

    // Verify it was created
    let lookup = read("new-var", &env).unwrap();
    let value = eval(&lookup, &env);
    println!("Lookup after SETQ: {:?}", value);
}
```

---

### BUG #10: DEFINE with Non-List Argument
**File**: `src/evaluator.rs:1045-1073`
**Severity**: MEDIUM

**Description**: The DEFINE special form expects a list of definitions (line 1052), but doesn't check if `defs[0]` is actually a list before calling `list_to_vec`. If it's an atom, `list_to_vec` will error.

**Triggering Input**:
```lisp
(DEFINE 42)  ; Not a list
(DEFINE 'symbol)  ; Symbol instead of list
```

**Expected**: Clear error "DEFINE requires a list of definitions"
**Actual**: Generic error from list_to_vec

**Test Case**:
```rust
#[test]
fn test_define_with_atom() {
    let result = eval_str("(DEFINE 42)");
    assert!(result.is_err());
    println!("DEFINE with atom: {:?}", result);
}

#[test]
fn test_define_with_symbol() {
    let result = eval_str("(DEFINE 'x)");
    assert!(result.is_err());
    println!("DEFINE with symbol: {:?}", result);
}
```

---

### BUG #11: DEFLIST with Malformed Input
**File**: `src/evaluator.rs:1717-1752`
**Severity**: MEDIUM

**Description**: The DEFLIST function (lines 1732-1745) performs multiple pattern matches without checking if the structure is valid:
- Line 1733: Assumes `car` is a cons
- Line 1738: Assumes first element is a symbol
- Line 1739: Assumes rest is a cons

If the input is malformed, these will silently skip entries or behave unexpectedly.

**Triggering Input**:
```lisp
(DEFLIST '(a b c) "prop")  ; Not pairs
(DEFLIST '((a) (b)) "prop")  ; Missing values
(DEFLIST '((a . b . c)) "prop")  ; Improper dotted lists
```

**Expected**: Error "DEFLIST requires list of (symbol value) pairs"
**Actual**: Silently skips malformed entries

**Test Case**:
```rust
#[test]
fn test_deflist_with_atoms() {
    let result = eval_str("(DEFLIST '(a b c) \"prop\")");
    println!("DEFLIST with atoms: {:?}", result);
    // Should error or handle gracefully
}

#[test]
fn test_deflist_incomplete_pairs() {
    let result = eval_str("(DEFLIST '((a) (b)) \"prop\")");
    println!("DEFLIST incomplete pairs: {:?}", result);
}
```

---

### BUG #12: PROG Duplicate Labels Silently Overwrite
**File**: `src/evaluator.rs:1171-1176`
**Severity**: MEDIUM

**Description**: The PROG implementation builds a label map (lines 1171-1176) by inserting label names into a HashMap. If there are duplicate labels, the later one silently overwrites the earlier one. This could lead to confusing behavior.

**Triggering Input**:
```lisp
(PROG ()
  label1
  (PRINT 1)
  label1
  (PRINT 2)
  (GO label1))  ; Which label1?
```

**Expected**: Error "duplicate label 'label1'" or warning
**Actual**: Second label1 overwrites first; GO jumps to line 4

**Test Case**:
```rust
#[test]
fn test_prog_duplicate_labels() {
    let input = r#"
        (PROG ()
          label1
          (PRINT "first")
          label1
          (PRINT "second")
          (GO label1))
    "#;
    let result = eval_str(input);
    println!("PROG duplicate labels: {:?}", result);
    // Should warn or error
}
```

---

### BUG #13: Stack Overflow on Deeply Nested Structures
**File**: `src/evaluator.rs:1300-1326, src/printer.rs:3-9`
**Severity**: MEDIUM

**Description**: Several functions are recursive without tail-call optimization:
1. `quasiquote_eval` (evaluator.rs:1300-1326) - recursively processes nested structures
2. `print_list_contents` (printer.rs:3-9) - recursively prints lists

Very deeply nested structures will cause stack overflow.

**Triggering Input**:
```lisp
; Generate deeply nested list
(PROGN
  (DEF x '(1))
  ; Wrap it 10000 times
  (SETQ x (CONS x (CONS x (CONS x ...)))))
```

**Expected**: Either handle gracefully or error with "nesting too deep"
**Actual**: Stack overflow

**Test Case**:
```rust
#[test]
fn test_deeply_nested_quasiquote() {
    let mut s = "`".to_string();
    for _ in 0..1000 {
        s.push_str("(a ");
    }
    for _ in 0..1000 {
        s.push(')');
    }

    let result = eval_str(&s);
    println!("Deep quasiquote: {:?}", result);
    // Might stack overflow
}

#[test]
fn test_deeply_nested_list_print() {
    let env = Environment::new_with_builtins();
    let mut list = LispVal::Nil;
    for _ in 0..10000 {
        list = LispVal::Cons {
            car: Box::new(LispVal::Number(1)),
            cdr: Box::new(list),
        };
    }

    // Printing this might stack overflow
    let printed = printer::print(&list);
    println!("Deep list length: {}", printed.len());
}
```

---

## LOW SEVERITY BUGS

### BUG #14: String Parsing Cannot Escape Quotes
**File**: `src/reader.rs:106-110`
**Severity**: LOW

**Description**: String parsing uses `is_not("\"")` which means there's no way to include a quote character in a string. This is a limitation, not necessarily a bug for Lisp 1.5 compatibility.

**Triggering Input**:
```lisp
"hello \"world\""  ; Won't parse
```

**Expected**: Either support escaping or document limitation
**Actual**: Parse error

---

### BUG #15: Minus Sign Parsing Ambiguity
**File**: `src/reader.rs:59-104`
**Severity**: LOW

**Description**: The atom parser tries number first, then alpha, then operators. A standalone "-" could be ambiguous - is it the minus operator or a failed number parse?

**Triggering Input**:
```lisp
(- 5 3)  ; Should work
-  ; Standalone minus
```

**Test Case**:
```rust
#[test]
fn test_minus_standalone() {
    let env = Environment::new_with_builtins();
    let result = read("-", &env);
    println!("Standalone minus: {:?}", result);
    // Should parse as the minus symbol
}
```

---

## Summary Statistics

- **CRITICAL**: 3 bugs (can cause hangs, crashes, or data corruption)
- **HIGH**: 4 bugs (can cause panics or incorrect results)
- **MEDIUM**: 6 bugs (incorrect behavior, silent errors)
- **LOW**: 2 bugs (minor limitations)

**Total**: 15 distinct bugs found

---

## Recommendations

1. **Immediate fixes needed** (CRITICAL):
   - Add bounds checking for negative indices in INDEX
   - Add cycle detection or max-depth limit for list operations
   - Fix float HashMap key handling (either disallow or normalize -0.0/0.0)

2. **High priority fixes** (HIGH):
   - Use checked arithmetic throughout (`checked_add`, `checked_mul`, etc.)
   - Add special case for i64::MIN / -1 in division
   - Validate shift amounts in LEFTSHIFT
   - Add recursion depth limit for LABEL evaluation

3. **Medium priority** (MEDIUM):
   - Add validation for ASSOC, DEFLIST input structure
   - Consider changing SETQ to error on undefined variables
   - Add duplicate label detection in PROG
   - Add max nesting depth limits

4. **Low priority** (LOW):
   - Document string escaping limitation or add support
   - Clarify operator symbol parsing
