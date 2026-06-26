//! Core evaluation engine: special forms, builtin primitives, and TCO trampoline.
//!
//! ## Architecture
//!
//! Evaluation is split into three layers:
//!
//! 1. **[`eval`]** — public entry point.  Acquires a recursion-depth guard and
//!    delegates to `eval_impl`.
//! 2. **`eval_impl`** — a loop that repeatedly calls `eval_step`.  When
//!    `eval_step` returns `TcoStep::TailCall` the loop replaces the current
//!    expression and environment without growing the Rust stack (trampolining
//!    TCO).  `TcoStep::Done` exits the loop.
//! 3. **`eval_step`** — the actual pattern-match on the expression.  Returns
//!    either a finished value or a tail-call request.
//!
//! ## Recursion depth guard
//!
//! A thread-local counter tracks the nesting level of `eval` calls.  Once the
//! counter reaches [`DEFAULT_EVAL_DEPTH_LIMIT`] a [`LispError::Generic`] is
//! returned instead of overflowing the native stack.  The limit is adjustable
//! via [`set_eval_depth_limit`].
//!
//! ## Special forms
//!
//! Special forms are handled directly in `eval_step` before the general
//! function-application path:
//!
//! `QUOTE`, `QUASIQUOTE`/`UNQUOTE`, `IF`, `COND`, `AND`, `OR`,
//! `DEF`, `DEFDYNAMIC`/`DEFVAR`, `LAMBDA`, `FUNCTION`, `LABEL`,
//! `DEFINE`, `DEFEXPR`, `DEFMACRO`, `DEFSTRUCT`,
//! `PROGN`, `SETQ`, `PROG`, `RETURN`, `GO`, `FOR`, `WHILE`,
//! `LET`, `LET*`, `VAU`/`$VAU`.
//!
//! ## Builtin functions
//!
//! 100+ primitives are dispatched in `apply_builtin` from the [`BuiltinFunc`]
//! discriminant stored in [`LispVal::Builtin`] values.

#![allow(clippy::mutable_key_type)]
use crate::{BuiltinFunc, LispError, LispVal, environment::Environment};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

/// Default maximum recursion depth (number of nested `eval` frames) before a
/// recoverable error is returned instead of overflowing the native stack.
///
/// The tree-walking interpreter uses large stack frames, so this is calibrated
/// to fit comfortably within the large stack provided by [`crate::with_large_stack`]
/// (~512 MiB). Hosts that run the interpreter on a smaller stack should lower it
/// via [`set_eval_depth_limit`]. See issues #61 (this guard) and #62 (TCO).
pub const DEFAULT_EVAL_DEPTH_LIMIT: usize = 10_000;

thread_local! {
    static EVAL_DEPTH: Cell<usize> = const { Cell::new(0) };
    static EVAL_DEPTH_LIMIT: Cell<usize> = const { Cell::new(DEFAULT_EVAL_DEPTH_LIMIT) };
}

/// Set the maximum `eval` recursion depth for the current thread.
pub fn set_eval_depth_limit(limit: usize) {
    EVAL_DEPTH_LIMIT.with(|l| l.set(limit));
}

/// Get the current thread's maximum `eval` recursion depth.
pub fn eval_depth_limit() -> usize {
    EVAL_DEPTH_LIMIT.with(|l| l.get())
}

/// RAII guard that bumps the recursion depth on entry to `eval` and restores it
/// on every exit path (including `?` early returns and caught errors).
struct DepthGuard;

impl DepthGuard {
    fn enter() -> Result<DepthGuard, LispError> {
        EVAL_DEPTH.with(|depth| {
            let next = depth.get() + 1;
            let limit = EVAL_DEPTH_LIMIT.with(|l| l.get());
            if next > limit {
                Err(LispError::Generic(format!(
                    "recursion limit exceeded ({limit} eval frames); \
                     rewrite iteratively or raise it with set_eval_depth_limit"
                )))
            } else {
                depth.set(next);
                Ok(DepthGuard)
            }
        })
    }
}

impl Drop for DepthGuard {
    fn drop(&mut self) {
        EVAL_DEPTH.with(|depth| depth.set(depth.get().saturating_sub(1)));
    }
}

// Helper function to convert a Lisp list (Cons chain) to a Rust Vec.
fn list_to_vec(list: &LispVal) -> Result<Vec<LispVal>, LispError> {
    let mut vec = Vec::new();
    let mut current = list;
    while let LispVal::Cons { car, cdr } = current {
        vec.push(car.as_ref().clone());
        current = cdr;
    }
    if *current != LispVal::Nil {
        return Err(LispError::Generic(
            "list_to_vec: not a proper list".to_string(),
        ));
    }
    Ok(vec)
}

// Helper function to convert a Rust Vec to a Lisp list.
fn vec_to_list(vec: Vec<LispVal>) -> LispVal {
    vec.into_iter()
        .rev()
        .fold(LispVal::Nil, |cdr, car| LispVal::Cons {
            car: Rc::new(car),
            cdr: Rc::new(cdr),
        })
}

/// Evaluate a call's operands directly off the cons chain into a single `Vec`.
///
/// This is the hot path for ordinary function/lambda/builtin application. It
/// replaces the older `list_to_vec(rest)` + `.iter().map(eval).collect()` pair,
/// which allocated **two** vectors per call (one of cloned argument *expressions*,
/// one of evaluated results) and cloned every argument expression. Walking the
/// cons cells and evaluating each `car` in place does the same work with a
/// single allocation and no expression clones. (Like the other cons-walking
/// special forms here — `AND`/`OR`/`PROGN` — an improper tail is simply ignored.)
fn eval_operands(rest: &LispVal, env: &Rc<Environment>) -> Result<Vec<LispVal>, LispError> {
    let mut out = Vec::new();
    let mut cur = rest;
    while let LispVal::Cons { car, cdr } = cur {
        out.push(eval(car, env)?);
        cur = cdr;
    }
    Ok(out)
}

/// Wrap a body of one or more forms into a single evaluable expression.
///
/// A lone form is returned as-is; several forms are wrapped in `(PROGN ...)`
/// so the existing `PROGN` trampoline sequences them and keeps TCO on the last
/// form. Used by the binding special forms (`LET`/`LET*`) to accept multi-form
/// bodies. `forms` is expected to be non-empty; an empty slice yields `(PROGN)`,
/// which evaluates to `NIL`.
fn wrap_body_forms(forms: &[LispVal], env: &Rc<Environment>) -> LispVal {
    if forms.len() == 1 {
        forms[0].clone()
    } else {
        let mut list = Vec::with_capacity(forms.len() + 1);
        list.push(LispVal::Symbol(env.intern_symbol("PROGN")));
        list.extend_from_slice(forms);
        vec_to_list(list)
    }
}

#[inline(never)]
fn apply_apply(args: &[LispVal], env: &Rc<Environment>) -> Result<LispVal, LispError> {
    if args.len() != 2 {
        return Err(LispError::Generic(
            "APPLY requires exactly two arguments".to_string(),
        ));
    }
    let func_arg = &args[0];
    let arg_list = &args[1];

    let func = match func_arg {
        LispVal::Symbol(s) => env
            .get(&s.borrow().name)
            .ok_or_else(|| LispError::Generic(format!("Function not found: {}", s.borrow().name))),
        _ => Ok(func_arg.clone()),
    }?;

    let unpacked_args = match list_to_vec(arg_list) {
        Ok(vec) => vec,
        Err(_) => {
            return Err(LispError::Generic(
                "APPLY second argument must be a proper list".to_string(),
            ));
        }
    };

    match &func {
        LispVal::Macro(m) => {
            let expanded = expand_macro(m, &unpacked_args, env)?;
            eval(&expanded, env)
        }
        LispVal::Fexpr(f) => {
            let new_env = Environment::new_child_with_dynamic(&f.env, env);
            if f.params.len() == 1 {
                // Single-param fexpr: bind the whole arg list to the one parameter.
                let fexpr_arg_list = vec_to_list(unpacked_args);
                new_env.set(f.params[0].clone(), fexpr_arg_list);
            } else {
                // Multi-param fexpr: bind each arg to the corresponding parameter.
                if unpacked_args.len() != f.params.len() {
                    return Err(LispError::Generic(format!(
                        "APPLY: fexpr expected {} arguments, got {}",
                        f.params.len(),
                        unpacked_args.len()
                    )));
                }
                for (param, arg) in f.params.iter().zip(unpacked_args) {
                    new_env.set(param.clone(), arg);
                }
            }
            eval(&f.body, &new_env)
        }
        LispVal::Vau(v) => {
            // Via APPLY, args are already evaluated; treat them as the operand list.
            let arg_list = vec_to_list(unpacked_args);
            let new_env = Environment::new_child(&v.env);
            new_env.set(v.operands_param.clone(), arg_list);
            new_env.set(v.env_param.clone(), LispVal::Environment(env.clone()));
            eval(&v.body, &new_env)
        }
        _ => apply(&func, &unpacked_args, env),
    }
}

fn is_truthy(val: &LispVal) -> bool {
    !matches!(val, LispVal::Nil)
}

#[inline(never)]
fn apply_math_op(
    op: &BuiltinFunc,
    args: &[LispVal],
    env: &Rc<Environment>,
) -> Result<LispVal, LispError> {
    // If any argument is a float, promote all to float arithmetic
    let has_float = args.iter().any(|a| matches!(a, LispVal::Float(_)));
    if has_float {
        let floats: Result<Vec<f64>, LispError> = args
            .iter()
            .map(|arg| match arg {
                LispVal::Number(n) => Ok(*n as f64),
                LispVal::Char(b) => Ok(*b as f64),
                LispVal::Float(f) => Ok(*f),
                _ => Err(LispError::Generic(
                    "Math functions only accept numbers".to_string(),
                )),
            })
            .collect();
        let floats = floats?;
        return match op {
            BuiltinFunc::Plus => Ok(LispVal::Float(floats.iter().sum())),
            BuiltinFunc::Minus => {
                if floats.is_empty() {
                    return Err(LispError::Generic(
                        "- requires at least one argument".to_string(),
                    ));
                }
                if floats.len() == 1 {
                    Ok(LispVal::Float(-floats[0]))
                } else {
                    Ok(LispVal::Float(floats[0] - floats[1..].iter().sum::<f64>()))
                }
            }
            BuiltinFunc::Multiply => Ok(LispVal::Float(floats.iter().product())),
            BuiltinFunc::Divide => {
                if floats.len() != 2 {
                    return Err(LispError::Generic(
                        "/ requires exactly two arguments".to_string(),
                    ));
                }
                if floats[1] == 0.0 {
                    return Err(LispError::Generic("Division by zero".to_string()));
                }
                Ok(LispVal::Float(floats[0] / floats[1]))
            }
            _ => Err(LispError::Generic("Not a math operation".to_string())),
        };
    }

    // Integer path: fold directly over the arguments without materialising an
    // intermediate `Vec<i64>`. Arithmetic runs in every loop body, so avoiding
    // a per-call allocation here is a measurable hot-path win.
    #[inline]
    fn as_int(arg: &LispVal) -> Result<i64, LispError> {
        match arg {
            LispVal::Number(n) => Ok(*n),
            LispVal::Char(b) => Ok(*b as i64),
            _ => Err(LispError::Generic(
                "Math functions only accept numbers".to_string(),
            )),
        }
    }

    match op {
        BuiltinFunc::Plus => {
            let mut result = 0i64;
            for arg in args {
                let num = as_int(arg)?;
                match result.checked_add(num) {
                    Some(v) => result = v,
                    None => {
                        env.set_flag("OVERFLOW");
                        result = result.wrapping_add(num);
                    }
                }
            }
            Ok(LispVal::Number(result))
        }
        BuiltinFunc::Minus => {
            if args.is_empty() {
                return Err(LispError::Generic(
                    "- requires at least one argument".to_string(),
                ));
            }
            let first = as_int(&args[0])?;
            if args.len() == 1 {
                match first.checked_neg() {
                    Some(v) => Ok(LispVal::Number(v)),
                    None => {
                        env.set_flag("OVERFLOW");
                        Ok(LispVal::Number(first.wrapping_neg()))
                    }
                }
            } else {
                let mut result = first;
                for arg in &args[1..] {
                    let num = as_int(arg)?;
                    match result.checked_sub(num) {
                        Some(v) => result = v,
                        None => {
                            env.set_flag("OVERFLOW");
                            result = result.wrapping_sub(num);
                        }
                    }
                }
                Ok(LispVal::Number(result))
            }
        }
        BuiltinFunc::Multiply => {
            let mut result = 1i64;
            for arg in args {
                let num = as_int(arg)?;
                match result.checked_mul(num) {
                    Some(v) => result = v,
                    None => {
                        env.set_flag("OVERFLOW");
                        result = result.wrapping_mul(num);
                    }
                }
            }
            Ok(LispVal::Number(result))
        }
        BuiltinFunc::Divide => {
            if args.len() != 2 {
                return Err(LispError::Generic(
                    "/ requires exactly two arguments".to_string(),
                ));
            }
            let x = as_int(&args[0])?;
            let y = as_int(&args[1])?;
            if y == 0 {
                return Err(LispError::Generic("Division by zero".to_string()));
            }
            // Check for i64::MIN / -1 overflow
            if x == i64::MIN && y == -1 {
                env.set_flag("OVERFLOW");
                Ok(LispVal::Number(x.wrapping_div(y)))
            } else {
                Ok(LispVal::Number(x / y))
            }
        }
        _ => Err(LispError::Generic("Not a math operation".to_string())),
    }
}

#[inline(never)]
fn apply_list_op(op: &BuiltinFunc, args: &[LispVal]) -> Result<LispVal, LispError> {
    match op {
        BuiltinFunc::Car => {
            if args.len() != 1 {
                return Err(LispError::Generic(
                    "car requires exactly one argument".to_string(),
                ));
            }
            match &args[0] {
                LispVal::Cons { car, .. } => Ok(car.as_ref().clone()),
                LispVal::Nil => Ok(LispVal::Nil),
                _ => Err(LispError::Generic("car requires a list".to_string())),
            }
        }
        BuiltinFunc::Cdr => {
            if args.len() != 1 {
                return Err(LispError::Generic(
                    "cdr requires exactly one argument".to_string(),
                ));
            }
            match &args[0] {
                LispVal::Cons { cdr, .. } => Ok(cdr.as_ref().clone()),
                LispVal::Nil => Ok(LispVal::Nil),
                _ => Err(LispError::Generic("cdr requires a list".to_string())),
            }
        }
        BuiltinFunc::Cons => {
            if args.len() != 2 {
                return Err(LispError::Generic(
                    "cons requires exactly two arguments".to_string(),
                ));
            }
            Ok(LispVal::Cons {
                car: Rc::new(args[0].clone()),
                cdr: Rc::new(args[1].clone()),
            })
        }
        _ => Err(LispError::Generic("Not a list operation".to_string())),
    }
}

#[inline(never)]
fn apply_string_op(op: &BuiltinFunc, args: &[LispVal]) -> Result<LispVal, LispError> {
    match op {
        BuiltinFunc::Concat => {
            let strs: Result<Vec<String>, LispError> = args
                .iter()
                .map(|arg| match arg {
                    LispVal::String(s) => Ok(s.clone()),
                    _ => Err(LispError::Generic(
                        "concat only accepts strings".to_string(),
                    )),
                })
                .collect();
            Ok(LispVal::String(strs?.concat()))
        }
        BuiltinFunc::Index => {
            if args.len() != 2 {
                return Err(LispError::Generic(
                    "index requires exactly two arguments".to_string(),
                ));
            }
            let s = if let LispVal::String(s) = &args[0] {
                s
            } else {
                return Err(LispError::Generic(
                    "index requires a string as its first argument".to_string(),
                ));
            };
            let i = if let LispVal::Number(n) = &args[1] {
                *n as usize
            } else {
                return Err(LispError::Generic(
                    "index requires a number as its second argument".to_string(),
                ));
            };
            if let Some(ch) = s.chars().nth(i) {
                Ok(LispVal::String(ch.to_string()))
            } else {
                Err(LispError::Generic("index out of bounds".to_string()))
            }
        }
        _ => Err(LispError::Generic("Not a string operation".to_string())),
    }
}

/// Coerce a numeric `LispVal` (Number or Float) to `f64`.
fn as_f64(v: &LispVal, ctx: &str) -> Result<f64, LispError> {
    match v {
        LispVal::Number(n) => Ok(*n as f64),
        LispVal::Char(b) => Ok(*b as f64),
        LispVal::Float(f) => Ok(*f),
        _ => Err(LispError::Generic(format!("{ctx} requires a number"))),
    }
}

/// Math library builtins implemented in Rust (issue #148).
///
/// Transcendentals (`sqrt`/`sin`/`cos`/`tan`/`log`/`exp`) accept any number and
/// return an `f64`. Rounding (`floor`/`ceiling`/`round`/`truncate`) accepts any
/// number and returns an `i64`. Integer ops (`gcd`/`lcm`/`isqrt`) require
/// integers; `signum` preserves the input's int/float kind.
#[inline(never)]
fn apply_math_lib(op: &BuiltinFunc, args: &[LispVal]) -> Result<LispVal, LispError> {
    // gcd/lcm are variadic in spirit but we keep them binary here; the rest are unary.
    let want = |n: usize, name: &str| -> Result<(), LispError> {
        if args.len() != n {
            Err(LispError::Generic(format!(
                "{name} requires exactly {n} argument(s)"
            )))
        } else {
            Ok(())
        }
    };
    match op {
        BuiltinFunc::Sqrt => {
            want(1, "sqrt")?;
            Ok(LispVal::Float(as_f64(&args[0], "sqrt")?.sqrt()))
        }
        BuiltinFunc::Sin => {
            want(1, "sin")?;
            Ok(LispVal::Float(as_f64(&args[0], "sin")?.sin()))
        }
        BuiltinFunc::Cos => {
            want(1, "cos")?;
            Ok(LispVal::Float(as_f64(&args[0], "cos")?.cos()))
        }
        BuiltinFunc::Tan => {
            want(1, "tan")?;
            Ok(LispVal::Float(as_f64(&args[0], "tan")?.tan()))
        }
        BuiltinFunc::Exp => {
            want(1, "exp")?;
            Ok(LispVal::Float(as_f64(&args[0], "exp")?.exp()))
        }
        BuiltinFunc::Log => {
            // (log x) -> natural log; (log x base) -> log base b
            match args.len() {
                1 => Ok(LispVal::Float(as_f64(&args[0], "log")?.ln())),
                2 => {
                    let x = as_f64(&args[0], "log")?;
                    let b = as_f64(&args[1], "log")?;
                    Ok(LispVal::Float(x.log(b)))
                }
                _ => Err(LispError::Generic("log takes 1 or 2 arguments".to_string())),
            }
        }
        BuiltinFunc::Floor => {
            want(1, "floor")?;
            Ok(LispVal::Number(as_f64(&args[0], "floor")?.floor() as i64))
        }
        BuiltinFunc::Ceiling => {
            want(1, "ceiling")?;
            Ok(LispVal::Number(as_f64(&args[0], "ceiling")?.ceil() as i64))
        }
        BuiltinFunc::Round => {
            want(1, "round")?;
            // Round half away from zero (Rust f64::round semantics).
            Ok(LispVal::Number(as_f64(&args[0], "round")?.round() as i64))
        }
        BuiltinFunc::Truncate => {
            want(1, "truncate")?;
            Ok(LispVal::Number(as_f64(&args[0], "truncate")?.trunc() as i64))
        }
        BuiltinFunc::Gcd => {
            want(2, "gcd")?;
            match (&args[0], &args[1]) {
                (LispVal::Number(a), LispVal::Number(b)) => Ok(LispVal::Number(gcd_i64(*a, *b))),
                _ => Err(LispError::Generic("gcd requires integers".to_string())),
            }
        }
        BuiltinFunc::Lcm => {
            want(2, "lcm")?;
            match (&args[0], &args[1]) {
                (LispVal::Number(a), LispVal::Number(b)) => {
                    if *a == 0 || *b == 0 {
                        Ok(LispVal::Number(0))
                    } else {
                        let g = gcd_i64(*a, *b);
                        Ok(LispVal::Number((a / g * b).abs()))
                    }
                }
                _ => Err(LispError::Generic("lcm requires integers".to_string())),
            }
        }
        BuiltinFunc::Isqrt => {
            want(1, "isqrt")?;
            match &args[0] {
                LispVal::Number(n) if *n >= 0 => Ok(LispVal::Number((*n as f64).sqrt() as i64)),
                LispVal::Number(_) => Err(LispError::Generic(
                    "isqrt requires a non-negative integer".to_string(),
                )),
                _ => Err(LispError::Generic("isqrt requires an integer".to_string())),
            }
        }
        BuiltinFunc::Signum => {
            want(1, "signum")?;
            match &args[0] {
                LispVal::Number(n) => Ok(LispVal::Number(n.signum())),
                LispVal::Float(f) => Ok(LispVal::Float(if *f == 0.0 { 0.0 } else { f.signum() })),
                _ => Err(LispError::Generic("signum requires a number".to_string())),
            }
        }
        _ => Err(LispError::Generic("Not a math operation".to_string())),
    }
}

fn gcd_i64(mut a: i64, mut b: i64) -> i64 {
    a = a.abs();
    b = b.abs();
    while b != 0 {
        let t = b;
        b = a % b;
        a = t;
    }
    a
}

