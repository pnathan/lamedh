# DETAILED LINE-BY-LINE BUG ANALYSIS

This document provides exact file locations, line numbers, code snippets, and precise fixes for each bug.

---

## BUG #1: Division MIN/-1 Overflow [HIGH - VERIFIED PANIC]

**Location**: `src/evaluator.rs:114-124`

**Current Code**:
```rust
BuiltinFunc::Divide => {
    if nums.len() != 2 {
        return Err(LispError::Generic(
            "/ requires exactly two arguments".to_string(),
        ));
    }
    if nums[1] == 0 {                              // Line 120 - only checks zero
        return Err(LispError::Generic("Division by zero".to_string()));
    }
    Ok(LispVal::Number(nums[0] / nums[1]))         // Line 123 - PANICS on MIN/-1
}
```

**Problem**: Line 123 panics when `nums[0] == i64::MIN` and `nums[1] == -1` because the result would be `i64::MAX + 1`.

**Test Case**:
```lisp
(QUOTIENT -9223372036854775808 -1)
; Output: thread panicked: attempt to divide with overflow
```

**Fix**:
```rust
BuiltinFunc::Divide => {
    if nums.len() != 2 {
        return Err(LispError::Generic(
            "/ requires exactly two arguments".to_string(),
        ));
    }
    if nums[1] == 0 {
        return Err(LispError::Generic("Division by zero".to_string()));
    }
    // Check for overflow case
    if nums[0] == i64::MIN && nums[1] == -1 {
        return Err(LispError::Generic("Division overflow".to_string()));
    }
    Ok(LispVal::Number(nums[0] / nums[1]))
}
```

---

## BUG #2: Remainder MIN/-1 Overflow [HIGH - VERIFIED PANIC]

**Location**: `src/evaluator.rs:262-274`

**Current Code**:
```rust
BuiltinFunc::Remainder => {
    if args.len() != 2 {
        return Err(LispError::Generic("remainder requires 2 args".to_string()));
    }
    if let (LispVal::Number(x), LispVal::Number(y)) = (&args[0], &args[1]) {
        if *y == 0 {                               // Line 267 - only checks zero
            return Err(LispError::Generic("Division by zero".to_string()));
        }
        Ok(LispVal::Number(x % y))                 // Line 270 - PANICS on MIN%-1
    } else {
        Err(LispError::Generic("remainder requires numbers".to_string()))
    }
}
```

**Problem**: Same as division - `i64::MIN % -1` panics.

**Test Case**:
```lisp
(REMAINDER -9223372036854775808 -1)
; Output: thread panicked: attempt to calculate the remainder with overflow
```

**Fix**:
```rust
if *y == 0 {
    return Err(LispError::Generic("Division by zero".to_string()));
}
if *x == i64::MIN && *y == -1 {
    return Err(LispError::Generic("Remainder overflow".to_string()));
}
Ok(LispVal::Number(x % y))
```

---

## BUG #3: Multiplication Overflow [HIGH - VERIFIED PANIC]

**Location**: `src/evaluator.rs:113`

**Current Code**:
```rust
BuiltinFunc::Multiply => Ok(LispVal::Number(nums.iter().product())),
```

**Problem**: `iter().product()` uses unchecked multiplication. Panics in debug, wraps in release.

**Test Case**:
```lisp
(TIMES 9223372036854775807 2)
; Debug: thread panicked: attempt to multiply with overflow
; Release: Returns -2 (wrapped)
```

**Fix - Option 1 (Iterator-based)**:
```rust
BuiltinFunc::Multiply => {
    let mut result = 1i64;
    for &num in &nums {
        result = result.checked_mul(num)
            .ok_or_else(|| LispError::Generic("Multiplication overflow".to_string()))?;
    }
    Ok(LispVal::Number(result))
}
```

