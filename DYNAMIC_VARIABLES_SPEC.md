# Dynamic Variables Specification for Lamedh

## Overview

This specification defines dynamic variables (also called special variables) for Lamedh, following the Common Lisp approach. Dynamic variables use dynamic scoping instead of lexical scoping, allowing functions to access bindings from their caller's environment rather than their definition environment.

## Motivation

Lisp 1.5 used dynamic scoping for all variables. Modern Lisps like Common Lisp combine lexical scoping (default) with optional dynamic scoping (for special variables). This spec adds dynamic variables to Lamedh while preserving backward compatibility with existing lexical scoping.

## Syntax

### DEFDYNAMIC

```lisp
(defdynamic symbol initial-value [docstring])
```

**Aliases**: `DEFVAR` (for Common Lisp compatibility)

**Arguments**:
- `symbol`: Symbol to define as dynamic (required)
- `initial-value`: Initial value to bind (required, evaluated)
- `docstring`: Optional documentation string

**Returns**: The symbol

**Side Effects**:
- Marks `symbol` as a dynamic variable globally
- Sets initial global value of `symbol` to `initial-value`
- If `docstring` provided, stores it in symbol's property list

**Naming Convention**: Dynamic variables should be named with surrounding asterisks (earmuffs):
```lisp
(defdynamic *debug* nil)           ; ✓ Good: has *earmuffs*
(defdynamic *connection* nil)      ; ✓ Good
(defdynamic debug nil)             ; ✗ Warning: missing *earmuffs*
```

**Warning**: If symbol does not start and end with `*`, print a style warning:
```
Warning: Dynamic variable 'DEBUG' does not follow naming convention *NAME*
```

### Examples

```lisp
;; Define dynamic variables
(defdynamic *debug* nil "Enable debug logging")
(defdynamic *trace-calls* nil)
(defdynamic *max-depth* 100)
(defdynamic *connection* nil "Database connection")

;; Warning example
(defdynamic debug nil)
;; => Warning: Dynamic variable 'DEBUG' does not follow naming convention *NAME*
;; => DEBUG
```

## Semantics

### Dynamic vs Lexical Scoping

**Lexical scoping** (default): Variables are looked up in the environment where the function was **defined**

```lisp
(def x 'global)

(def get-x (lambda () x))

(def test-lexical
  (lambda ()
    (def x 'local)
    (get-x)))

(test-lexical)  ; => GLOBAL (lexical: sees x from definition environment)
```

**Dynamic scoping**: Variables are looked up in the environment where the function is **called**

```lisp
(defdynamic *x* 'global)

(def get-x (lambda () *x*))

(def test-dynamic
  (lambda ()
    (let ((*x* 'local))
      (get-x))))

(test-dynamic)  ; => LOCAL (dynamic: sees *x* from caller environment)
```

### Variable Lookup Rules

When evaluating a symbol:

1. **Check if symbol is marked as dynamic**:
   - If YES: Look up in current dynamic environment (caller's bindings)
   - If NO: Look up in lexical environment (captured at definition)

2. **Dynamic lookup** walks the call stack:
   - Start at current frame
   - Check each parent frame (via parent chain)
   - Return first binding found
   - Error if no binding found

3. **Lexical lookup** (existing behavior):
   - Use captured environment from lambda/closure
   - Walk parent chain from captured environment
   - Error if no binding found

### Binding Dynamic Variables

Dynamic variables can be bound in several contexts:

#### 1. Global Binding (DEFDYNAMIC/DEFVAR)

```lisp
(defdynamic *config* "default")
*config*  ; => "default"
```

#### 2. LET Binding

When a LET binds a dynamic variable, it creates a **dynamic binding** visible to called functions:

```lisp
(defdynamic *debug* nil)

(defun log-message (msg)
  (if *debug*
      (print msg)
      nil))

(let ((*debug* t))
  (log-message "Debug is on"))   ; Prints "Debug is on"

(log-message "Debug is off")     ; Returns nil (global *debug* is nil)
```

#### 3. SETQ on Dynamic Variables

`SETQ` on a dynamic variable modifies the current dynamic binding:

```lisp
(defdynamic *counter* 0)

(defun increment ()
  (setq *counter* (+ *counter* 1)))

(let ((*counter* 10))
  (increment)
  (increment)
  *counter*)  ; => 12

*counter*  ; => 0 (global binding unchanged)
```

#### 4. Function Parameters (Advanced)

If a function parameter shadows a dynamic variable, the parameter binding is dynamic:

```lisp
(defdynamic *x* 'global)

(defun show-x () *x*)

(defun caller (x)  ; Parameter x shadows dynamic *x*
  (show-x))        ; But this is subtle - see Implementation Notes

;; Simpler: Use LET to rebind
(defun caller (value)
  (let ((*x* value))
    (show-x)))
```

**Note**: For simplicity, this spec does NOT require function parameters to create dynamic bindings. Use LET explicitly to rebind dynamic variables.

## Implementation Requirements

### 1. Data Structures

#### Environment Changes

Add to `Environment` struct:

```rust
pub struct Environment {
    parent: Option<Rc<Environment>>,
    bindings: Rc<RefCell<HashMap<String, LispVal>>>,
    symbols: Rc<RefCell<SymbolTable>>,
    // NEW: Track which variables are dynamic
    dynamic_vars: Rc<RefCell<HashSet<String>>>,
}
```

The `dynamic_vars` set is **shared globally** (not per-environment). When a child environment is created, it should share the same `dynamic_vars` reference as its parent.

#### Environment Methods

Add these methods to `Environment`:

```rust
impl Environment {
    /// Check if a variable is marked as dynamic
    pub fn is_dynamic(&self, name: &str) -> bool {
        self.dynamic_vars.borrow().contains(name)
    }

    /// Mark a variable as dynamic (global registration)
    pub fn mark_dynamic(&self, name: String) {
        self.dynamic_vars.borrow_mut().insert(name);
    }

    /// Get variable value (dynamic or lexical)
    /// This replaces/extends existing get() to handle both cases
    pub fn get_var(&self, name: &str) -> Option<LispVal> {
        if self.is_dynamic(name) {
            // Dynamic lookup: search current environment chain
            self.get_dynamic(name)
        } else {
            // Lexical lookup: existing behavior
            self.get(name)
        }
    }

    /// Dynamic lookup: search current frame chain
    fn get_dynamic(&self, name: &str) -> Option<LispVal> {
        // Same as existing get() - walks parent chain
        if let Some(val) = self.bindings.borrow().get(name) {
            return Some(val.clone());
        }
        if let Some(parent) = &self.parent {
            return parent.get_dynamic(name);
        }
        None
    }
}
```

**Key insight**: Dynamic and lexical lookup use the **same** environment chain, but:
- Lexical: Uses captured environment (from lambda's `env` field)
- Dynamic: Uses current environment (passed to `eval()`)

### 2. Special Forms

#### Add DEFDYNAMIC

In `evaluator.rs`, add to the special form match:

```rust
"DEFDYNAMIC" | "DEFVAR" => {
    let args = list_to_vec(rest)?;
    if args.len() < 2 || args.len() > 3 {
        return Err(LispError::Generic(
            "defdynamic requires 2 or 3 arguments: (defdynamic symbol value [docstring])"
                .to_string(),
        ));
    }

    // Get symbol
    let symbol = if let LispVal::Symbol(s) = &args[0] {
        s
    } else {
        return Err(LispError::Generic(
            "defdynamic first argument must be a symbol".to_string(),
        ));
    };

    let name = symbol.borrow().name.clone();

    // Check naming convention (*earmuffs*)
    if !name.starts_with('*') || !name.ends_with('*') || name.len() < 3 {
        eprintln!(
            "Warning: Dynamic variable '{}' does not follow naming convention *NAME*",
            name
        );
    }

    // Mark as dynamic
    env.mark_dynamic(name.clone());

    // Evaluate initial value
    let value = eval(&args[1], env)?;

    // Set global value
    env.set(name, value);

    // Optional docstring
    if args.len() == 3 {
        if let LispVal::String(doc) = &args[2] {
            symbol.borrow_mut().plist.insert(
                "docstring".to_string(),
                LispVal::String(doc.clone()),
            );
        } else {
            return Err(LispError::Generic(
                "defdynamic docstring must be a string".to_string(),
            ));
        }
    }

    Ok(LispVal::Symbol(symbol.clone()))
}
```

### 3. Symbol Evaluation

Modify symbol evaluation in `eval()`:

```rust
LispVal::Symbol(s) => {
    let name = &s.borrow().name;

    // Use new get_var method that handles both dynamic and lexical
    env.get_var(name)
        .ok_or_else(|| LispError::Generic(format!("Unbound variable: {}", name)))
}
```

### 4. LET Binding

The existing `LET` implementation should work without changes! It:
1. Evaluates binding values in current environment
2. Creates new child environment
3. Sets bindings in child environment
4. Evaluates body in child environment

For dynamic variables, this creates dynamic bindings because:
- Dynamic lookup uses the current environment (the LET's child env)
- Functions called from within LET will see the new bindings

No changes needed to `LET` implementation.

### 5. SETQ Modification

`SETQ` should work without changes! It uses `Environment::update()` which:
- Walks the environment chain looking for existing binding
- Updates the first binding found
- Creates new binding in current environment if not found

For dynamic variables, this does the right thing:
- Updates the most recent dynamic binding in the call chain
- Creates global binding if no dynamic binding exists

No changes needed to `SETQ` implementation.

## Test Cases

Create `tests/dynamic_variables_test.lisp`:

```lisp
;;; Test 1: Basic DEFDYNAMIC
(defdynamic *test-var* 42)
(print *test-var*)  ; Should print: 42

;;; Test 2: DEFVAR alias
(defvar *another-var* 100)
(print *another-var*)  ; Should print: 100

;;; Test 3: Dynamic binding with LET
(defdynamic *x* 'global)

(defun get-x () *x*)

(print (get-x))  ; Should print: GLOBAL

(let ((*x* 'local))
  (print (get-x)))  ; Should print: LOCAL

(print (get-x))  ; Should print: GLOBAL

;;; Test 4: Nested dynamic bindings
(defdynamic *level* 0)

(defun show-level ()
  (print *level*))

(show-level)  ; Should print: 0

(let ((*level* 1))
  (show-level)  ; Should print: 1
  (let ((*level* 2))
    (show-level)  ; Should print: 2
    (let ((*level* 3))
      (show-level))))  ; Should print: 3

(show-level)  ; Should print: 0

;;; Test 5: SETQ on dynamic variable
(defdynamic *counter* 0)

(defun increment ()
  (setq *counter* (+ *counter* 1)))

(increment)
(print *counter*)  ; Should print: 1

(let ((*counter* 10))
  (increment)
  (increment)
  (print *counter*))  ; Should print: 12

(print *counter*)  ; Should print: 1 (not 12)

;;; Test 6: Multiple dynamic variables
(defdynamic *debug* nil)
(defdynamic *verbose* nil)

(defun log (msg)
  (if *debug*
      (if *verbose*
          (print (concat "VERBOSE: " msg))
          (print msg))
      nil))

(log "Test 1")  ; Should print nothing

(let ((*debug* t))
  (log "Test 2"))  ; Should print: Test 2

(let ((*debug* t) (*verbose* t))
  (log "Test 3"))  ; Should print: VERBOSE: Test 3

;;; Test 7: Lexical vs Dynamic - The Classic Test
(def lexical-x 'lexical-global)
(defdynamic *dynamic-x* 'dynamic-global)

(def get-lexical (lambda () lexical-x))
(def get-dynamic (lambda () *dynamic-x*))

(def test-both
  (lambda ()
    (def lexical-x 'lexical-local)  ; Shadows lexically
    (let ((*dynamic-x* 'dynamic-local))  ; Shadows dynamically
      (print (get-lexical))  ; Should print: LEXICAL-GLOBAL
      (print (get-dynamic))))) ; Should print: DYNAMIC-LOCAL

(test-both)

;;; Test 8: Dynamic variable in recursive function
(defdynamic *depth* 0)

(defun recursive-print (n)
  (if (zerop n)
      (print "Done")
      (let ((*depth* (+ *depth* 1)))
        (print *depth*)
        (recursive-print (- n 1)))))

(recursive-print 3)
; Should print: 1, 2, 3, Done

;;; Test 9: Documentation string
(defdynamic *documented* 42 "This is a test variable")
(print (getp '*documented* "docstring"))
; Should print: "This is a test variable"

;;; Test 10: Warning for missing earmuffs
(defdynamic bad-name 123)
; Should print warning: Warning: Dynamic variable 'BAD-NAME' does not follow naming convention *NAME*
; Should still work though
(print bad-name)  ; Should print: 123
```

Create `tests/dynamic_variables.rs`:

```rust
use lamedh::{environment::Environment, eval_line, load_file};

#[test]
fn test_dynamic_variables() {
    let env = Environment::new_with_builtins();

    // Test basic defdynamic
    let result = eval_line("(defdynamic *test* 42)", &env);
    assert_eq!(result, "*TEST*");

    let result = eval_line("*test*", &env);
    assert_eq!(result, "42");
}

#[test]
fn test_defvar_alias() {
    let env = Environment::new_with_builtins();

    let result = eval_line("(defvar *var* 100)", &env);
    assert_eq!(result, "*VAR*");

    let result = eval_line("*var*", &env);
    assert_eq!(result, "100");
}

#[test]
fn test_dynamic_binding() {
    let env = Environment::new_with_builtins();

    eval_line("(defdynamic *x* 'global)", &env);
    eval_line("(defun get-x () *x*)", &env);

    let result = eval_line("(get-x)", &env);
    assert_eq!(result, "GLOBAL");

    let result = eval_line("(let ((*x* 'local)) (get-x))", &env);
    assert_eq!(result, "LOCAL");

    let result = eval_line("(get-x)", &env);
    assert_eq!(result, "GLOBAL");
}

#[test]
fn test_lexical_vs_dynamic() {
    let env = Environment::new_with_builtins();

    // Lexical variable
    eval_line("(def lex 'global)", &env);
    eval_line("(def get-lex (lambda () lex))", &env);

    // Dynamic variable
    eval_line("(defdynamic *dyn* 'global)", &env);
    eval_line("(def get-dyn (lambda () *dyn*))", &env);

    // Test from different binding context
    eval_line("(defun test () (progn (def lex 'local) (let ((*dyn* 'local)) (print (get-lex)) (print (get-dyn)))))", &env);

    // Capture output and verify
    eval_line("(test)", &env);
    // get-lex should return GLOBAL (lexical)
    // get-dyn should return LOCAL (dynamic)
}

#[test]
fn test_dynamic_counter() {
    let env = Environment::new_with_builtins();

    eval_line("(defdynamic *counter* 0)", &env);
    eval_line("(defun inc () (setq *counter* (+ *counter* 1)))", &env);

    eval_line("(inc)", &env);
    let result = eval_line("*counter*", &env);
    assert_eq!(result, "1");

    eval_line("(let ((*counter* 10)) (inc) (inc))", &env);
    let result = eval_line("*counter*", &env);
    assert_eq!(result, "1");  // Global unchanged
}

#[test]
fn test_run_comprehensive_tests() {
    let env = Environment::new_with_builtins();
    load_file("tests/dynamic_variables_test.lisp", &env).expect("Failed to load test file");
}
```

## Edge Cases and Clarifications

### 1. Redefining Dynamic Variables

```lisp
(defdynamic *x* 1)
(defdynamic *x* 2)  ; OK: updates global value, already marked dynamic
*x*  ; => 2
```

### 2. DEF vs DEFDYNAMIC

```lisp
(def *y* 10)        ; Lexical variable (even with earmuffs)
(defdynamic *y* 20) ; Now dynamic, overwrites

(defun get-y () *y*)
(let ((*y* 30))
  (get-y))  ; => 30 (dynamic)
```

Once a variable is marked dynamic, it stays dynamic (global flag).

### 3. Unbound Dynamic Variables

```lisp
(defdynamic *unbound* nil)
(setq *unbound* nil)  ; Sets to NIL (bound to NIL)

; vs

(defun use-undefined ()
  *undefined-dynamic*)  ; Error: Unbound variable

; To check if bound (future extension):
(boundp '*unbound*)  ; => T
(boundp '*undefined-dynamic*)  ; => NIL
```

### 4. Symbol Naming

```lisp
(defdynamic *good* 1)     ; ✓ No warning
(defdynamic **also** 2)   ; ✓ No warning (starts and ends with *)
(defdynamic *x 3)         ; ✗ Warning: doesn't end with *
(defdynamic x* 4)         ; ✗ Warning: doesn't start with *
(defdynamic ** 5)         ; ✗ Warning: len < 3 (no content between *)
(defdynamic * 6)          ; ✗ Warning: len < 3
```

### 5. Environment Lifetime

Dynamic bindings are automatically cleaned up when LET exits:

```lisp
(defdynamic *temp* 'global)

(defun outer ()
  (let ((*temp* 'outer))
    (inner)))

(defun inner ()
  *temp*)  ; Sees 'outer from caller

(outer)  ; => OUTER
*temp*   ; => GLOBAL (binding cleaned up)
```

## Implementation Notes

### Key Differences from Lexical Variables

| Aspect | Lexical | Dynamic |
|--------|---------|---------|
| Lookup context | Definition environment | Caller environment |
| Captured in closures | Yes | No (always current binding) |
| Scope | Static (textual) | Dynamic (call stack) |
| Performance | Fast (direct reference) | Slightly slower (chain walk) |
| Use case | Normal variables | Configuration, context |

### Performance Considerations

Dynamic variable lookup requires walking the environment chain on every access. Optimizations (optional, future work):

1. **Shallow binding** (MacLisp-style): Value cells + specpdl
2. **Caching**: Cache dynamic bindings in thread-local storage
3. **Special case**: Optimize for unbound dynamic variables (global only)

For initial implementation, use simple chain walking (same as lexical). Performance is acceptable for typical use cases.

### Thread Safety

This spec assumes **single-threaded execution**. Dynamic variables are inherently thread-local in nature. If threading is added later:

- Each thread should have its own environment chain
- Dynamic variables should be thread-local by default
- Shared dynamic variables would require explicit synchronization

## Integration with Existing Code

### Backward Compatibility

All existing code continues to work:
- Existing variables are lexical by default
- No behavior changes for non-dynamic variables
- LET, SETQ, DEF all work as before

### Standard Library Updates

Consider defining standard dynamic variables in `lib/`:

```lisp
;;; lib/06-dynamic.lisp

;; Standard dynamic variables
(defdynamic *print-pretty* t "Enable pretty printing")
(defdynamic *print-escape* t "Escape special characters when printing")
(defdynamic *print-level* nil "Maximum depth to print nested structures")
(defdynamic *print-length* nil "Maximum length to print lists")

;; Error handling context
(defdynamic *error-handler* nil "Current error handler")

;; Debugging
(defdynamic *trace-output* nil "Where to send trace output")
```

### Documentation Updates

Update `CLAUDE.md`:

```markdown
### Dynamic Variables

Lamedh supports both lexical and dynamic scoping:

- **Lexical variables** (default): Resolved in definition environment
- **Dynamic variables**: Resolved in caller environment

Define dynamic variables with `DEFDYNAMIC` or `DEFVAR`:

    (defdynamic *debug* nil)

By convention, dynamic variables use *earmuffs* (surrounding asterisks).

Dynamic variables are useful for:
- Configuration parameters
- Debugging flags
- Context propagation
- Implicit parameters

See `DYNAMIC_VARIABLES_SPEC.md` for complete specification.
```

## Summary for Implementer

### Minimum Viable Implementation

1. **Add to Environment**:
   - `dynamic_vars: Rc<RefCell<HashSet<String>>>`
   - `is_dynamic(name)` method
   - `mark_dynamic(name)` method
   - Modify `get()` to call `get_var()` which dispatches on `is_dynamic()`

2. **Add DEFDYNAMIC special form**:
   - Parse symbol, value, optional docstring
   - Check for *earmuffs*, print warning if missing
   - Call `env.mark_dynamic(name)`
   - Evaluate value and set global binding
   - Store docstring in plist if provided

3. **Modify symbol evaluation**:
   - Replace `env.get(name)` with `env.get_var(name)`
   - `get_var()` checks `is_dynamic()` and dispatches appropriately

4. **Test**:
   - Run test suite in `tests/dynamic_variables_test.lisp`
   - Run Rust tests in `tests/dynamic_variables.rs`
   - Verify backward compatibility with existing tests

That's it! LET, SETQ, and other forms work automatically because they use the standard environment chain.

### Expected Implementation Time

- Data structures: 30 minutes
- DEFDYNAMIC form: 1 hour
- Symbol evaluation change: 30 minutes
- Testing: 1 hour
- **Total**: ~3 hours

### Success Criteria

- All tests in `dynamic_variables_test.lisp` pass
- All Rust tests pass
- Existing test suite still passes (backward compatibility)
- Warning printed for variables without *earmuffs*
- Lexical and dynamic variables coexist correctly

Good luck! This should be a clean, self-contained addition to lamedh.