/// String-operation kernel primitives (issue #147). These cannot be expressed
/// in pure Lisp; the convenience layer (split/join/trim/upcase/...) is built on
/// top of them in `lib/`.
#[inline(never)]
fn apply_string_lib(op: &BuiltinFunc, args: &[LispVal]) -> Result<LispVal, LispError> {
    let get_str = |i: usize, name: &str| -> Result<String, LispError> {
        match args.get(i) {
            Some(LispVal::String(s)) => Ok(s.clone()),
            _ => Err(LispError::Generic(format!(
                "{name} requires a string argument"
            ))),
        }
    };
    match op {
        BuiltinFunc::StringLength => {
            if args.len() != 1 {
                return Err(LispError::Generic(
                    "string-length requires exactly one argument".to_string(),
                ));
            }
            Ok(LispVal::Number(
                get_str(0, "string-length")?.chars().count() as i64,
            ))
        }
        BuiltinFunc::Substring => {
            // (substring s start [end]) — char indices, end exclusive, end
            // defaults to the string length. Clamped to valid bounds.
            if args.len() != 2 && args.len() != 3 {
                return Err(LispError::Generic(
                    "substring takes 2 or 3 arguments: (substring s start [end])".to_string(),
                ));
            }
            let s = get_str(0, "substring")?;
            let chars: Vec<char> = s.chars().collect();
            let len = chars.len();
            let start = match &args[1] {
                LispVal::Number(n) if *n >= 0 => (*n as usize).min(len),
                _ => {
                    return Err(LispError::Generic(
                        "substring start must be a non-negative integer".to_string(),
                    ));
                }
            };
            let end = if args.len() == 3 {
                match &args[2] {
                    LispVal::Number(n) if *n >= 0 => (*n as usize).min(len),
                    _ => {
                        return Err(LispError::Generic(
                            "substring end must be a non-negative integer".to_string(),
                        ));
                    }
                }
            } else {
                len
            };
            if start > end {
                return Err(LispError::Generic(
                    "substring start must not exceed end".to_string(),
                ));
            }
            Ok(LispVal::String(chars[start..end].iter().collect()))
        }
        BuiltinFunc::CharCode => {
            // (char-code x) — code point of x, where x is a Char or a one-char string.
            match args.first() {
                Some(LispVal::Char(b)) => Ok(LispVal::Number(*b as i64)),
                _ => {
                    let s = get_str(0, "char-code")?;
                    match s.chars().next() {
                        Some(c) => Ok(LispVal::Number(c as i64)),
                        None => Err(LispError::Generic(
                            "char-code requires a non-empty string".to_string(),
                        )),
                    }
                }
            }
        }
        BuiltinFunc::CodeChar => {
            // (code-char n) — one-character string for code point n.
            match args.first() {
                Some(LispVal::Number(n)) if *n >= 0 => char::from_u32(*n as u32)
                    .map(|c| LispVal::String(c.to_string()))
                    .ok_or_else(|| {
                        LispError::Generic(format!("code-char: {n} is not a valid code point"))
                    }),
                _ => Err(LispError::Generic(
                    "code-char requires a non-negative integer".to_string(),
                )),
            }
        }
        BuiltinFunc::MakeChar => {
            // (make-char n) — create a Char value from integer code point 0–255.
            match args.first() {
                Some(LispVal::Number(n)) if *n >= 0 && *n <= 255 => Ok(LispVal::Char(*n as u8)),
                Some(LispVal::Number(n)) => Err(LispError::Generic(format!(
                    "make-char: {n} out of range 0–255"
                ))),
                _ => Err(LispError::Generic(
                    "make-char requires a non-negative integer".to_string(),
                )),
            }
        }
        BuiltinFunc::StringToNumber => {
            // (string->number s) — parse as integer, else float, else NIL.
            let s = get_str(0, "string->number")?;
            let t = s.trim();
            if let Ok(n) = t.parse::<i64>() {
                Ok(LispVal::Number(n))
            } else if let Ok(f) = t.parse::<f64>() {
                Ok(LispVal::Float(f))
            } else {
                Ok(LispVal::Nil)
            }
        }
        BuiltinFunc::NumberToString => match args.first() {
            Some(LispVal::Number(n)) => Ok(LispVal::String(n.to_string())),
            Some(LispVal::Float(f)) => Ok(LispVal::String(f.to_string())),
            _ => Err(LispError::Generic(
                "number->string requires a number".to_string(),
            )),
        },
        BuiltinFunc::Prin1ToString => match args.first() {
            // Readable representation (strings are quoted), via the printer.
            Some(v) => Ok(LispVal::String(crate::printer::print(v))),
            None => Err(LispError::Generic(
                "prin1-to-string requires one argument".to_string(),
            )),
        },
        BuiltinFunc::PrincToString => match args.first() {
            // Human representation: a top-level string yields its raw contents,
            // mirroring PRINC; everything else uses the printer.
            Some(LispVal::String(s)) => Ok(LispVal::String(s.clone())),
            Some(v) => Ok(LispVal::String(crate::printer::print(v))),
            None => Err(LispError::Generic(
                "princ-to-string requires one argument".to_string(),
            )),
        },
        _ => Err(LispError::Generic("Not a string operation".to_string())),
    }
}

/// `(sort list comparator)` — stable, non-destructive sort (issue #144).
///
/// The comparator is a `lessp`-style strict-ordering predicate: it receives two
/// elements and returns non-NIL iff the first should come before the second.
/// Returns a freshly built list; the input is never mutated (consistent with
/// the deferred-mutation decision, see #114).
#[inline(never)]
fn apply_sort(args: &[LispVal], env: &Rc<Environment>) -> Result<LispVal, LispError> {
    if args.len() != 2 {
        return Err(LispError::Generic(
            "sort requires exactly two arguments: (sort list comparator)".to_string(),
        ));
    }
    let mut items = list_to_vec(&args[0])?;
    // Resolve a symbol comparator to its function value, like funcall does.
    let cmp = match &args[1] {
        LispVal::Symbol(s) => env.get(&s.borrow().name).ok_or_else(|| {
            LispError::Generic(format!("sort: comparator not found: {}", s.borrow().name))
        })?,
        other => other.clone(),
    };
    // sort_by needs a total order via Ordering; we derive it from the strict
    // less-than predicate. We surface comparator errors after sorting since
    // sort_by's closure cannot return Result.
    let mut err: Option<LispError> = None;
    items.sort_by(|a, b| {
        use std::cmp::Ordering;
        if err.is_some() {
            return Ordering::Equal;
        }
        let a_lt_b = match apply(&cmp, &[a.clone(), b.clone()], env) {
            Ok(v) => v != LispVal::Nil,
            Err(e) => {
                err = Some(e);
                return Ordering::Equal;
            }
        };
        if a_lt_b {
            return Ordering::Less;
        }
        let b_lt_a = match apply(&cmp, &[b.clone(), a.clone()], env) {
            Ok(v) => v != LispVal::Nil,
            Err(e) => {
                err = Some(e);
                return Ordering::Equal;
            }
        };
        if b_lt_a {
            Ordering::Greater
        } else {
            Ordering::Equal
        }
    });
    if let Some(e) = err {
        return Err(e);
    }
    Ok(vec_to_list(items))
}

/// First-class error/condition value operations (LispVal::Error).
#[inline(never)]
fn apply_error_value_op(
    op: &BuiltinFunc,
    args: &[LispVal],
    env: &Rc<Environment>,
) -> Result<LispVal, LispError> {
    match op {
        BuiltinFunc::MakeError => {
            // (make-error message [data]) — message is a string (any other value
            // is rendered); data defaults to NIL.
            if args.is_empty() {
                return Err(LispError::Generic(
                    "make-error requires a message".to_string(),
                ));
            }
            let message = match &args[0] {
                LispVal::String(s) => s.clone(),
                other => crate::printer::print(other),
            };
            let data = args.get(1).cloned().unwrap_or(LispVal::Nil);
            Ok(LispVal::Error(Rc::new(crate::ErrorObj { message, data })))
        }
        BuiltinFunc::ErrorP => {
            if args.len() != 1 {
                return Err(LispError::Generic(
                    "error-p requires exactly one argument".to_string(),
                ));
            }
            Ok(match &args[0] {
                LispVal::Error(_) => LispVal::Symbol(env.intern_symbol("T")),
                _ => LispVal::Nil,
            })
        }
        BuiltinFunc::ErrorMessage => match args.first() {
            Some(LispVal::Error(e)) => Ok(LispVal::String(e.message.clone())),
            _ => Err(LispError::Generic(
                "error-message requires an error value".to_string(),
            )),
        },
        BuiltinFunc::ErrorData => match args.first() {
            Some(LispVal::Error(e)) => Ok(e.data.clone()),
            _ => Err(LispError::Generic(
                "error-data requires an error value".to_string(),
            )),
        },
        _ => Err(LispError::Generic("Not an error operation".to_string())),
    }
}

#[inline(never)]
fn apply_numeric_primitives(
    op: &BuiltinFunc,
    args: &[LispVal],
    env: &Rc<Environment>,
) -> Result<LispVal, LispError> {
    match op {
        BuiltinFunc::Lessp => {
            if args.len() != 2 {
                return Err(LispError::Generic("lessp requires 2 args".to_string()));
            }
            // Integer fast path; fall back to f64 for any int/float mix.
            let less = match (&args[0], &args[1]) {
                (LispVal::Number(x), LispVal::Number(y)) => x < y,
                (LispVal::Char(x), LispVal::Char(y)) => x < y,
                _ => as_f64(&args[0], "lessp")? < as_f64(&args[1], "lessp")?,
            };
            Ok(if less {
                LispVal::Symbol(env.intern_symbol("T"))
            } else {
                LispVal::Nil
            })
        }
        BuiltinFunc::Greaterp => {
            if args.len() != 2 {
                return Err(LispError::Generic("greaterp requires 2 args".to_string()));
            }
            let greater = match (&args[0], &args[1]) {
                (LispVal::Number(x), LispVal::Number(y)) => x > y,
                (LispVal::Char(x), LispVal::Char(y)) => x > y,
                _ => as_f64(&args[0], "greaterp")? > as_f64(&args[1], "greaterp")?,
            };
            Ok(if greater {
                LispVal::Symbol(env.intern_symbol("T"))
            } else {
                LispVal::Nil
            })
        }
        BuiltinFunc::Zerop => {
            if args.len() != 1 {
                return Err(LispError::Generic("zerop requires 1 arg".to_string()));
            }
            if let LispVal::Number(x) = &args[0] {
                Ok(if *x == 0 {
                    LispVal::Symbol(env.intern_symbol("T"))
                } else {
                    LispVal::Nil
                })
            } else {
                Err(LispError::Generic("zerop requires number".to_string()))
            }
        }
        BuiltinFunc::Remainder => {
            if args.len() != 2 {
                return Err(LispError::Generic("remainder requires 2 args".to_string()));
            }
            if let (LispVal::Number(x), LispVal::Number(y)) = (&args[0], &args[1]) {
                if *y == 0 {
                    return Err(LispError::Generic("Division by zero".to_string()));
                }
                // Check for i64::MIN % -1 overflow
                if *x == i64::MIN && *y == -1 {
                    env.set_flag("OVERFLOW");
                    Ok(LispVal::Number(x.wrapping_rem(*y)))
                } else {
                    Ok(LispVal::Number(x % y))
                }
            } else {
                Err(LispError::Generic("remainder requires numbers".to_string()))
            }
        }
        BuiltinFunc::Expt => {
            if args.len() != 2 {
                return Err(LispError::Generic("expt requires 2 args".to_string()));
            }
            match (&args[0], &args[1]) {
                (LispVal::Number(base), LispVal::Number(exp)) => {
                    if *exp < 0 {
                        // negative integer exponent → float result
                        return Ok(LispVal::Float((*base as f64).powi(*exp as i32)));
                    }
                    if *exp > u32::MAX as i64 {
                        return Err(LispError::Generic("exponent too large".to_string()));
                    }
                    base.checked_pow(*exp as u32)
                        .map(LispVal::Number)
                        .ok_or_else(|| LispError::Generic("exponentiation overflow".to_string()))
                }
                (LispVal::Float(base), LispVal::Number(exp)) => {
                    Ok(LispVal::Float(base.powi(*exp as i32)))
                }
                (LispVal::Number(base), LispVal::Float(exp)) => {
                    Ok(LispVal::Float((*base as f64).powf(*exp)))
                }
                (LispVal::Float(base), LispVal::Float(exp)) => Ok(LispVal::Float(base.powf(*exp))),
                _ => Err(LispError::Generic("expt requires numbers".to_string())),
            }
        }
        _ => Err(LispError::Generic("Not a numeric primitive".to_string())),
    }
}

#[inline(never)]
fn apply_logical_op(
    op: &BuiltinFunc,
    args: &[LispVal],
    env: &Rc<Environment>,
) -> Result<LispVal, LispError> {
    match op {
        BuiltinFunc::Eq => {
            if args.len() != 2 {
                return Err(LispError::Generic(
                    "eq requires exactly two arguments".to_string(),
                ));
            }
            // EQ is defined only for atoms (Lisp 1.5 manual). Cons cells are never EQ.
            let is_atom = |v: &LispVal| !matches!(v, LispVal::Cons { .. });
            if !is_atom(&args[0]) || !is_atom(&args[1]) {
                return Ok(LispVal::Nil);
            }
            if args[0] == args[1] {
                Ok(LispVal::Symbol(env.intern_symbol("T")))
            } else {
                Ok(LispVal::Nil)
            }
        }
        BuiltinFunc::Not => {
            if args.len() != 1 {
                return Err(LispError::Generic(
                    "not requires exactly one argument".to_string(),
                ));
            }
            if is_truthy(&args[0]) {
                Ok(LispVal::Nil)
            } else {
                Ok(LispVal::Symbol(env.intern_symbol("T")))
            }
        }
        BuiltinFunc::NumericEquals => {
            if args.len() != 2 {
                return Err(LispError::Generic(
                    "= requires exactly two arguments".to_string(),
                ));
            }
            let coerce = |v: &LispVal| -> Option<i64> {
                match v {
                    LispVal::Number(n) => Some(*n),
                    LispVal::Char(b) => Some(*b as i64),
                    _ => None,
                }
            };
            match (coerce(&args[0]), coerce(&args[1])) {
                (Some(a), Some(b)) => {
                    if a == b {
                        Ok(LispVal::Symbol(env.intern_symbol("T")))
                    } else {
                        Ok(LispVal::Nil)
                    }
                }
                _ => {
                    // Fall back to float for mixed int/float. Exact equality
                    // (like Common Lisp `=`): no epsilon fuzz, so distinct
                    // floats never compare equal.
                    match (&args[0], &args[1]) {
                        (LispVal::Float(_), _) | (_, LispVal::Float(_)) => {
                            let a = as_f64(&args[0], "=")?;
                            let b = as_f64(&args[1], "=")?;
                            Ok(if a == b {
                                LispVal::Symbol(env.intern_symbol("T"))
                            } else {
                                LispVal::Nil
                            })
                        }
                        _ => Err(LispError::Generic(
                            "= requires numeric arguments".to_string(),
                        )),
                    }
                }
            }
        }
        _ => Err(LispError::Generic("Not a logical operation".to_string())),
    }
}

#[inline(never)]
fn apply_hashtable_op(
    op: &BuiltinFunc,
    args: &[LispVal],
    env: &Rc<Environment>,
) -> Result<LispVal, LispError> {
    match op {
        BuiltinFunc::MakeHashTable => {
            if !args.is_empty() {
                return Err(LispError::Generic(
                    "make-hash-table takes no arguments".to_string(),
                ));
            }
            Ok(LispVal::HashTable(Rc::new(RefCell::new(HashMap::new()))))
        }
        BuiltinFunc::Set => {
            if args.len() != 3 {
                return Err(LispError::Generic(
                    "set! takes exactly three arguments".to_string(),
                ));
            }
            if let LispVal::HashTable(h) = &args[0] {
                let key = args[1].clone();
                let val = args[2].clone();
                h.borrow_mut().insert(key, val);
                Ok(LispVal::Symbol(env.intern_symbol("T")))
            } else {
                Err(LispError::Generic(
                    "set! requires a hash table as its first argument".to_string(),
                ))
            }
        }
        BuiltinFunc::Get => {
            if args.len() != 2 {
                return Err(LispError::Generic(
                    "get takes exactly two arguments".to_string(),
                ));
            }
            if let LispVal::HashTable(h) = &args[0] {
                let key = &args[1];
                if let Some(val) = h.borrow().get(key) {
                    Ok(val.clone())
                } else {
                    Ok(LispVal::Nil)
                }
            } else {
                Err(LispError::Generic(
                    "get requires a hash table as its first argument".to_string(),
                ))
            }
        }
        BuiltinFunc::DeleteKey => {
            if args.len() != 2 {
                return Err(LispError::Generic(
                    "delete-key! takes exactly two arguments".to_string(),
                ));
            }
            if let LispVal::HashTable(h) = &args[0] {
                let key = &args[1];
                h.borrow_mut().remove(key);
                Ok(LispVal::Symbol(env.intern_symbol("T")))
            } else {
                Err(LispError::Generic(
                    "delete-key! requires a hash table as its first argument".to_string(),
                ))
            }
        }
        BuiltinFunc::CurrentEnvironment => {
            if !args.is_empty() {
                return Err(LispError::Generic(
                    "current-environment takes no arguments".to_string(),
                ));
            }
            let bindings = env.all_bindings();
            let mut hash_map = HashMap::new();
            for (k, v) in bindings {
                hash_map.insert(LispVal::Symbol(env.intern_symbol(&k)), v);
            }
            Ok(LispVal::HashTable(Rc::new(RefCell::new(hash_map))))
        }
        BuiltinFunc::Keys => {
            if args.len() != 1 {
                return Err(LispError::Generic(
                    "keys requires exactly one argument".to_string(),
                ));
            }
            if let LispVal::HashTable(h) = &args[0] {
                let keys = h.borrow().keys().cloned().collect();
                Ok(vec_to_list(keys))
            } else {
                Err(LispError::Generic(
                    "keys requires a hash table as its first argument".to_string(),
                ))
            }
        }
        _ => Err(LispError::Generic("Not a hash table operation".to_string())),
    }
}

/// Extract a feature name (symbol or string) from a single-argument builtin.
fn feature_name_arg(args: &[LispVal], who: &str) -> Result<String, LispError> {
    if args.len() != 1 {
        return Err(LispError::Generic(format!(
            "{who} requires exactly one argument"
        )));
    }
    match &args[0] {
        LispVal::Symbol(s) => Ok(s.borrow().name.clone()),
        LispVal::String(s) => Ok(s.clone()),
        _ => Err(LispError::Generic(format!(
            "{who} requires a symbol or string"
        ))),
    }
}

/// Capability guards for the three filesystem feature tiers.
///
/// Each returns `Ok(())` if the feature is enabled, or a descriptive error otherwise.
fn require_read_fs(env: &Rc<Environment>) -> Result<(), LispError> {
    if env.feature_enabled("READ-FS") {
        Ok(())
    } else {
        Err(LispError::Generic(
            "READ-FS capability is not enabled (grant it via --capability READ-FS or the host API)"
                .to_string(),
        ))
    }
}

fn require_create_fs(env: &Rc<Environment>) -> Result<(), LispError> {
    if env.feature_enabled("CREATE-FS") {
        Ok(())
    } else {
        Err(LispError::Generic(
            "CREATE-FS capability is not enabled (grant it via --capability CREATE-FS or the host API)"
                .to_string(),
        ))
    }
}

fn require_temp_fs(env: &Rc<Environment>) -> Result<(), LispError> {
    if env.feature_enabled("TEMP-FS") {
        Ok(())
    } else {
        Err(LispError::Generic(
            "TEMP-FS capability is not enabled (grant it via --capability TEMP-FS or the host API)"
                .to_string(),
        ))
    }
}

/// Build a unique path inside the system temp directory.
///
/// Uses the process ID and a per-process monotone counter so that concurrent
/// calls within the same process produce distinct names without any locking.
fn make_temp_path(prefix: &str, suffix: &str) -> std::path::PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let pid = std::process::id();
    let name = if prefix.is_empty() {
        format!("lamedh-{pid}-{n}{suffix}")
    } else {
        format!("{prefix}-{pid}-{n}{suffix}")
    };
    std::env::temp_dir().join(name)
}

/// Run an external program. Gated behind the SHELL capability (off by default).
///
/// - `(SHELL "cmd ...")`  -> run the single string via `sh -c`
/// - `(SHELL "prog" a b)` -> exec program with args, no shell interpolation
///
/// Returns `(exit-code stdout-string stderr-string)`.
#[inline(never)]
fn apply_shell(args: &[LispVal], env: &Rc<Environment>) -> Result<LispVal, LispError> {
    if !env.feature_enabled("SHELL") {
        return Err(LispError::Generic(
            "SHELL capability is not enabled (grant it via --capability SHELL or the host API)"
                .to_string(),
        ));
    }
    if args.is_empty() {
        return Err(LispError::Generic(
            "shell requires at least one argument".to_string(),
        ));
    }

    let mut parts: Vec<String> = Vec::with_capacity(args.len());
    for a in args {
        match a {
            LispVal::String(s) => parts.push(s.clone()),
            LispVal::Symbol(s) => parts.push(s.borrow().name.clone()),
            LispVal::Number(n) => parts.push(n.to_string()),
            LispVal::Float(f) => parts.push(f.to_string()),
            _ => {
                return Err(LispError::Generic(
                    "shell arguments must be strings, symbols, or numbers".to_string(),
                ));
            }
        }
    }

    use std::process::Command;
    let result = if parts.len() == 1 {
        Command::new("sh").arg("-c").arg(&parts[0]).output()
    } else {
        Command::new(&parts[0]).args(&parts[1..]).output()
    };
    let output =
        result.map_err(|e| LispError::Generic(format!("shell: failed to run command: {e}")))?;

    let code = output.status.code().unwrap_or(-1) as i64;
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();

    // Build (code stdout stderr) so Lisp can compose on the result.
    Ok(LispVal::Cons {
        car: Rc::new(LispVal::Number(code)),
        cdr: Rc::new(LispVal::Cons {
            car: Rc::new(LispVal::String(stdout)),
            cdr: Rc::new(LispVal::Cons {
                car: Rc::new(LispVal::String(stderr)),
                cdr: Rc::new(LispVal::Nil),
            }),
        }),
    })
}

