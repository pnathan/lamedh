# Continuations and Coroutines Design for Lamedh

## Overview

This document explores how to add first-class continuations (like Scheme's `call/cc`) and coroutines to Lamedh. These are advanced control flow features that allow non-local control transfer and cooperative multitasking.

## Current State Analysis

### Lamedh's Current Evaluation Model

**Architecture**: Direct-style recursive interpreter
- `eval()` function calls itself recursively
- Call stack is the Rust call stack
- Return values propagate through Rust's return mechanism
- No explicit continuation representation

**Key limitations for continuations**:
1. **No reified continuations**: Cannot capture "what happens next"
2. **No stack manipulation**: Cannot save/restore evaluation state
3. **Direct recursion**: Control flow is implicit in Rust's stack

**Current control flow primitives**:
- `PROG`/`GO`/`RETURN`: Non-local exits via `LispError::Go` and `LispError::Return`
- These are limited to lexical scope (within a single PROG)

### What Continuations Provide

A **continuation** represents "the rest of the computation" - what should happen after the current expression evaluates.

```scheme
;; Scheme example
(+ 1 (call/cc (lambda (k)
                (+ 2 (k 3)))))
;; Returns 4, not 6!
;; k captures "the rest" which is (+ 1 [here])
;; Calling (k 3) aborts current computation and returns 3 to that context
```

### What Coroutines Provide

**Coroutines** are functions that can suspend execution and later resume from where they left off.

```lisp
;; Hypothetical Interlisp-style coroutine
(setq producer (coroutine (lambda ()
                            (progn
                              (resume consumer 1)
                              (resume consumer 2)
                              (resume consumer 3)))))

(setq consumer (coroutine (lambda (x)
                            (print x))))
```

## Historical Context

### Scheme's call/cc

[Scheme](https://www.scheme.com/tspl3/further.html) introduced `call-with-current-continuation` (call/cc), which:
- Takes a procedure `p` of one argument
- Constructs a concrete representation of the current continuation
- Passes it to `p` as a first-class function
- Can implement exceptions, backtracking, coroutines, generators, threads

### Interlisp's Spaghetti Stack

[Interlisp](https://123dok.net/article/spaghetti-stack-function-lambda-nil.yngm44wk) used a "spaghetti stack" designed by Bobrow and Hartley in 1970:
- Stack represented as a **tree of linked frames** instead of linear stack
- Frames had: variable bindings (BLINK), access link (ALINK), control link (CLINK)
- Allowed backtracking and coroutines
- Users could search/manipulate the stack directly
- Enabled evaluation in different contexts

### Common Lisp

Common Lisp does **not** have first-class continuations (too expensive to implement efficiently on traditional hardware). Instead it provides:
- Non-local exits: `catch`/`throw`, `block`/`return-from`
- Special operators for dynamic control flow
- No true coroutines (though libraries like `cl-cont` exist)

## Implementation Approaches

### Approach 1: Continuation-Passing Style (CPS) Transform

Transform the evaluator to pass continuations explicitly.

#### Design

[CPS](https://en.wikipedia.org/wiki/Continuation-passing_style) makes continuations explicit by passing them as arguments:

```rust
// Current signature
fn eval(val: &LispVal, env: &Rc<Environment>) -> Result<LispVal, LispError>

// CPS signature
fn eval_cps<K>(val: &LispVal, env: &Rc<Environment>, cont: K) -> Result<LispVal, LispError>
where K: FnOnce(LispVal) -> Result<LispVal, LispError>
```

Every recursive call becomes a tail call with a new continuation.

#### Example Transformation

```rust
// Current (direct style)
"IF" => {
    let cond_result = eval(&args[0], env)?;
    if is_truthy(&cond_result) {
        eval(&args[1], env)
    } else {
        eval(&args[2], env)
    }
}

// CPS style
"IF" => {
    eval_cps(&args[0], env, |cond_result| {
        if is_truthy(&cond_result) {
            eval_cps(&args[1], env, cont)
        } else {
            eval_cps(&args[2], env, cont)
        }
    })
}
```

#### Implementing call/cc

```rust
"CALL/CC" => {
    let args = list_to_vec(rest)?;
    if args.len() != 1 {
        return Err(LispError::Generic("call/cc requires one argument".to_string()));
    }

    // Capture current continuation as a LispVal
    let current_cont = capture_continuation(cont);

    // Call the function with the continuation
    let func = eval_cps(&args[0], env, |f| Ok(f))?;
    apply_cps(&func, &[current_cont], env, cont)
}

fn capture_continuation<K>(cont: K) -> LispVal
where K: FnOnce(LispVal) -> Result<LispVal, LispError> + 'static
{
    LispVal::Continuation(Box::new(cont))
}
```

#### Pros
- **Conceptually clean**: Makes control flow explicit
- **Enables call/cc**: Natural implementation
- **All calls become tail calls**: No stack growth
- **Used by Scheme compilers**: Well-understood technique

#### Cons
- **Complete rewrite required**: Every function in evaluator must be transformed
- **Complex implementation**: CPS is hard to write and maintain
- **Performance overhead**: Many heap allocations for closures
- **Rust limitations**: Lifetime and borrowing issues with boxed closures
- **No debug info**: Rust stack traces become unhelpful

### Approach 2: Stack Copying

Copy the Rust call stack when capturing a continuation.

#### Design

[Stack copying](https://link.springer.com/chapter/10.1007/978-3-540-40018-9_27) involves:
1. Represent active continuation as evaluation frames on stack
2. When capturing continuation: copy entire stack to heap
3. When invoking continuation: restore stack and continue

#### Implementation Strategy

```rust
// Add continuation support to eval
#[derive(Clone)]
struct EvalFrame {
    expr: LispVal,
    env: Rc<Environment>,
    state: FrameState,
}

enum FrameState {
    Evaluating,
    EvaluatedCond(LispVal), // For IF
    EvaluatingArgs(Vec<LispVal>), // For function calls
    // ... more states
}

// Global evaluation stack (thread_local!)
thread_local! {
    static EVAL_STACK: RefCell<Vec<EvalFrame>> = RefCell::new(Vec::new());
}

fn eval_with_stack(val: &LispVal, env: &Rc<Environment>) -> Result<LispVal, LispError> {
    EVAL_STACK.with(|stack| {
        stack.borrow_mut().push(EvalFrame {
            expr: val.clone(),
            env: env.clone(),
            state: FrameState::Evaluating,
        });

        let result = eval(val, env);

        stack.borrow_mut().pop();
        result
    })
}

// Capture continuation
fn capture_current_continuation() -> LispVal {
    EVAL_STACK.with(|stack| {
        let frames = stack.borrow().clone();
        LispVal::Continuation(Continuation {
            frames,
        })
    })
}

// Invoke continuation
fn invoke_continuation(cont: &Continuation, value: LispVal) -> Result<LispVal, LispError> {
    EVAL_STACK.with(|stack| {
        *stack.borrow_mut() = cont.frames.clone();
        Ok(value)
    })
}
```

#### Pros
- **Less invasive**: Can keep most of current eval structure
- **Easier to implement**: Incremental changes
- **Debugging works**: Stack traces still meaningful

#### Cons
- **Expensive**: Copying entire stack on each capture
- **Thread-local global state**: Not truly functional
- **Limited in Rust**: Can't actually capture Rust stack frames reliably
- **Still significant refactoring**: Need to track state explicitly

### Approach 3: Trampoline with Explicit Stack

Use a trampoline pattern with an explicit evaluation stack.

#### Design

Instead of recursion, use a loop with an explicit stack:

```rust
enum Bounce {
    Continue(LispVal, Rc<Environment>),
    Done(LispVal),
    CallCC(Box<dyn FnOnce(LispVal) -> Bounce>),
}

fn eval_trampoline(val: &LispVal, env: &Rc<Environment>) -> Result<LispVal, LispError> {
    let mut stack = vec![(val.clone(), env.clone())];

    loop {
        let (current_val, current_env) = stack.pop()
            .ok_or_else(|| LispError::Generic("Empty stack".to_string()))?;

        match eval_step(&current_val, &current_env)? {
            Bounce::Continue(next_val, next_env) => {
                stack.push((next_val, next_env));
            }
            Bounce::Done(result) => {
                if stack.is_empty() {
                    return Ok(result);
                }
                // Continue with result...
            }
            Bounce::CallCC(cont_func) => {
                // Capture stack as continuation
                let captured_stack = stack.clone();
                let cont = LispVal::Continuation(Continuation {
                    stack: captured_stack
                });
                // Apply cont_func...
            }
        }
    }
}
```

#### Pros
- **Explicit control flow**: Easy to see what's happening
- **Stack is data**: Can save/restore easily
- **No recursion**: Tail call optimization built-in
- **Rust-friendly**: No lifetime issues

#### Cons
- **Major rewrite**: Change from recursive to iterative
- **Complex state machine**: Many bounce cases
- **Performance**: More overhead than direct recursion
- **Debugging harder**: Control flow is indirect

### Approach 4: Delimited Continuations (shift/reset)

Implement delimited continuations instead of full continuations.

#### Design

[Delimited continuations](https://en.wikipedia.org/wiki/Delimited_continuation) capture only part of the stack:

```scheme
;; reset marks the boundary
(reset (+ 1 (shift k (+ 2 (k 3)))))
;; k captures only up to the reset: (+ 1 [here])
;; Returns 6
```

#### Implementation

```rust
// Add reset/shift special forms
"RESET" => {
    // Save current prompt
    let old_prompt = env.get_prompt();
    env.set_prompt(Prompt::new());

    let result = eval(&args[0], env)?;

    env.restore_prompt(old_prompt);
    Ok(result)
}

"SHIFT" => {
    // Capture continuation up to nearest reset
    let delim_cont = capture_delimited_continuation(env)?;

    // Call the function with the delimited continuation
    apply(&args[0], &[delim_cont], env)
}
```

#### Pros
- **More practical than full call/cc**: Easier to reason about
- **Composable**: Multiple resets can be nested
- **Efficient implementation**: Only capture partial stack
- **Modern approach**: Used in effect systems, algebraic effects

#### Cons
- **Still complex**: Requires explicit stack or CPS
- **Different semantics**: Not the same as Scheme's call/cc
- **Less historical**: Not what Lisp 1.5/MacLisp had

## Coroutines

### Symmetric Coroutines

Each coroutine can explicitly transfer control to any other.

```rust
#[derive(Clone)]
struct Coroutine {
    stack: Vec<EvalFrame>,
    env: Rc<Environment>,
    suspended_at: LispVal,
}

// Create coroutine
"COROUTINE" => {
    let lambda = eval(&args[0], env)?;
    Ok(LispVal::Coroutine(Coroutine {
        stack: vec![],
        env: env.clone(),
        suspended_at: lambda,
    }))
}

// Resume coroutine
"RESUME" => {
    let coroutine = &args[0];
    let value = &args[1];

    // Save current state
    save_current_coroutine();

    // Restore coroutine state
    restore_coroutine(coroutine, value)?;

    // Continue execution
    eval(&coroutine.suspended_at, &coroutine.env)
}
```

### Asymmetric Coroutines (Generators)

Simpler: only yield back to caller.

```rust
"YIELD" => {
    let value = eval(&args[0], env)?;

    // Suspend current coroutine
    Err(LispError::Yield(Box::new(value)))
}

// Catch yield in generator
fn run_generator(gen: &Generator) -> Result<LispVal, LispError> {
    match eval(&gen.body, &gen.env) {
        Err(LispError::Yield(val)) => {
            // Save state and return value
            gen.save_state();
            Ok(*val)
        }
        Ok(final_val) => {
            gen.mark_done();
            Ok(final_val)
        }
        Err(e) => Err(e),
    }
}
```

## Recommendation

Given Lamedh's current architecture and goals, I recommend:

### Short Term: Delimited Continuations (Approach 4)

**Why**:
1. **More practical**: Easier to use correctly than full call/cc
2. **Modern**: Aligns with effect systems and algebraic effects
3. **Incremental**: Can be added with explicit stack (Approach 3 lite)
4. **Educational**: Teaches important PL concepts

**Implementation**:
- Start with explicit eval stack (mini Approach 3)
- Add `reset`/`shift` special forms
- Implement generators as a use case

### Long Term: Consider Full call/cc via CPS (Approach 1)

**Why**:
1. **Historically accurate**: Matches Scheme
2. **Principled**: Clean semantics
3. **Enables everything**: call/cc is universal

**However**:
- Wait until you need it
- Consider if cost justifies benefit
- May be better as a separate project/fork

### For Coroutines: Asymmetric Generators

**Why**:
1. **Practical**: Most use cases are asymmetric
2. **Simple**: Easier to implement and use
3. **Python-like**: Familiar to many programmers

**Add**:
- `YIELD` special form
- Generator type with resumption
- Works well with delimited continuations

## Implementation Plan (Delimited Continuations + Generators)

### Phase 1: Explicit Evaluation Stack (2-3 days)

1. Add `EvalFrame` struct to track evaluation state
2. Add thread-local `EVAL_STACK`
3. Modify `eval()` to push/pop frames
4. Test that existing code still works

### Phase 2: Reset/Shift (2-3 days)

1. Add prompt markers to environment
2. Implement `RESET` special form
3. Implement `SHIFT` special form
4. Add tests for basic delimited continuations

### Phase 3: Generators (1-2 days)

1. Add `LispError::Yield` variant
2. Add `YIELD` special form
3. Add `Generator` type with state
4. Implement resume mechanism
5. Add tests for generators

### Phase 4: Documentation & Examples (1 day)

1. Document in CLAUDE.md
2. Add example programs using continuations
3. Add to standard library

## Example Use Cases

### Exception Handling (Reset/Shift)

```lisp
(defun safe-div (a b)
  (reset
    (if (= b 0)
        (shift k (cons 'error "Division by zero"))
        (/ a b))))

(safe-div 10 2)  ;; => 5
(safe-div 10 0)  ;; => (error . "Division by zero")
```

### Generators

```lisp
(defun range (n)
  (generator
    (prog ((i 0))
     loop
      (if (< i n)
          (progn
            (yield i)
            (setq i (+ i 1))
            (go loop))
          (return nil)))))

(def r (range 5))
(next r)  ;; => 0
(next r)  ;; => 1
(next r)  ;; => 2
```

### Backtracking Search

```lisp
(defun amb (choices)
  (shift k
    (mapcar k choices)))

(reset
  (let ((x (amb '(1 2 3)))
        (y (amb '(4 5 6))))
    (if (= (+ x y) 7)
        (list x y)
        (fail))))
;; => ((1 6) (2 5) (3 4))
```

## Further Reading

- [Scheme TSPL: Going Further](https://www.scheme.com/tspl3/further.html) - call/cc explanation
- [Continuation-Passing Style](https://en.wikipedia.org/wiki/Continuation-passing_style) - CPS overview
- [Interlisp Spaghetti Stack](https://123dok.net/article/spaghetti-stack-function-lambda-nil.yngm44wk) - Historical implementation
- [Implementation Strategies for First-Class Continuations](https://hlopko.com/2014/11/19/first-class-continuations/) - Comparison of techniques
- [Lazy Stack Copying](https://link.springer.com/chapter/10.1007/978-3-540-40018-9_27) - Optimization technique
- [CPS Interpreters](https://theincredibleholk.org/blog/2013/11/27/continuation-passing-style-interpreters/) - Practical guide

## Conclusion

Continuations and coroutines are powerful but complex features. The recommended approach is:

1. **Start with delimited continuations** (reset/shift) - more practical than full call/cc
2. **Add asymmetric generators** (yield) - cover most use cases
3. **Consider full call/cc later** if needed - requires CPS transform

This provides 80% of the value with 20% of the complexity, and matches modern language design trends (effect systems, async/await, etc.).