**Fix - Option 2 (Fold-based)**:
```rust
BuiltinFunc::Multiply => {
    nums.iter()
        .try_fold(1i64, |acc, &n| {
            acc.checked_mul(n)
               .ok_or_else(|| LispError::Generic("Multiplication overflow".to_string()))
        })
        .map(LispVal::Number)
}
```

---

## BUG #4: Addition Overflow [HIGH]

**Location**: `src/evaluator.rs:96`

**Current Code**:
```rust
BuiltinFunc::Plus => Ok(LispVal::Number(nums.iter().sum())),
```

**Problem**: `iter().sum()` can overflow.

**Test Case**:
```lisp
(PLUS 9223372036854775807 1)
; Debug: panics
; Release: returns -9223372036854775808 (wrapped to MIN)
```

**Fix**:
```rust
BuiltinFunc::Plus => {
    nums.iter()
        .try_fold(0i64, |acc, &n| {
            acc.checked_add(n)
               .ok_or_else(|| LispError::Generic("Addition overflow".to_string()))
        })
        .map(LispVal::Number)
}
```

---

## BUG #5: Subtraction Underflow [HIGH - VERIFIED PANIC]

**Location**: `src/evaluator.rs:97-111`

**Current Code**:
```rust
BuiltinFunc::Minus => {
    if nums.is_empty() {
        return Err(LispError::Generic(
            "- requires at least one argument".to_string(),
        ));
    }
    if nums.len() == 1 {
        Ok(LispVal::Number(-nums[0]))
    } else {
        let mut result = nums[0];
        for &num in &nums[1..] {
            result -= num;                          // Line 108 - UNCHECKED
        }
        Ok(LispVal::Number(result))
    }
}
```

**Problem**: Line 108 can underflow.

**Test Case**:
```lisp
(DIFFERENCE -9223372036854775808 1)
; Debug: thread panicked: attempt to subtract with overflow
```

**Fix**:
```rust
BuiltinFunc::Minus => {
    if nums.is_empty() {
        return Err(LispError::Generic(
            "- requires at least one argument".to_string(),
        ));
    }
    if nums.len() == 1 {
        // Check for negating MIN
        nums[0].checked_neg()
            .ok_or_else(|| LispError::Generic("Negation overflow".to_string()))
            .map(LispVal::Number)
    } else {
        let mut result = nums[0];
        for &num in &nums[1..] {
            result = result.checked_sub(num)
                .ok_or_else(|| LispError::Generic("Subtraction underflow".to_string()))?;
        }
        Ok(LispVal::Number(result))
    }
}
```

---

## BUG #6: Left Shift Overflow [HIGH - VERIFIED PANIC]

**Location**: `src/evaluator.rs:1663-1680`

**Current Code**:
```rust
BuiltinFunc::Leftshift => {
    if args.len() != 2 {
        return Err(LispError::Generic(
            "leftshift requires exactly two arguments".to_string(),
        ));
    }
    if let (LispVal::Number(n), LispVal::Number(shift)) = (&args[0], &args[1]) {
        if *shift < 0 {
            Ok(LispVal::Number(n >> (-shift)))      // Line 1671 - PANICS if shift<=-64
        } else {
            Ok(LispVal::Number(n << shift))         // Line 1673 - PANICS if shift>=64
        }
    } else {
        Err(LispError::Generic(
            "leftshift requires integer arguments".to_string(),
        ))
    }
}
```

**Problem**: Shifting by >=64 or <=-64 bits is undefined and panics.

**Test Cases**:
```lisp
(LEFTSHIFT 1 64)      ; panic: attempt to shift left with overflow
(LEFTSHIFT 128 -100)  ; panic: attempt to shift right with overflow
```