fn apply(func: &LispVal, args: &[LispVal], env: &Rc<Environment>) -> Result<LispVal, LispError> {
    match func {
        LispVal::Builtin(builtin) => match builtin {
            BuiltinFunc::Plus
            | BuiltinFunc::Minus
            | BuiltinFunc::Multiply
            | BuiltinFunc::Divide => apply_math_op(builtin, args, env),
            BuiltinFunc::Lessp
            | BuiltinFunc::Greaterp
            | BuiltinFunc::Zerop
            | BuiltinFunc::Remainder
            | BuiltinFunc::Expt => apply_numeric_primitives(builtin, args, env),
            BuiltinFunc::Car | BuiltinFunc::Cdr | BuiltinFunc::Cons => apply_list_op(builtin, args),
            BuiltinFunc::Concat | BuiltinFunc::Index => apply_string_op(builtin, args),
            BuiltinFunc::Sort => apply_sort(args, env),
            BuiltinFunc::Sqrt
            | BuiltinFunc::Sin
            | BuiltinFunc::Cos
            | BuiltinFunc::Tan
            | BuiltinFunc::Log
            | BuiltinFunc::Exp
            | BuiltinFunc::Floor
            | BuiltinFunc::Ceiling
            | BuiltinFunc::Round
            | BuiltinFunc::Truncate
            | BuiltinFunc::Gcd
            | BuiltinFunc::Lcm
            | BuiltinFunc::Isqrt
            | BuiltinFunc::Signum => apply_math_lib(builtin, args),
            BuiltinFunc::StringLength
            | BuiltinFunc::Substring
            | BuiltinFunc::CharCode
            | BuiltinFunc::CodeChar
            | BuiltinFunc::MakeChar
            | BuiltinFunc::StringToNumber
            | BuiltinFunc::NumberToString
            | BuiltinFunc::Prin1ToString
            | BuiltinFunc::PrincToString => apply_string_lib(builtin, args),
            BuiltinFunc::MakeError
            | BuiltinFunc::ErrorP
            | BuiltinFunc::ErrorMessage
            | BuiltinFunc::ErrorData => apply_error_value_op(builtin, args, env),
            BuiltinFunc::Evlis => {
                // evlis[m;a] — evaluate each element of m in environment a
                let (list, eval_env) = match args.len() {
                    1 => (&args[0], env.clone()),
                    2 => {
                        if let LispVal::Environment(e) = &args[1] {
                            (&args[0], e.clone())
                        } else {
                            return Err(LispError::Generic(
                                "evlis: second argument must be an environment".to_string(),
                            ));
                        }
                    }
                    _ => {
                        return Err(LispError::Generic(
                            "evlis takes 1 or 2 arguments".to_string(),
                        ));
                    }
                };
                let mut result = vec![];
                let mut cur = list.clone();
                while let LispVal::Cons { car, cdr } = cur {
                    result.push(eval(&car, &eval_env)?);
                    cur = cdr.as_ref().clone();
                }
                let mut out = LispVal::Nil;
                for v in result.into_iter().rev() {
                    out = LispVal::Cons {
                        car: Rc::new(v),
                        cdr: Rc::new(out),
                    };
                }
                Ok(out)
            }
            BuiltinFunc::Evcon => {
                // evcon[c;a] — evaluate clauses until one passes, return its value
                // Clauses: ((test value) ...) evaluated in env a
                let (clauses, eval_env) = match args.len() {
                    1 => (&args[0], env.clone()),
                    2 => {
                        if let LispVal::Environment(e) = &args[1] {
                            (&args[0], e.clone())
                        } else {
                            return Err(LispError::Generic(
                                "evcon: second argument must be an environment".to_string(),
                            ));
                        }
                    }
                    _ => {
                        return Err(LispError::Generic(
                            "evcon takes 1 or 2 arguments".to_string(),
                        ));
                    }
                };
                let mut cur = clauses.clone();
                loop {
                    match cur {
                        LispVal::Nil => return Ok(LispVal::Nil),
                        LispVal::Cons { car, cdr } => {
                            let clause = list_to_vec(&car)?;
                            if clause.len() != 2 {
                                return Err(LispError::Generic(
                                    "evcon: each clause must be (test value)".to_string(),
                                ));
                            }
                            let test = eval(&clause[0], &eval_env)?;
                            if test != LispVal::Nil {
                                return eval(&clause[1], &eval_env);
                            }
                            cur = cdr.as_ref().clone();
                        }
                        _ => {
                            return Err(LispError::Generic(
                                "evcon: clauses must be a proper list".to_string(),
                            ));
                        }
                    }
                }
            }
            BuiltinFunc::Eval => match args.len() {
                1 => eval(&args[0], env),
                2 => {
                    if let LispVal::Environment(eval_env) = &args[1] {
                        eval(&args[0], eval_env)
                    } else {
                        Err(LispError::Generic(
                            "eval: second argument must be an environment".to_string(),
                        ))
                    }
                }
                _ => Err(LispError::Generic(
                    "eval takes 1 or 2 arguments".to_string(),
                )),
            },
            BuiltinFunc::Eq | BuiltinFunc::Not | BuiltinFunc::NumericEquals => {
                apply_logical_op(builtin, args, env)
            }
            BuiltinFunc::MakeHashTable
            | BuiltinFunc::Get
            | BuiltinFunc::Set
            | BuiltinFunc::DeleteKey
            | BuiltinFunc::CurrentEnvironment
            | BuiltinFunc::Keys => apply_hashtable_op(builtin, args, env),
            BuiltinFunc::Atom => {
                if args.len() != 1 {
                    return Err(LispError::Generic(
                        "atom requires exactly one argument".to_string(),
                    ));
                }
                match &args[0] {
                    LispVal::Cons { .. } => Ok(LispVal::Nil),
                    _ => Ok(LispVal::Symbol(env.intern_symbol("T"))),
                }
            }
            BuiltinFunc::Print => {
                for arg in args {
                    print!("{}", crate::printer::print(arg));
                }
                println!();
                Ok(LispVal::Nil)
            }
            BuiltinFunc::GetP | BuiltinFunc::PutP => apply_symbol_op(builtin, args, env),
            BuiltinFunc::Stringp => {
                if args.len() != 1 {
                    return Err(LispError::Generic(
                        "stringp requires exactly one argument".to_string(),
                    ));
                }
                match &args[0] {
                    LispVal::String(_) => Ok(LispVal::Symbol(env.intern_symbol("T"))),
                    _ => Ok(LispVal::Nil),
                }
            }
            BuiltinFunc::Numberp => {
                if args.len() != 1 {
                    return Err(LispError::Generic(
                        "numberp requires exactly one argument".to_string(),
                    ));
                }
                match &args[0] {
                    LispVal::Number(_) | LispVal::Float(_) => {
                        Ok(LispVal::Symbol(env.intern_symbol("T")))
                    }
                    _ => Ok(LispVal::Nil),
                }
            }

            BuiltinFunc::Apply => apply_apply(args, env),

            // I/O functions
            BuiltinFunc::Read
            | BuiltinFunc::Prin1
            | BuiltinFunc::Princ
            | BuiltinFunc::Terpri
            | BuiltinFunc::Spaces => apply_io_op(builtin, args, env),

            // Error handling
            BuiltinFunc::Error | BuiltinFunc::Errorset => apply_error_op(builtin, args, env),

            // List processing
            BuiltinFunc::Subst
            | BuiltinFunc::Sublis
            | BuiltinFunc::Assoc
            | BuiltinFunc::Maplist
            | BuiltinFunc::Mapcar
            | BuiltinFunc::Rplaca
            | BuiltinFunc::Rplacd => apply_list_processing(builtin, args, env),

            // Bitwise operations
            BuiltinFunc::Logor
            | BuiltinFunc::Logand
            | BuiltinFunc::Logxor
            | BuiltinFunc::Leftshift => apply_bitwise_op(builtin, args, env),

            // Property list functions
            BuiltinFunc::Remprop | BuiltinFunc::Deflist => apply_plist_op(builtin, args, env),

            // Type predicates
            BuiltinFunc::Fixp => {
                if args.len() != 1 {
                    return Err(LispError::Generic(
                        "fixp requires exactly one argument".to_string(),
                    ));
                }
                match &args[0] {
                    LispVal::Number(_) => Ok(LispVal::Symbol(env.intern_symbol("T"))),
                    _ => Ok(LispVal::Nil),
                }
            }
            BuiltinFunc::Charp => {
                if args.len() != 1 {
                    return Err(LispError::Generic(
                        "charp requires exactly one argument".to_string(),
                    ));
                }
                match &args[0] {
                    LispVal::Char(_) => Ok(LispVal::Symbol(env.intern_symbol("T"))),
                    _ => Ok(LispVal::Nil),
                }
            }
            BuiltinFunc::Floatp => {
                if args.len() != 1 {
                    return Err(LispError::Generic(
                        "floatp requires exactly one argument".to_string(),
                    ));
                }
                match &args[0] {
                    LispVal::Float(_) => Ok(LispVal::Symbol(env.intern_symbol("T"))),
                    _ => Ok(LispVal::Nil),
                }
            }

            // New type predicates
            BuiltinFunc::Symbolp
            | BuiltinFunc::Boundp
            | BuiltinFunc::Functionp
            | BuiltinFunc::Macrop
            | BuiltinFunc::Arrayp
            | BuiltinFunc::Extensionp => apply_type_predicates(builtin, args, env),
            BuiltinFunc::ExtensionTypeName => {
                if args.len() != 1 {
                    return Err(LispError::Generic(
                        "extension-type takes exactly one argument".to_string(),
                    ));
                }
                if let LispVal::Extension(e) = &args[0] {
                    Ok(LispVal::String(e.type_name().to_string()))
                } else {
                    Err(LispError::Generic(
                        "extension-type: argument must be an extension value".to_string(),
                    ))
                }
            }

            // New list operations
            BuiltinFunc::List
            | BuiltinFunc::Last
            | BuiltinFunc::Nth
            | BuiltinFunc::Nthcdr
            | BuiltinFunc::Efface => apply_new_list_ops(builtin, args, env),

            // New numeric operations
            BuiltinFunc::Mod
            | BuiltinFunc::Plusp
            | BuiltinFunc::Evenp
            | BuiltinFunc::Oddp
            | BuiltinFunc::Add1
            | BuiltinFunc::Sub1
            | BuiltinFunc::Random => apply_new_numeric_ops(builtin, args, env),

            // New bitwise operations
            BuiltinFunc::Ash | BuiltinFunc::Lognot | BuiltinFunc::Rot => {
                apply_new_bitwise_ops(builtin, args, env)
            }

            // Function operations
            BuiltinFunc::Funcall | BuiltinFunc::Macroexpand => {
                apply_function_ops(builtin, args, env)
            }

            // String/Symbol operations
            BuiltinFunc::Explode
            | BuiltinFunc::Implode
            | BuiltinFunc::Maknam
            | BuiltinFunc::Gensym
            | BuiltinFunc::Intern
            | BuiltinFunc::Plist => apply_string_symbol_ops(builtin, args, env),
            // Float comparisons (handle -0.0 vs 0.0 correctly)
            BuiltinFunc::FloatEqual => {
                if args.len() != 2 {
                    return Err(LispError::Generic(
                        "float= requires exactly two arguments".to_string(),
                    ));
                }
                let f1 = match &args[0] {
                    LispVal::Float(f) => *f,
                    LispVal::Number(n) => *n as f64,
                    _ => {
                        return Err(LispError::Generic(
                            "float= requires numeric arguments".to_string(),
                        ));
                    }
                };
                let f2 = match &args[1] {
                    LispVal::Float(f) => *f,
                    LispVal::Number(n) => *n as f64,
                    _ => {
                        return Err(LispError::Generic(
                            "float= requires numeric arguments".to_string(),
                        ));
                    }
                };
                // Use bitwise equality to distinguish -0.0 from 0.0
                if f1.to_bits() == f2.to_bits() {
                    Ok(LispVal::Symbol(env.intern_symbol("T")))
                } else {
                    Ok(LispVal::Nil)
                }
            }

            BuiltinFunc::FloatLessp => {
                if args.len() != 2 {
                    return Err(LispError::Generic(
                        "float< requires exactly two arguments".to_string(),
                    ));
                }
                let f1 = match &args[0] {
                    LispVal::Float(f) => *f,
                    LispVal::Number(n) => *n as f64,
                    _ => {
                        return Err(LispError::Generic(
                            "float< requires numeric arguments".to_string(),
                        ));
                    }
                };
                let f2 = match &args[1] {
                    LispVal::Float(f) => *f,
                    LispVal::Number(n) => *n as f64,
                    _ => {
                        return Err(LispError::Generic(
                            "float< requires numeric arguments".to_string(),
                        ));
                    }
                };
                if f1 < f2 {
                    Ok(LispVal::Symbol(env.intern_symbol("T")))
                } else {
                    Ok(LispVal::Nil)
                }
            }

            BuiltinFunc::FloatGreaterp => {
                if args.len() != 2 {
                    return Err(LispError::Generic(
                        "float> requires exactly two arguments".to_string(),
                    ));
                }
                let f1 = match &args[0] {
                    LispVal::Float(f) => *f,
                    LispVal::Number(n) => *n as f64,
                    _ => {
                        return Err(LispError::Generic(
                            "float> requires numeric arguments".to_string(),
                        ));
                    }
                };
                let f2 = match &args[1] {
                    LispVal::Float(f) => *f,
                    LispVal::Number(n) => *n as f64,
                    _ => {
                        return Err(LispError::Generic(
                            "float> requires numeric arguments".to_string(),
                        ));
                    }
                };
                if f1 > f2 {
                    Ok(LispVal::Symbol(env.intern_symbol("T")))
                } else {
                    Ok(LispVal::Nil)
                }
            }

            BuiltinFunc::LoadFile => {
                require_read_fs(env)?;
                if args.len() != 1 {
                    return Err(LispError::Generic(
                        "load-file requires exactly one argument".to_string(),
                    ));
                }

                let filename = if let LispVal::String(path) = &args[0] {
                    path.clone()
                } else {
                    return Err(LispError::Generic(
                        "load-file requires a string filename".to_string(),
                    ));
                };

                crate::load_file(&filename, env)?;
                Ok(LispVal::Symbol(env.intern_symbol("T")))
            }

            BuiltinFunc::ReadFile => {
                require_read_fs(env)?;
                if args.len() != 1 {
                    return Err(LispError::Generic(
                        "read-file requires exactly one argument".to_string(),
                    ));
                }
                let path = match &args[0] {
                    LispVal::String(s) => s.clone(),
                    _ => {
                        return Err(LispError::Generic(
                            "read-file: path must be a string".to_string(),
                        ));
                    }
                };
                let contents = std::fs::read_to_string(&path)
                    .map_err(|e| LispError::Generic(format!("read-file: {e}")))?;
                Ok(LispVal::String(contents))
            }

            BuiltinFunc::ReadFileByte => {
                require_read_fs(env)?;
                if args.len() != 2 {
                    return Err(LispError::Generic(
                        "read-file-byte requires exactly two arguments: path offset".to_string(),
                    ));
                }
                let path = match &args[0] {
                    LispVal::String(s) => s.clone(),
                    _ => {
                        return Err(LispError::Generic(
                            "read-file-byte: path must be a string".to_string(),
                        ));
                    }
                };
                let offset = match &args[1] {
                    LispVal::Number(n) if *n >= 0 => *n as u64,
                    _ => {
                        return Err(LispError::Generic(
                            "read-file-byte: offset must be a non-negative integer".to_string(),
                        ));
                    }
                };
                use std::io::{Read, Seek, SeekFrom};
                let mut file = std::fs::File::open(&path)
                    .map_err(|e| LispError::Generic(format!("read-file-byte: {e}")))?;
                file.seek(SeekFrom::Start(offset))
                    .map_err(|e| LispError::Generic(format!("read-file-byte: seek: {e}")))?;
                let mut buf = [0u8; 1];
                let n = file
                    .read(&mut buf)
                    .map_err(|e| LispError::Generic(format!("read-file-byte: {e}")))?;
                if n == 0 {
                    Ok(LispVal::Nil)
                } else {
                    Ok(LispVal::Number(buf[0] as i64))
                }
            }

            BuiltinFunc::ReadFileSection => {
                require_read_fs(env)?;
                if args.len() != 3 {
                    return Err(LispError::Generic(
                        "read-file-section requires exactly three arguments: path offset len"
                            .to_string(),
                    ));
                }
                let path = match &args[0] {
                    LispVal::String(s) => s.clone(),
                    _ => {
                        return Err(LispError::Generic(
                            "read-file-section: path must be a string".to_string(),
                        ));
                    }
                };
                let offset = match &args[1] {
                    LispVal::Number(n) if *n >= 0 => *n as u64,
                    _ => {
                        return Err(LispError::Generic(
                            "read-file-section: offset must be a non-negative integer".to_string(),
                        ));
                    }
                };
                let len = match &args[2] {
                    LispVal::Number(n) if *n >= 0 => *n as usize,
                    _ => {
                        return Err(LispError::Generic(
                            "read-file-section: len must be a non-negative integer".to_string(),
                        ));
                    }
                };
                use std::io::{Read, Seek, SeekFrom};
                let mut file = std::fs::File::open(&path)
                    .map_err(|e| LispError::Generic(format!("read-file-section: {e}")))?;
                file.seek(SeekFrom::Start(offset))
                    .map_err(|e| LispError::Generic(format!("read-file-section: seek: {e}")))?;
                let mut buf = vec![0u8; len];
                let n = file
                    .read(&mut buf)
                    .map_err(|e| LispError::Generic(format!("read-file-section: {e}")))?;
                buf.truncate(n);
                Ok(LispVal::String(String::from_utf8_lossy(&buf).into_owned()))
            }

            BuiltinFunc::WriteFile => {
                require_create_fs(env)?;
                if args.len() != 2 {
                    return Err(LispError::Generic(
                        "write-file requires exactly two arguments: path content".to_string(),
                    ));
                }
                let path = match &args[0] {
                    LispVal::String(s) => s.clone(),
                    _ => {
                        return Err(LispError::Generic(
                            "write-file: path must be a string".to_string(),
                        ));
                    }
                };
                let content = match &args[1] {
                    LispVal::String(s) => s.clone(),
                    _ => {
                        return Err(LispError::Generic(
                            "write-file: content must be a string".to_string(),
                        ));
                    }
                };
                std::fs::write(&path, content.as_bytes())
                    .map_err(|e| LispError::Generic(format!("write-file: {e}")))?;
                Ok(LispVal::Symbol(env.intern_symbol("T")))
            }

            // ── File metadata predicates ────────────────────────────────────
            BuiltinFunc::FileExistsP => {
                require_read_fs(env)?;
                if args.len() != 1 {
                    return Err(LispError::Generic(
                        "file-exists-p requires exactly one argument".to_string(),
                    ));
                }
                let path = match &args[0] {
                    LispVal::String(s) => s.clone(),
                    _ => {
                        return Err(LispError::Generic(
                            "file-exists-p: path must be a string".to_string(),
                        ));
                    }
                };
                if std::path::Path::new(&path).exists() {
                    Ok(LispVal::Symbol(env.intern_symbol("T")))
                } else {
                    Ok(LispVal::Nil)
                }
            }

            BuiltinFunc::DirectoryP => {
                require_read_fs(env)?;
                if args.len() != 1 {
                    return Err(LispError::Generic(
                        "directory-p requires exactly one argument".to_string(),
                    ));
                }
                let path = match &args[0] {
                    LispVal::String(s) => s.clone(),
                    _ => {
                        return Err(LispError::Generic(
                            "directory-p: path must be a string".to_string(),
                        ));
                    }
                };
                if std::path::Path::new(&path).is_dir() {
                    Ok(LispVal::Symbol(env.intern_symbol("T")))
                } else {
                    Ok(LispVal::Nil)
                }
            }

            BuiltinFunc::FileP => {
                require_read_fs(env)?;
                if args.len() != 1 {
                    return Err(LispError::Generic(
                        "file-p requires exactly one argument".to_string(),
                    ));
                }
                let path = match &args[0] {
                    LispVal::String(s) => s.clone(),
                    _ => {
                        return Err(LispError::Generic(
                            "file-p: path must be a string".to_string(),
                        ));
                    }
                };
                if std::path::Path::new(&path).is_file() {
                    Ok(LispVal::Symbol(env.intern_symbol("T")))
                } else {
                    Ok(LispVal::Nil)
                }
            }

            BuiltinFunc::FileReadableP => {
                require_read_fs(env)?;
                if args.len() != 1 {
                    return Err(LispError::Generic(
                        "file-readable-p requires exactly one argument".to_string(),
                    ));
                }
                let path = match &args[0] {
                    LispVal::String(s) => s.clone(),
                    _ => {
                        return Err(LispError::Generic(
                            "file-readable-p: path must be a string".to_string(),
                        ));
                    }
                };
                // Opening for read is the most reliable check with std-only.
                if std::fs::File::open(&path).is_ok() {
                    Ok(LispVal::Symbol(env.intern_symbol("T")))
                } else {
                    Ok(LispVal::Nil)
                }
            }

            BuiltinFunc::FileWritableP => {
                require_read_fs(env)?;
                if args.len() != 1 {
                    return Err(LispError::Generic(
                        "file-writable-p requires exactly one argument".to_string(),
                    ));
                }
                let path = match &args[0] {
                    LispVal::String(s) => s.clone(),
                    _ => {
                        return Err(LispError::Generic(
                            "file-writable-p: path must be a string".to_string(),
                        ));
                    }
                };
                let writable = std::fs::metadata(&path)
                    .map(|m| !m.permissions().readonly())
                    .unwrap_or(false);
                if writable {
                    Ok(LispVal::Symbol(env.intern_symbol("T")))
                } else {
                    Ok(LispVal::Nil)
                }
            }

            BuiltinFunc::FileExecutableP => {
                require_read_fs(env)?;
                if args.len() != 1 {
                    return Err(LispError::Generic(
                        "file-executable-p requires exactly one argument".to_string(),
                    ));
                }
                let path = match &args[0] {
                    LispVal::String(s) => s.clone(),
                    _ => {
                        return Err(LispError::Generic(
                            "file-executable-p: path must be a string".to_string(),
                        ));
                    }
                };
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    let executable = std::fs::metadata(&path)
                        .map(|m| m.permissions().mode() & 0o111 != 0)
                        .unwrap_or(false);
                    Ok(if executable {
                        LispVal::Symbol(env.intern_symbol("T"))
                    } else {
                        LispVal::Nil
                    })
                }
                #[cfg(not(unix))]
                Ok(LispVal::Nil)
            }

            BuiltinFunc::FileSize => {
                require_read_fs(env)?;
                if args.len() != 1 {
                    return Err(LispError::Generic(
                        "file-size requires exactly one argument".to_string(),
                    ));
                }
                let path = match &args[0] {
                    LispVal::String(s) => s.clone(),
                    _ => {
                        return Err(LispError::Generic(
                            "file-size: path must be a string".to_string(),
                        ));
                    }
                };
                let size = std::fs::metadata(&path)
                    .map_err(|e| LispError::Generic(format!("file-size: {e}")))?
                    .len();
                Ok(LispVal::Number(size as i64))
            }

            BuiltinFunc::DirectoryFiles => {
                require_read_fs(env)?;
                if args.len() != 1 {
                    return Err(LispError::Generic(
                        "directory-files requires exactly one argument".to_string(),
                    ));
                }
                let path = match &args[0] {
                    LispVal::String(s) => s.clone(),
                    _ => {
                        return Err(LispError::Generic(
                            "directory-files: path must be a string".to_string(),
                        ));
                    }
                };
                let mut names: Vec<String> = std::fs::read_dir(&path)
                    .map_err(|e| LispError::Generic(format!("directory-files: {e}")))?
                    .filter_map(|entry| entry.ok().and_then(|e| e.file_name().into_string().ok()))
                    .collect();
                names.sort();
                let list = names
                    .into_iter()
                    .rev()
                    .fold(LispVal::Nil, |cdr, name| LispVal::Cons {
                        car: Rc::new(LispVal::String(name)),
                        cdr: Rc::new(cdr),
                    });
                Ok(list)
            }

            BuiltinFunc::FileNewerP => {
                require_read_fs(env)?;
                if args.len() != 2 {
                    return Err(LispError::Generic(
                        "file-newer-p requires exactly two arguments: path1 path2".to_string(),
                    ));
                }
                let (p1, p2) = match (&args[0], &args[1]) {
                    (LispVal::String(a), LispVal::String(b)) => (a.clone(), b.clone()),
                    _ => {
                        return Err(LispError::Generic(
                            "file-newer-p: both arguments must be strings".to_string(),
                        ));
                    }
                };
                let mtime1 = std::fs::metadata(&p1)
                    .and_then(|m| m.modified())
                    .map_err(|e| LispError::Generic(format!("file-newer-p: {p1}: {e}")))?;
                let mtime2 = std::fs::metadata(&p2)
                    .and_then(|m| m.modified())
                    .map_err(|e| LispError::Generic(format!("file-newer-p: {p2}: {e}")))?;
                if mtime1 > mtime2 {
                    Ok(LispVal::Symbol(env.intern_symbol("T")))
                } else {
                    Ok(LispVal::Nil)
                }
            }

            // ── File mutation ───────────────────────────────────────────────
            BuiltinFunc::Chmod => {
                require_create_fs(env)?;
                if args.len() != 2 {
                    return Err(LispError::Generic(
                        "chmod requires exactly two arguments: path mode".to_string(),
                    ));
                }
                let path = match &args[0] {
                    LispVal::String(s) => s.clone(),
                    _ => {
                        return Err(LispError::Generic(
                            "chmod: path must be a string".to_string(),
                        ));
                    }
                };
                // Mode: integer (use directly) or octal string like "755".
                let mode: u32 = match &args[1] {
                    LispVal::Number(n) if *n >= 0 => *n as u32,
                    LispVal::String(s) => u32::from_str_radix(s, 8).map_err(|_| {
                        LispError::Generic(format!("chmod: cannot parse \"{s}\" as an octal mode"))
                    })?,
                    _ => {
                        return Err(LispError::Generic(
                            "chmod: mode must be an integer or octal string".to_string(),
                        ));
                    }
                };
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    let perms = std::fs::Permissions::from_mode(mode);
                    std::fs::set_permissions(&path, perms)
                        .map_err(|e| LispError::Generic(format!("chmod: {e}")))?;
                    Ok(LispVal::Symbol(env.intern_symbol("T")))
                }
                #[cfg(not(unix))]
                Err(LispError::Generic(
                    "chmod is only supported on Unix platforms".to_string(),
                ))
            }

            BuiltinFunc::CreateDirectory => {
                require_create_fs(env)?;
                if args.len() != 1 {
                    return Err(LispError::Generic(
                        "create-directory requires exactly one argument".to_string(),
                    ));
                }
                let path = match &args[0] {
                    LispVal::String(s) => s.clone(),
                    _ => {
                        return Err(LispError::Generic(
                            "create-directory: path must be a string".to_string(),
                        ));
                    }
                };
                std::fs::create_dir_all(&path)
                    .map_err(|e| LispError::Generic(format!("create-directory: {e}")))?;
                Ok(LispVal::Symbol(env.intern_symbol("T")))
            }

            BuiltinFunc::DeleteFile => {
                require_create_fs(env)?;
                if args.len() != 1 {
                    return Err(LispError::Generic(
                        "delete-file requires exactly one argument".to_string(),
                    ));
                }
                let path = match &args[0] {
                    LispVal::String(s) => s.clone(),
                    _ => {
                        return Err(LispError::Generic(
                            "delete-file: path must be a string".to_string(),
                        ));
                    }
                };
                std::fs::remove_file(&path)
                    .map_err(|e| LispError::Generic(format!("delete-file: {e}")))?;
                Ok(LispVal::Symbol(env.intern_symbol("T")))
            }

            BuiltinFunc::RenameFile => {
                require_create_fs(env)?;
                if args.len() != 2 {
                    return Err(LispError::Generic(
                        "rename-file requires exactly two arguments: from to".to_string(),
                    ));
                }
                let (from, to) = match (&args[0], &args[1]) {
                    (LispVal::String(a), LispVal::String(b)) => (a.clone(), b.clone()),
                    _ => {
                        return Err(LispError::Generic(
                            "rename-file: both arguments must be strings".to_string(),
                        ));
                    }
                };
                std::fs::rename(&from, &to)
                    .map_err(|e| LispError::Generic(format!("rename-file: {e}")))?;
                Ok(LispVal::Symbol(env.intern_symbol("T")))
            }

            // ── Temp filesystem ─────────────────────────────────────────────
            BuiltinFunc::MakeTempFile => {
                require_temp_fs(env)?;
                let prefix = match args.first() {
                    Some(LispVal::String(s)) => s.clone(),
                    None => String::new(),
                    _ => {
                        return Err(LispError::Generic(
                            "make-temp-file: optional prefix must be a string".to_string(),
                        ));
                    }
                };
                let path = make_temp_path(&prefix, "");
                // Create the file atomically; fail if it somehow already exists.
                std::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&path)
                    .map_err(|e| LispError::Generic(format!("make-temp-file: {e}")))?;
                Ok(LispVal::String(path.to_string_lossy().into_owned()))
            }

            BuiltinFunc::MakeTempDirectory => {
                require_temp_fs(env)?;
                let prefix = match args.first() {
                    Some(LispVal::String(s)) => s.clone(),
                    None => String::new(),
                    _ => {
                        return Err(LispError::Generic(
                            "make-temp-directory: optional prefix must be a string".to_string(),
                        ));
                    }
                };
                let path = make_temp_path(&prefix, "");
                std::fs::create_dir(&path)
                    .map_err(|e| LispError::Generic(format!("make-temp-directory: {e}")))?;
                Ok(LispVal::String(path.to_string_lossy().into_owned()))
            }

            // Condition flags
            BuiltinFunc::SetFlag => {
                if args.len() != 1 {
                    return Err(LispError::Generic(
                        "set-flag requires exactly one argument".to_string(),
                    ));
                }
                let flag_name = match &args[0] {
                    LispVal::Symbol(s) => s.borrow().name.clone(),
                    LispVal::String(s) => s.clone(),
                    _ => {
                        return Err(LispError::Generic(
                            "set-flag requires a symbol or string".to_string(),
                        ));
                    }
                };
                env.set_flag(&flag_name);
                Ok(LispVal::Symbol(env.intern_symbol("T")))
            }

            BuiltinFunc::ClearFlag => {
                if args.len() != 1 {
                    return Err(LispError::Generic(
                        "clear-flag requires exactly one argument".to_string(),
                    ));
                }
                let flag_name = match &args[0] {
                    LispVal::Symbol(s) => s.borrow().name.clone(),
                    LispVal::String(s) => s.clone(),
                    _ => {
                        return Err(LispError::Generic(
                            "clear-flag requires a symbol or string".to_string(),
                        ));
                    }
                };
                env.clear_flag(&flag_name);
                Ok(LispVal::Symbol(env.intern_symbol("T")))
            }

            BuiltinFunc::FlagSetP => {
                if args.len() != 1 {
                    return Err(LispError::Generic(
                        "flag-set-p requires exactly one argument".to_string(),
                    ));
                }
                let flag_name = match &args[0] {
                    LispVal::Symbol(s) => s.borrow().name.clone(),
                    LispVal::String(s) => s.clone(),
                    _ => {
                        return Err(LispError::Generic(
                            "flag-set-p requires a symbol or string".to_string(),
                        ));
                    }
                };
                if env.flag_set(&flag_name) {
                    Ok(LispVal::Symbol(env.intern_symbol("T")))
                } else {
                    Ok(LispVal::Nil)
                }
            }

            BuiltinFunc::ClearAllFlags => {
                if !args.is_empty() {
                    return Err(LispError::Generic(
                        "clear-all-flags takes no arguments".to_string(),
                    ));
                }
                env.clear_all_flags();
                Ok(LispVal::Symbol(env.intern_symbol("T")))
            }

            // Capabilities / features (read-only from Lisp)
            BuiltinFunc::FeatureEnabledP => {
                let name = feature_name_arg(args, "feature-enabled-p")?;
                if env.feature_enabled(&name) {
                    Ok(LispVal::Symbol(env.intern_symbol("T")))
                } else {
                    Ok(LispVal::Nil)
                }
            }
            BuiltinFunc::Features => {
                if !args.is_empty() {
                    return Err(LispError::Generic(
                        "features takes no arguments".to_string(),
                    ));
                }
                let mut names = env.features_list();
                names.sort();
                let list = names
                    .into_iter()
                    .rev()
                    .fold(LispVal::Nil, |cdr, n| LispVal::Cons {
                        car: Rc::new(LispVal::String(n)),
                        cdr: Rc::new(cdr),
                    });
                Ok(list)
            }
            BuiltinFunc::Shell => apply_shell(args, env),
            BuiltinFunc::TheEnvironment => {
                if !args.is_empty() {
                    return Err(LispError::Generic(
                        "the-environment takes no arguments".to_string(),
                    ));
                }
                Ok(LispVal::Environment(Rc::clone(env)))
            }
            BuiltinFunc::MakeEnvironment => match args.len() {
                0 => Ok(LispVal::Environment(Environment::new_with_builtins())),
                1 => {
                    if let LispVal::Environment(parent) = &args[0] {
                        Ok(LispVal::Environment(Environment::new_child(parent)))
                    } else {
                        Err(LispError::Generic(
                            "make-environment: argument must be an environment".to_string(),
                        ))
                    }
                }
                _ => Err(LispError::Generic(
                    "make-environment takes 0 or 1 arguments".to_string(),
                )),
            },
            BuiltinFunc::Optimize => {
                if args.len() != 1 {
                    return Err(LispError::Generic(
                        "optimize takes exactly one argument".to_string(),
                    ));
                }
                Ok(crate::optimizer::optimize(&args[0]))
            }
            // ── Arrays ─────────────────────────────────────────────────────
            BuiltinFunc::MakeArray => {
                if args.len() != 1 {
                    return Err(LispError::Generic(
                        "array takes exactly one argument".to_string(),
                    ));
                }
                let n = match &args[0] {
                    LispVal::Number(n) if *n >= 0 => *n as usize,
                    _ => {
                        return Err(LispError::Generic(
                            "array: size must be a non-negative integer".to_string(),
                        ));
                    }
                };
                let v = vec![LispVal::Nil; n];
                Ok(LispVal::Array(Rc::new(RefCell::new(v))))
            }
            BuiltinFunc::ArrayFetch => {
                if args.len() != 2 {
                    return Err(LispError::Generic(
                        "fetch takes exactly two arguments".to_string(),
                    ));
                }
                if let LispVal::Array(a) = &args[0] {
                    let idx = match &args[1] {
                        LispVal::Number(n) if *n >= 0 => *n as usize,
                        _ => {
                            return Err(LispError::Generic(
                                "fetch: index must be a non-negative integer".to_string(),
                            ));
                        }
                    };
                    let v = a.borrow();
                    if idx >= v.len() {
                        return Err(LispError::Generic(format!(
                            "fetch: index {idx} out of bounds (length {})",
                            v.len()
                        )));
                    }
                    Ok(v[idx].clone())
                } else {
                    Err(LispError::Generic(
                        "fetch: first argument must be an array".to_string(),
                    ))
                }
            }
            BuiltinFunc::ArrayStore => {
                if args.len() != 3 {
                    return Err(LispError::Generic(
                        "store takes exactly three arguments".to_string(),
                    ));
                }
                if let LispVal::Array(a) = &args[0] {
                    let idx = match &args[1] {
                        LispVal::Number(n) if *n >= 0 => *n as usize,
                        _ => {
                            return Err(LispError::Generic(
                                "store: index must be a non-negative integer".to_string(),
                            ));
                        }
                    };
                    let val = args[2].clone();
                    let mut v = a.borrow_mut();
                    if idx >= v.len() {
                        return Err(LispError::Generic(format!(
                            "store: index {idx} out of bounds (length {})",
                            v.len()
                        )));
                    }
                    v[idx] = val.clone();
                    Ok(val)
                } else {
                    Err(LispError::Generic(
                        "store: first argument must be an array".to_string(),
                    ))
                }
            }
            BuiltinFunc::ArrayLength => {
                if args.len() != 1 {
                    return Err(LispError::Generic(
                        "array-length takes exactly one argument".to_string(),
                    ));
                }
                if let LispVal::Array(a) = &args[0] {
                    Ok(LispVal::Number(a.borrow().len() as i64))
                } else {
                    Err(LispError::Generic(
                        "array-length: argument must be an array".to_string(),
                    ))
                }
            }
        },
        LispVal::Lambda(lambda) => {
            // Create new environment with:
            // - Lexical parent: lambda.env (captured closure environment)
            // - Dynamic parent: env (caller's environment for dynamic variable lookup)
            let new_env = Environment::new_child_with_dynamic(&lambda.env, env);
            if let Some(rest_param_name) = &lambda.rest_param {
                if args.len() < lambda.params.len() {
                    return Err(LispError::Generic(format!(
                        "lambda expected at least {} arguments, got {}",
                        lambda.params.len(),
                        args.len()
                    )));
                }
                for (param, arg) in lambda.params.iter().zip(args.iter()) {
                    new_env.set(param.clone(), arg.clone());
                }
                let rest_args = vec_to_list(args[lambda.params.len()..].to_vec());
                new_env.set(rest_param_name.clone(), rest_args);
            } else {
                if lambda.params.len() != args.len() {
                    return Err(LispError::Generic(format!(
                        "lambda expected {} arguments, got {}",
                        lambda.params.len(),
                        args.len()
                    )));
                }
                for (param, arg) in lambda.params.iter().zip(args) {
                    new_env.set(param.clone(), arg.clone());
                }
            }

            eval(&lambda.body, &new_env)
        }
        LispVal::Native(f) => f(args, env),
        _ => Err(LispError::Generic(format!("Not a function: {func:?}"))),
    }
}

