# Lisp 1.5 Implementation TODO

This file lists the tasks required to implement a relaxed superset of Lisp 1.5, based on the provided technical specification.

## Part I: Data Types and Representation

-   [ ] **Atomic Symbols**:
    -   [ ] Modify the `reader` to enforce the strict `LETTER (LETTER | DIGIT)*` syntax for atomic symbols. The current implementation is too permissive.
-   [ ] **Numbers**:
    -   [ ] Add a `Float` variant to the `LispVal` enum.
    -   [ ] Update the `reader` to parse floating-point numbers (e.g., `3.14`, `-2.5E+10`).
    -   [ ] Add an `Octal` variant to the `LispVal` enum.
    -   [ ] Update the `reader` to parse octal numbers (e.g., `1750Q`).
    -   [ ] Ensure `T` and `F` are parsed and evaluated correctly (`*T*` and `NIL`). The current implementation uses `t` and `nil`.

## Part II: Elementary Functions

-   [ ] **Predicates**:
    -   [ ] Implement `null[x]`.
    -   [ ] Implement `equal[x;y]` for recursive structural equality. The current `eq` is insufficient.
    -   [ ] Modify `eq[x;y]` to be defined *only* for atomic arguments, as per the spec.

## Part III: Special Forms

-   [ ] **LABEL**:
    -   [ ] Implement the `LABEL` special form for defining recursive functions.
-   [ ] **FUNCTION**:
    -   [ ] Review if the current closure implementation for `LAMBDA` is sufficient to cover the `FUNCTION` special form's purpose (solving the FUNARG problem). Add `FUNCTION` if it's deemed necessary for full compliance or if it provides distinct behavior.

## Part IV: Program Feature (PROG)

-   [ ] **PROG**:
    -   [ ] Implement the `PROG` special form to create a local variable scope and execute statements sequentially.
    -   [ ] Implement `SETQ` for variable assignment within `PROG` and globally.
    -   [ ] Implement `GO` for transferring control to labels within a `PROG`.
    -   [ ] Implement `RETURN` for exiting a `PROG` with a value.

## Part V: List Processing Functions

-   [ ] **List Construction and Manipulation**:
    -   [ ] Implement `append[x;y]`.
    -   [ ] Implement `member[x;y]`.
    -   [ ] Implement `length[x]`.
    -   [ ] Implement `reverse[x]`.
-   [ ] **Substitution and Association**:
    -   [ ] Implement `subst[x;y;z]`.
    -   [ ] Implement `assoc[x;a]`.
    -   [ ] Implement `pairlis[x;y;a]`.
-   [ ] **Mapping Functions**:
    -   [ ] Implement `maplist[x;fn]`.
    -   [ ] Implement `mapcar[x;fn]`.

## Part VI: Arithmetic Functions and Predicates

-   [ ] **Function Naming**:
    -   [ ] Rename existing arithmetic functions to match the spec (e.g., `Plus` -> `plus`, `Minus` -> `difference`).
    -   [ ] Adjust function signatures to match spec (e.g., `difference` is binary).
-   [ ] **Basic Arithmetic Operations**:
    -   [ ] Implement `remainder[x;y]`.
    -   [ ] Implement `minus[x]` (unary negation).
    -   [ ] Implement `add1[x]` and `sub1[x]`.
    -   [ ] Implement `max[x₁;...;xₙ]` and `min[x₁;...;xₙ]`.
    -   [ ] Implement `expt[x;y]`.
    -   [ ] Add type coercion for mixed fixed-point and floating-point arithmetic.
-   [ ] **Arithmetic Predicates**:
    -   [ ] Implement `lessp[x;y]`.
    -   [ ] Implement `greaterp[x;y]`.
    -   [ ] Implement `zerop[x]`.
    -   [ ] Implement `onep[x]`.
    -   [ ] Implement `minusp[x]`.
    -   [ ] Implement `numberp[x]`.
    -   [ ] Implement `fixp[x]`.
    -   [ ] Implement `floatp[x]`.

## Part VII: Logical Operations

-   [ ] **Boolean Functions**:
    -   [ ] Ensure `and` and `or` behave as specified (short-circuiting). They are currently implemented as special forms, which is acceptable.
    -   [ ] Implement `not[x]`. The current implementation is a special form; the spec lists it as a function. This should be reviewed.
-   [ ] **Bitwise Operations**:
    -   [ ] Implement `logor`.
    -   [ ] Implement `logand`.
    -   [ ] Implement `logxor`.
    -   [ ] Implement `leftshift`.

## Part VIII: Property List Functions

-   [ ] **Property Management**:
    -   [ ] Implement `remprop[x;indicator]`.
    -   [ ] Implement `deflist[x;indicator]`.
    -   [ ] Rename `get-p` and `put-p` to `get` and `put`.

## Part IX: Destructive Operations

-   [ ] **List Structure Modification**:
    -   [ ] Implement `rplaca[x;y]`.
    -   [ ] Implement `rplacd[x;y]`.

## Part X: System Functions

-   [ ] **Function Definition**:
    -   [ ] Implement `define[definitions]`.
    -   [ ] Implement `cset[var;value]`.
-   [ ] **Debugging and Tracing**:
    -   [ ] Implement `trace[function-list]`.
    -   [ ] Implement `untrace[function-list]`.
-   [ ] **Error Handling**:
    -   [ ] Implement `error[message;form]`.
    -   [ ] Implement `errorset[form;flag]`.
-   [ ] **Input/Output**:
    -   [ ] Implement `read[]`.
    -   [ ] Implement `prin1[x]`.
    -   [ ] Implement `terpri[]`.

## Part XII: Extended Functions

-   [ ] **Compositions of CAR and CDR**:
    -   [ ] Implement `caar`, `cadr`, `cdar`, `cddr`.
    -   [ ] Implement 3-level compositions (`caaar` through `cdddr`).
    -   [ ] Implement 4-level compositions (`caaaar` through `cddddr`). A macro or function generator would be ideal for this.

## Part XIV: MACRO System Extension

-   [ ] **MACRO**: The current implementation has a `defmacro` form.
    -   [ ] Review the existing macro system against the AIM-057 paper to ensure it meets the spec's intent for a "relaxed superset".

## Appendix B: Standard Library Summary

-   [ ] **Final Check**:
    -   [ ] Review all implemented functions against the standard library summary to ensure completeness.