**Fix**:
```rust
BuiltinFunc::Leftshift => {
    if args.len() != 2 {
        return Err(LispError::Generic(
            "leftshift requires exactly two arguments".to_string(),
        ));
    }
    if let (LispVal::Number(n), LispVal::Number(shift)) = (&args[0], &args[1]) {
        // Validate shift amount (must be in range for i64)
        if *shift < -63 || *shift > 63 {
            return Err(LispError::Generic(format!(
                "Shift amount {} out of range (must be -63 to 63)",
                shift
            )));
        }

        if *shift < 0 {
            Ok(LispVal::Number(n >> (-shift)))
        } else {
            // Use checked_shl for safety
            n.checked_shl(*shift as u32)
                .ok_or_else(|| LispError::Generic("Left shift overflow".to_string()))
                .map(LispVal::Number)
        }
    } else {
        Err(LispError::Generic(
            "leftshift requires integer arguments".to_string(),
        ))
    }
}
```

**Note**: The range is -63 to 63 because shifting by exactly 64 would shift out all bits.

---

## BUG #7: Float HashMap Key Violations [CRITICAL]

**Location**: `src/lib.rs:176-202, 206-227`

**Current Code (PartialEq)**:
```rust
impl PartialEq for LispVal {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            // ...
            (LispVal::Float(a), LispVal::Float(b)) => a == b,  // Line 181
            // ...
        }
    }
}
```

**Current Code (Hash)**:
```rust
impl Hash for LispVal {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self {
            // ...
            LispVal::Float(f) => f.to_bits().hash(state),      // Line 211
            // ...
        }
    }
}
```

**Problem**:
1. In Rust, `-0.0 == 0.0` is `true`, but `(-0.0).to_bits() != (0.0).to_bits()`
2. This violates the HashMap contract: equal items must hash equally
3. NaN handling is also problematic: `NaN != NaN` but can be used as key

**Test Case**:
```lisp
(PROGN
  (DEF h (MAKE-HASH-TABLE))
  (SET-BANG h 0.0 "zero")
  (SET-BANG h -0.0 "neg-zero")
  (KEYS h))
; Might return both 0.0 and -0.0 as separate keys
```

**Fix - Option 1: Normalize floats**:
```rust
impl Hash for LispVal {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self {
            // ...
            LispVal::Float(f) => {
                // Normalize -0.0 to 0.0 for hashing
                let normalized = if *f == 0.0 { 0.0 } else { *f };
                // Still use to_bits for consistent hashing
                normalized.to_bits().hash(state)
            },
            // ...
        }
    }
}
```

**Fix - Option 2: Reject float keys in hash operations**:
```rust
// In apply_hashtable_op (evaluator.rs)
fn validate_hash_key(key: &LispVal) -> Result<(), LispError> {
    if matches!(key, LispVal::Float(_)) {
        return Err(LispError::Generic(
            "Floats cannot be used as hash table keys due to equality semantics".to_string()
        ));
    }
    Ok(())
}

// Then call this in Set, Get, etc.
```

**Recommended**: Option 2 (reject float keys) is safer and matches Common Lisp behavior.

---

## BUG #8: Infinite Loop on Circular Lists [CRITICAL]

**Location**: `src/evaluator.rs:8-21`

**Current Code**:
```rust
fn list_to_vec(list: &LispVal) -> Result<Vec<LispVal>, LispError> {
    let mut vec = Vec::new();
    let mut current = list;
    while let LispVal::Cons { car, cdr } = current {
        vec.push(*car.clone());
        current = cdr;                              // No cycle detection
    }
    if *current != LispVal::Nil {
        return Err(LispError::Generic(
            "list_to_vec: not a proper list".to_string(),
        ));
    }
    Ok(vec)
}
```

**Problem**: No cycle detection. If the list is circular, this loops forever.