#[inline(never)]
fn make_lambda(
    params: &LispVal,
    body: &LispVal,
    env: &Rc<Environment>,
) -> Result<LispVal, LispError> {
    let p_list = list_to_vec(params)?;
    let mut params_vec = Vec::new();
    let mut rest_param = None;
    let mut iter = p_list.iter();

    while let Some(p) = iter.next() {
        if let LispVal::Symbol(s) = p {
            if s.borrow().name == "&REST" {
                if let Some(LispVal::Symbol(rest_p_sym)) = iter.next() {
                    if iter.next().is_some() {
                        return Err(LispError::Generic(
                            "Only one symbol can follow &rest".to_string(),
                        ));
                    }
                    rest_param = Some(rest_p_sym.borrow().name.clone());
                    break; // No more params after &rest
                } else {
                    return Err(LispError::Generic(
                        "&rest must be followed by a symbol".to_string(),
                    ));
                }
            } else {
                params_vec.push(s.borrow().name.clone());
            }
        } else {
            return Err(LispError::Generic(
                "lambda parameters must be symbols".to_string(),
            ));
        }
    }

    Ok(LispVal::Lambda(Box::new(crate::Lambda {
        params: params_vec,
        rest_param,
        body: Box::new(body.clone()),
        env: env.clone(),
    })))
}

#[inline(never)]
fn make_fexpr(
    params: &LispVal,
    body: &LispVal,
    env: &Rc<Environment>,
) -> Result<LispVal, LispError> {
    let p_list = list_to_vec(params)?;
    let params_vec: Result<Vec<String>, _> = p_list
        .iter()
        .map(|p| {
            if let LispVal::Symbol(s) = p {
                Ok(s.borrow().name.clone())
            } else {
                Err(LispError::Generic(
                    "fexpr parameters must be symbols".to_string(),
                ))
            }
        })
        .collect();

    Ok(LispVal::Fexpr(Box::new(crate::Fexpr {
        params: params_vec?,
        body: Box::new(body.clone()),
        env: env.clone(),
    })))
}

#[inline(never)]
fn make_macro(
    params: &LispVal,
    body: &LispVal,
    env: &Rc<Environment>,
) -> Result<LispVal, LispError> {
    let p_list = list_to_vec(params)?;
    let mut params_vec = Vec::new();
    let mut rest_param = None;
    let mut iter = p_list.iter();

    while let Some(p) = iter.next() {
        if let LispVal::Symbol(s) = p {
            if s.borrow().name == "&REST" {
                if let Some(LispVal::Symbol(rest_p_sym)) = iter.next() {
                    if iter.next().is_some() {
                        return Err(LispError::Generic(
                            "Only one symbol can follow &rest".to_string(),
                        ));
                    }
                    rest_param = Some(rest_p_sym.borrow().name.clone());
                    break; // No more params after &rest
                } else {
                    return Err(LispError::Generic(
                        "&rest must be followed by a symbol".to_string(),
                    ));
                }
            } else {
                params_vec.push(s.borrow().name.clone());
            }
        } else {
            return Err(LispError::Generic(
                "macro parameters must be symbols".to_string(),
            ));
        }
    }

    Ok(LispVal::Macro(Box::new(crate::Macro {
        params: params_vec,
        rest_param,
        body: Box::new(body.clone()),
        env: env.clone(),
    })))
}

#[inline(never)]
fn expand_macro(
    m: &crate::Macro,
    args: &[LispVal],
    _env: &Rc<Environment>,
) -> Result<LispVal, LispError> {
    let macro_env = Environment::new_child(&m.env);

    if let Some(rest_param_name) = &m.rest_param {
        if args.len() < m.params.len() {
            return Err(LispError::Generic(format!(
                "macro expected at least {} arguments, got {}",
                m.params.len(),
                args.len()
            )));
        }
        for (param, arg) in m.params.iter().zip(args.iter()) {
            macro_env.set(param.clone(), arg.clone());
        }
        let rest_args = vec_to_list(args[m.params.len()..].to_vec());
        macro_env.set(rest_param_name.clone(), rest_args);
    } else {
        if m.params.len() != args.len() {
            return Err(LispError::Generic(format!(
                "macro expected {} arguments, got {}",
                m.params.len(),
                args.len()
            )));
        }
        for (param, arg) in m.params.iter().zip(args) {
            macro_env.set(param.clone(), arg.clone());
        }
    }

    eval(&m.body, &macro_env)
}

/// Public entry point for evaluation. Acquires a depth-guard frame (issue #61)
/// Evaluate a single Lisp expression in `env`.
///
/// This is the primary entry point for evaluation.  It acquires a
/// recursion-depth guard and delegates to `eval_impl`, which uses a trampoline
/// loop for tail-call optimisation: tail positions (`IF` branches, `PROGN`
/// last form, `LET` body, lambda bodies) are handled without growing the Rust
/// call stack.
///
/// Non-tail recursive calls (e.g. evaluating function arguments) go through
/// `eval` again so the depth guard applies.
///
/// # Errors
///
/// Returns [`LispError::Generic`] on any runtime error, including recursion
/// depth exceeded.  [`LispError::Return`] and [`LispError::Go`] are internal
/// control-flow signals for `PROG`/`RETURN`/`GO` and should not escape a
/// top-level `eval` call under normal use.
pub fn eval(val: &LispVal, env: &Rc<Environment>) -> Result<LispVal, LispError> {
    // Bound recursion so deep/infinite recursion is a recoverable error rather
    // than a native stack overflow that aborts the whole process (issue #61).
    let _depth_guard = DepthGuard::enter()?;
    eval_impl(val.clone(), env.clone())
}

/// Represents the outcome of one iteration of the TCO trampoline.
/// Either we have a final value/error, or we have a tail call to continue with.
enum TcoStep {
    /// Evaluation is complete; return this value.
    Done(Result<LispVal, LispError>),
    /// Tail call: evaluate `val` in `env` next, reusing this stack frame.
    TailCall(LispVal, Rc<Environment>),
    /// Apply a non-tail callable (builtin/native) to already-evaluated args.
    /// Deferred to the driver loop so the `apply` runs *after* `eval_step`
    /// returns — releasing any borrow `eval_step` holds on the head symbol
    /// (e.g. the special-form dispatch `s.borrow()`). Without this, calling
    /// `apply` inline panics when a symbol-mutating builtin re-borrows the
    /// same interned symbol used as the call head, e.g. `(putp 'putp ...)`
    /// (issue #156).
    Apply(LispVal, Vec<LispVal>),
}

/// Internal trampoline evaluator. Runs a loop that reuses the current Rust
/// stack frame for tail calls, achieving proper TCO without consuming extra
/// native stack depth for each Lisp tail-recursive call.
///
/// All non-tail recursive calls (e.g. evaluating an IF condition, evaluating
/// function arguments) still go through the public `eval()` so that the depth
/// guard is correctly applied to non-tail frames.
fn eval_impl(initial_val: LispVal, initial_env: Rc<Environment>) -> Result<LispVal, LispError> {
    let mut current_val: LispVal = initial_val;
    let mut current_env: Rc<Environment> = initial_env;

    loop {
        // Each iteration computes a TcoStep, then either returns or loops.
        // All borrows of current_val/current_env are scoped inside this block
        // so they are released before we potentially assign to them.
        let step = {
            let val = &current_val;
            let env = &current_env;
            eval_step(val, env)
        }?;

        match step {
            TcoStep::Done(result) => return result,
            TcoStep::Apply(func, args) => return apply(&func, &args, &current_env),
            TcoStep::TailCall(new_val, new_env) => {
                current_val = new_val;
                current_env = new_env;
                // continue the loop
            }
        }
    }
}

/// Build the `Native` membrane entry for a typed function `name`.
fn make_typed_native(name: String) -> LispVal {
    LispVal::Native(Rc::new(
        move |args: &[LispVal], env: &Rc<Environment>| -> Result<LispVal, LispError> {
            let (ptys, _ret) = env.jit_signature(&name).ok_or_else(|| {
                LispError::Generic(format!("typed function {name} is not defined"))
            })?;
            if args.len() != ptys.len() {
                return Err(LispError::Generic(format!(
                    "{name}: expected {} args, got {}",
                    ptys.len(),
                    args.len()
                )));
            }
            let mut vals = Vec::with_capacity(args.len());
            for (a, ty) in args.iter().zip(ptys.iter()) {
                vals.push(lispval_to_typed(a, *ty).map_err(LispError::Generic)?);
            }
            match env.jit_call(&name, &vals) {
                Some(Ok(v)) => Ok(typed_to_lispval(v, env)),
                Some(Err(e)) => Err(LispError::Generic(e)),
                None => Err(LispError::Generic(format!(
                    "typed function {name} is not defined"
                ))),
            }
        },
    ))
}

/// Coerce a `LispVal` to a typed [`crate::jit::Value`] for a parameter of type
/// `ty`. `int64` accepts `Number`; `float64` accepts `Float` or widens `Number`;
/// `bool` follows Lisp truthiness (`nil` is false, everything else true).
fn lispval_to_typed(lv: &LispVal, ty: crate::jit::Ty) -> Result<crate::jit::Value, String> {
    use crate::jit::{Ty, Value};
    match ty {
        Ty::Int64 => match lv {
            LispVal::Number(n) => Ok(Value::Int(*n)),
            other => Err(format!("expected int64 argument, got {other:?}")),
        },
        Ty::Float64 => match lv {
            LispVal::Float(f) => Ok(Value::Float(*f)),
            LispVal::Number(n) => Ok(Value::Float(*n as f64)),
            other => Err(format!("expected float64 argument, got {other:?}")),
        },
        Ty::Bool => Ok(Value::Bool(!matches!(lv, LispVal::Nil))),
        Ty::Char => match lv {
            LispVal::Char(b) => Ok(Value::Char(*b)),
            LispVal::Number(n) => Ok(Value::Char(*n as u8)),
            other => Err(format!("expected char argument, got {other:?}")),
        },
    }
}

/// Re-box a typed [`crate::jit::Value`] result as a `LispVal` (`bool` → `T`/`NIL`).
fn typed_to_lispval(v: crate::jit::Value, env: &Rc<Environment>) -> LispVal {
    use crate::jit::Value;
    match v {
        Value::Int(n) => LispVal::Number(n),
        Value::Float(f) => LispVal::Float(f),
        Value::Bool(b) => {
            if b {
                LispVal::Symbol(env.intern_symbol("T"))
            } else {
                LispVal::Nil
            }
        }
        Value::Char(b) => LispVal::Char(b),
    }
}

