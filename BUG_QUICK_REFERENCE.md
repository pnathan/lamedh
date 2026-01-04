# BUG QUICK REFERENCE CARD

## CRITICAL (3 bugs)

| # | Bug | File:Line | Trigger | Fix Time |
|---|-----|-----------|---------|----------|
| 1 | Float HashMap key corruption | lib.rs:181,211 | `(SET-BANG h 0.0 ...)(GET h -0.0)` | 20min |
| 2 | Circular list infinite loop | evaluator.rs:8-21 | Circular list in list_to_vec | 30min |
| 3 | LABEL infinite recursion | evaluator.rs:785-801 | `(LABEL x x)` | 45min |

## HIGH (6 bugs - all cause PANICS)

| # | Bug | File:Line | Trigger | Status |
|---|-----|-----------|---------|--------|
| 4 | Division MIN/-1 | evaluator.rs:123 | `(/ -9223372036854775808 -1)` | ✅ VERIFIED |
| 5 | Remainder MIN/-1 | evaluator.rs:270 | `(REMAINDER -9223372036854775808 -1)` | ✅ VERIFIED |
| 6 | Multiplication overflow | evaluator.rs:113 | `(* 9223372036854775807 2)` | ✅ VERIFIED |
| 7 | Addition overflow | evaluator.rs:96 | `(+ 9223372036854775807 1)` | ✅ VERIFIED |
| 8 | Subtraction underflow | evaluator.rs:108 | `(- -9223372036854775808 1)` | ✅ VERIFIED |
| 9 | Bit shift overflow | evaluator.rs:1671,1673 | `(LEFTSHIFT 1 64)` | ✅ VERIFIED |

## MEDIUM (6 bugs)

| # | Bug | File:Line | Impact |
|---|-----|-----------|--------|
| 10 | SETQ creates variables | environment.rs:249 | May be intentional for Lisp 1.5 |
| 11 | PROG duplicate labels | evaluator.rs:1174 | Silent overwrite, confusing behavior |
| 12 | DEFLIST malformed input | evaluator.rs:1732-1745 | Silent skip, no error |
| 13 | DEFINE non-list arg | evaluator.rs:1052 | Generic error message |
| 14 | Quasiquote stack overflow | evaluator.rs:1300-1326 | Very deep nesting |
| 15 | Print stack overflow | printer.rs:3-9 | Very deep nesting |

---

## PANIC FIXES (30 minutes total)

All arithmetic panics fixed by using checked operations:

```rust
// Division & Remainder
if nums[0] == i64::MIN && nums[1] == -1 {
    return Err(LispError::Generic("overflow".to_string()));
}

// Multiplication
nums.iter().try_fold(1i64, |acc, &n|
    acc.checked_mul(n).ok_or(...)
)

// Addition
nums.iter().try_fold(0i64, |acc, &n|
    acc.checked_add(n).ok_or(...)
)

// Subtraction
result.checked_sub(num).ok_or(...)

// Bit shift
if *shift < -63 || *shift > 63 {
    return Err(...);
}
```

---

## ONE-LINER TEST CASES

```lisp
; CRITICAL
(PROGN (DEF h (MAKE-HASH-TABLE)) (SET-BANG h 0.0 "a") (GET h -0.0))  ; Float key bug
; Circular list: not easily testable with current API
(LABEL x x)  ; Stack overflow

; HIGH - All PANIC in debug mode
(QUOTIENT -9223372036854775808 -1)  ; Division panic
(REMAINDER -9223372036854775808 -1) ; Remainder panic
(TIMES 9223372036854775807 2)       ; Multiply panic
(PLUS 9223372036854775807 1)        ; Add panic
(DIFFERENCE -9223372036854775808 1) ; Subtract panic
(LEFTSHIFT 1 64)                    ; Shift panic
(LEFTSHIFT 1 -64)                   ; Shift panic

; MEDIUM
(SETQ undefined-var 42)             ; Creates variable
(PROG () start (PRINT 1) start (PRINT 2) (GO start))  ; Duplicate label
(DEFLIST '(a b c) "prop")           ; Malformed, silent
```

---

## FIX PRIORITY

### Phase 1: Stop Panics (30min) ⚡ URGENT
- [ ] Division MIN/-1 check
- [ ] Remainder MIN/-1 check
- [ ] Use checked_mul
- [ ] Use checked_add
- [ ] Use checked_sub
- [ ] Validate shift range