**Fix - Add iteration limit**:
```rust
fn list_to_vec(list: &LispVal) -> Result<Vec<LispVal>, LispError> {
    const MAX_LIST_LENGTH: usize = 100_000; // Reasonable limit

    let mut vec = Vec::new();
    let mut current = list;
    let mut count = 0;

    while let LispVal::Cons { car, cdr } = current {
        count += 1;
        if count > MAX_LIST_LENGTH {
            return Err(LispError::Generic(
                "List too long (possible circular reference)".to_string(),
            ));
        }
        vec.push(*car.clone());
        current = cdr;
    }

    if *current != LispVal::Nil {
        return Err(LispError::Generic(
            "list_to_vec: not a proper list".to_string(),
        ));
    }
    Ok(vec)
}
```

**Alternative Fix - Tortoise and Hare**:
```rust
fn list_to_vec(list: &LispVal) -> Result<Vec<LispVal>, LispError> {
    use std::rc::Rc;
    use std::collections::HashSet;

    let mut vec = Vec::new();
    let mut current = list;
    let mut seen: HashSet<*const LispVal> = HashSet::new();

    while let LispVal::Cons { car, cdr } = current {
        // Check for cycles using pointer equality
        let ptr = current as *const LispVal;
        if !seen.insert(ptr) {
            return Err(LispError::Generic(
                "Circular list detected".to_string(),
            ));
        }

        vec.push(*car.clone());
        current = cdr;
    }

    if *current != LispVal::Nil {
        return Err(LispError::Generic(
            "list_to_vec: not a proper list".to_string(),
        ));
    }
    Ok(vec)
}
```

**Recommended**: Use the iteration limit approach (simpler and faster).

---

## BUG #9: LABEL Infinite Recursion [CRITICAL]

**Location**: `src/evaluator.rs:785-801` (symbol evaluation) and `1021-1044` (LABEL special form)

**Current Code (Symbol Evaluation)**:
```rust
LispVal::Symbol(s) => {
    let value = env
        .get(&s.borrow().name)
        .ok_or_else(|| LispError::Generic(format!("Unbound variable: {}", s.borrow().name)))?;

    // If the value is a LABEL expression, evaluate it
    // This handles recursive LABEL definitions
    if let LispVal::Cons { car, cdr: _ } = &value {
        if let LispVal::Symbol(sym) = &**car {
            if sym.borrow().name == "LABEL" {
                return eval(&value, env);            // Line 795 - Can recurse infinitely
            }
        }
    }

    Ok(value)
}
```

**Problem**: Evaluating `(LABEL x x)` causes infinite recursion:
1. LABEL binds `x` to `(LABEL x x)`
2. Evaluating `x` looks it up, finds `(LABEL x x)`
3. Detects it's a LABEL, evaluates it again
4. Goto step 2 (infinite loop)

**Test Case**:
```lisp
(LABEL x x)
; Output: stack overflow
```

**Fix - Add recursion depth tracking**:
```rust
// In lib.rs, add a field to Environment or pass through eval
pub struct EvalContext {
    pub recursion_depth: usize,
    pub max_recursion_depth: usize,
}

// Then in eval:
pub fn eval_with_context(
    val: &LispVal,
    env: &Rc<Environment>,
    ctx: &mut EvalContext
) -> Result<LispVal, LispError> {
    ctx.recursion_depth += 1;
    if ctx.recursion_depth > ctx.max_recursion_depth {
        return Err(LispError::Generic("Maximum recursion depth exceeded".to_string()));
    }

    let result = eval_impl(val, env, ctx);

    ctx.recursion_depth -= 1;
    result
}
```

**Simpler Fix - Remove auto-evaluation of LABEL**:
```rust
// Just remove the LABEL re-evaluation code entirely
LispVal::Symbol(s) => {
    env.get(&s.borrow().name)
        .ok_or_else(|| LispError::Generic(format!("Unbound variable: {}", s.borrow().name)))
}
```

Then LABEL would work like this:
```lisp
; User must explicitly call the LABEL result
(DEF factorial (LABEL f (LAMBDA (n) ...)))
```

---

## BUG #10: ASSOC with Non-Cons Alist Entries [MEDIUM]