/// Handle the `DEFSTRUCT` special form. Kept out-of-line (`#[inline(never)]`)
/// so its large stack frame does not bloat the recursive `eval_step` hot path
/// (see issue #76 — a fat `eval_step` frame exhausts the native stack before
/// the recursion-depth guard can fire).
///
/// `(defstruct Name field1 field2 ...)` defines, via the embedded array ops:
///   - `(make-Name v1 v2 ...)` positional constructor (array, index 0 = type tag)
///   - `(Name-p x)` type predicate
///   - `(Name-field x)` field accessor and `(set-Name-field! x v)` mutator
#[inline(never)]
fn eval_defstruct(rest: &LispVal, env: &Rc<Environment>) -> Result<TcoStep, LispError> {
    let sargs = list_to_vec(rest)?;
    if sargs.is_empty() {
        return Ok(TcoStep::Done(Err(LispError::Generic(
            "defstruct requires a name".to_string(),
        ))));
    }
    let name_sym = if let LispVal::Symbol(s) = &sargs[0] {
        s.clone()
    } else {
        return Ok(TcoStep::Done(Err(LispError::Generic(
            "defstruct: first argument must be a symbol".to_string(),
        ))));
    };
    let type_name = name_sym.borrow().name.clone();
    let fields: Vec<String> = sargs[1..]
        .iter()
        .map(|f| {
            if let LispVal::Symbol(s) = f {
                Ok(s.borrow().name.clone())
            } else {
                Err(LispError::Generic(
                    "defstruct: fields must be symbols".to_string(),
                ))
            }
        })
        .collect::<Result<Vec<_>, _>>()?;
    let n_fields = fields.len();

    // Constructor: (make-NAME v1 v2 ...) — positional.
    // Builds (lambda (f1 ...) (let ((s (array N+1))) (store s 0 'TYPE) (store s i fi) ... s))
    {
        let tn = type_name.clone();
        let params: Vec<LispVal> = fields
            .iter()
            .map(|f| LispVal::Symbol(env.intern_symbol(f)))
            .collect();
        let mut stmts: Vec<LispVal> = vec![
            crate::reader::read(&format!("(store s 0 '{tn})"), env).map_err(LispError::Generic)?,
        ];
        for (i, f) in fields.iter().enumerate() {
            stmts.push(
                crate::reader::read(&format!("(store s {} {f})", i + 1), env)
                    .map_err(LispError::Generic)?,
            );
        }
        stmts.push(LispVal::Symbol(env.intern_symbol("S")));
        let progn = LispVal::Cons {
            car: Rc::new(LispVal::Symbol(env.intern_symbol("PROGN"))),
            cdr: Rc::new(vec_to_list(stmts)),
        };
        let let_form = crate::reader::read(&format!("(array {})", n_fields + 1), env)
            .map_err(LispError::Generic)?;
        let binding = LispVal::Cons {
            car: Rc::new(LispVal::Cons {
                car: Rc::new(LispVal::Symbol(env.intern_symbol("S"))),
                cdr: Rc::new(LispVal::Cons {
                    car: Rc::new(let_form),
                    cdr: Rc::new(LispVal::Nil),
                }),
            }),
            cdr: Rc::new(LispVal::Nil),
        };
        let full_let = LispVal::Cons {
            car: Rc::new(LispVal::Symbol(env.intern_symbol("LET"))),
            cdr: Rc::new(LispVal::Cons {
                car: Rc::new(binding),
                cdr: Rc::new(LispVal::Cons {
                    car: Rc::new(progn),
                    cdr: Rc::new(LispVal::Nil),
                }),
            }),
        };
        let lambda_form = LispVal::Cons {
            car: Rc::new(LispVal::Symbol(env.intern_symbol("LAMBDA"))),
            cdr: Rc::new(LispVal::Cons {
                car: Rc::new(vec_to_list(params)),
                cdr: Rc::new(LispVal::Cons {
                    car: Rc::new(full_let),
                    cdr: Rc::new(LispVal::Nil),
                }),
            }),
        };
        let ctor = eval(&lambda_form, env)?;
        env.set(format!("MAKE-{}", tn), ctor);
    }

    // Predicate: (lambda (x) (and (arrayp x) (eq (fetch x 0) 'TypeName)))
    {
        let tn = type_name.clone();
        let form = crate::reader::read(
            &format!("(lambda (x) (and (arrayp x) (eq (fetch x 0) '{tn})))"),
            env,
        )
        .map_err(LispError::Generic)?;
        let pred = eval(&form, env)?;
        env.set(format!("{}-P", tn), pred);
    }

    // Accessors: (Name-field s) → (fetch s idx); mutators: (set-Name-field! s v) → (store s idx v)
    for (i, field) in fields.iter().enumerate() {
        let idx = i + 1;
        let acc_form = crate::reader::read(&format!("(lambda (s) (fetch s {idx}))"), env)
            .map_err(LispError::Generic)?;
        env.set(format!("{}-{}", type_name, field), eval(&acc_form, env)?);

        let mut_form = crate::reader::read(&format!("(lambda (s v) (store s {idx} v))"), env)
            .map_err(LispError::Generic)?;
        env.set(
            format!("SET-{}-{}!", type_name, field),
            eval(&mut_form, env)?,
        );
    }
    Ok(TcoStep::Done(Ok(LispVal::Symbol(name_sym))))
}

/// Handle the `FOR` special form: a fast integer-counted loop.
///
/// `(for (var start end [step]) body...)`
///
/// `var` is bound to successive integers from `start` to `end` **inclusive**,
/// advancing by `step` (default `1`). A positive step counts up while
/// `var <= end`; a negative step counts down while `var >= end`; a zero step is
/// an error. `start`, `end`, and `step` are each evaluated **once** before the
/// loop begins and must be integers.
///
/// Speed notes (this is the whole point of `FOR`): a single child environment
/// frame is allocated for the entire loop and the counter slot is mutated in
/// place each iteration — no per-iteration frame allocation, no `COND`/`GO`
/// dispatch, and no error-unwinding jumps the way a `PROG`/`GO` loop pays.
///
/// Because the one frame is reused, closures created inside the body all share
/// the same `var` slot (they observe its final value), and assigning to `var`
/// inside the body does not change the iteration sequence — the loop driver
/// overwrites the slot at the top of each pass. `FOR` always returns `NIL`.
#[inline(never)]
fn eval_for(rest: &LispVal, env: &Rc<Environment>) -> Result<TcoStep, LispError> {
    let args = list_to_vec(rest)?;
    if args.is_empty() {
        return Ok(TcoStep::Done(Err(LispError::Generic(
            "for requires a spec list (var start end [step]) and a body".to_string(),
        ))));
    }

    let spec = list_to_vec(&args[0])?;
    if spec.len() != 3 && spec.len() != 4 {
        return Ok(TcoStep::Done(Err(LispError::Generic(
            "for spec must be (var start end [step])".to_string(),
        ))));
    }

    let var_name = if let LispVal::Symbol(s) = &spec[0] {
        s.borrow().name.clone()
    } else {
        return Ok(TcoStep::Done(Err(LispError::Generic(
            "for loop variable must be a symbol".to_string(),
        ))));
    };

    // Evaluate the bounds once, up front.
    let as_int = |v: &LispVal, who: &str| -> Result<i64, LispError> {
        match v {
            LispVal::Number(n) => Ok(*n),
            other => Err(LispError::Generic(format!(
                "for {who} must be an integer, got {other:?}"
            ))),
        }
    };
    let start = as_int(&eval(&spec[1], env)?, "start")?;
    let end = as_int(&eval(&spec[2], env)?, "end")?;
    let step = if spec.len() == 4 {
        as_int(&eval(&spec[3], env)?, "step")?
    } else {
        1
    };
    if step == 0 {
        return Ok(TcoStep::Done(Err(LispError::Generic(
            "for step must be non-zero".to_string(),
        ))));
    }

    let body = &args[1..];
    let loop_env = Environment::new_child(env);

    let mut i = start;
    loop {
        // Inclusive bound; direction depends on the sign of step.
        if (step > 0 && i > end) || (step < 0 && i < end) {
            break;
        }
        loop_env.set(var_name.clone(), LispVal::Number(i));
        for form in body {
            eval(form, &loop_env)?;
        }
        // Guard against overflow so a runaway step can't panic in release-with-checks
        // or silently wrap into an infinite loop.
        match i.checked_add(step) {
            Some(n) => i = n,
            None => break,
        }
    }

    Ok(TcoStep::Done(Ok(LispVal::Nil)))
}

/// Handle the `WHILE` special form.
///
/// `(while cond body...)` — evaluate `cond`; while it is truthy, evaluate each
/// body form in order, then re-test `cond`. The body runs in the current
/// environment (like `PROGN`), so no per-iteration frame is allocated. Returns
/// `NIL` once `cond` becomes false (which is immediately if it starts false).
#[inline(never)]
fn eval_while(rest: &LispVal, env: &Rc<Environment>) -> Result<TcoStep, LispError> {
    if let LispVal::Cons {
        car: cond_expr,
        cdr: body_list,
    } = rest
    {
        loop {
            let test = eval(cond_expr, env)?;
            if !is_truthy(&test) {
                break;
            }
            let mut current = &**body_list;
            while let LispVal::Cons { car, cdr } = current {
                eval(car, env)?;
                current = cdr;
            }
        }
        Ok(TcoStep::Done(Ok(LispVal::Nil)))
    } else {
        Ok(TcoStep::Done(Err(LispError::Generic(
            "while requires a condition and a body".to_string(),
        ))))
    }
}

