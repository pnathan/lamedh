//! Lisp-to-Lisp source optimizer.
//!
//! The two public entry points are:
//!
//! - [`optimize`] — rewrite a [`crate::LispVal`] expression into a semantically
//!   equivalent but cheaper form (pure, no side effects).
//! - [`optimize_eval`] — optimize then evaluate; called by the `OPTIMIZE` builtin.
//!
//! The Lisp-level optimizer passes (dead-binding removal, constant inlining,
//! PROGN flattening) live in `lib/11-optimizer-vau.lisp` and invoke `(OPTIMIZE
//! expr)` as the constant-folding back-end.
//!
//! ## Safe transforms implemented
//!
//! - **Constant folding**: `(+ 1 2)` → `3`, `(* 3 4)` → `12`
//! - **Algebraic identities**: `(+ x 0)` → `x`, `(* x 1)` → `x`, `(* x 0)` → `0`
//! - **Branch elimination**: `(if t a b)` → `a`, `(if nil a b)` → `b`
//! - **Dead code in PROGN**: pure non-final forms are dropped; `(progn x)` → `x`
//!
//! ## Intentionally not applied
//!
//! - Folding inside fexpr/vau operands (they see unevaluated forms)
//! - Any transform requiring side-effect evaluation
//! - Macro expansion (done lazily at eval time so redefinition works)

use crate::environment::Environment;
use crate::{LispError, LispVal};
use std::rc::Rc;