**Location**: `src/evaluator.rs:1512-1533`

**Current Code**:
```rust
BuiltinFunc::Assoc => {
    if args.len() != 2 {
        return Err(LispError::Generic(
            "assoc requires exactly two arguments".to_string(),
        ));
    }
    let key = &args[0];
    let mut alist = &args[1];
    while let LispVal::Cons { car, cdr } = alist {
        if let LispVal::Cons {                       // Line 1521 - Assumes car is Cons
            car: pair_car,
            cdr: _,
        } = &**car
        {
            if **pair_car == *key {
                return Ok(*car.clone());
            }
        }
        alist = cdr;
    }
    Ok(LispVal::Nil)
}
```

**Problem**: If an alist element is not a Cons, it's silently skipped. This is actually correct behavior for ASSOC!

**Test Case**:
```lisp
(ASSOC 'A '((A . 1) B (C . 3)))
; Correctly returns (A . 1), ignoring the atom B
```

**Status**: NOT A BUG - this is correct ASSOC behavior per Lisp 1.5 spec.

---

## BUG #11: SETQ Creates Variables [MEDIUM - May Be Intentional]

**Location**: `src/environment.rs:237-250`

**Current Code**:
```rust
pub fn update(env: &Rc<Environment>, name: &str, val: LispVal) {
    let mut maybe_env = Some(env.clone());
    while let Some(current_env) = maybe_env {
        if current_env.bindings.borrow().contains_key(name) {
            current_env
                .bindings
                .borrow_mut()
                .insert(name.to_string(), val);
            return;
        }
        maybe_env = current_env.parent.clone();
    }
    env.set(name.to_string(), val);                  // Line 249 - Creates if not found
}
```

**Problem**: If variable doesn't exist anywhere in the environment chain, it creates it in the current environment.

**Test Case**:
```lisp
(SETQ new-var 123)  ; Variable doesn't exist
new-var             ; Returns 123 (was created)
```

**Current Behavior**: Creates variable
**Common Lisp Behavior**: Would error "unbound variable"

**Fix (if desired)**:
```rust
pub fn update(env: &Rc<Environment>, name: &str, val: LispVal) -> Result<(), LispError> {
    let mut maybe_env = Some(env.clone());
    while let Some(current_env) = maybe_env {
        if current_env.bindings.borrow().contains_key(name) {
            current_env
                .bindings
                .borrow_mut()
                .insert(name.to_string(), val);
            return Ok(());
        }
        maybe_env = current_env.parent.clone();
    }
    // Instead of creating, return error
    Err(LispError::Generic(format!("Undefined variable: {}", name)))
}
```

**Note**: Check Lisp 1.5 spec to see if current behavior is correct for that dialect.

---

## BUG #12: PROG Duplicate Labels [MEDIUM]

**Location**: `src/evaluator.rs:1171-1176`

**Current Code**:
```rust
let mut labels = HashMap::new();
for (i, item) in body.iter().enumerate() {
    if let LispVal::Symbol(s) = item {
        labels.insert(s.borrow().name.clone(), i);   // Line 1174 - Overwrites duplicates
    }
}
```

**Problem**: If there are duplicate labels, later ones silently overwrite earlier ones.

**Test Case**:
```lisp
(PROG ()
  start
  (PRINT "first")
  start
  (PRINT "second")
  (GO start))  ; Jumps to second occurrence
```

**Fix**:
```rust
let mut labels = HashMap::new();
for (i, item) in body.iter().enumerate() {
    if let LispVal::Symbol(s) = item {
        let name = s.borrow().name.clone();
        if labels.insert(name.clone(), i).is_some() {
            return Err(LispError::Generic(format!(
                "Duplicate label in PROG: {}",
                name
            )));
        }
    }
}
```

---

## BUG #13: Deep Nesting Stack Overflow [MEDIUM]

