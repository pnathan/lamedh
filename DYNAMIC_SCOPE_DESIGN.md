# Dynamic Scope Design for Lamedh

## Overview

This document explores different approaches to add dynamic scoping to Lamedh, which currently uses lexical scoping. The goal is to provide historically accurate Lisp 1.5 semantics while maintaining compatibility with existing code.

## Current State

- **Scoping**: Lexical (environment captured at lambda definition time)
- **Environment**: Parent chain with lexical lookup
- **Lambda**: Stores `env: Rc<Environment>` captured at definition

Key files:
- `src/environment.rs`: Environment management
- `src/evaluator.rs`: Lambda creation and application
- `src/lib.rs`: Core data structures

## Approach 1: Special Variables (Recommended)

Similar to Common Lisp's special variables. Combines lexical and dynamic scoping.

### Design

1. **Mark variables as dynamic**:
   - Add `dynamic_vars: HashSet<String>` to `Environment`
   - Use `(DEFVAR name value)` or `(DECLARE (SPECIAL name))` to mark variables as dynamic

2. **Separate lookup paths**:
   - Lexical variables: Look up in closure's environment (current behavior)
   - Dynamic variables: Look up in caller's environment (dynamic stack)

3. **Dynamic binding stack**:
   - Add `dynamic_bindings: Rc<RefCell<HashMap<String, LispVal>>>` to `Environment`
   - When evaluating function, temporarily push dynamic bindings
   - Restore on exit

### Implementation Steps

```rust
// In environment.rs
pub struct Environment {
    parent: Option<Rc<Environment>>,
    bindings: Rc<RefCell<HashMap<String, LispVal>>>,
    symbols: Rc<RefCell<SymbolTable>>,
    // NEW: Track which variables are dynamic
    dynamic_vars: Rc<RefCell<HashSet<String>>>,
}

impl Environment {
    // NEW: Check if variable is dynamic
    pub fn is_dynamic(&self, name: &str) -> bool {
        self.dynamic_vars.borrow().contains(name)
    }

    // NEW: Mark variable as dynamic
    pub fn mark_dynamic(&self, name: String) {
        self.dynamic_vars.borrow_mut().insert(name);
    }
}
```

```rust
// In evaluator.rs - modify symbol evaluation
LispVal::Symbol(s) => {
    let name = &s.borrow().name;

    // Check if this is a dynamic variable
    if env.is_dynamic(name) {
        // Look up in current dynamic environment
        env.get(name)
            .ok_or_else(|| LispError::Generic(format!("Unbound dynamic variable: {}", name)))
    } else {
        // Look up in lexical environment (existing behavior)
        env.get(name)
            .ok_or_else(|| LispError::Generic(format!("Unbound variable: {}", name)))
    }
}
```

Add new special forms:
- `(DEFVAR name value)` - Define dynamic variable
- `(DECLARE (SPECIAL name1 name2 ...))` - Declare variables as dynamic

### Pros
- Maintains backward compatibility
- Follows Common Lisp convention
- Fine-grained control
- Both scoping modes coexist

### Cons
- More complex implementation
- Need to track which variables are dynamic

## Approach 2: FLUID-LET / LET-DYNAMIC

Add a special form for temporary dynamic bindings.

### Design

```lisp
(fluid-let ((x 10)
            (y 20))
  (body))
```

When `body` calls functions, they see the new values of `x` and `y`.

### Implementation

```rust
// In evaluator.rs - add special form handler
"FLUID-LET" => {
    let args = list_to_vec(rest)?;
    if args.len() != 2 {
        return Err(LispError::Generic(
            "fluid-let takes exactly two arguments".to_string(),
        ));
    }

    let bindings_vec = list_to_vec(&args[0])?;
    let body = &args[1];

    // Save old values and create new bindings
    let mut old_values = vec![];
    for binding in &bindings_vec {
        let pair = list_to_vec(binding)?;
        if pair.len() != 2 {
            return Err(LispError::Generic("binding must be a pair".to_string()));
        }

        if let LispVal::Symbol(s) = &pair[0] {
            let name = s.borrow().name.clone();
            let old_val = env.get(&name);
            let new_val = eval(&pair[1], env)?;

            old_values.push((name.clone(), old_val));
            Environment::update(env, &name, new_val);
        }
    }

    // Evaluate body
    let result = eval(body, env);

    // Restore old values
    for (name, old_val) in old_values {
        if let Some(val) = old_val {
            Environment::update(env, &name, val);
        }
    }

    result
}
```

### Pros
- Simple to implement
- Clear when dynamic binding is used
- No global state changes