/// Fold a pure arithmetic/numeric call on literal arguments.
/// Returns `Some(result)` if all args are literals and the op is safe.
fn try_fold_numeric(op: &str, args: &[LispVal]) -> Option<LispVal> {
    // Only fold if ALL args are number literals (not floats — to avoid float semantics surprises)
    let nums: Option<Vec<i64>> = args
        .iter()
        .map(|a| {
            if let LispVal::Number(n) = a {
                Some(*n)
            } else {
                None
            }
        })
        .collect();
    let nums = nums?;
    if nums.is_empty() {
        return None;
    }

    match op {
        "+" => nums
            .into_iter()
            .try_fold(0i64, |a, b| a.checked_add(b))
            .map(LispVal::Number),
        "-" => {
            if nums.len() == 1 {
                nums[0].checked_neg().map(LispVal::Number)
            } else {
                let mut acc = nums[0];
                for &n in &nums[1..] {
                    acc = acc.checked_sub(n)?;
                }
                Some(LispVal::Number(acc))
            }
        }
        "*" => nums
            .into_iter()
            .try_fold(1i64, |a, b| a.checked_mul(b))
            .map(LispVal::Number),
        "/" => {
            if nums.len() == 2 && nums[1] != 0 {
                Some(LispVal::Number(nums[0] / nums[1]))
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Apply algebraic identity reductions for arithmetic ops with one literal arg.
/// e.g. `(+ x 0)` → `x`, `(* x 1)` → `x`
fn try_algebraic_identity(op: &str, args: &[LispVal]) -> Option<LispVal> {
    if args.len() != 2 {
        return None;
    }
    let (a, b) = (&args[0], &args[1]);
    match op {
        "+" | "-" => {
            if let LispVal::Number(0) = b {
                return Some(a.clone());
            }
            if op == "+"
                && let LispVal::Number(0) = a
            {
                return Some(b.clone());
            }
            None
        }
        "*" => {
            if let LispVal::Number(1) = b {
                return Some(a.clone());
            }
            if let LispVal::Number(1) = a {
                return Some(b.clone());
            }
            if let LispVal::Number(0) = b {
                return Some(LispVal::Number(0));
            }
            if let LispVal::Number(0) = a {
                return Some(LispVal::Number(0));
            }
            None
        }
        "/" => {
            if let LispVal::Number(1) = b {
                return Some(a.clone());
            }
            None
        }
        _ => None,
    }
}

/// Returns true if an expression is "pure" (no side effects, safe to drop).
/// Conservative: only literals and QUOTE are unconditionally pure.
fn is_pure(expr: &LispVal) -> bool {
    match expr {
        LispVal::Number(_) | LispVal::Float(_) | LispVal::String(_) | LispVal::Nil => true,
        LispVal::Symbol(_) => true, // reading a variable is pure (no side effects)
        LispVal::Cons { car, cdr } => {
            // (quote ...) is always pure
            if let LispVal::Symbol(s) = car.as_ref()
                && s.borrow().name == "QUOTE"
            {
                return true;
            }
            // (+ ...) (- ...) (* ...) (/) on pure args — pure
            if let LispVal::Symbol(s) = car.as_ref() {
                let name = s.borrow().name.clone();
                if matches!(
                    name.as_str(),
                    "+" | "-" | "*" | "/" | "CAR" | "CDR" | "CONS" | "NULL" | "ATOM" | "EQ" | "NOT"
                ) {
                    // Check all args are pure
                    let mut rest = cdr.as_ref();
                    loop {
                        match rest {
                            LispVal::Nil => return true,
                            LispVal::Cons { car, cdr } => {
                                if !is_pure(car) {
                                    return false;
                                }
                                rest = cdr;
                            }
                            _ => return false,
                        }
                    }
                }
            }
            false
        }
        _ => false,
    }
}

/// Build a cons list from a vec of LispVals.
fn vec_to_list(v: Vec<LispVal>) -> LispVal {
    v.into_iter()
        .rev()
        .fold(LispVal::Nil, |cdr, car| LispVal::Cons {
            car: Rc::new(car),
            cdr: Rc::new(cdr),
        })
}

/// Collect a proper list into a Vec, returning None if it's not proper.
fn list_to_vec(v: &LispVal) -> Option<Vec<LispVal>> {
    let mut result = Vec::new();
    let mut cur = v;
    loop {
        match cur {
            LispVal::Nil => return Some(result),
            LispVal::Cons { car, cdr } => {
                result.push(car.as_ref().clone());
                cur = cdr;
            }
            _ => return None, // improper list
        }
    }
}

/// Recursively rewrite `expr` into a semantically equivalent, cheaper form.
///
/// This is a **pure** function: it does not call `eval`, does not inspect the
/// runtime environment, and has no observable side effects.  It is safe to call
/// on any expression before or instead of evaluating it.
///
/// Atoms and quoted forms are returned unchanged.  Compound forms are walked
/// recursively; each sub-expression is optimized before the parent form applies
/// its own reduction rule (e.g. constant folding requires that arguments have
/// already been reduced to literals).
///
/// Returns the original value cloned if no rewrite applies.
pub fn optimize(expr: &LispVal) -> LispVal {
    match expr {
        // Atoms and literals are already optimal
        LispVal::Number(_)
        | LispVal::Char(_)
        | LispVal::Float(_)
        | LispVal::String(_)
        | LispVal::Nil
        | LispVal::Symbol(_)
        | LispVal::Builtin(_)
        | LispVal::Lambda(_)
        | LispVal::Fexpr(_)
        | LispVal::Macro(_)
        | LispVal::Vau(_)
        | LispVal::HashTable(_)
        | LispVal::Native(_)
        | LispVal::Environment(_)
        | LispVal::Array(_)
        | LispVal::Struct(_)
        | LispVal::Extension(_)
        | LispVal::Error(_) => expr.clone(),

        LispVal::Cons {
            car: head,
            cdr: rest,
        } => {
            // Try to recognize known special forms by head symbol
            if let LispVal::Symbol(s) = head.as_ref() {
                let name = s.borrow().name.clone();
                match name.as_str() {
                    // QUOTE: don't recurse into the quoted form
                    "QUOTE" => return expr.clone(),

                    // QUASIQUOTE: don't recurse (may contain UNQUOTE)
                    "QUASIQUOTE" => return expr.clone(),

                    // IF: branch elimination on literal condition
                    "IF" => {
                        if let Some(args) = list_to_vec(rest)
                            && args.len() >= 2
                        {
                            let cond = optimize(&args[0]);
                            match &cond {
                                LispVal::Nil => {
                                    // (if nil then else) -> else (or nil if no else)
                                    if args.len() >= 3 {
                                        return optimize(&args[2]);
                                    } else {
                                        return LispVal::Nil;
                                    }
                                }
                                LispVal::Number(_) | LispVal::String(_) | LispVal::Float(_) => {
                                    // Truthy literal condition: (if <truthy> then else) -> then
                                    return optimize(&args[1]);
                                }
                                LispVal::Symbol(sym) if sym.borrow().name == "T" => {
                                    // (if t then else) -> then
                                    return optimize(&args[1]);
                                }
                                _ => {
                                    // Unknown condition: optimize both branches
                                    let then_opt = optimize(&args[1]);
                                    let else_opt = if args.len() >= 3 {
                                        optimize(&args[2])
                                    } else {
                                        LispVal::Nil
                                    };
                                    let mut parts = vec![head.as_ref().clone(), cond, then_opt];
                                    if args.len() >= 3 {
                                        parts.push(else_opt);
                                    }
                                    return LispVal::Cons {
                                        car: Rc::new(parts[0].clone()),
                                        cdr: Rc::new(vec_to_list(parts[1..].to_vec())),
                                    };
                                }
                            }
                        }
                    }

                    // PROGN: dead-code elimination on non-final pure forms
                    "PROGN" => {
                        if let Some(forms) = list_to_vec(rest) {
                            if forms.is_empty() {
                                return LispVal::Nil;
                            }
                            // Optimize each form
                            let mut opt_forms: Vec<LispVal> = forms.iter().map(optimize).collect();
                            // Drop pure non-final forms (dead code)
                            let last = opt_forms.pop().unwrap();
                            let kept: Vec<LispVal> =
                                opt_forms.into_iter().filter(|f| !is_pure(f)).collect();
                            if kept.is_empty() {
                                // Single effective form: unwrap the PROGN
                                return last;
                            }
                            let mut all = kept;
                            all.push(last);
                            return LispVal::Cons {
                                car: Rc::new(head.as_ref().clone()),
                                cdr: Rc::new(vec_to_list(all)),
                            };
                        }
                    }

                    // Arithmetic: constant folding + algebraic identities
                    "+" | "-" | "*" | "/" => {
                        if let Some(args) = list_to_vec(rest) {
                            let opt_args: Vec<LispVal> = args.iter().map(optimize).collect();
                            // Try constant folding
                            if let Some(folded) = try_fold_numeric(&name, &opt_args) {
                                return folded;
                            }
                            // Try algebraic identity
                            if let Some(simplified) = try_algebraic_identity(&name, &opt_args) {
                                return simplified;
                            }
                            // Rebuild with optimized args
                            return LispVal::Cons {
                                car: Rc::new(head.as_ref().clone()),
                                cdr: Rc::new(vec_to_list(opt_args)),
                            };
                        }
                    }

                    // Special forms that take unevaluated arguments: don't recurse
                    "VAU" | "$VAU" | "LAMBDA" | "DEFEXPR" | "DEFMACRO" | "FUNCTION" | "LABEL"
                    | "DEFINE" | "DEF" | "DEFUN" | "GO" | "RETURN" | "SETQ" | "DEFDYNAMIC"
                    | "DEFVAR" => {
                        return expr.clone();
                    }

                    _ => {
                        // General: optimize each subexpression
                        let opt_head = optimize(head);
                        if let Some(args) = list_to_vec(rest) {
                            let opt_args: Vec<LispVal> = args.iter().map(optimize).collect();
                            return LispVal::Cons {
                                car: Rc::new(opt_head),
                                cdr: Rc::new(vec_to_list(opt_args)),
                            };
                        }
                    }
                }
            }

            // Non-symbol head or irregular list: optimize head and each element
            let opt_head = optimize(head);
            if let Some(args) = list_to_vec(rest) {
                let opt_args: Vec<LispVal> = args.iter().map(optimize).collect();
                LispVal::Cons {
                    car: Rc::new(opt_head),
                    cdr: Rc::new(vec_to_list(opt_args)),
                }
            } else {
                // Improper list (dotted pair): optimize both sides
                LispVal::Cons {
                    car: Rc::new(opt_head),
                    cdr: Rc::new(optimize(rest)),
                }
            }
        }
    }
}

/// Optimize `expr` and then evaluate the result in `env`.
///
/// This is the implementation of the `(OPTIMIZE expr)` builtin.  Callers that
/// only want constant folding without evaluation should use [`optimize`]
/// directly.
///
/// # Errors
///
/// Propagates any [`crate::LispError`] returned by the evaluator.
pub fn optimize_eval(expr: &LispVal, env: &Rc<Environment>) -> Result<LispVal, LispError> {
    let optimized = optimize(expr);
    crate::evaluator::eval(&optimized, env)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::environment::Environment;
    use crate::printer::print;
    use std::rc::Rc;

    fn e() -> Rc<Environment> {
        Rc::new(Environment::new())
    }
    fn num(n: i64) -> LispVal {
        LispVal::Number(n)
    }
    fn sym(s: &str, env: &Rc<Environment>) -> LispVal {
        LispVal::Symbol(env.intern_symbol(s))
    }
    fn list(items: Vec<LispVal>) -> LispVal {
        vec_to_list(items)
    }
    fn p(v: &LispVal) -> String {
        print(v)
    }

    #[test]
    fn test_constant_fold_add() {
        let env = e();
        let expr = list(vec![sym("+", &env), num(1), num(2)]);
        assert_eq!(optimize(&expr), num(3));
    }

    #[test]
    fn test_constant_fold_mul() {
        let env = e();
        let expr = list(vec![sym("*", &env), num(3), num(4)]);
        assert_eq!(optimize(&expr), num(12));
    }

    #[test]
    fn test_constant_fold_sub() {
        let env = e();
        let expr = list(vec![sym("-", &env), num(10), num(3)]);
        assert_eq!(optimize(&expr), num(7));
    }

    #[test]
    fn test_constant_fold_div() {
        let env = e();
        let expr = list(vec![sym("/", &env), num(10), num(2)]);
        assert_eq!(optimize(&expr), num(5));
    }

    #[test]
    fn test_constant_fold_no_div_by_zero() {
        let env = e();
        let expr = list(vec![sym("/", &env), num(10), num(0)]);
        assert!(matches!(optimize(&expr), LispVal::Cons { .. }));
    }

    #[test]
    fn test_algebraic_add_zero() {
        let env = e();
        let expr = list(vec![sym("+", &env), sym("X", &env), num(0)]);
        // (+ X 0) -> X: the symbol should be preserved
        assert_eq!(p(&optimize(&expr)), "X");
    }

    #[test]
    fn test_algebraic_mul_one() {
        let env = e();
        let expr = list(vec![sym("*", &env), sym("X", &env), num(1)]);
        assert_eq!(p(&optimize(&expr)), "X");
    }

    #[test]
    fn test_algebraic_mul_zero() {
        let env = e();
        let expr = list(vec![sym("*", &env), sym("X", &env), num(0)]);
        assert_eq!(optimize(&expr), num(0));
    }

    #[test]
    fn test_if_true_branch() {
        let env = e();
        let expr = list(vec![sym("IF", &env), sym("T", &env), num(42), num(99)]);
        assert_eq!(optimize(&expr), num(42));
    }

    #[test]
    fn test_if_false_branch() {
        let expr = list(vec![
            LispVal::Symbol(e().intern_symbol("IF")),
            LispVal::Nil,
            num(42),
            num(99),
        ]);
        assert_eq!(optimize(&expr), num(99));
    }

    #[test]
    fn test_progn_single_pure() {
        let env = e();
        // (progn 1 2 3) -> 3 (drops pure non-final)
        let expr = list(vec![sym("PROGN", &env), num(1), num(2), num(3)]);
        assert_eq!(optimize(&expr), num(3));
    }

    #[test]
    fn test_progn_unwrap_single() {
        let env = e();
        // (progn x) -> x
        let expr = list(vec![sym("PROGN", &env), sym("X", &env)]);
        assert_eq!(p(&optimize(&expr)), "X");
    }

    #[test]
    fn test_nested_fold() {
        let env = e();
        // (+ (* 2 3) 4) -> (+ 6 4) -> 10
        let inner = list(vec![sym("*", &env), num(2), num(3)]);
        let outer = list(vec![sym("+", &env), inner, num(4)]);
        assert_eq!(optimize(&outer), num(10));
    }

    #[test]
    fn test_quote_preserved() {
        let env = e();
        let expr = list(vec![sym("QUOTE", &env), sym("FOO", &env)]);
        let opt = optimize(&expr);
        assert!(
            matches!(&opt, LispVal::Cons { car, .. } if matches!(car.as_ref(), LispVal::Symbol(s) if s.borrow().name == "QUOTE"))
        );
    }

    #[test]
    fn test_overflow_not_folded() {
        let env = e();
        // i64::MAX + 1 should not fold (overflow)
        let expr = list(vec![sym("+", &env), num(i64::MAX), num(1)]);
        assert!(matches!(optimize(&expr), LispVal::Cons { .. }));
    }
}