**Location**: Multiple locations
1. `src/evaluator.rs:1300-1326` (quasiquote_eval)
2. `src/printer.rs:3-9` (print_list_contents)

**Current Code (quasiquote_eval)**:
```rust
fn quasiquote_eval(val: &LispVal, env: &Rc<Environment>) -> Result<LispVal, LispError> {
    if let LispVal::Cons { car, cdr } = val {
        // ... check for UNQUOTE ...
        let car_eval = quasiquote_eval(car, env)?;    // Recursive call
        let cdr_eval = quasiquote_eval(cdr, env)?;    // Recursive call
        Ok(LispVal::Cons {
            car: Box::new(car_eval),
            cdr: Box::new(cdr_eval),
        })
    } else {
        Ok(val.clone())
    }
}
```

**Problem**: Very deep nesting causes stack overflow.

**Test Case**:
```lisp
; 10000 levels of nesting
`(a (a (a (a ... (a) ...))))
```

**Fix - Add depth limit**:
```rust
fn quasiquote_eval_with_depth(
    val: &LispVal,
    env: &Rc<Environment>,
    depth: usize
) -> Result<LispVal, LispError> {
    const MAX_DEPTH: usize = 1000;

    if depth > MAX_DEPTH {
        return Err(LispError::Generic(
            "Quasiquote nesting too deep".to_string()
        ));
    }

    if let LispVal::Cons { car, cdr } = val {
        if let LispVal::Symbol(s) = &**car
            && s.borrow().name == "UNQUOTE"
        {
            if let LispVal::Cons {
                car: unquoted_val,
                cdr: rest,
            } = &**cdr
                && **rest == LispVal::Nil
            {
                return eval(unquoted_val, env);
            }
            return Err(LispError::Generic(
                "unquote takes exactly one argument".to_string(),
            ));
        }
        let car_eval = quasiquote_eval_with_depth(car, env, depth + 1)?;
        let cdr_eval = quasiquote_eval_with_depth(cdr, env, depth + 1)?;
        Ok(LispVal::Cons {
            car: Box::new(car_eval),
            cdr: Box::new(cdr_eval),
        })
    } else {
        Ok(val.clone())
    }
}

// Public wrapper
fn quasiquote_eval(val: &LispVal, env: &Rc<Environment>) -> Result<LispVal, LispError> {
    quasiquote_eval_with_depth(val, env, 0)
}
```

---

## Summary of Required Changes

### Immediate (Prevent Panics)
1. ✅ `evaluator.rs:123` - Add i64::MIN/-1 check for division
2. ✅ `evaluator.rs:270` - Add i64::MIN/-1 check for remainder
3. ✅ `evaluator.rs:113` - Use checked_mul for multiplication
4. ✅ `evaluator.rs:96` - Use checked_add for addition
5. ✅ `evaluator.rs:108` - Use checked_sub for subtraction
6. ✅ `evaluator.rs:1671-1673` - Validate shift amounts

### High Priority (Data Integrity)
7. ✅ `lib.rs:211` - Fix float hashing or reject float keys
8. ✅ `evaluator.rs:8-21` - Add cycle detection/length limit
9. ✅ `evaluator.rs:785-801` - Add recursion depth limit or remove LABEL auto-eval

### Medium Priority (Code Quality)
10. ✅ `evaluator.rs:1174` - Warn on duplicate PROG labels
11. ✅ `evaluator.rs:1300-1326` - Add depth limit to quasiquote
12. ✅ `printer.rs:3-9` - Add depth limit to printer

### Consider/Review
13. ⚠️  `environment.rs:249` - Review SETQ behavior (may be intentional)
14. ⚠️  `evaluator.rs:1521` - ASSOC behavior is actually correct

---

## Estimated Fix Time
- Arithmetic overflow fixes: 30 minutes
- Float key handling: 20 minutes
- Recursion limits: 45 minutes
- Testing and verification: 1-2 hours
- **Total: 3-4 hours**
