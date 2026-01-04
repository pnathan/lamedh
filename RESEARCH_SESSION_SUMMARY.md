# Research Session Summary: Dynamic Scope, Continuations, and Spaghetti Stacks

## Session Overview

This research session explored advanced control flow mechanisms for Lamedh, starting with dynamic scope and expanding into continuations, coroutines, and the fascinating history of spaghetti stacks across Lisp, Prolog, and Forth.

## Documents Created

### 1. DYNAMIC_SCOPE_DESIGN.md
**Purpose**: Explore approaches to adding dynamic scoping to Lamedh

**Content**:
- Four approaches to dynamic scope:
  1. Special variables (Common Lisp style) ⭐ Recommended
  2. Fluid-let (temporary bindings)
  3. Pure dynamic scope (historical Lisp 1.5)
  4. DLAMBDA (new function type)
- Implementation strategies and trade-offs
- Examples and use cases

**Key Finding**: Lamedh currently uses lexical scoping (modern), not dynamic scoping like original Lisp 1.5 or MacLisp interpreters. Confirmed via test showing closures capture definition-time environment.

### 2. CONTINUATIONS_COROUTINES_DESIGN.md
**Purpose**: Explore adding first-class continuations and coroutines

**Content**:
- Four implementation approaches:
  1. CPS transformation (complete rewrite)
  2. Stack copying (heap-allocated frames)
  3. Trampoline with explicit stack (Rust-friendly)
  4. Delimited continuations (reset/shift) ⭐ Recommended
- Coroutines (symmetric and asymmetric/generators)
- Implementation complexity analysis
- M:N threading concerns

**Key Finding**: Full call/cc requires fundamental changes to evaluation model. Delimited continuations + generators provide practical middle ground without threading complexity.

**Recommendation**: Avoid mixing continuations with OS threads (M:N threading nightmare seen in Go, Haskell GHC, Erlang).

### 3. SPAGHETTI_STACKS_HISTORY.md
**Purpose**: Deep dive into alternative control flow mechanisms

**Content** (762 lines):
- **Interlisp's spaghetti stack** (Bobrow & Hartley, 1970):
  - ALINK/BLINK/CLINK frame structure
  - Tree of frames instead of linear stack
  - Stack manipulation primitives (STKPOS, RETFROM, EVALV)
  - Enabled backtracking, coroutines, debugging

- **Prolog's Warren Abstract Machine** (1983):
  - Four memory areas: heap, local stack, trail, PDL
  - Choice points for backtracking
  - Trail for undoing variable bindings

- **Forth's dual stack architecture** (~1970):
  - Data stack + return stack
  - User-accessible return stack (>R, R>, R@)
  - Explicit low-level control flow

- **Henry Baker's innovations**:
  - Shallow binding (1978): MacLisp's O(1) dynamic variable lookup
  - Cheney on the M.T.A. (1995): Use C stack as Scheme heap
  - Treadmill GC (1992): In-place garbage collection

- **The FUNARG problem**: Upward vs downward funargs
- **Actor model vs continuations**: Hewitt vs Sussman/Steele debate
- **Modern applications**: Cactus stacks, work-stealing, effect systems

**Key Insights**:
- Stack as first-class data structure unlocks powerful control flow
- ALINK ≠ CLINK enables lexical scope + higher-order functions
- Trade-offs: pushdown (fast, simple) vs tree (flexible, complex)

### 4. DYNAMIC_VARIABLES_SPEC.md ⭐ IMPLEMENTATION READY
**Purpose**: Complete specification for implementing dynamic variables

**Content**:
- **Syntax**: `(defdynamic symbol initial-value [docstring])` with `DEFVAR` alias
- **Naming convention**: `*earmuffs*` with warning for non-compliance
- **Semantics**: Dynamic lookup in caller environment vs lexical in definition environment
- **Implementation requirements**:
  - Data structures (`dynamic_vars: HashSet<String>`)
  - Methods (`is_dynamic()`, `mark_dynamic()`, `get_var()`)
  - DEFDYNAMIC special form
  - Symbol evaluation changes
- **Complete test suite**: Lisp + Rust tests
- **Edge cases**: Redefining, DEF vs DEFDYNAMIC, unbound variables
- **Integration**: Backward compatible, works with existing LET/SETQ

**Estimated implementation time**: ~3 hours

**Success criteria**:
- Variables with `*earmuffs*` get dynamic scoping
- Warning for variables without earmuffs
- Lexical and dynamic variables coexist
- All existing tests still pass

## Design Decisions

### Chosen Approach: Common Lisp Style Dynamic Variables

**Why**:
- ✅ Historically accurate (MacLisp evolved into this)
- ✅ Industry standard (Common Lisp, widespread understanding)
- ✅ Backward compatible (lexical scope remains default)
- ✅ Practical and useful (configuration, debugging, context)
- ✅ Low implementation complexity (~3 hours)
- ✅ No concurrency issues (single-threaded)

**Rejected**:
- ❌ Full call/cc: Too complex, requires CPS or stack copying
- ❌ Delimited continuations: Interesting but overkill for current goals
- ❌ Generators: Nice-to-have but not historical
- ❌ OS threads: Explicitly out of scope
- ❌ Pure dynamic scope: Breaking change to existing code
- ❌ Spaghetti stack: Fascinating history, impractical for implementation

### The M:N Threading Discussion

Confirmed that mixing continuations with OS threads is problematic:
- **Stack ownership**: Which thread's stack?
- **Synchronization**: Race conditions on captured environments
- **Stack copying**: Performance disaster
- **GC complexity**: When to collect continuation?
- **Thread-local storage**: Undefined behavior

