# The Spaghetti Stack: A Deep Dive into Alternative Control Flow

## Introduction

The **spaghetti stack** is one of the most fascinating innovations in programming language implementation. Unlike the traditional pushdown stack, it represents the call stack as a *tree* of frames, enabling powerful control flow features like backtracking, coroutines, and first-class continuations.

This document explores the spaghetti stack and related innovations across Lisp, Prolog, Forth, and other language implementations, tracing the evolution of control flow mechanisms from the 1960s to today.

## The Traditional Stack Problem

### Why Traditional Stacks Are Limited

A **pushdown stack** is a linear data structure:
```
[Frame N]      ← Stack pointer (most recent)
[Frame N-1]
[Frame N-2]
...
[Frame 1]
[Frame 0]      (base)
```

**Properties**:
- LIFO (Last In, First Out)
- Only the top frame is accessible
- Frames are destroyed when popped
- Control flow is strictly nested
- Fast: O(1) push/pop

**Limitations**:
- **Cannot save continuations**: Once a frame is popped, it's gone
- **Cannot backtrack**: Can't return to earlier choice points
- **Cannot implement coroutines**: Can't switch between multiple call stacks
- **No tree-structured control flow**: Can't have multiple "next" states

### The FUNARG Problem

The [FUNARG problem](https://groups.google.com/g/comp.lang.lisp/c/miEM_T9yL0s) arose in early Lisp when trying to pass functions as arguments or return them as results:

**Downward FUNARG**: Passing functions down as arguments
```lisp
;; Pass a function that references x
(defun caller ()
  (let ((x 10))
    (callee (lambda () x))))  ; lambda captures x
```
Solution: Stack allocation works! The frame containing `x` is still alive when lambda executes.

**Upward FUNARG**: Returning functions that outlive their environment
```lisp
;; Return a function that references x
(defun make-adder (x)
  (lambda (y) (+ x y)))  ; x must outlive make-adder's frame!

(setq add5 (make-adder 5))
(funcall add5 10)  ; => 15, but x's frame is gone!
```
Solution: Need heap-allocated closures or spaghetti stack!

Early Lisps like MacLisp and Franz Lisp handled downward FUNARGs but **forbade upward FUNARGs** - you couldn't return local functions. Common Lisp and Scheme require full lexical closures.

## Interlisp's Spaghetti Stack (1970)

### Origins: Bobrow and Wegbreit (1973)

In 1970, **Daniel Bobrow** and **Alice K. Hartley** designed and implemented the "spaghetti stack" for Interlisp. The theoretical foundation was published as ["A Model and Stack Implementation for Multiple Environments"](https://123dok.net/article/spaghetti-stack-function-lambda-nil.yngm44wk) by Bobrow and Wegbreit in *Communications of the ACM*, Vol. 16, No. 10, October 1973.

### Frame Structure

Instead of a linear stack, frames form a **tree**:

```
                [Frame A]
                /       \
          [Frame B]   [Frame C]
              /            \
        [Frame D]      [Frame E]
```

Each **frame extension** (frame) contains:

1. **Frame name**: Identifier (function name)
2. **BLINK** (Binding Link): Pointer to variable bindings
3. **ALINK** (Access Link): Pointer to lexical parent (for variable lookup)
4. **CLINK** (Control Link): Pointer to caller (for returns)

### The Three Links Explained

```lisp
;; Example
(defun outer (x)          ; Frame 1
  (defun inner (y)        ; Frame 2
    (+ x y))              ; References outer's x
  (inner 5))

;; When inner executes:
;; Frame 2:
;;   BLINK → bindings for y (y=5)
;;   ALINK → Frame 1 (to find x)
;;   CLINK → Frame 1 (to return to outer)
```

**BLINK**: Points to this frame's local variables
- Stores bindings created in this function call
- Forms the "binding environment"

**ALINK**: Points to lexical parent frame
- Used for lexical scoping (finding free variables)
- Follows the static nesting structure
- `inner` needs `outer`'s `x`, so ALINK → Frame 1

**CLINK**: Points to caller frame
- Used for returns (control flow)
- Follows the dynamic call chain
- When `inner` returns, follow CLINK back to caller

### Why Three Separate Links?

In traditional languages with dynamic scope, ALINK = CLINK (caller is also lexical parent).

In Lisp with lexical scope and higher-order functions:
- ALINK follows **lexical nesting** (where function was defined)
- CLINK follows **call chain** (where function was called)
- These can be different!

```lisp
;; ALINK ≠ CLINK example
(defun make-adder (x)        ; Frame 1
  (lambda (y) (+ x y)))      ; Frame 2 (closure)

(setq add5 (make-adder 5))   ; ALINK → Frame 1 (for x)

(defun caller ()             ; Frame 3
  (funcall add5 10))         ; Frame 4 (executing closure)

;; When closure executes (Frame 4):
;;   BLINK → bindings for y (y=10)
;;   ALINK → Frame 1 (to find x=5, where closure was DEFINED)
;;   CLINK → Frame 3 (caller, where closure was CALLED)
```

### Advanced Features

[Interlisp provided primitives](https://www.softwarepreservation.org/projects/LISP/interlisp/Interlisp-Oct_1974.pdf) for stack manipulation:

**STKPOS**: Get stack pointer to frame
```lisp
(stkpos 'function-name n pos)
;; Returns pointer to Nth frame with given name
;; Search along CLINK if n < 0 (control chain)
;; Search along ALINK if n > 0 (access chain)
```

**RETFROM**: Return from arbitrary frame
```lisp
(retfrom pos value)
;; Exit from frame at pos with value
;; Unwind stack to that point
```

**EVALV**: Evaluate in different context
```lisp
(evalv expr pos)
;; Evaluate expr in environment of frame pos
;; ALINK temporarily points to different frame
```

These enabled:
- **Backtracking**: Save frame pointers, return to earlier choice points
- **Coroutines**: Switch between frames in different branches
- **Debugging**: Inspect/modify variables in any frame
- **Exception handling**: Non-local exits

### Spaghetti Stack vs Cactus Stack

The terms are related but slightly different:

**Spaghetti Stack** (Interlisp):
- Full tree structure with ALINK/BLINK/CLINK
- Frames can have multiple children (multiple possible continuations)
- Enables backtracking to any earlier frame

**[Cactus Stack](https://en.wikipedia.org/wiki/Parent_pointer_tree)** (modern term):
- Tree where each node has parent pointer
- Used in functional languages for closures
- Garbage collected when no longer referenced
- Simpler than full spaghetti stack (just parent links, not three separate links)

Modern functional languages use cactus stacks:
```
Each closure points to its defining frame:

    [Global]
       |
    [Frame A]
     /     \
[Frame B] [Frame C]
```

Frames are heap-allocated and garbage collected.

## Prolog's Warren Abstract Machine (1983)

### The WAM Memory Architecture

David H. D. Warren designed the [Warren Abstract Machine (WAM)](https://en.wikipedia.org/wiki/Warren_Abstract_Machine) in 1983 as the standard target for Prolog compilation. It has **four memory areas**:

1. **Heap** (global stack): Store compound terms
2. **Local Stack**: Environment frames and choice points
3. **Trail**: Record variable bindings for undo
4. **PDL** (Push Down List): Temporary storage during unification

### Choice Points: Structured Backtracking

When multiple clause heads match, the WAM creates a **choice point**:

```prolog
append([], L, L).
append([H|T1], L2, [H|T3]) :- append(T1, L2, T3).

?- append([1,2], [3,4], X).
```

**Choice point structure**:
```
┌─────────────────────────┐
│ Continuation Ptr (CP)   │  Where to continue after success
│ Previous Choice (B)      │  Link to previous choice point
│ Trail Pointer (TR)       │  Position in trail for undo
│ Heap Pointer (H)         │  Heap state for backtracking
│ Next Clause (L)          │  Address of alternative clause
│ Saved Arguments          │  Original query arguments
└─────────────────────────┘
```

**Backtracking mechanism**:
1. On failure: Restore state from choice point
2. Undo variable bindings using trail
3. Reset heap pointer
4. Try next clause

**The Trail**: Linear array recording bindings
```
[Var1 → Term1]
[Var2 → Term2]
[Var3 → Term3]
...
```

On backtrack: Walk trail backwards, unbind variables.

### Similarity to Spaghetti Stack

WAM choice points are like spaghetti stack frames:
- Multiple choice points can be active (tree structure)
- Can backtrack to any choice point (non-linear control)
- Must save/restore state
- Heap allocation for persistence

Key difference:
- Spaghetti stack: Arbitrary frame switching
- WAM: Structured backtracking (chronological, stack-based)

## Forth's Dual Stack Architecture (1970)

### Two Stacks: Data and Return

[Forth](https://en.wikipedia.org/wiki/Forth_(programming_language)) (created by Charles Moore ~1970) uses **two stacks**:

**Data Stack**: Expression evaluation
```
: SQUARE ( n -- n² )
  DUP *  ; \ Duplicate top, multiply

5 SQUARE  \ Data stack: [5] → [5 5] → [25]
```

**Return Stack**: Control flow and temporaries
```
: SAVE-X ( n1 n2 -- n2 )
  SWAP >R    \ Move n1 to return stack
  ( do work with n2 )
  R> DROP ;  \ Restore and discard n1
```

### Return Stack Operations

[Three key words](https://www.complang.tuwien.ac.at/forth/gforth/Docs-html/Return-Stack-Tutorial.html):

- **>R** (to-R): Move top of data stack to return stack
- **R>** (from-R): Move top of return stack to data stack
- **R@** (R-fetch): Copy top of return stack to data stack

**Critical constraint**: Must be balanced within a word!
```forth
: BROKEN
  >R        \ Push to return stack
  ;         \ WRONG! Return address corrupted

: CORRECT
  >R        \ Push to return stack
  ...
  R>        \ Pop from return stack
  ;         \ Balanced
```

### How Returns Work

```forth
: INNER
  42 ;

: OUTER
  INNER
  PRINT ;

\ Execution:
\ OUTER called
\ → Push return address (after INNER call) to return stack
\ → Jump to INNER
\ INNER executes
\ → 42 on data stack
\ → Pop return address from return stack
\ → Jump to that address (back to OUTER)
\ PRINT executes
```

The return stack IS the call stack, but it's also **user-accessible** for temporary storage!

### Why Dual Stacks?

**Advantages**:
- **Fast parameter passing**: No registers needed, just push/pop
- **Explicit control**: Can manually manipulate return addresses
- **Temporary storage**: Use return stack as scratch space
- **Tail call optimization**: Just jump, no return address needed

**Constraints**:
- Must balance return stack operations
- Can't use >R/R> across loop boundaries
- Loops use return stack for index, so no room for user data

### Forth and Continuations?

Forth's return stack is **too exposed** for safe continuations:
- User code can corrupt return addresses
- No frame boundaries (just raw stack)
- No automatic save/restore

But Forth's philosophy: "Give programmers power, trust them not to shoot their foot."

Some Forth systems implement coroutines by **swapping return stack pointers**:
```forth
: YIELD
  RSP@         \ Get current return stack pointer
  SAVE-COROUTINE
  LOAD-OTHER-COROUTINE
  RSP! ;       \ Switch to other return stack
```

## Henry Baker's Innovations

### Shallow Binding (1978)

Baker's paper ["Shallow Binding in Lisp 1.5"](https://www.plover.com/misc/hbaker-archive/ShallowBinding.html) (CACM 1978) explained MacLisp's optimization:

**Problem in Lisp 1.5**: Dynamic scoping via association lists
```lisp
;; Environment: ((x . 1) (y . 2) (z . 3))
;; To find x: Linear search through list - O(n)
```

**MacLisp's solution**: Value cells with shallow binding
```
Each symbol has a value cell (direct pointer to value)

Symbol X → [Value Cell: 42]

;; Lookup: O(1) - just dereference pointer
```

**On function call**:
1. Save old value cells to **specpdl** (special bindings stack)
2. Update value cells with new bindings
3. Execute function (all lookups are O(1))
4. Restore old values from specpdl on return

**Trade-off**:
- Variable access: O(1) (was O(n))
- Context switch: O(n) where n = number of bindings
- Win: Access >> switches in typical programs

This is "shallow binding" - current bindings are at top level (shallow), not deep in a stack.

**Limitation**: Can't handle upward FUNARGs!
```lisp
(defun make-adder (x)
  (lambda (y) (+ x y)))

;; When make-adder returns:
;; - Specpdl restored
;; - x's value cell points to ???
;; - Lambda's reference to x is BROKEN
```

Solution: Need heap-allocated closures (which MacLisp compiler added).

### Cheney on the M.T.A. (1995)

Baker's ["CONS Should Not CONS Its Arguments, Part II: Cheney on the M.T.A."](http://home.pipeline.com/~hbaker1/CheneyMTA.html) (1995) showed how to compile Scheme to C:

**Key insight**: Use C's stack as Scheme's heap!

**The technique**:
1. **Transform to CPS**: All functions tail-call continuations
2. **Never return**: C functions never execute `return`
3. **Allocate on C stack**: All data is `auto` variables
4. **Stack = heap**: C stack grows forever (until GC)
5. **GC triggers on stack overflow**: Copy live data to new area
6. **Restart with fresh stack**: Jump to continuation with new stack

```c
// Normal C
int fact(int n) {
  if (n == 0) return 1;
  return n * fact(n-1);
}

// CPS (Cheney on the MTA)
void fact_cps(int n, void (*cont)(int)) {
  if (n == 0) {
    cont(1);  // Tail call - never returns
  } else {
    fact_cps(n-1, [=](int result) {  // Capture continuation
      cont(n * result);
    });
  }
}
```

**Garbage collection**:
```
C Stack (grows down):
┌────────────────┐ ← Stack limit
│                │
│ Live closures  │
│ Live data      │
│                │
├────────────────┤ ← Stack pointer (allocation pointer)
│                │
│ Dead frames    │ (Will be collected)
│                │
└────────────────┘

On overflow:
1. Walk live objects from roots
2. Copy to new memory area (Cheney copying GC)
3. Reset stack pointer to bottom
4. Jump to continuation
```

**Used by**:
- [CHICKEN Scheme](https://wiki.call-cc.org/chicken-compilation-process)
- [Cyclone Scheme](https://justinethier.github.io/cyclone/docs/Garbage-Collector)

**Why "M.T.A."?**: Reference to "Charlie on the MTA", a folk song about a man trapped on the Boston subway (he never returns!). The C functions never return, just like Charlie.

### Treadmill GC (1992)

Baker's ["The Treadmill: Real-Time Garbage Collection without Motion Sickness"](http://home.pipeline.com/~hbaker1/NoMotionGC.html) (1992) presented a non-moving collector for systems with continuations:

**Problem**: Copying GC moves objects, breaking C pointers
**Solution**: In-place GC using a circular doubly-linked list

**The Treadmill**: All objects on one circular list
```
    ┌───────────────────────────────┐
    │                               │
    ▼                               │
[Black] → [Black] → [Grey] → [White] → [White]
             ▲                          │
             └──────────────────────────┘

Scan pointer moves objects from White → Grey → Black
```

**Tri-color marking**:
- **White**: Not scanned, may be garbage
- **Grey**: Scanned but children not yet scanned
- **Black**: Scanned, all children scanned, definitely live

**Incremental**: Move a few objects per allocation
**Real-time**: Bounded pause times
**No motion**: Objects never move in memory

Critical for systems with C FFI or first-class continuations where object addresses matter.

## The Actor Model and Continuations (1973)

### Hewitt's Actor Model

[Carl Hewitt's Actor Model](https://en.wikipedia.org/wiki/Actor_model) (1973) proposed computation as asynchronous message passing:

**Actor**: Independent unit with:
- Private state
- Mailbox for messages
- Behavior (how to process messages)

**Operations**:
- **Send**: Asynchronously send message to actor
- **Create**: Create new actor
- **Become**: Change behavior for next message

```
Actor A            Actor B
  │                  │
  ├──message────────→│
  │                  │ Process message
  │                  ├── Create new actor C
  │                  ├── Become (new behavior)
  │                  └── Send to C
  │
```

### Continuations as Reply-To Addresses

**Key insight**: The "reply-to" address in a message IS a continuation!

```
Send message: (operation args... reply-to)
                                  ^^^^^^^^
                                  Continuation: where to send result
```

Traditional style:
```lisp
(let ((result (call-function args)))
  (continue-with result))
```

Actor style:
```lisp
(send function-actor args (actor (result)
  (continue-with result)))
```

The continuation is reified as an actor!

### Scheme vs. Actors: The Great Debate

**Sussman and Steele (1975)**: ["Scheme: An Interpreter for Extended Lambda Calculus"](https://en.wikipedia.org/wiki/Lambda_the_Ultimate)

Their conclusion:
> "We discovered that the 'actors' and the lambda expressions were identical in implementation."

They found:
- Actors = Closures that never return but invoke continuations
- Message passing = Function application with continuation
- Therefore: Actors ≈ Lambda calculus + continuations

**Hewitt's response**: "Not so fast!"

[Hewitt argued](https://en.wikipedia.org/wiki/History_of_the_Actor_model):
- Actors have **true concurrency** (parallel message processing)
- Lambda calculus is **sequential**
- Actors can change **local state**
- Actor message passing is **asynchronous**
- **Fairness** guarantees in message delivery

The debate highlighted fundamental questions:
- Can sequential lambda calculus express parallelism?
- Are continuations enough for concurrency?
- What's the difference between concurrency and parallelism?

**Historical impact**:
- Scheme got `call/cc` from this investigation
- Actor model influenced Erlang, Akka, Orleans
- Both models are still used today

## Modern Applications

### Cactus Stacks in Functional Languages

Modern functional languages use heap-allocated frame trees:

**Haskell**: Lazy evaluation creates complex frame graphs
```haskell
-- Each thunk captures its environment
let x = expensive_computation
    y = another_computation x
    z = third_computation y
in if condition then y else z

-- Frames form tree: only evaluate needed branches
```

**Concurrent Haskell**: Green threads with separate cactus stacks
- Each thread has own stack (cactus branch)
- Parent frames shared
- Garbage collected when unreachable

### Work-Stealing Runtime Systems

[Parallel runtimes use cactus stacks](https://www.cse.wustl.edu/~angelee/home_page/papers/stacks.pdf) for work stealing:

```
Thread 1's stack:        Thread 2's stack:
    [Frame A]                [Frame A]  (shared parent)
       │                        │
    [Frame B]                [Frame C]
       │
    [Frame D]

Thread 2 "steals" Frame C to work in parallel
```

Used by:
- Cilk/Cilk Plus
- Intel TBB
- Java Fork/Join Framework
- Go's goroutines (with some differences)

### Effect Systems and Algebraic Effects

Modern effect systems (OCaml 5, Koka, Eff) use delimited continuations:

```ocaml
(* OCaml 5 effects *)
effect Ask : string -> int

let computation () =
  let x = perform (Ask "What is 2+2?") in
  x * 10

let () =
  try computation () with
  | effect (Ask question) k ->
      continue k 4  (* k is delimited continuation *)
```

The continuation `k` captures "what happens next" (multiply by 10).

This is implemented with **cactus stacks** and **stack copying**.

## Implementation Strategies Summary

| Approach | Structure | Allocation | GC | Use Case |
|----------|-----------|------------|-----|----------|
| **Pushdown Stack** | Linear | Stack | None | Simple calls, no continuations |
| **Spaghetti Stack** | Tree (ALINK/BLINK/CLINK) | Heap | Mark-sweep | Backtracking, coroutines, debugging |
| **Cactus Stack** | Tree (parent pointers) | Heap | GC | Functional languages, closures |
| **WAM (Prolog)** | Hybrid (stack + trail) | Stack+heap | Trail-based | Logic programming, backtracking |
| **Forth Dual Stack** | Two linear stacks | Stack | None | Low-level control, embedded |
| **Shallow Binding** | Value cells + specpdl | Mixed | Traditional | Fast dynamic scoping (MacLisp) |
| **Cheney on MTA** | C stack as heap | "Stack" | Copying GC | Compiling Scheme to C |
| **Treadmill** | Circular list | Heap | In-place | Real-time, no motion |

## Trade-offs

### Time Complexity

| Operation | Pushdown | Spaghetti | Cactus | Shallow Binding |
|-----------|----------|-----------|--------|-----------------|
| Call | O(1) | O(1) | O(1) | O(n) save bindings |
| Return | O(1) | O(1) | O(1) | O(n) restore bindings |
| Var lookup | O(1) | O(d) walk ALINK | O(d) walk parents | O(1) value cell |
| Save continuation | ✗ | O(1) copy pointer | O(1) copy pointer | ✗ |
| Backtrack | ✗ | O(1) | O(1) | ✗ |

d = lexical depth

### Space Complexity

**Pushdown**: O(depth of calls)
**Spaghetti/Cactus**: O(total frames created) - GC reclaims unreachable
**WAM**: O(depth) + O(choice points)
**Shallow binding**: O(depth) for specpdl

### When to Use What?

**Pushdown stack**:
- Simple languages (C, Pascal)
- No continuations needed
- Performance critical

**Spaghetti/Cactus stack**:
- First-class continuations
- Backtracking
- Coroutines
- Functional languages with closures

**WAM-style (Prolog)**:
- Logic programming
- Structured backtracking
- Unification

**Dual stack (Forth)**:
- Embedded systems
- Explicit low-level control
- Minimal runtime

**Shallow binding**:
- Dynamic scoping with performance requirements
- When upward FUNARGs not needed
- Can combine with closures for full solution

## Conclusion

The evolution from pushdown stacks to spaghetti stacks represents a fundamental shift in how we think about control flow:

**1960s**: Stack is hardware, control is linear (Fortran, Algol)
**1970s**: Stack is data structure, control is tree (Interlisp, Prolog, Forth)
**1980s**: Heap-allocated frames, continuations (Scheme, Common Lisp)
**1990s**: Compilation techniques (Cheney on MTA, CPS)
**2000s-today**: Effects, async/await, work-stealing (OCaml 5, Rust, Go)

Key insights:
- **Stack as data**: Once you heap-allocate frames, they become first-class
- **Multiple pointers**: ALINK ≠ CLINK enables lexical scope + higher-order functions
- **Tree structure**: Enables backtracking, coroutines, parallelism
- **Garbage collection**: Essential for frames that outlive their callers
- **Trade-offs**: Speed vs. flexibility, space vs. time

The spaghetti stack isn't just a clever hack - it's a deep idea about the relationship between control flow and data structures. Every modern language with closures, continuations, or coroutines uses some variant of this idea.

## References

### Primary Sources

- Bobrow & Wegbreit, ["A Model and Stack Implementation for Multiple Environments"](https://123dok.net/article/spaghetti-stack-function-lambda-nil.yngm44wk), CACM 1973
- Baker, ["Shallow Binding in Lisp 1.5"](https://www.plover.com/misc/hbaker-archive/ShallowBinding.html), CACM 1978
- Warren, ["An Abstract Prolog Instruction Set"](https://en.wikipedia.org/wiki/Warren_Abstract_Machine), Technical Note 309, SRI International, 1983
- Baker, ["The Treadmill: Real-Time Garbage Collection without Motion Sickness"](http://home.pipeline.com/~hbaker1/NoMotionGC.html), ACM SIGPLAN Notices 1992
- Baker, ["CONS Should Not CONS Its Arguments, Part II: Cheney on the M.T.A."](http://home.pipeline.com/~hbaker1/CheneyMTA.html), ACM SIGPLAN Notices 1995

### Historical Context

- Hewitt et al., ["Actor Model"](https://en.wikipedia.org/wiki/Actor_model), 1973
- Sussman & Steele, ["Scheme: An Interpreter for Extended Lambda Calculus"](https://en.wikipedia.org/wiki/Lambda_the_Ultimate), AI Memo 349, MIT, 1975
- Steele & Gabriel, "The Evolution of Lisp", ACM HOPL-II, 1993
- [Interlisp Reference Manual](https://www.softwarepreservation.org/projects/LISP/interlisp/Interlisp-Oct_1974.pdf), 1974, 1978, 1983

### Implementation Resources

- [Gforth Return Stack Tutorial](https://www.complang.tuwien.ac.at/forth/gforth/Docs-html/Return-Stack-Tutorial.html)
- [CHICKEN Scheme Compilation Process](https://wiki.call-cc.org/chicken-compilation-process)
- [Work-Stealing with Cactus Stacks](https://www.cse.wustl.edu/~angelee/home_page/papers/stacks.pdf)
- [Understanding Prolog WAM](https://www.researchgate.net/publication/259692583_Understanding_the_Compilation_of_PrologTowards_a_Clear_and_Good_Description_and_an_Interactive_Tutorial_of_the_Warren_Abstract_Machine)

### Wikipedia and General

- [Warren Abstract Machine](https://en.wikipedia.org/wiki/Warren_Abstract_Machine)
- [Forth Programming Language](https://en.wikipedia.org/wiki/Forth_(programming_language))
- [Parent Pointer Tree (Cactus Stack)](https://en.wikipedia.org/wiki/Parent_pointer_tree)
- [History of the Actor Model](https://en.wikipedia.org/wiki/History_of_the_Actor_model)
- [Alice K. Hartley](https://en.wikipedia.org/wiki/Alice_K._Hartley)

### Modern Applications

- [OCaml 5 Effects](https://www.metalevel.at/prolog/efficiency)
- [Cyclone Scheme GC](https://justinethier.github.io/cyclone/docs/Garbage-Collector)
- [Cactus Stack Implementations](https://github.com/softdevteam/cactus)