/// Perform one evaluation step. Returns `TcoStep::Done` for final results
/// and `TcoStep::TailCall` for tail positions (caller loops instead of recursing).
fn eval_step(val: &LispVal, env: &Rc<Environment>) -> Result<TcoStep, LispError> {
    match val {
        LispVal::Nil => Ok(TcoStep::Done(Ok(LispVal::Nil))),
        LispVal::Symbol(s) => {
            // Resolve straight from the interned symbol: global/function refs read
            // the symbol's value cell directly (no hash, no chain walk), locals
            // walk their frames. Only the cold unbound path formats the name.
            let value = env.resolve(s).ok_or_else(|| {
                LispError::Generic(format!("Unbound variable: {}", s.borrow().name))
            })?;

            // If the value is a LABEL expression, tail-call evaluate it
            // This handles recursive LABEL definitions (TCO: TailCall instead of recurse)
            if let LispVal::Cons { car, cdr: _ } = &value
                && let LispVal::Symbol(sym) = &**car
                && sym.borrow().name == "LABEL"
            {
                return Ok(TcoStep::TailCall(value, env.clone()));
            }

            Ok(TcoStep::Done(Ok(value)))
        }
        LispVal::Number(_)
        | LispVal::Char(_)
        | LispVal::Float(_)
        | LispVal::String(_)
        | LispVal::Builtin(_)
        | LispVal::Lambda(_)
        | LispVal::Fexpr(_)
        | LispVal::Macro(_)
        | LispVal::Vau(_)
        | LispVal::HashTable(_)
        | LispVal::Native(_)
        | LispVal::Environment(_)
        | LispVal::Array(_)
        | LispVal::Extension(_)
        | LispVal::Error(_) => Ok(TcoStep::Done(Ok(val.clone()))),

        LispVal::Cons {
            car: first,
            cdr: rest,
        } => {
            if let LispVal::Symbol(s) = &**first {
                match s.borrow().name.as_str() {
                    "QUOTE" => {
                        if let LispVal::Cons { car, cdr } = &**rest
                            && **cdr == LispVal::Nil
                        {
                            return Ok(TcoStep::Done(Ok(car.as_ref().clone())));
                        }
                        Ok(TcoStep::Done(Err(LispError::Generic(
                            "quote takes exactly one argument".to_string(),
                        ))))
                    }
                    "QUASIQUOTE" => {
                        if let LispVal::Cons { car, cdr } = &**rest
                            && **cdr == LispVal::Nil
                        {
                            return Ok(TcoStep::Done(quasiquote_eval(car, env)));
                        }
                        Ok(TcoStep::Done(Err(LispError::Generic(
                            "quasiquote takes exactly one argument".to_string(),
                        ))))
                    }
                    "COND" => {
                        let mut current_clause = &**rest;
                        loop {
                            match current_clause {
                                LispVal::Cons {
                                    car: clause,
                                    cdr: next_clauses,
                                } => {
                                    if let LispVal::Cons {
                                        car: predicate,
                                        cdr: expressions,
                                    } = &**clause
                                    {
                                        let predicate_result = eval(predicate, env)?;
                                        if is_truthy(&predicate_result) {
                                            if **expressions == LispVal::Nil {
                                                return Ok(TcoStep::Done(Ok(predicate_result)));
                                            } else {
                                                // Eval all but last normally; TCO for last expr
                                                let mut current_expr = &**expressions;
                                                loop {
                                                    match current_expr {
                                                        LispVal::Cons {
                                                            car: expr,
                                                            cdr: next_exprs,
                                                        } if **next_exprs != LispVal::Nil => {
                                                            eval(expr, env)?;
                                                            current_expr = next_exprs;
                                                        }
                                                        LispVal::Cons {
                                                            car: last_expr, ..
                                                        } => {
                                                            // Last expression in the clause body: TCO
                                                            return Ok(TcoStep::TailCall(
                                                                last_expr.as_ref().clone(),
                                                                env.clone(),
                                                            ));
                                                        }
                                                        _ => {
                                                            return Ok(TcoStep::Done(Ok(
                                                                LispVal::Nil,
                                                            )));
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    } else {
                                        return Ok(TcoStep::Done(Err(LispError::Generic(
                                            "cond clauses must be lists".to_string(),
                                        ))));
                                    }
                                    current_clause = next_clauses;
                                }
                                _ => return Ok(TcoStep::Done(Ok(LispVal::Nil))), // No clause was true
                            }
                        }
                    }
                    "IF" => {
                        // Destructure (cond then else) directly off the cons cells —
                        // IF runs on every conditional and loop iteration, so we skip
                        // the `list_to_vec` allocation the general path would do.
                        if let LispVal::Cons {
                            car: cond_expr,
                            cdr: rest1,
                        } = &**rest
                            && let LispVal::Cons {
                                car: then_expr,
                                cdr: rest2,
                            } = &**rest1
                            && let LispVal::Cons {
                                car: else_expr,
                                cdr: rest3,
                            } = &**rest2
                            && **rest3 == LispVal::Nil
                        {
                            // Evaluate condition normally (non-tail), then TCO into the branch.
                            let cond_result = eval(cond_expr, env)?;
                            let next_val = if is_truthy(&cond_result) {
                                then_expr.as_ref().clone()
                            } else {
                                else_expr.as_ref().clone()
                            };
                            return Ok(TcoStep::TailCall(next_val, env.clone()));
                        }
                        Ok(TcoStep::Done(Err(LispError::Generic(
                            "if takes exactly three arguments".to_string(),
                        ))))
                    }
                    "AND" => {
                        let mut last_val = LispVal::Symbol(env.intern_symbol("T"));
                        let mut current = &**rest;
                        while let LispVal::Cons { car, cdr } = current {
                            last_val = eval(car, env)?;
                            if !is_truthy(&last_val) {
                                return Ok(TcoStep::Done(Ok(LispVal::Nil)));
                            }
                            current = cdr;
                        }
                        Ok(TcoStep::Done(Ok(last_val)))
                    }
                    "OR" => {
                        let mut current = &**rest;
                        while let LispVal::Cons { car, cdr } = current {
                            let v = eval(car, env)?;
                            if is_truthy(&v) {
                                return Ok(TcoStep::Done(Ok(v)));
                            }
                            current = cdr;
                        }
                        Ok(TcoStep::Done(Ok(LispVal::Nil)))
                    }
                    "UNWIND-PROTECT" => {
                        // (unwind-protect body cleanup...) — evaluate BODY, then
                        // always evaluate the CLEANUP forms (even if BODY errors
                        // or performs a non-local exit), then propagate BODY's
                        // result or error.
                        let forms = list_to_vec(rest)?;
                        if forms.is_empty() {
                            return Ok(TcoStep::Done(Err(LispError::Generic(
                                "unwind-protect requires a body form".to_string(),
                            ))));
                        }
                        let result = eval(&forms[0], env);
                        for cleanup in &forms[1..] {
                            // Cleanup errors shadow nothing: run them for effect.
                            let _ = eval(cleanup, env);
                        }
                        Ok(TcoStep::Done(result))
                    }
                    "CATCH" => {
                        // (catch tag body...) — establish a catch point for TAG
                        // (evaluated). A (throw TAG value) with an EQUAL tag
                        // unwinds to here and yields VALUE.
                        let forms = list_to_vec(rest)?;
                        if forms.is_empty() {
                            return Ok(TcoStep::Done(Err(LispError::Generic(
                                "catch requires a tag".to_string(),
                            ))));
                        }
                        let tag = eval(&forms[0], env)?;
                        let mut last = LispVal::Nil;
                        for form in &forms[1..] {
                            match eval(form, env) {
                                Ok(v) => last = v,
                                Err(LispError::Throw {
                                    tag: thrown_tag,
                                    value,
                                }) => {
                                    if *thrown_tag == tag {
                                        return Ok(TcoStep::Done(Ok(*value)));
                                    }
                                    return Ok(TcoStep::Done(Err(LispError::Throw {
                                        tag: thrown_tag,
                                        value,
                                    })));
                                }
                                Err(other) => return Ok(TcoStep::Done(Err(other))),
                            }
                        }
                        Ok(TcoStep::Done(Ok(last)))
                    }
                    "THROW" => {
                        // (throw tag value) — non-local exit to the matching CATCH.
                        let forms = list_to_vec(rest)?;
                        if forms.len() != 2 {
                            return Ok(TcoStep::Done(Err(LispError::Generic(
                                "throw requires exactly two arguments: (throw tag value)"
                                    .to_string(),
                            ))));
                        }
                        let tag = eval(&forms[0], env)?;
                        let value = eval(&forms[1], env)?;
                        Ok(TcoStep::Done(Err(LispError::Throw {
                            tag: Box::new(tag),
                            value: Box::new(value),
                        })))
                    }
                    "BLOCK" => {
                        // (block name body...) — NAME is an unevaluated symbol.
                        // A (return-from name value) inside BODY unwinds here.
                        let forms = list_to_vec(rest)?;
                        if forms.is_empty() {
                            return Ok(TcoStep::Done(Err(LispError::Generic(
                                "block requires a name".to_string(),
                            ))));
                        }
                        let name = match &forms[0] {
                            LispVal::Symbol(s) => s.borrow().name.clone(),
                            _ => {
                                return Ok(TcoStep::Done(Err(LispError::Generic(
                                    "block name must be a symbol".to_string(),
                                ))));
                            }
                        };
                        let mut last = LispVal::Nil;
                        for form in &forms[1..] {
                            match eval(form, env) {
                                Ok(v) => last = v,
                                Err(LispError::ReturnFrom { name: rname, value }) => {
                                    if rname == name {
                                        return Ok(TcoStep::Done(Ok(*value)));
                                    }
                                    return Ok(TcoStep::Done(Err(LispError::ReturnFrom {
                                        name: rname,
                                        value,
                                    })));
                                }
                                Err(other) => return Ok(TcoStep::Done(Err(other))),
                            }
                        }
                        Ok(TcoStep::Done(Ok(last)))
                    }
                    "RETURN-FROM" => {
                        // (return-from name [value]) — NAME unevaluated.
                        let forms = list_to_vec(rest)?;
                        if forms.is_empty() {
                            return Ok(TcoStep::Done(Err(LispError::Generic(
                                "return-from requires a block name".to_string(),
                            ))));
                        }
                        let name = match &forms[0] {
                            LispVal::Symbol(s) => s.borrow().name.clone(),
                            _ => {
                                return Ok(TcoStep::Done(Err(LispError::Generic(
                                    "return-from name must be a symbol".to_string(),
                                ))));
                            }
                        };
                        let value = if forms.len() >= 2 {
                            eval(&forms[1], env)?
                        } else {
                            LispVal::Nil
                        };
                        Ok(TcoStep::Done(Err(LispError::ReturnFrom {
                            name,
                            value: Box::new(value),
                        })))
                    }
                    "HANDLER-CASE" => {
                        // (handler-case expr (error (var) handler-body...))
                        // Evaluate EXPR; on a trapped error bind VAR to the
                        // condition value (a LispVal::Error) and run the handler.
                        // Control-flow signals propagate untrapped.
                        let forms = list_to_vec(rest)?;
                        if forms.len() != 2 {
                            return Ok(TcoStep::Done(Err(LispError::Generic(
                                "handler-case takes an expression and one (error (var) ...) clause"
                                    .to_string(),
                            ))));
                        }
                        let condition = match eval(&forms[0], env) {
                            Ok(v) => return Ok(TcoStep::Done(Ok(v))),
                            Err(LispError::Signaled(c)) => *c,
                            Err(LispError::Generic(msg)) => {
                                LispVal::Error(Rc::new(crate::ErrorObj {
                                    message: msg,
                                    data: LispVal::Nil,
                                }))
                            }
                            Err(other) => return Ok(TcoStep::Done(Err(other))),
                        };
                        {
                            let clause = list_to_vec(&forms[1])?;
                            if clause.len() < 2 {
                                return Ok(TcoStep::Done(Err(LispError::Generic(
                                    "handler-case clause must be (error (var) body...)".to_string(),
                                ))));
                            }
                            let var_list = list_to_vec(&clause[1])?;
                            let handler_env = Environment::new_child(env);
                            if let Some(LispVal::Symbol(s)) = var_list.first() {
                                handler_env.set(s.borrow().name.clone(), condition);
                            }
                            let mut last = LispVal::Nil;
                            for form in &clause[2..] {
                                last = eval(form, &handler_env)?;
                            }
                            Ok(TcoStep::Done(Ok(last)))
                        }
                    }
                    "DEF" => {
                        let args = list_to_vec(rest)?;
                        if args.len() != 2 && args.len() != 3 {
                            return Ok(TcoStep::Done(Err(LispError::Generic(
                                "def takes two or three arguments".to_string(),
                            ))));
                        }
                        if let LispVal::Symbol(s) = &args[0] {
                            let name = s.borrow().name.clone();
                            let v = eval(&args[1], env)?;
                            if args.len() == 3 {
                                if let LispVal::String(doc) = &args[2] {
                                    s.borrow_mut().plist.insert(
                                        "docstring".to_string(),
                                        LispVal::String(doc.clone()),
                                    );
                                } else {
                                    return Ok(TcoStep::Done(Err(LispError::Generic(
                                        "docstring must be a string".to_string(),
                                    ))));
                                }
                            }
                            env.set(name, v);
                            Ok(TcoStep::Done(Ok(LispVal::Symbol(s.clone()))))
                        } else {
                            Ok(TcoStep::Done(Err(LispError::Generic(
                                "def requires a symbol as its first argument".to_string(),
                            ))))
                        }
                    }
                    "DEFDYNAMIC" | "DEFVAR" => {
                        let args = list_to_vec(rest)?;
                        if args.len() < 2 || args.len() > 3 {
                            return Ok(TcoStep::Done(Err(LispError::Generic(
                                "defdynamic requires 2 or 3 arguments: (defdynamic symbol value [docstring])"
                                    .to_string(),
                            ))));
                        }

                        // Get symbol
                        let symbol = if let LispVal::Symbol(s) = &args[0] {
                            s
                        } else {
                            return Ok(TcoStep::Done(Err(LispError::Generic(
                                "defdynamic first argument must be a symbol".to_string(),
                            ))));
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
                                symbol
                                    .borrow_mut()
                                    .plist
                                    .insert("docstring".to_string(), LispVal::String(doc.clone()));
                            } else {
                                return Ok(TcoStep::Done(Err(LispError::Generic(
                                    "defdynamic docstring must be a string".to_string(),
                                ))));
                            }
                        }

                        Ok(TcoStep::Done(Ok(LispVal::Symbol(symbol.clone()))))
                    }
                    "LAMBDA" => {
                        if let LispVal::Cons {
                            car: params,
                            cdr: body_list,
                        } = &**rest
                        {
                            let body_exprs = list_to_vec(body_list)?;
                            let final_body = if body_exprs.len() == 1 {
                                body_exprs[0].clone()
                            } else {
                                let progn_sym = LispVal::Symbol(env.intern_symbol("PROGN"));
                                vec_to_list([vec![progn_sym], body_exprs].concat())
                            };
                            return Ok(TcoStep::Done(make_lambda(params, &final_body, env)));
                        }
                        Ok(TcoStep::Done(Err(LispError::Generic(
                            "lambda requires params and at least one body expression".to_string(),
                        ))))
                    }
                    "FUNCTION" => {
                        let args = list_to_vec(rest)?;
                        if args.len() != 1 {
                            return Ok(TcoStep::Done(Err(LispError::Generic(
                                "FUNCTION takes exactly one argument".to_string(),
                            ))));
                        }
                        let arg = &args[0];

                        // Case 1: Argument is a literal LAMBDA expression
                        if let LispVal::Cons {
                            car: lambda_sym,
                            cdr: lambda_body,
                        } = arg
                            && let LispVal::Symbol(s) = &**lambda_sym
                            && s.borrow().name == "LAMBDA"
                            && let LispVal::Cons {
                                car: params,
                                cdr: body_list,
                            } = &**lambda_body
                        {
                            let body_exprs = list_to_vec(body_list)?;
                            let final_body = if body_exprs.len() == 1 {
                                body_exprs[0].clone()
                            } else {
                                let progn_sym = LispVal::Symbol(env.intern_symbol("PROGN"));
                                vec_to_list([vec![progn_sym], body_exprs].concat())
                            };
                            return Ok(TcoStep::Done(make_lambda(params, &final_body, env)));
                        }

                        // Case 2: Argument is a symbol bound to a function
                        if let LispVal::Symbol(s) = arg {
                            let func = env.get(&s.borrow().name).ok_or_else(|| {
                                LispError::Generic(format!(
                                    "Undefined function: {}",
                                    s.borrow().name
                                ))
                            })?;

                            match func {
                                LispVal::Lambda(_)
                                | LispVal::Builtin(_)
                                | LispVal::Fexpr(_)
                                | LispVal::Macro(_)
                                | LispVal::Native(_) => return Ok(TcoStep::Done(Ok(func))),
                                _ => {
                                    return Ok(TcoStep::Done(Err(LispError::Generic(format!(
                                        "Symbol '{}' is not bound to a function",
                                        s.borrow().name
                                    )))));
                                }
                            }
                        }

                        Ok(TcoStep::Done(Err(LispError::Generic(
                            "FUNCTION argument must be a LAMBDA expression or a symbol bound to a function"
                                .to_string(),
                        ))))
                    }
                    "LABEL" => {
                        let args = list_to_vec(rest)?;
                        if args.len() != 2 {
                            return Ok(TcoStep::Done(Err(LispError::Generic(
                                "LABEL requires a name and an expression".to_string(),
                            ))));
                        }
                        let name_val = &args[0];
                        let expr_val = &args[1];

                        if let LispVal::Symbol(name_sym) = name_val {
                            // Check for pathological case: (LABEL x x)
                            if let LispVal::Symbol(expr_sym) = expr_val
                                && name_sym.borrow().name == expr_sym.borrow().name
                            {
                                return Ok(TcoStep::Done(Err(LispError::Generic(format!(
                                    "LABEL: pathological self-reference (LABEL {} {}) would cause infinite recursion",
                                    name_sym.borrow().name,
                                    expr_sym.borrow().name
                                )))));
                            }

                            let new_env = Environment::new_child(env);
                            let label_expr = LispVal::Cons {
                                car: Rc::new(LispVal::Symbol(env.intern_symbol("LABEL"))),
                                cdr: rest.clone(),
                            };
                            new_env.set(name_sym.borrow().name.clone(), label_expr);
                            // TCO: tail call into expr_val with new_env
                            Ok(TcoStep::TailCall(expr_val.clone(), new_env))
                        } else {
                            Ok(TcoStep::Done(Err(LispError::Generic(
                                "LABEL name must be a symbol".to_string(),
                            ))))
                        }
                    }
                    "DEFINE" => {
                        let defs = list_to_vec(rest)?;
                        if defs.len() != 1 {
                            return Ok(TcoStep::Done(Err(LispError::Generic(
                                "DEFINE takes a list of definitions".to_string(),
                            ))));
                        }
                        // DEFINE may be called with or without a quote:
                        //   (define '((name val) ...))  — evaluate (strips the quote)
                        //   (define  ((name val) ...))  — raw list, use directly
                        let def_list_val = match &defs[0] {
                            LispVal::Cons { car, .. } if matches!(car.as_ref(), LispVal::Symbol(s) if s.borrow().name == "QUOTE") => {
                                eval(&defs[0], env)?
                            }
                            _ => defs[0].clone(),
                        };
                        let def_list = list_to_vec(&def_list_val)?;
                        let mut defined_names = vec![];
                        for def in def_list {
                            let def_pair = list_to_vec(&def)?;
                            if def_pair.len() != 2 {
                                return Ok(TcoStep::Done(Err(LispError::Generic(
                                    "Each definition must be a pair of name and value".to_string(),
                                ))));
                            }
                            if let LispVal::Symbol(s) = &def_pair[0] {
                                let name = s.borrow().name.clone();
                                let v = eval(&def_pair[1], env)?;
                                env.set(name.clone(), v.clone());
                                // Also set EXPR on the symbol's plist (Lisp 1.5: define = deflist[x;EXPR])
                                s.borrow_mut().plist.insert("EXPR".to_string(), v);
                                defined_names.push(LispVal::Symbol(s.clone()));
                            } else {
                                return Ok(TcoStep::Done(Err(LispError::Generic(
                                    "Definition name must be a symbol".to_string(),
                                ))));
                            }
                        }
                        Ok(TcoStep::Done(Ok(vec_to_list(defined_names))))
                    }
                    "DEFEXPR" | "DEFMACRO" => {
                        let args = list_to_vec(rest)?;
                        if args.len() < 3 || args.len() > 4 {
                            return Ok(TcoStep::Done(Err(LispError::Generic(
                                format!("{} takes three or four arguments", s.borrow().name)
                                    .to_string(),
                            ))));
                        }
                        if let LispVal::Symbol(name_sym) = &args[0] {
                            let params = &args[1];
                            let mut body_idx = 2;
                            if args.len() == 4 {
                                if let LispVal::String(doc) = &args[2] {
                                    name_sym.borrow_mut().plist.insert(
                                        "docstring".to_string(),
                                        LispVal::String(doc.clone()),
                                    );
                                    body_idx = 3;
                                } else {
                                    return Ok(TcoStep::Done(Err(LispError::Generic(
                                        "docstring must be a string".to_string(),
                                    ))));
                                }
                            }
                            let body = &args[body_idx];
                            let func = if s.borrow().name == "DEFEXPR" {
                                make_fexpr(params, body, env)?
                            } else {
                                make_macro(params, body, env)?
                            };
                            // Bind the name into a local first: env.set may write
                            // the symbol's global value cell, which needs a mutable
                            // borrow of this very symbol — so the read borrow must
                            // be released before the call.
                            let def_name = name_sym.borrow().name.clone();
                            env.set(def_name, func);
                            Ok(TcoStep::Done(Ok(LispVal::Symbol(name_sym.clone()))))
                        } else {
                            Ok(TcoStep::Done(Err(LispError::Generic(
                                format!(
                                    "{} requires a symbol as its first argument",
                                    s.borrow().name
                                )
                                .to_string(),
                            ))))
                        }
                    }
                    "DEFSTRUCT" => eval_defstruct(rest, env),
                    "DEFFUN-TYPED" => {
                        // Type-check + compile into the shared typed registry,
                        // then install a Native entry so the typed function is
                        // callable from ordinary (untyped) Lisp code through the
                        // membrane. This is how the typed subset "lands" in the
                        // running language.
                        let form = LispVal::Cons {
                            car: first.clone(),
                            cdr: rest.clone(),
                        };
                        match env.jit_define(&form) {
                            Ok(name) => {
                                env.set(name.clone(), make_typed_native(name.clone()));
                                Ok(TcoStep::Done(Ok(LispVal::Symbol(env.intern_symbol(&name)))))
                            }
                            Err(e) => Ok(TcoStep::Done(Err(LispError::Generic(e)))),
                        }
                    }
                    "DECLARE-TYPED" => {
                        // Forward-declare a typed signature (for REPL-level mutual
                        // recursion). Installs the same membrane entry; calling it
                        // before the body is defined returns a clean error.
                        let form = LispVal::Cons {
                            car: first.clone(),
                            cdr: rest.clone(),
                        };
                        match env.jit_declare(&form) {
                            Ok(name) => {
                                env.set(name.clone(), make_typed_native(name.clone()));
                                Ok(TcoStep::Done(Ok(LispVal::Symbol(env.intern_symbol(&name)))))
                            }
                            Err(e) => Ok(TcoStep::Done(Err(LispError::Generic(e)))),
                        }
                    }
                    "PROGN" => {
                        let mut current = &**rest;
                        loop {
                            match current {
                                LispVal::Cons { car, cdr } if **cdr != LispVal::Nil => {
                                    // Non-last form: evaluate normally
                                    eval(car, env)?;
                                    current = cdr;
                                }
                                LispVal::Cons { car: last_expr, .. } => {
                                    // Last form: TCO
                                    return Ok(TcoStep::TailCall(
                                        last_expr.as_ref().clone(),
                                        env.clone(),
                                    ));
                                }
                                _ => return Ok(TcoStep::Done(Ok(LispVal::Nil))),
                            }
                        }
                    }
                    "SETQ" => {
                        // SETQ: Set a variable's value
                        // (SETQ var1 val1 var2 val2 ...)
                        // NOTE: If a variable doesn't exist, SETQ will CREATE it in the
                        // current environment. This is intentional behavior that allows
                        // dynamic variable creation. The newly created variable is NOT
                        // "undefined" - it takes on the value provided to SETQ.
                        // This behavior differs from some Lisp dialects that require
                        // variables to be declared before assignment.
                        // Walk the (var val var val ...) chain in place — no
                        // intermediate Vec — since SETQ runs in every loop body.
                        let mut last_val = LispVal::Nil;
                        let mut cur = &**rest;
                        loop {
                            let LispVal::Cons { car: var, cdr } = cur else {
                                if *cur == LispVal::Nil {
                                    break;
                                }
                                return Ok(TcoStep::Done(Err(LispError::Generic(
                                    "SETQ requires an even number of arguments".to_string(),
                                ))));
                            };
                            let LispVal::Cons {
                                car: val_expr,
                                cdr: tail,
                            } = &**cdr
                            else {
                                return Ok(TcoStep::Done(Err(LispError::Generic(
                                    "SETQ requires an even number of arguments".to_string(),
                                ))));
                            };
                            if let LispVal::Symbol(s) = &**var {
                                let v = eval(val_expr, env)?;
                                // Release the read borrow before update: a global
                                // SETQ writes the symbol's value cell (borrow_mut).
                                let var_name = s.borrow().name.clone();
                                Environment::update(env, &var_name, v.clone());
                                last_val = v;
                            } else {
                                return Ok(TcoStep::Done(Err(LispError::Generic(
                                    "SETQ variable name must be a symbol".to_string(),
                                ))));
                            }
                            cur = &**tail;
                        }
                        Ok(TcoStep::Done(Ok(last_val)))
                    }
                    "PROG" => {
                        let args = list_to_vec(rest)?;
                        if args.is_empty() {
                            return Ok(TcoStep::Done(Err(LispError::Generic(
                                "PROG requires at least a var list".to_string(),
                            ))));
                        }

                        let var_list = list_to_vec(&args[0])?;
                        let body = &args[1..];

                        let prog_env = Environment::new_child(env);

                        for var in var_list {
                            if let LispVal::Symbol(s) = var {
                                prog_env.set(s.borrow().name.clone(), LispVal::Nil);
                            } else {
                                return Ok(TcoStep::Done(Err(LispError::Generic(
                                    "PROG variable list must contain only symbols".to_string(),
                                ))));
                            }
                        }

                        // Collect labels and warn on duplicates
                        // NOTE: Duplicate labels are allowed but the later label wins.
                        // This may be surprising behavior, so we warn about it.
                        let mut labels = HashMap::new();
                        for (i, item) in body.iter().enumerate() {
                            if let LispVal::Symbol(s) = item {
                                let label_name = s.borrow().name.clone();
                                if let Some(old_idx) = labels.insert(label_name.clone(), i) {
                                    eprintln!(
                                        "Warning: PROG has duplicate label '{}' at positions {} and {}. Later label (position {}) will be used.",
                                        label_name, old_idx, i, i
                                    );
                                }
                            }
                        }

                        let mut pc = 0;
                        let result = loop {
                            if pc >= body.len() {
                                break Ok(LispVal::Nil); // Fell off the end
                            }

                            let item = &body[pc];

                            // If it's a label, just skip it.
                            if let LispVal::Symbol(_) = item {
                                pc += 1;
                                continue;
                            }

                            match eval(item, &prog_env) {
                                Ok(_) => {
                                    pc += 1;
                                }
                                Err(LispError::Return(val)) => {
                                    break Ok(*val);
                                }
                                Err(LispError::Go(label)) => {
                                    if let Some(new_pc) = labels.get(&label) {
                                        pc = *new_pc;
                                    } else {
                                        break Err(LispError::Generic(format!(
                                            "GO: label not found in PROG: {label}"
                                        )));
                                    }
                                }
                                Err(e) => {
                                    break Err(e);
                                }
                            }
                        };
                        Ok(TcoStep::Done(result))
                    }
                    "RETURN" => {
                        let args = list_to_vec(rest)?;
                        if args.len() != 1 {
                            return Ok(TcoStep::Done(Err(LispError::Generic(
                                "RETURN takes exactly one argument".to_string(),
                            ))));
                        }
                        let retval = eval(&args[0], env)?;
                        Ok(TcoStep::Done(Err(LispError::Return(Box::new(retval)))))
                    }
                    "GO" => {
                        let args = list_to_vec(rest)?;
                        if args.len() != 1 {
                            return Ok(TcoStep::Done(Err(LispError::Generic(
                                "GO takes exactly one argument".to_string(),
                            ))));
                        }
                        if let LispVal::Symbol(s) = &args[0] {
                            Ok(TcoStep::Done(Err(LispError::Go(s.borrow().name.clone()))))
                        } else {
                            Ok(TcoStep::Done(Err(LispError::Generic(
                                "GO argument must be a symbol".to_string(),
                            ))))
                        }
                    }
                    "FOR" => eval_for(rest, env),
                    "WHILE" => eval_while(rest, env),
                    "LET" => {
                        let args = list_to_vec(rest)?;
                        if args.len() < 2 {
                            return Ok(TcoStep::Done(Err(LispError::Generic(
                                "let requires a binding list and at least one body form"
                                    .to_string(),
                            ))));
                        }
                        let bindings_vec = list_to_vec(&args[0])?;
                        let body = wrap_body_forms(&args[1..], env);

                        let mut params = vec![];
                        let mut arg_exprs = vec![];
                        for binding in bindings_vec {
                            let pair = list_to_vec(&binding)?;
                            if pair.len() != 2 {
                                return Ok(TcoStep::Done(Err(LispError::Generic(
                                    "let binding must be a pair".to_string(),
                                ))));
                            }
                            params.push(pair[0].clone());
                            arg_exprs.push(pair[1].clone());
                        }

                        // TCO: Instead of calling eval on an application form, inline the
                        // binding setup and continue the loop with the body expression.
                        let let_env = Environment::new_child(env);
                        for (param, arg_expr) in params.iter().zip(arg_exprs.iter()) {
                            if let LispVal::Symbol(s) = param {
                                let v = eval(arg_expr, env)?;
                                let_env.set(s.borrow().name.clone(), v);
                            } else {
                                return Ok(TcoStep::Done(Err(LispError::Generic(
                                    "let binding name must be a symbol".to_string(),
                                ))));
                            }
                        }
                        Ok(TcoStep::TailCall(body, let_env))
                    }
                    // let* binds sequentially in a SINGLE frame: each binding is
                    // evaluated in that frame and then written into it, so later
                    // bindings see earlier ones without allocating a frame per
                    // binding (the difference from desugaring to nested LETs).
                    "LET*" => {
                        let args = list_to_vec(rest)?;
                        if args.len() < 2 {
                            return Ok(TcoStep::Done(Err(LispError::Generic(
                                "let* requires a binding list and at least one body form"
                                    .to_string(),
                            ))));
                        }
                        let bindings_vec = list_to_vec(&args[0])?;
                        let body = wrap_body_forms(&args[1..], env);

                        let let_env = Environment::new_child(env);
                        for binding in bindings_vec {
                            let pair = list_to_vec(&binding)?;
                            if pair.len() != 2 {
                                return Ok(TcoStep::Done(Err(LispError::Generic(
                                    "let* binding must be a pair".to_string(),
                                ))));
                            }
                            if let LispVal::Symbol(s) = &pair[0] {
                                let v = eval(&pair[1], &let_env)?;
                                let_env.set(s.borrow().name.clone(), v);
                            } else {
                                return Ok(TcoStep::Done(Err(LispError::Generic(
                                    "let* binding name must be a symbol".to_string(),
                                ))));
                            }
                        }
                        Ok(TcoStep::TailCall(body, let_env))
                    }
                    "VAU" | "$VAU" => {
                        // (vau (operands-param env-param) body...)
                        // operands-param receives the unevaluated operand list;
                        // env-param receives the caller's environment.
                        let args = list_to_vec(rest)?;
                        if args.is_empty() {
                            return Ok(TcoStep::Done(Err(LispError::Generic(
                                "vau requires at least a parameter list".to_string(),
                            ))));
                        }
                        let param_list = list_to_vec(&args[0])?;
                        if param_list.len() != 2 {
                            return Ok(TcoStep::Done(Err(LispError::Generic(
                                "vau parameter list must have exactly two symbols: (operands-param env-param)".to_string(),
                            ))));
                        }
                        let op_param = if let LispVal::Symbol(s) = &param_list[0] {
                            s.borrow().name.clone()
                        } else {
                            return Ok(TcoStep::Done(Err(LispError::Generic(
                                "vau operands parameter must be a symbol".to_string(),
                            ))));
                        };
                        let env_param = if let LispVal::Symbol(s) = &param_list[1] {
                            s.borrow().name.clone()
                        } else {
                            return Ok(TcoStep::Done(Err(LispError::Generic(
                                "vau environment parameter must be a symbol".to_string(),
                            ))));
                        };
                        let body = if args.len() == 2 {
                            args[1].clone()
                        } else {
                            // Wrap multiple body forms in PROGN
                            let progn_sym = LispVal::Symbol(env.intern_symbol("PROGN"));
                            let mut progn = LispVal::Nil;
                            for form in args[1..].iter().rev() {
                                progn = LispVal::Cons {
                                    car: Rc::new(form.clone()),
                                    cdr: Rc::new(progn),
                                };
                            }
                            LispVal::Cons {
                                car: Rc::new(progn_sym),
                                cdr: Rc::new(progn),
                            }
                        };
                        Ok(TcoStep::Done(Ok(LispVal::Vau(Box::new(crate::Vau {
                            operands_param: op_param,
                            env_param,
                            body: Box::new(body),
                            env: env.clone(),
                        })))))
                    }
                    _ => {
                        // Function call: evaluate the function head
                        let func = eval(first, env)?;

                        // Macro expansion: TCO — continue with the expanded form.
                        // Macros receive UNEVALUATED operands, so build that list
                        // only on this (comparatively cold) path.
                        if let LispVal::Macro(m) = &func {
                            let args_list = list_to_vec(rest)?;
                            let expanded = expand_macro(m, &args_list, env)?;
                            return Ok(TcoStep::TailCall(expanded, env.clone()));
                        }

                        // Vau application: bind operands (unevaluated) and caller env, TCO into body
                        if let LispVal::Vau(vau) = &func {
                            let new_env = Environment::new_child(&vau.env);
                            new_env.set(vau.operands_param.clone(), rest.as_ref().clone());
                            new_env.set(vau.env_param.clone(), LispVal::Environment(env.clone()));
                            return Ok(TcoStep::TailCall(*vau.body.clone(), new_env));
                        }

                        // Fexpr application: TCO — continue with fexpr body
                        if let LispVal::Fexpr(fexpr) = &func {
                            let new_env = Environment::new_child_with_dynamic(&fexpr.env, env);
                            if fexpr.params.len() == 1 {
                                // Single-param: bind entire unevaluated arg list to the one parameter.
                                new_env.set(fexpr.params[0].clone(), rest.as_ref().clone());
                            } else {
                                // Multi-param: bind each unevaluated arg to its parameter.
                                let unevaluated_args = list_to_vec(rest)?;
                                if unevaluated_args.len() != fexpr.params.len() {
                                    return Ok(TcoStep::Done(Err(LispError::Generic(format!(
                                        "fexpr expected {} arguments, got {}",
                                        fexpr.params.len(),
                                        unevaluated_args.len()
                                    )))));
                                }
                                for (param, arg) in fexpr.params.iter().zip(unevaluated_args) {
                                    new_env.set(param.clone(), arg);
                                }
                            }
                            return Ok(TcoStep::TailCall(*fexpr.body.clone(), new_env));
                        }

                        // Evaluated-argument callables (lambda/builtin/native): evaluate
                        // operands straight off the cons chain — one allocation, no
                        // intermediate list of cloned argument expressions.
                        let eval_args = eval_operands(rest, env)?;

                        // Lambda application: TCO — set up new env and continue with body
                        if let LispVal::Lambda(lambda) = &func {
                            let new_env = Environment::new_child_with_dynamic(&lambda.env, env);
                            if let Some(rest_param_name) = &lambda.rest_param {
                                if eval_args.len() < lambda.params.len() {
                                    return Ok(TcoStep::Done(Err(LispError::Generic(format!(
                                        "lambda expected at least {} arguments, got {}",
                                        lambda.params.len(),
                                        eval_args.len()
                                    )))));
                                }
                                for (param, arg) in lambda.params.iter().zip(eval_args.iter()) {
                                    new_env.set(param.clone(), arg.clone());
                                }
                                let rest_args =
                                    vec_to_list(eval_args[lambda.params.len()..].to_vec());
                                new_env.set(rest_param_name.clone(), rest_args);
                            } else {
                                if lambda.params.len() != eval_args.len() {
                                    return Ok(TcoStep::Done(Err(LispError::Generic(format!(
                                        "lambda expected {} arguments, got {}",
                                        lambda.params.len(),
                                        eval_args.len()
                                    )))));
                                }
                                for (param, arg) in lambda.params.iter().zip(eval_args.iter()) {
                                    new_env.set(param.clone(), arg.clone());
                                }
                            }
                            return Ok(TcoStep::TailCall(*lambda.body.clone(), new_env));
                        }

                        // All other callables (builtins, natives): no TCO needed.
                        // Defer the apply to the driver loop so the head-symbol
                        // borrow held by the dispatch match above is released
                        // first (issue #156).
                        Ok(TcoStep::Apply(func, eval_args))
                    }
                }
            } else {
                // Non-symbol head: evaluate the head expression, then apply
                let func = eval(first, env)?;

                // Vau: must intercept BEFORE evaluating args
                if let LispVal::Vau(vau) = &func {
                    let new_env = Environment::new_child(&vau.env);
                    new_env.set(vau.operands_param.clone(), rest.as_ref().clone());
                    new_env.set(vau.env_param.clone(), LispVal::Environment(env.clone()));
                    return Ok(TcoStep::TailCall(*vau.body.clone(), new_env));
                }

                // Fexpr: must intercept BEFORE evaluating args
                if let LispVal::Fexpr(fexpr) = &func {
                    let new_env = Environment::new_child_with_dynamic(&fexpr.env, env);
                    if fexpr.params.len() == 1 {
                        new_env.set(fexpr.params[0].clone(), rest.as_ref().clone());
                    } else {
                        let unevaluated_args = list_to_vec(rest)?;
                        if unevaluated_args.len() != fexpr.params.len() {
                            return Ok(TcoStep::Done(Err(LispError::Generic(format!(
                                "fexpr expected {} arguments, got {}",
                                fexpr.params.len(),
                                unevaluated_args.len()
                            )))));
                        }
                        for (param, arg) in fexpr.params.iter().zip(unevaluated_args) {
                            new_env.set(param.clone(), arg);
                        }
                    }
                    return Ok(TcoStep::TailCall(*fexpr.body.clone(), new_env));
                }

                // Evaluate operands straight off the cons chain (single allocation).
                let eval_args = eval_operands(rest, env)?;

                // Lambda application: TCO
                if let LispVal::Lambda(lambda) = &func {
                    let new_env = Environment::new_child_with_dynamic(&lambda.env, env);
                    if let Some(rest_param_name) = &lambda.rest_param {
                        if eval_args.len() < lambda.params.len() {
                            return Ok(TcoStep::Done(Err(LispError::Generic(format!(
                                "lambda expected at least {} arguments, got {}",
                                lambda.params.len(),
                                eval_args.len()
                            )))));
                        }
                        for (param, arg) in lambda.params.iter().zip(eval_args.iter()) {
                            new_env.set(param.clone(), arg.clone());
                        }
                        let rest_args = vec_to_list(eval_args[lambda.params.len()..].to_vec());
                        new_env.set(rest_param_name.clone(), rest_args);
                    } else {
                        if lambda.params.len() != eval_args.len() {
                            return Ok(TcoStep::Done(Err(LispError::Generic(format!(
                                "lambda expected {} arguments, got {}",
                                lambda.params.len(),
                                eval_args.len()
                            )))));
                        }
                        for (param, arg) in lambda.params.iter().zip(eval_args.iter()) {
                            new_env.set(param.clone(), arg.clone());
                        }
                    }
                    return Ok(TcoStep::TailCall(*lambda.body.clone(), new_env));
                }

                Ok(TcoStep::Done(apply(&func, &eval_args, env)))
            }
        }
    }
}

#[inline(never)]
fn quasiquote_eval(val: &LispVal, env: &Rc<Environment>) -> Result<LispVal, LispError> {
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
        // A splicing form `,@e` appearing as a list element: evaluate `e` to a
        // list and graft its elements into the surrounding list, ahead of the
        // result of processing the remaining elements (the cdr).
        if let Some(spliced) = unquote_splicing_arg(car) {
            let spliced_list = eval(spliced, env)?;
            let cdr_eval = quasiquote_eval(cdr, env)?;
            return append_lists(&spliced_list, cdr_eval);
        }
        let car_eval = quasiquote_eval(car, env)?;
        let cdr_eval = quasiquote_eval(cdr, env)?;
        Ok(LispVal::Cons {
            car: Rc::new(car_eval),
            cdr: Rc::new(cdr_eval),
        })
    } else {
        Ok(val.clone())
    }
}

/// If `val` is a well-formed `(UNQUOTE-SPLICING e)` form, return a reference to
/// `e`; otherwise return `None` (including ill-arity forms, which then fall
/// through to ordinary template processing).
fn unquote_splicing_arg(val: &LispVal) -> Option<&LispVal> {
    if let LispVal::Cons { car, cdr } = val
        && let LispVal::Symbol(s) = &**car
        && s.borrow().name == "UNQUOTE-SPLICING"
        && let LispVal::Cons {
            car: arg,
            cdr: rest,
        } = &**cdr
        && **rest == LispVal::Nil
    {
        return Some(arg);
    }
    None
}

/// Build a fresh cons chain holding every element of the proper list `front`
/// followed by `tail`. The cons cells of `front` are copied so the original is
/// left untouched; a non-list `front` (or an improper tail) yields an error.
fn append_lists(front: &LispVal, tail: LispVal) -> Result<LispVal, LispError> {
    match front {
        LispVal::Nil => Ok(tail),
        LispVal::Cons { car, cdr } => {
            let rest = append_lists(cdr, tail)?;
            Ok(LispVal::Cons {
                car: car.clone(),
                cdr: Rc::new(rest),
            })
        }
        _ => Err(LispError::Generic(
            "unquote-splicing requires a list argument".to_string(),
        )),
    }
}

#[inline(never)]
fn apply_symbol_op(
    op: &BuiltinFunc,
    args: &[LispVal],
    env: &Rc<Environment>,
) -> Result<LispVal, LispError> {
    match op {
        BuiltinFunc::GetP => {
            if args.len() != 2 {
                return Err(LispError::Generic(
                    "get-p takes exactly two arguments".to_string(),
                ));
            }
            let prop = match &args[1] {
                LispVal::String(s) => s.clone(),
                LispVal::Symbol(s) => s.borrow().name.clone(),
                _ => {
                    return Err(LispError::Generic(
                        "get-p requires a symbol or string as its second argument".to_string(),
                    ));
                }
            };
            if let LispVal::Symbol(s) = &args[0] {
                if let Some(val) = s.borrow().plist.get(&prop) {
                    Ok(val.clone())
                } else {
                    Ok(LispVal::Nil)
                }
            } else {
                Err(LispError::Generic(
                    "get-p requires a symbol as its first argument".to_string(),
                ))
            }
        }
        BuiltinFunc::PutP => {
            if args.len() != 3 {
                return Err(LispError::Generic(
                    "put-p takes exactly three arguments".to_string(),
                ));
            }
            let prop = match &args[1] {
                LispVal::String(s) => s.clone(),
                LispVal::Symbol(s) => s.borrow().name.clone(),
                _ => {
                    return Err(LispError::Generic(
                        "put-p requires a symbol or string as its second argument".to_string(),
                    ));
                }
            };
            if let LispVal::Symbol(s) = &args[0] {
                let val = args[2].clone();
                s.borrow_mut().plist.insert(prop, val);
                Ok(LispVal::Symbol(env.intern_symbol("T")))
            } else {
                Err(LispError::Generic(
                    "put-p requires a symbol as its first argument".to_string(),
                ))
            }
        }
        _ => Err(LispError::Generic("Not a symbol operation".to_string())),
    }
}

// I/O operations
#[inline(never)]
fn apply_io_op(
    op: &BuiltinFunc,
    args: &[LispVal],
    env: &Rc<Environment>,
) -> Result<LispVal, LispError> {
    match op {
        BuiltinFunc::Read => {
            if !env.feature_enabled("IO") {
                return Err(LispError::Generic(
                    "IO capability is not enabled (grant it via --capability IO or the host API)"
                        .to_string(),
                ));
            }
            if !args.is_empty() {
                return Err(LispError::Generic("read takes no arguments".to_string()));
            }
            use std::io::{self, BufRead};
            let stdin = io::stdin();
            let mut line = String::new();
            stdin
                .lock()
                .read_line(&mut line)
                .map_err(|e| LispError::Generic(format!("Failed to read input: {}", e)))?;
            crate::reader::read(&line, env)
                .map_err(|e| LispError::Generic(format!("Failed to parse input: {}", e)))
        }
        BuiltinFunc::Prin1 => {
            if args.len() != 1 {
                return Err(LispError::Generic(
                    "prin1 requires exactly one argument".to_string(),
                ));
            }
            print!("{}", crate::printer::print(&args[0]));
            use std::io::{self, Write};
            io::stdout()
                .flush()
                .map_err(|e| LispError::Generic(format!("Failed to flush output: {}", e)))?;
            Ok(args[0].clone())
        }
        BuiltinFunc::Princ => {
            if args.len() != 1 {
                return Err(LispError::Generic(
                    "princ requires exactly one argument".to_string(),
                ));
            }
            let output = match &args[0] {
                LispVal::String(s) => s.clone(),
                other => crate::printer::print(other),
            };
            print!("{}", output);
            use std::io::{self, Write};
            io::stdout()
                .flush()
                .map_err(|e| LispError::Generic(format!("Failed to flush output: {}", e)))?;
            Ok(args[0].clone())
        }
        BuiltinFunc::Terpri => {
            if !args.is_empty() {
                return Err(LispError::Generic("terpri takes no arguments".to_string()));
            }
            println!();
            Ok(LispVal::Nil)
        }
        BuiltinFunc::Spaces => {
            if args.len() != 1 {
                return Err(LispError::Generic(
                    "spaces takes exactly one argument".to_string(),
                ));
            }
            if let LispVal::Number(n) = &args[0] {
                let n = (*n).max(0) as usize;
                print!("{}", " ".repeat(n));
                use std::io::Write;
                let _ = std::io::stdout().flush();
                Ok(LispVal::Nil)
            } else {
                Err(LispError::Generic(
                    "spaces requires a number argument".to_string(),
                ))
            }
        }
        _ => Err(LispError::Generic("Not an I/O operation".to_string())),
    }
}

// Error handling operations
#[inline(never)]
fn apply_error_op(
    op: &BuiltinFunc,
    args: &[LispVal],
    env: &Rc<Environment>,
) -> Result<LispVal, LispError> {
    match op {
        BuiltinFunc::Error => {
            // (error)                       -> signal a generic error
            // (error existing-error-value)  -> re-signal it unchanged
            // (error message irritants...)  -> signal (make-error message irritants)
            if args.is_empty() {
                return Err(LispError::Signaled(Box::new(LispVal::Error(Rc::new(
                    crate::ErrorObj {
                        message: "Error".to_string(),
                        data: LispVal::Nil,
                    },
                )))));
            }
            if let LispVal::Error(_) = &args[0] {
                return Err(LispError::Signaled(Box::new(args[0].clone())));
            }
            let message = match &args[0] {
                LispVal::String(s) => s.clone(),
                other => crate::printer::print(other),
            };
            // (error message [data]) — mirrors make-error: an optional single
            // data payload (a cons or any value), defaulting to NIL.
            let data = args.get(1).cloned().unwrap_or(LispVal::Nil);
            Err(LispError::Signaled(Box::new(LispVal::Error(Rc::new(
                crate::ErrorObj { message, data },
            )))))
        }
        BuiltinFunc::Errorset => {
            if args.len() != 1 && args.len() != 2 {
                return Err(LispError::Generic(
                    "errorset requires one or two arguments".to_string(),
                ));
            }
            let form = &args[0];
            match eval(form, env) {
                Ok(result) => Ok(vec_to_list(vec![result])),
                // Trap ordinary errors and signalled conditions only; let
                // non-local control flow (RETURN/GO/THROW/RETURN-FROM) pass
                // through unchanged.
                Err(LispError::Generic(_)) | Err(LispError::Signaled(_)) => Ok(LispVal::Nil),
                Err(other) => Err(other),
            }
        }
        _ => Err(LispError::Generic(
            "Not an error handling operation".to_string(),
        )),
    }
}

// List processing operations
#[inline(never)]
fn apply_list_processing(
    op: &BuiltinFunc,
    args: &[LispVal],
    env: &Rc<Environment>,
) -> Result<LispVal, LispError> {
    match op {
        BuiltinFunc::Subst => {
            if args.len() != 3 {
                return Err(LispError::Generic(
                    "subst requires exactly three arguments".to_string(),
                ));
            }
            let new_val = &args[0];
            let old_val = &args[1];
            let tree = &args[2];
            fn subst_helper(new: &LispVal, old: &LispVal, tree: &LispVal) -> LispVal {
                if tree == old {
                    new.clone()
                } else if let LispVal::Cons { car, cdr } = tree {
                    LispVal::Cons {
                        car: Rc::new(subst_helper(new, old, car)),
                        cdr: Rc::new(subst_helper(new, old, cdr)),
                    }
                } else {
                    tree.clone()
                }
            }
            Ok(subst_helper(new_val, old_val, tree))
        }
        BuiltinFunc::Sublis => {
            // SUBLIS: Perform multiple substitutions using an association list
            // (SUBLIS alist tree)
            // Returns tree with all atoms that appear as keys in alist replaced with their values
            if args.len() != 2 {
                return Err(LispError::Generic(
                    "sublis requires exactly two arguments".to_string(),
                ));
            }
            let alist = &args[0];
            let tree = &args[1];

            // Helper function to look up a value in the alist
            fn lookup_in_alist(key: &LispVal, alist: &LispVal) -> Option<LispVal> {
                let mut current = alist;
                while let LispVal::Cons { car, cdr } = current {
                    if let LispVal::Cons {
                        car: pair_key,
                        cdr: pair_val,
                    } = &**car
                        && **pair_key == *key
                    {
                        return Some(pair_val.as_ref().clone());
                    }
                    current = cdr;
                }
                None
            }

            // Recursive substitution helper
            fn sublis_helper(alist: &LispVal, tree: &LispVal) -> LispVal {
                match tree {
                    LispVal::Cons { car, cdr } => {
                        // Recursively process both car and cdr
                        LispVal::Cons {
                            car: Rc::new(sublis_helper(alist, car)),
                            cdr: Rc::new(sublis_helper(alist, cdr)),
                        }
                    }
                    _ => {
                        // For atoms, try to find replacement in alist
                        lookup_in_alist(tree, alist).unwrap_or_else(|| tree.clone())
                    }
                }
            }

            Ok(sublis_helper(alist, tree))
        }
        BuiltinFunc::Assoc => {
            // ASSOC: Search an association list for a key
            // (ASSOC key alist)
            // Returns the first pair (key . value) where the car equals key.
            // NOTE: Malformed alist elements (non-cons) are skipped with a warning.
            // This is intentional to allow graceful degradation with imperfect data.
            if args.len() != 2 {
                return Err(LispError::Generic(
                    "assoc requires exactly two arguments".to_string(),
                ));
            }
            let key = &args[0];
            let mut alist = &args[1];
            while let LispVal::Cons { car, cdr } = alist {
                if let LispVal::Cons {
                    car: pair_car,
                    cdr: _,
                } = &**car
                {
                    if **pair_car == *key {
                        return Ok(car.as_ref().clone());
                    }
                } else {
                    // Warn about malformed alist element
                    eprintln!("Warning: ASSOC skipping non-cons alist element: {:?}", car);
                }
                alist = cdr;
            }
            Ok(LispVal::Nil)
        }
        BuiltinFunc::Maplist => {
            // Arg order: (maplist fn list) — function first, matching Common Lisp
            // and the rest of the functional toolkit (differs from the Lisp 1.5
            // manual's maplist[x;fn]; alignment is intentional).
            if args.len() != 2 {
                return Err(LispError::Generic(
                    "maplist requires exactly two arguments".to_string(),
                ));
            }
            let func = &args[0];
            let list = &args[1];
            let mut result = Vec::new();
            let mut current = list.clone();
            while let LispVal::Cons { car: _, cdr } = &current {
                let applied = apply(func, &[current.clone()], env)?;
                result.push(applied);
                current = cdr.as_ref().clone();
            }
            Ok(vec_to_list(result))
        }
        BuiltinFunc::Mapcar => {
            // Arg order: (mapcar fn list) — function first, matching Common Lisp
            // (and the rest of the functional toolkit). Note this differs from
            // the Lisp 1.5 manual's mapcar[x;fn]; the alignment is intentional.
            if args.len() != 2 {
                return Err(LispError::Generic(
                    "mapcar requires exactly two arguments".to_string(),
                ));
            }
            let func = &args[0];
            let list = &args[1];
            let mut result = Vec::new();
            let mut current = list;
            while let LispVal::Cons { car, cdr } = current {
                let applied = apply(func, &[car.as_ref().clone()], env)?;
                result.push(applied);
                current = cdr;
            }
            Ok(vec_to_list(result))
        }
        BuiltinFunc::Rplaca => {
            // RPLACA: Replace the CAR of a cons cell
            // (RPLACA cons new-car)
            // IMPORTANT: This implementation returns a NEW cons cell rather than
            // modifying the original. This is a FUNCTIONAL approach that prevents
            // circular list creation, avoiding potential infinite loops in list
            // traversal operations. Circular lists are therefore NOT possible in
            // this implementation, which is an intentional safety feature.
            if args.len() != 2 {
                return Err(LispError::Generic(
                    "rplaca requires exactly two arguments".to_string(),
                ));
            }
            if let LispVal::Cons { car: _, cdr } = &args[0] {
                Ok(LispVal::Cons {
                    car: Rc::new(args[1].clone()),
                    cdr: cdr.clone(),
                })
            } else {
                Err(LispError::Generic(
                    "rplaca requires a cons cell as its first argument".to_string(),
                ))
            }
        }
        BuiltinFunc::Rplacd => {
            // RPLACD: Replace the CDR of a cons cell
            // (RPLACD cons new-cdr)
            // IMPORTANT: This implementation returns a NEW cons cell rather than
            // modifying the original. This is a FUNCTIONAL approach that prevents
            // circular list creation, avoiding potential infinite loops in list
            // traversal operations. Circular lists are therefore NOT possible in
            // this implementation, which is an intentional safety feature.
            if args.len() != 2 {
                return Err(LispError::Generic(
                    "rplacd requires exactly two arguments".to_string(),
                ));
            }
            if let LispVal::Cons { car, cdr: _ } = &args[0] {
                Ok(LispVal::Cons {
                    car: car.clone(),
                    cdr: Rc::new(args[1].clone()),
                })
            } else {
                Err(LispError::Generic(
                    "rplacd requires a cons cell as its first argument".to_string(),
                ))
            }
        }
        _ => Err(LispError::Generic(
            "Not a list processing operation".to_string(),
        )),
    }
}

// Bitwise operations
#[inline(never)]
fn apply_bitwise_op(
    op: &BuiltinFunc,
    args: &[LispVal],
    env: &Rc<Environment>,
) -> Result<LispVal, LispError> {
    match op {
        BuiltinFunc::Logor => {
            let mut result = 0i64;
            for arg in args {
                if let LispVal::Number(n) = arg {
                    result |= n;
                } else {
                    return Err(LispError::Generic(
                        "logor requires integer arguments".to_string(),
                    ));
                }
            }
            Ok(LispVal::Number(result))
        }
        BuiltinFunc::Logand => {
            if args.is_empty() {
                return Ok(LispVal::Number(-1));
            }
            let mut result = if let LispVal::Number(n) = &args[0] {
                *n
            } else {
                return Err(LispError::Generic(
                    "logand requires integer arguments".to_string(),
                ));
            };
            for arg in &args[1..] {
                if let LispVal::Number(n) = arg {
                    result &= n;
                } else {
                    return Err(LispError::Generic(
                        "logand requires integer arguments".to_string(),
                    ));
                }
            }
            Ok(LispVal::Number(result))
        }
        BuiltinFunc::Logxor => {
            let mut result = 0i64;
            for arg in args {
                if let LispVal::Number(n) = arg {
                    result ^= n;
                } else {
                    return Err(LispError::Generic(
                        "logxor requires integer arguments".to_string(),
                    ));
                }
            }
            Ok(LispVal::Number(result))
        }
        BuiltinFunc::Leftshift => {
            if args.len() != 2 {
                return Err(LispError::Generic(
                    "leftshift requires exactly two arguments".to_string(),
                ));
            }
            if let (LispVal::Number(n), LispVal::Number(shift)) = (&args[0], &args[1]) {
                // Validate shift amount to avoid overflow panic
                if *shift >= 64 || *shift <= -64 {
                    env.set_flag("OVERFLOW");
                    // Return 0 or -1 depending on sign for extreme shifts
                    if *shift >= 64 {
                        Ok(LispVal::Number(0))
                    } else {
                        // Right shift by >= 64 is effectively sign extension
                        Ok(LispVal::Number(if *n < 0 { -1 } else { 0 }))
                    }
                } else if *shift < 0 {
                    Ok(LispVal::Number(n >> (-shift)))
                } else {
                    // shift is in [0, 63]; wrapping_shl never panics here
                    Ok(LispVal::Number(n.wrapping_shl(*shift as u32)))
                }
            } else {
                Err(LispError::Generic(
                    "leftshift requires integer arguments".to_string(),
                ))
            }
        }
        _ => Err(LispError::Generic("Not a bitwise operation".to_string())),
    }
}

// New list operations
#[inline(never)]
fn apply_new_list_ops(
    op: &BuiltinFunc,
    args: &[LispVal],
    _env: &Rc<Environment>,
) -> Result<LispVal, LispError> {
    match op {
        BuiltinFunc::List => Ok(vec_to_list(args.to_vec())),
        BuiltinFunc::Last => {
            if args.len() != 1 {
                return Err(LispError::Generic(
                    "last requires exactly one argument".to_string(),
                ));
            }
            let mut current = &args[0];
            while let LispVal::Cons { car: _, cdr } = current {
                if **cdr == LispVal::Nil {
                    return Ok(current.clone());
                }
                current = cdr;
            }
            Ok(LispVal::Nil)
        }
        BuiltinFunc::Nth => {
            if args.len() != 2 {
                return Err(LispError::Generic(
                    "nth requires exactly two arguments".to_string(),
                ));
            }
            let n = if let LispVal::Number(n) = &args[0] {
                *n as usize
            } else {
                return Err(LispError::Generic(
                    "nth requires a number as first argument".to_string(),
                ));
            };
            let mut current = &args[1];
            for _ in 0..n {
                if let LispVal::Cons { car: _, cdr } = current {
                    current = cdr;
                } else {
                    return Ok(LispVal::Nil);
                }
            }
            if let LispVal::Cons { car, cdr: _ } = current {
                Ok(car.as_ref().clone())
            } else {
                Ok(LispVal::Nil)
            }
        }
        BuiltinFunc::Nthcdr => {
            if args.len() != 2 {
                return Err(LispError::Generic(
                    "nthcdr requires exactly two arguments".to_string(),
                ));
            }
            let n = if let LispVal::Number(n) = &args[0] {
                *n as usize
            } else {
                return Err(LispError::Generic(
                    "nthcdr requires a number as first argument".to_string(),
                ));
            };
            let mut current = args[1].clone();
            for _ in 0..n {
                if let LispVal::Cons { car: _, cdr } = current {
                    current = cdr.as_ref().clone();
                } else {
                    return Ok(LispVal::Nil);
                }
            }
            Ok(current)
        }
        BuiltinFunc::Efface => {
            if args.len() != 2 {
                return Err(LispError::Generic(
                    "efface requires exactly two arguments".to_string(),
                ));
            }
            let item = &args[0];
            let list = &args[1];

            // Build a new list without the first occurrence of item
            let items = list_to_vec(list)?;
            let mut found = false;
            let result: Vec<LispVal> = items
                .into_iter()
                .filter(|x| {
                    if !found && x == item {
                        found = true;
                        false
                    } else {
                        true
                    }
                })
                .collect();
            Ok(vec_to_list(result))
        }
        _ => Err(LispError::Generic("Not a list operation".to_string())),
    }
}

// New numeric operations
#[inline(never)]
fn apply_new_numeric_ops(
    op: &BuiltinFunc,
    args: &[LispVal],
    env: &Rc<Environment>,
) -> Result<LispVal, LispError> {
    match op {
        BuiltinFunc::Mod => {
            if args.len() != 2 {
                return Err(LispError::Generic(
                    "mod requires exactly two arguments".to_string(),
                ));
            }
            if let (LispVal::Number(x), LispVal::Number(y)) = (&args[0], &args[1]) {
                if *y == 0 {
                    return Err(LispError::Generic("Division by zero".to_string()));
                }
                // Use checked_rem_euclid to handle i64::MIN % -1 (overflow)
                Ok(LispVal::Number(x.checked_rem_euclid(*y).unwrap_or(0)))
            } else {
                Err(LispError::Generic(
                    "mod requires integer arguments".to_string(),
                ))
            }
        }
        BuiltinFunc::Plusp => {
            if args.len() != 1 {
                return Err(LispError::Generic(
                    "plusp requires exactly one argument".to_string(),
                ));
            }
            match &args[0] {
                LispVal::Number(n) => {
                    if *n > 0 {
                        Ok(LispVal::Symbol(env.intern_symbol("T")))
                    } else {
                        Ok(LispVal::Nil)
                    }
                }
                LispVal::Float(f) => {
                    if *f > 0.0 {
                        Ok(LispVal::Symbol(env.intern_symbol("T")))
                    } else {
                        Ok(LispVal::Nil)
                    }
                }
                _ => Err(LispError::Generic("plusp requires a number".to_string())),
            }
        }
        BuiltinFunc::Evenp => {
            if args.len() != 1 {
                return Err(LispError::Generic(
                    "evenp requires exactly one argument".to_string(),
                ));
            }
            if let LispVal::Number(n) = &args[0] {
                if n % 2 == 0 {
                    Ok(LispVal::Symbol(env.intern_symbol("T")))
                } else {
                    Ok(LispVal::Nil)
                }
            } else {
                Err(LispError::Generic("evenp requires an integer".to_string()))
            }
        }
        BuiltinFunc::Oddp => {
            if args.len() != 1 {
                return Err(LispError::Generic(
                    "oddp requires exactly one argument".to_string(),
                ));
            }
            if let LispVal::Number(n) = &args[0] {
                if n % 2 != 0 {
                    Ok(LispVal::Symbol(env.intern_symbol("T")))
                } else {
                    Ok(LispVal::Nil)
                }
            } else {
                Err(LispError::Generic("oddp requires an integer".to_string()))
            }
        }
        BuiltinFunc::Add1 => {
            if args.len() != 1 {
                return Err(LispError::Generic(
                    "add1 requires exactly one argument".to_string(),
                ));
            }
            match &args[0] {
                LispVal::Number(n) => Ok(LispVal::Number(n + 1)),
                LispVal::Float(f) => Ok(LispVal::Float(f + 1.0)),
                _ => Err(LispError::Generic("add1 requires a number".to_string())),
            }
        }
        BuiltinFunc::Sub1 => {
            if args.len() != 1 {
                return Err(LispError::Generic(
                    "sub1 requires exactly one argument".to_string(),
                ));
            }
            match &args[0] {
                LispVal::Number(n) => Ok(LispVal::Number(n - 1)),
                LispVal::Float(f) => Ok(LispVal::Float(f - 1.0)),
                _ => Err(LispError::Generic("sub1 requires a number".to_string())),
            }
        }
        BuiltinFunc::Random => {
            if args.len() != 1 {
                return Err(LispError::Generic(
                    "random requires exactly one argument".to_string(),
                ));
            }
            if let LispVal::Number(n) = &args[0] {
                if *n <= 0 {
                    return Err(LispError::Generic(
                        "random requires a positive integer".to_string(),
                    ));
                }
                // Simple linear congruential generator using system time as seed
                use std::time::{SystemTime, UNIX_EPOCH};
                let seed = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos() as u64;
                let random_val = (seed % (*n as u64)) as i64;
                Ok(LispVal::Number(random_val))
            } else {
                Err(LispError::Generic("random requires an integer".to_string()))
            }
        }
        _ => Err(LispError::Generic("Not a numeric operation".to_string())),
    }
}

// Type predicate operations
#[inline(never)]
fn apply_type_predicates(
    op: &BuiltinFunc,
    args: &[LispVal],
    env: &Rc<Environment>,
) -> Result<LispVal, LispError> {
    if args.len() != 1 {
        return Err(LispError::Generic(
            "Type predicate requires exactly one argument".to_string(),
        ));
    }
    let arg = &args[0];
    let result = match op {
        BuiltinFunc::Symbolp => matches!(arg, LispVal::Symbol(_)),
        BuiltinFunc::Boundp => {
            if let LispVal::Symbol(s) = arg {
                env.is_bound(&s.borrow().name)
            } else {
                return Err(LispError::Generic("boundp requires a symbol".to_string()));
            }
        }
        BuiltinFunc::Functionp => matches!(
            arg,
            LispVal::Lambda(_)
                | LispVal::Builtin(_)
                | LispVal::Fexpr(_)
                | LispVal::Native(_)
                | LispVal::Vau(_)
        ),
        BuiltinFunc::Macrop => matches!(arg, LispVal::Macro(_)),
        BuiltinFunc::Arrayp => matches!(arg, LispVal::Array(_)),
        BuiltinFunc::Extensionp => matches!(arg, LispVal::Extension(_)),
        _ => return Err(LispError::Generic("Not a type predicate".to_string())),
    };
    if result {
        Ok(LispVal::Symbol(env.intern_symbol("T")))
    } else {
        Ok(LispVal::Nil)
    }
}

// Function operations
#[inline(never)]
fn apply_function_ops(
    op: &BuiltinFunc,
    args: &[LispVal],
    env: &Rc<Environment>,
) -> Result<LispVal, LispError> {
    match op {
        BuiltinFunc::Funcall => {
            if args.is_empty() {
                return Err(LispError::Generic(
                    "funcall requires at least one argument".to_string(),
                ));
            }
            // If the first arg is a symbol, look it up to get the function
            let func = match &args[0] {
                LispVal::Symbol(s) => env.get(&s.borrow().name).ok_or_else(|| {
                    LispError::Generic(format!("Function not found: {}", s.borrow().name))
                })?,
                other => other.clone(),
            };
            let func_args = &args[1..];
            apply(&func, func_args, env)
        }
        BuiltinFunc::Macroexpand => {
            if args.len() != 1 {
                return Err(LispError::Generic(
                    "macroexpand requires exactly one argument".to_string(),
                ));
            }
            let form = &args[0];
            if let LispVal::Cons { car, cdr } = form
                && let LispVal::Symbol(s) = &**car
                && let Some(LispVal::Macro(m)) = env.get(&s.borrow().name)
            {
                let macro_args = list_to_vec(cdr)?;
                return expand_macro(&m, &macro_args, env);
            }
            // Not a macro call, return as-is
            Ok(form.clone())
        }
        _ => Err(LispError::Generic("Not a function operation".to_string())),
    }
}

// String/Symbol operations
#[inline(never)]
fn apply_string_symbol_ops(
    op: &BuiltinFunc,
    args: &[LispVal],
    env: &Rc<Environment>,
) -> Result<LispVal, LispError> {
    match op {
        BuiltinFunc::Explode => {
            if args.len() != 1 {
                return Err(LispError::Generic(
                    "explode requires exactly one argument".to_string(),
                ));
            }
            let chars: Vec<LispVal> = match &args[0] {
                LispVal::Symbol(s) => s
                    .borrow()
                    .name
                    .chars()
                    .map(|c| LispVal::Symbol(env.intern_symbol(&c.to_string())))
                    .collect(),
                LispVal::String(s) => s
                    .chars()
                    .map(|c| LispVal::Symbol(env.intern_symbol(&c.to_string())))
                    .collect(),
                LispVal::Number(n) => n
                    .to_string()
                    .chars()
                    .map(|c| LispVal::Symbol(env.intern_symbol(&c.to_string())))
                    .collect(),
                _ => {
                    return Err(LispError::Generic(
                        "explode requires a symbol, string, or number".to_string(),
                    ));
                }
            };
            Ok(vec_to_list(chars))
        }
        BuiltinFunc::Implode => {
            if args.len() != 1 {
                return Err(LispError::Generic(
                    "implode requires exactly one argument".to_string(),
                ));
            }
            let chars = list_to_vec(&args[0])?;
            let mut result = String::new();
            for ch in chars {
                match ch {
                    LispVal::Symbol(s) => result.push_str(&s.borrow().name),
                    LispVal::String(s) => result.push_str(&s),
                    _ => {
                        return Err(LispError::Generic(
                            "implode requires a list of symbols or strings".to_string(),
                        ));
                    }
                }
            }
            Ok(LispVal::Symbol(env.intern_symbol(&result)))
        }
        BuiltinFunc::Maknam => {
            // Same as implode in our implementation
            if args.len() != 1 {
                return Err(LispError::Generic(
                    "maknam requires exactly one argument".to_string(),
                ));
            }
            let chars = list_to_vec(&args[0])?;
            let mut result = String::new();
            for ch in chars {
                match ch {
                    LispVal::Symbol(s) => result.push_str(&s.borrow().name),
                    LispVal::String(s) => result.push_str(&s),
                    _ => {
                        return Err(LispError::Generic(
                            "maknam requires a list of symbols or strings".to_string(),
                        ));
                    }
                }
            }
            Ok(LispVal::Symbol(env.intern_symbol(&result)))
        }
        BuiltinFunc::Gensym => {
            if !args.is_empty() {
                return Err(LispError::Generic("gensym takes no arguments".to_string()));
            }
            Ok(LispVal::Symbol(env.gensym()))
        }
        BuiltinFunc::Intern => {
            if args.len() != 1 {
                return Err(LispError::Generic(
                    "intern requires exactly one argument".to_string(),
                ));
            }
            let name = match &args[0] {
                LispVal::String(s) => s.to_uppercase(),
                LispVal::Symbol(s) => s.borrow().name.clone(),
                _ => {
                    return Err(LispError::Generic(
                        "intern requires a string or symbol".to_string(),
                    ));
                }
            };
            Ok(LispVal::Symbol(env.intern_symbol(&name)))
        }
        BuiltinFunc::Plist => {
            if args.len() != 1 {
                return Err(LispError::Generic(
                    "plist requires exactly one argument".to_string(),
                ));
            }
            if let LispVal::Symbol(s) = &args[0] {
                let plist = &s.borrow().plist;
                let mut result = Vec::new();
                for (key, val) in plist.iter() {
                    result.push(LispVal::String(key.clone()));
                    result.push(val.clone());
                }
                Ok(vec_to_list(result))
            } else {
                Err(LispError::Generic(
                    "plist requires a symbol as its argument".to_string(),
                ))
            }
        }
        _ => Err(LispError::Generic(
            "Not a string/symbol operation".to_string(),
        )),
    }
}

// New bitwise operations
#[inline(never)]
fn apply_new_bitwise_ops(
    op: &BuiltinFunc,
    args: &[LispVal],
    env: &Rc<Environment>,
) -> Result<LispVal, LispError> {
    match op {
        BuiltinFunc::Ash => {
            if args.len() != 2 {
                return Err(LispError::Generic(
                    "ash requires exactly two arguments".to_string(),
                ));
            }
            if let (LispVal::Number(n), LispVal::Number(shift)) = (&args[0], &args[1]) {
                if *shift == 0 {
                    Ok(LispVal::Number(*n))
                } else if *shift < 0 {
                    // Right shift: if -shift >= 64, sign-extend to 0 or -1
                    let rshift = -*shift;
                    if rshift >= 64 {
                        Ok(LispVal::Number(if *n < 0 { -1 } else { 0 }))
                    } else {
                        Ok(LispVal::Number(n >> (rshift as u32)))
                    }
                } else {
                    // Left shift: guard against shift >= 64
                    if *shift >= 64 {
                        env.set_flag("OVERFLOW");
                        Ok(LispVal::Number(0))
                    } else {
                        // shift is in [0, 63]; wrapping_shl never panics here
                        Ok(LispVal::Number(n.wrapping_shl(*shift as u32)))
                    }
                }
            } else {
                Err(LispError::Generic(
                    "ash requires integer arguments".to_string(),
                ))
            }
        }
        BuiltinFunc::Lognot => {
            if args.len() != 1 {
                return Err(LispError::Generic(
                    "lognot requires exactly one argument".to_string(),
                ));
            }
            if let LispVal::Number(n) = &args[0] {
                Ok(LispVal::Number(!n))
            } else {
                Err(LispError::Generic(
                    "lognot requires an integer argument".to_string(),
                ))
            }
        }
        BuiltinFunc::Rot => {
            if args.len() != 2 {
                return Err(LispError::Generic(
                    "rot requires exactly two arguments".to_string(),
                ));
            }
            if let (LispVal::Number(n), LispVal::Number(count)) = (&args[0], &args[1]) {
                // rem_euclid on i64 always returns a value in [0, 63]
                let count = count.rem_euclid(64) as u32;
                Ok(LispVal::Number(((*n as u64).rotate_left(count)) as i64))
            } else {
                Err(LispError::Generic(
                    "rot requires integer arguments".to_string(),
                ))
            }
        }
        _ => Err(LispError::Generic("Not a bitwise operation".to_string())),
    }
}

// Property list operations
#[inline(never)]
fn apply_plist_op(
    op: &BuiltinFunc,
    args: &[LispVal],
    env: &Rc<Environment>,
) -> Result<LispVal, LispError> {
    match op {
        BuiltinFunc::Remprop => {
            if args.len() != 2 {
                return Err(LispError::Generic(
                    "remprop requires exactly two arguments".to_string(),
                ));
            }
            let prop = match &args[1] {
                LispVal::String(s) => s.clone(),
                LispVal::Symbol(s) => s.borrow().name.clone(),
                _ => {
                    return Err(LispError::Generic(
                        "remprop requires a symbol or string as its second argument".to_string(),
                    ));
                }
            };
            if let LispVal::Symbol(s) = &args[0] {
                let removed = s.borrow_mut().plist.remove(&prop);
                if removed.is_some() {
                    Ok(LispVal::Symbol(env.intern_symbol("T")))
                } else {
                    Ok(LispVal::Nil)
                }
            } else {
                Err(LispError::Generic(
                    "remprop requires a symbol as its first argument".to_string(),
                ))
            }
        }
        BuiltinFunc::Deflist => {
            if args.len() != 2 {
                return Err(LispError::Generic(
                    "deflist requires exactly two arguments".to_string(),
                ));
            }
            let pairs = &args[0];
            let indicator = match &args[1] {
                LispVal::String(s) => s.clone(),
                LispVal::Symbol(s) => s.borrow().name.clone(),
                _ => {
                    return Err(LispError::Generic(
                        "deflist requires a symbol or string as its second argument".to_string(),
                    ));
                }
            };
            let mut current = pairs;
            while let LispVal::Cons { car, cdr } = current {
                if let LispVal::Cons {
                    car: sym,
                    cdr: rest,
                } = &**car
                    && let LispVal::Symbol(s) = &**sym
                    && let LispVal::Cons { car: val, cdr: _ } = &**rest
                {
                    s.borrow_mut()
                        .plist
                        .insert(indicator.clone(), val.as_ref().clone());
                }
                current = cdr;
            }
            Ok(LispVal::Symbol(env.intern_symbol("T")))
        }
        _ => Err(LispError::Generic(
            "Not a property list operation".to_string(),
        )),
    }
}

#[cfg(test)]
mod evaluator_internal_tests {
    use super::*;

    fn dummy_env() -> Rc<Environment> {
        Environment::new_with_builtins()
    }

    // ---- apply_math_op fallthrough ----
    // Pass a BuiltinFunc that is not handled by apply_math_op (e.g. Car)
    // to hit the `_ => Err(...)` arm at line ~223.
    #[test]
    fn test_apply_math_op_fallthrough() {
        let env = dummy_env();
        let result = apply_math_op(&BuiltinFunc::Car, &[LispVal::Number(1)], &env);
        assert!(result.is_err(), "apply_math_op with Car should error");
    }

    // ---- apply_list_op fallthrough ----
    // Pass a BuiltinFunc not handled by apply_list_op (e.g. Plus)
    // to hit the `_ => Err(...)` arm at line ~264.
    #[test]
    fn test_apply_list_op_fallthrough() {
        let result = apply_list_op(&BuiltinFunc::Plus, &[]);
        assert!(result.is_err(), "apply_list_op with Plus should error");
    }

    // ---- apply_string_op fallthrough ----
    // Pass a BuiltinFunc not handled by apply_string_op (e.g. Car)
    // to hit the `_ => Err(...)` arm at line ~308.
    #[test]
    fn test_apply_string_op_fallthrough() {
        let result = apply_string_op(&BuiltinFunc::Car, &[]);
        assert!(result.is_err(), "apply_string_op with Car should error");
    }

    // ---- apply_numeric_primitives fallthrough ----
    // Pass a BuiltinFunc not handled by apply_numeric_primitives (e.g. Car)
    // to hit the `_ => Err(...)` arm at line ~401.
    #[test]
    fn test_apply_numeric_primitives_fallthrough() {
        let env = dummy_env();
        let result = apply_numeric_primitives(&BuiltinFunc::Car, &[], &env);
        assert!(
            result.is_err(),
            "apply_numeric_primitives with Car should error"
        );
    }

    // ---- apply_logical_op fallthrough ----
    // Pass a BuiltinFunc not handled by apply_logical_op (e.g. Car)
    // to hit the `_ => Err(...)` arm at line ~453.
    #[test]
    fn test_apply_logical_op_fallthrough() {
        let env = dummy_env();
        let result = apply_logical_op(&BuiltinFunc::Car, &[], &env);
        assert!(result.is_err(), "apply_logical_op with Car should error");
    }

    // ---- apply_hashtable_op fallthrough ----
    // Pass a BuiltinFunc not handled by apply_hashtable_op (e.g. Car)
    // to hit the `_ => Err(...)` arm at line ~551.
    #[test]
    fn test_apply_hashtable_op_fallthrough() {
        let env = dummy_env();
        let result = apply_hashtable_op(&BuiltinFunc::Car, &[], &env);
        assert!(result.is_err(), "apply_hashtable_op with Car should error");
    }

    #[test]
    fn test_apply_symbol_op_fallthrough() {
        let env = dummy_env();
        let result = apply_symbol_op(&BuiltinFunc::Car, &[], &env);
        assert!(result.is_err(), "apply_symbol_op with Car should error");
    }

    #[test]
    fn test_apply_io_op_fallthrough() {
        let env = dummy_env();
        let result = apply_io_op(&BuiltinFunc::Car, &[], &env);
        assert!(result.is_err(), "apply_io_op with Car should error");
    }

    #[test]
    fn test_apply_error_op_fallthrough() {
        let env = dummy_env();
        let result = apply_error_op(&BuiltinFunc::Car, &[], &env);
        assert!(result.is_err(), "apply_error_op with Car should error");
    }

    #[test]
    fn test_apply_list_processing_fallthrough() {
        let env = dummy_env();
        let result = apply_list_processing(&BuiltinFunc::Car, &[], &env);
        assert!(
            result.is_err(),
            "apply_list_processing with Car should error"
        );
    }

    #[test]
    fn test_apply_bitwise_op_fallthrough() {
        let env = dummy_env();
        let result = apply_bitwise_op(&BuiltinFunc::Car, &[], &env);
        assert!(result.is_err(), "apply_bitwise_op with Car should error");
    }

    #[test]
    fn test_apply_new_list_ops_fallthrough() {
        let env = dummy_env();
        let result = apply_new_list_ops(&BuiltinFunc::Car, &[], &env);
        assert!(result.is_err(), "apply_new_list_ops with Car should error");
    }

    #[test]
    fn test_apply_new_numeric_ops_fallthrough() {
        let env = dummy_env();
        let result = apply_new_numeric_ops(&BuiltinFunc::Car, &[], &env);
        assert!(
            result.is_err(),
            "apply_new_numeric_ops with Car should error"
        );
    }

    #[test]
    fn test_apply_type_predicates_fallthrough() {
        let env = dummy_env();
        let result = apply_type_predicates(&BuiltinFunc::Car, &[LispVal::Nil], &env);
        assert!(
            result.is_err(),
            "apply_type_predicates with Car should error"
        );
    }

    #[test]
    fn test_apply_function_ops_fallthrough() {
        let env = dummy_env();
        let result = apply_function_ops(&BuiltinFunc::Car, &[], &env);
        assert!(result.is_err(), "apply_function_ops with Car should error");
    }

    #[test]
    fn test_apply_string_symbol_ops_fallthrough() {
        let env = dummy_env();
        let result = apply_string_symbol_ops(&BuiltinFunc::Car, &[], &env);
        assert!(
            result.is_err(),
            "apply_string_symbol_ops with Car should error"
        );
    }

    #[test]
    fn test_apply_new_bitwise_ops_fallthrough() {
        let env = dummy_env();
        let result = apply_new_bitwise_ops(&BuiltinFunc::Car, &[], &env);
        assert!(
            result.is_err(),
            "apply_new_bitwise_ops with Car should error"
        );
    }

    #[test]
    fn test_apply_plist_op_fallthrough() {
        let env = dummy_env();
        let result = apply_plist_op(&BuiltinFunc::Car, &[], &env);
        assert!(result.is_err(), "apply_plist_op with Car should error");
    }
}