**Wisdom from the field**:
- Go: M:N but NO continuations (only safe points)
- Erlang: Separate heaps, message passing, no shared continuations
- Haskell GHC: Very complex runtime, famously difficult bugs
- Most Scheme implementations: Forbid call/cc across threads or use GIL

**Decision**: Stay single-threaded, keep it simple.

## Historical Context Learned

### Evolution of Control Flow (1960s-2020s)

| Era | Innovation | Example | Key Idea |
|-----|------------|---------|----------|
| 1960s | Hardware pushdown stack | Fortran, Algol | Linear, fast, simple |
| 1970 | Spaghetti stack | Interlisp | Tree of frames, ALINK≠CLINK |
| 1970 | Dual stacks | Forth | User-accessible return stack |
| 1973 | Actor model | Hewitt | Continuations as reply-to |
| 1975 | call/cc | Scheme | First-class continuations |
| 1978 | Shallow binding | MacLisp | O(1) dynamic lookup |
| 1983 | WAM | Prolog | Structured backtracking |
| 1992 | Treadmill GC | Baker | In-place GC for continuations |
| 1995 | Cheney on M.T.A. | Baker | C stack as Scheme heap |
| 2000s+ | Effect systems | OCaml 5, Koka | Delimited continuations |

### The FUNARG Problem

**Downward FUNARG**: Passing functions as arguments
- Solution: Stack allocation works (frame still alive)

**Upward FUNARG**: Returning functions that outlive their frame
- Solutions:
  - Forbid it (Pascal, early Lisps)
  - Heap-allocated closures (Scheme, Common Lisp)
  - Spaghetti stack (Interlisp)

### MacLisp's Hybrid Approach

**Interpreter**: Dynamic scope (fast with shallow binding)
**Compiler**: Lexical scope (optimization, but no full closures)
**Special declarations**: Mark variables as dynamic in compiled code

This evolved into Common Lisp's special variables.

## Next Steps

### Immediate: Implement Dynamic Variables
1. Use `DYNAMIC_VARIABLES_SPEC.md` as implementation guide
2. Add `dynamic_vars` HashSet to Environment
3. Implement DEFDYNAMIC/DEFVAR special form
4. Modify symbol evaluation to check `is_dynamic()`
5. Run test suite
6. Update CLAUDE.md documentation

### Future Possibilities (Not Committed)
- Catch/throw for exception handling (extends PROG/GO/RETURN)
- Generators (if lazy evaluation becomes important)
- Delimited continuations (if effect systems interest grows)
- Performance optimization: Shallow binding (if dynamic variables used heavily)

### Not Planned
- Full call/cc (too complex for current goals)
- OS threads (out of scope)
- Spaghetti stack (historical interest only)

## Files Modified

All on branch `claude/lisp-dynamic-scope-research-Liy0r`:

```
DYNAMIC_SCOPE_DESIGN.md            (324 lines) - Four approaches analysis
CONTINUATIONS_COROUTINES_DESIGN.md (558 lines) - Continuations implementation
SPAGHETTI_STACKS_HISTORY.md        (762 lines) - Deep historical dive
DYNAMIC_VARIABLES_SPEC.md          (754 lines) - Implementation specification
RESEARCH_SESSION_SUMMARY.md        (this file) - Session summary
```

**Total**: ~2,400 lines of research, analysis, and specifications

## Key Takeaways

1. **Lamedh is lexically scoped** (modern), not dynamically scoped (historical Lisp 1.5)

2. **Dynamic variables are the right addition** because:
   - Historically informed (MacLisp → Common Lisp)
   - Practically useful (configuration, context)
   - Low complexity (few hours to implement)
   - No concurrency issues

3. **Continuations are fascinating but overkill** for current goals:
   - Require fundamental changes (CPS or stack copying)
   - Complex interactions with threads
   - Better suited for specialized languages

4. **Spaghetti stacks are a deep idea** about control flow:
   - Stack as tree instead of linear structure
   - Three pointers (ALINK/BLINK/CLINK) separate lexical from dynamic
   - Influenced modern systems (cactus stacks, work-stealing, effects)
   - Historical importance > practical implementation for Lamedh

5. **Simple is better**: Focus on features that provide value without complexity

## References

All primary sources, papers, and documentation linked throughout the documents. Key resources:

- Bobrow & Wegbreit (1973) - Spaghetti stack paper
- Baker (1978, 1992, 1995) - Shallow binding, Treadmill, Cheney on M.T.A.
- Warren (1983) - Warren Abstract Machine
- Interlisp Reference Manuals (1974, 1978, 1983)
- Common Lisp HyperSpec - Special variables
- Scheme standards - Continuations
- Academic papers on cactus stacks, work-stealing, effect systems

## Conclusion

This session provided comprehensive research on dynamic scope, continuations, and alternative control flow mechanisms. The pragmatic recommendation is to implement **Common Lisp-style dynamic variables** using the complete specification in `DYNAMIC_VARIABLES_SPEC.md`.

This gives Lamedh:
- ✅ Historical connection (Lisp 1.5 used dynamic scope)
- ✅ Modern best practices (Common Lisp approach)
- ✅ Practical utility (configuration, debugging)
- ✅ Backward compatibility (existing code unchanged)
- ✅ Manageable complexity (~3 hour implementation)

The other explorations (continuations, spaghetti stacks) provide valuable context and are documented for future reference, but are not recommended for immediate implementation.