### Cons
- Limited scope (only within fluid-let)
- Doesn't match original Lisp 1.5 semantics

## Approach 3: Pure Dynamic Scope (Historical)

Make all variables dynamically scoped (original Lisp 1.5 behavior).

### Design

Replace lexical lookup with dynamic lookup everywhere.

### Implementation

```rust
// In lib.rs - modify Lambda to NOT capture environment
pub struct Lambda {
    pub params: Vec<String>,
    pub rest_param: Option<String>,
    pub body: Box<LispVal>,
    // REMOVE: pub env: Rc<Environment>,
}

// In evaluator.rs - apply lambda in caller's environment
LispVal::Lambda(lambda) => {
    // Use CALLER's environment, not captured environment
    let new_env = Environment::new_child(env);  // env is caller's env

    for (param, arg) in lambda.params.iter().zip(args) {
        new_env.set(param.clone(), arg.clone());
    }

    eval(&lambda.body, &new_env)
}

// In evaluator.rs - create lambda without capturing environment
"LAMBDA" => {
    // ... parse params and body ...
    Ok(LispVal::Lambda(crate::Lambda {
        params: params_vec,
        rest_param,
        body: Box::new(final_body),
        // REMOVE: env: env.clone(),
    }))
}
```

### Pros
- Historically accurate
- Simpler mental model (no closures)
- Matches original Lisp 1.5

### Cons
- **BREAKING CHANGE** - breaks all existing code
- Loses closure benefits
- Harder to reason about
- Not what modern Lisps use

## Approach 4: New Function Type (DLAMBDA)

Add a new dynamically-scoped lambda type alongside existing lexical lambdas.

### Design

```lisp
(dlambda (x) (+ x y))  ; y is looked up dynamically
(lambda (x) (+ x y))   ; y is looked up lexically (existing behavior)
```

### Implementation

```rust
// In lib.rs - add new variant
pub enum LispVal {
    // ... existing variants ...
    DynamicLambda(DynamicLambda),
}

pub struct DynamicLambda {
    pub params: Vec<String>,
    pub rest_param: Option<String>,
    pub body: Box<LispVal>,
    // NO env field - lookup happens at call time
}

// In evaluator.rs - add DLAMBDA special form
"DLAMBDA" => {
    // Parse like LAMBDA but create DynamicLambda
    Ok(LispVal::DynamicLambda(DynamicLambda {
        params: params_vec,
        rest_param,
        body: Box::new(final_body),
    }))
}

// In evaluator.rs - apply DynamicLambda
LispVal::DynamicLambda(dlambda) => {
    let new_env = Environment::new_child(env);  // Caller's env!

    for (param, arg) in dlambda.params.iter().zip(args) {
        new_env.set(param.clone(), arg.clone());
    }

    eval(&dlambda.body, &new_env)
}
```

### Pros
- Backward compatible
- Clear distinction between scoping modes
- Explicit choice per function

### Cons
- Adds complexity
- Two ways to do the same thing
- Need to remember which to use

## Recommendation

**Approach 1: Special Variables** is the best choice because:

1. **Historical accuracy**: Allows emulating Lisp 1.5 dynamic scope where needed
2. **Backward compatible**: Doesn't break existing code
3. **Industry standard**: Matches Common Lisp's approach
4. **Flexible**: Can have both lexical and dynamic variables
5. **Opt-in**: Dynamic scope only where explicitly requested

### Implementation Plan

1. Add `dynamic_vars: HashSet<String>` to `Environment`
2. Implement `DEFVAR` special form
3. Modify symbol lookup to check for dynamic variables
4. Add tests for dynamic binding behavior
5. Document in CLAUDE.md

### Example Usage

```lisp
;; Define a dynamic variable
(defvar *debug* nil)

;; Function that uses dynamic variable
(defun log-message (msg)
  (if *debug*
      (print msg)
      nil))

;; Temporarily enable debugging
(let ((*debug* t))
  (log-message "Debug is on"))  ; Prints "Debug is on"

;; Debug is off again
(log-message "Debug is off")  ; Returns nil
```

## Alternative: Quick Hack with Fluid-Let

If you want to experiment with dynamic scope quickly, **Approach 2: FLUID-LET** is the easiest to implement (about 30 lines of code) and gives you dynamic binding for testing purposes.

## Further Reading

- Original Lisp 1.5 Programmer's Manual (1962) - uses pure dynamic scope
- Common Lisp HyperSpec - special variables
- Scheme R7RS - parameters (similar concept)
- "The Roots of Lisp" by Paul Graham - discusses scope evolution