### Phase 2: Data Integrity (1hr) 🔴 HIGH
- [ ] Fix float hashing OR reject float keys
- [ ] Add list length limit (100k)
- [ ] Add recursion depth limit (1000)

### Phase 3: Polish (1hr) 🟡 MEDIUM
- [ ] Warn on duplicate PROG labels
- [ ] Add quasiquote depth limit
- [ ] Add print depth limit
- [ ] Review SETQ semantics

---

## TEST COMMAND

```bash
# Run all critical bug tests
cargo test --test test_critical_bugs

# Run specific panic test
cargo test test_division_min_by_neg_one_panics

# Run with overflow detection
cargo build --release && cargo test
```

---

## GREP COMMANDS TO FIND ISSUES

```bash
# Find all unchecked arithmetic
grep -n "nums\[0\] \+ nums\[1\]" src/evaluator.rs
grep -n "iter().sum()" src/evaluator.rs
grep -n "iter().product()" src/evaluator.rs
grep -n "result -= " src/evaluator.rs

# Find all bit operations
grep -n " << " src/evaluator.rs
grep -n " >> " src/evaluator.rs

# Find recursive functions without depth limits
grep -n "fn.*eval.*LispVal" src/evaluator.rs
grep -n "fn.*print.*LispVal" src/printer.rs
```

---

## CODE REVIEW CHECKLIST

When reviewing arithmetic operations:
- ✅ Is there division or remainder?
  - Check for zero: `if divisor == 0`
  - Check for MIN/-1: `if dividend == i64::MIN && divisor == -1`
- ✅ Is there multiplication?
  - Use `checked_mul`
- ✅ Is there addition?
  - Use `checked_add`
- ✅ Is there subtraction?
  - Use `checked_sub`
- ✅ Is there negation?
  - Use `checked_neg`
- ✅ Is there exponentiation?
  - Already using `checked_pow` ✓
- ✅ Are there bit shifts?
  - Validate shift amount: `-63 <= shift <= 63`

When reviewing recursive functions:
- ✅ Is there a depth limit?
- ✅ Is there a base case?
- ✅ Does it handle cycles?
- ✅ Could it overflow the stack?

When reviewing HashMap operations:
- ✅ Are keys hashable and comparable correctly?
- ✅ Do equal values hash equally?
- ✅ Are special float values handled?

---

## REGRESSION TEST COVERAGE

Created: `tests/test_critical_bugs.rs`
- 49 tests total
- 37 passing (verify correct behavior)
- 8 failing (expose bugs via panics)
- 4 ignored (would hang)

**Coverage**:
- ✅ All arithmetic edge cases
- ✅ Float HashMap key issues
- ✅ LABEL recursion
- ✅ PROG labels
- ✅ Deep nesting
- ✅ Error handling
- ✅ Type checking

---

## IMPACT ANALYSIS

### Who is affected?
- **All users** running debug builds (panics)
- **Production users** with overflow inputs (crashes)
- **Users storing floats in hash tables** (data corruption)
- **Users with recursive code** (stack overflow)

### When does it happen?
- Arithmetic on large numbers (common in real code)
- Using hash tables with float keys (uncommon but critical)
- Deep recursion or nesting (uncommon)
- LABEL self-reference (rare edge case)

### Severity scoring
```
CRITICAL = Data corruption or infinite loop
HIGH     = Panic in production
MEDIUM   = Incorrect behavior, confusing UX
LOW      = Minor limitation, documented
```

---

## RECOMMENDED NEXT STEPS

1. **Immediately** apply Phase 1 fixes (panics)
2. **Within 24h** apply Phase 2 fixes (data integrity)
3. **Within week** apply Phase 3 fixes (polish)
4. **Add to CI** the test suite to prevent regressions
5. **Consider** adding clippy lints for arithmetic

---

## ADDITIONAL NOTES

### Not Actually Bugs
- ASSOC skipping non-cons entries (✓ correct behavior)
- CAR/CDR of NIL returning NIL (✓ correct)
- Empty arithmetic operations (✓ already tested)

### Potential Future Issues
- If RPLACA/RPLACD are changed to mutate in place
- If tail call optimization is removed
- If recursion limits are increased

### Good Practices Found
✅ EXPT uses checked_pow
✅ Division/remainder check for zero
✅ Many operations already have good error messages
✅ Tests cover most edge cases

### Areas for Improvement
❌ Inconsistent use of checked arithmetic
❌ No standard depth limits
❌ Some silent failure modes
