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

// Helper function to convert a Rust Vec to a Lisp list.
fn vec_to_list(vec: Vec<LispVal>) -> LispVal {
    vec.into_iter()
        .rev()
        .fold(LispVal::Nil, |cdr, car| LispVal::Cons {
            car: Box::new(car),
            cdr: Box::new(cdr),
        })
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
                for (param, arg) in f.params.iter().zip(unpacked_args.into_iter()) {
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
    let has_float = args
        .iter()
        .any(|a| matches!(a, LispVal::Float(_)));
    if has_float {
        let floats: Result<Vec<f64>, LispError> = args
            .iter()
            .map(|arg| match arg {
                LispVal::Number(n) => Ok(*n as f64),
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
                    Ok(LispVal::Float(
                        floats[0] - floats[1..].iter().sum::<f64>(),
                    ))
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

    let nums: Result<Vec<i64>, LispError> = args
        .iter()
        .map(|arg| match arg {
            LispVal::Number(n) => Ok(*n),
            _ => Err(LispError::Generic(
                "Math functions only accept numbers".to_string(),
            )),
        })
        .collect();
    let nums = nums?;

    match op {
        BuiltinFunc::Plus => {
            let mut result = 0i64;
            for &num in &nums {
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
            if nums.is_empty() {
                return Err(LispError::Generic(
                    "- requires at least one argument".to_string(),
                ));
            }
            if nums.len() == 1 {
                match nums[0].checked_neg() {
                    Some(v) => Ok(LispVal::Number(v)),
                    None => {
                        env.set_flag("OVERFLOW");
                        Ok(LispVal::Number(nums[0].wrapping_neg()))
                    }
                }
            } else {
                let mut result = nums[0];
                for &num in &nums[1..] {
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
            for &num in &nums {
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
            if nums.len() != 2 {
                return Err(LispError::Generic(
                    "/ requires exactly two arguments".to_string(),
                ));
            }
            if nums[1] == 0 {
                return Err(LispError::Generic("Division by zero".to_string()));
            }
            // Check for i64::MIN / -1 overflow
            if nums[0] == i64::MIN && nums[1] == -1 {
                env.set_flag("OVERFLOW");
                Ok(LispVal::Number(nums[0].wrapping_div(nums[1])))
            } else {
                Ok(LispVal::Number(nums[0] / nums[1]))
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
                LispVal::Cons { car, .. } => Ok(*car.clone()),
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
                LispVal::Cons { cdr, .. } => Ok(*cdr.clone()),
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
                car: Box::new(args[0].clone()),
                cdr: Box::new(args[1].clone()),
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
            if let (LispVal::Number(x), LispVal::Number(y)) = (&args[0], &args[1]) {
                Ok(if x < y {
                    LispVal::Symbol(env.intern_symbol("T"))
                } else {
                    LispVal::Nil
                })
            } else {
                Err(LispError::Generic("lessp requires numbers".to_string()))
            }
        }
        BuiltinFunc::Greaterp => {
            if args.len() != 2 {
                return Err(LispError::Generic("greaterp requires 2 args".to_string()));
            }
            if let (LispVal::Number(x), LispVal::Number(y)) = (&args[0], &args[1]) {
                Ok(if x > y {
                    LispVal::Symbol(env.intern_symbol("T"))
                } else {
                    LispVal::Nil
                })
            } else {
                Err(LispError::Generic("greaterp requires numbers".to_string()))
            }
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
                        return Ok(LispVal::Float(
                            (*base as f64).powi(*exp as i32),
                        ));
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
                (LispVal::Float(base), LispVal::Float(exp)) => {
                    Ok(LispVal::Float(base.powf(*exp)))
                }
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
            if let (LispVal::Number(a), LispVal::Number(b)) = (&args[0], &args[1]) {
                if a == b {
                    Ok(LispVal::Symbol(env.intern_symbol("T")))
                } else {
                    Ok(LispVal::Nil)
                }
            } else {
                Err(LispError::Generic(
                    "= requires numeric arguments".to_string(),
                ))
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
            "SHELL capability is not enabled; call (enable-feature \"SHELL\") first".to_string(),
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
        car: Box::new(LispVal::Number(code)),
        cdr: Box::new(LispVal::Cons {
            car: Box::new(LispVal::String(stdout)),
            cdr: Box::new(LispVal::Cons {
                car: Box::new(LispVal::String(stderr)),
                cdr: Box::new(LispVal::Nil),
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
                        ))
                    }
                };
                let mut result = vec![];
                let mut cur = list.clone();
                while let LispVal::Cons { car, cdr } = cur {
                    result.push(eval(&car, &eval_env)?);
                    cur = *cdr;
                }
                let mut out = LispVal::Nil;
                for v in result.into_iter().rev() {
                    out = LispVal::Cons {
                        car: Box::new(v),
                        cdr: Box::new(out),
                    };
                }
                Ok(out)
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
            BuiltinFunc::Read | BuiltinFunc::Prin1 | BuiltinFunc::Princ | BuiltinFunc::Terpri => {
                apply_io_op(builtin, args, env)
            }

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
                if !env.feature_enabled("FILE-IO") {
                    return Err(LispError::Generic(
                        "FILE-IO capability is not enabled; call (enable-feature \"FILE-IO\") first"
                            .to_string(),
                    ));
                }
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

            // Capabilities / features
            BuiltinFunc::EnableFeature => {
                let name = feature_name_arg(args, "enable-feature")?;
                env.enable_feature(&name);
                Ok(LispVal::Symbol(env.intern_symbol("T")))
            }
            BuiltinFunc::DisableFeature => {
                let name = feature_name_arg(args, "disable-feature")?;
                env.disable_feature(&name);
                Ok(LispVal::Symbol(env.intern_symbol("T")))
            }
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
                        car: Box::new(LispVal::String(n)),
                        cdr: Box::new(cdr),
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
                        ))
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
                            ))
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
                            ))
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

    Ok(LispVal::Lambda(crate::Lambda {
        params: params_vec,
        rest_param,
        body: Box::new(body.clone()),
        env: env.clone(),
    }))
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

    Ok(LispVal::Fexpr(crate::Fexpr {
        params: params_vec?,
        body: Box::new(body.clone()),
        env: env.clone(),
    }))
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

    Ok(LispVal::Macro(crate::Macro {
        params: params_vec,
        rest_param,
        body: Box::new(body.clone()),
        env: env.clone(),
    }))
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
/// for the outermost call, then delegates to `eval_impl` which performs a
/// trampoline loop for tail-call optimization (issue #62).
///
/// Non-tail recursive calls inside `eval_impl` call back into this function so
/// that the depth guard is applied to every non-tail frame.
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
            TcoStep::TailCall(new_val, new_env) => {
                current_val = new_val;
                current_env = new_env;
                // continue the loop
            }
        }
    }
}

/// Perform one evaluation step. Returns `TcoStep::Done` for final results
/// and `TcoStep::TailCall` for tail positions (caller loops instead of recursing).
fn eval_step(val: &LispVal, env: &Rc<Environment>) -> Result<TcoStep, LispError> {
    match val {
        LispVal::Nil => Ok(TcoStep::Done(Ok(LispVal::Nil))),
        LispVal::Symbol(s) => {
            let name = s.borrow().name.clone();

            // Use get_var which handles both dynamic and lexical scoping
            let value = env
                .get_var(&name)
                .ok_or_else(|| LispError::Generic(format!("Unbound variable: {}", name)))?;

            // If the value is a LABEL expression, tail-call evaluate it
            // This handles recursive LABEL definitions (TCO: TailCall instead of recurse)
            if let LispVal::Cons { car, cdr: _ } = &value {
                if let LispVal::Symbol(sym) = &**car {
                    if sym.borrow().name == "LABEL" {
                        return Ok(TcoStep::TailCall(value, env.clone()));
                    }
                }
            }

            Ok(TcoStep::Done(Ok(value)))
        }
        LispVal::Number(_)
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
        | LispVal::Extension(_) => Ok(TcoStep::Done(Ok(val.clone()))),

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
                            return Ok(TcoStep::Done(Ok(*car.clone())));
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
                                                                *last_expr.clone(),
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
                        let args = list_to_vec(rest)?;
                        if args.len() != 3 {
                            return Ok(TcoStep::Done(Err(LispError::Generic(
                                "if takes exactly three arguments".to_string(),
                            ))));
                        }
                        // Evaluate condition normally (non-tail)
                        let cond_result = eval(&args[0], env)?;
                        // TCO: tail position is either branch
                        let next_val = if is_truthy(&cond_result) {
                            args[1].clone()
                        } else {
                            args[2].clone()
                        };
                        Ok(TcoStep::TailCall(next_val, env.clone()))
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
                            if let LispVal::Symbol(expr_sym) = expr_val {
                                if name_sym.borrow().name == expr_sym.borrow().name {
                                    return Ok(TcoStep::Done(Err(LispError::Generic(format!(
                                        "LABEL: pathological self-reference (LABEL {} {}) would cause infinite recursion",
                                        name_sym.borrow().name,
                                        expr_sym.borrow().name
                                    )))));
                                }
                            }

                            let new_env = Environment::new_child(env);
                            let label_expr = LispVal::Cons {
                                car: Box::new(LispVal::Symbol(env.intern_symbol("LABEL"))),
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
                        let def_list = list_to_vec(&defs[0])?;
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
                                let v = &def_pair[1];
                                env.set(name, v.clone());
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
                            env.set(name_sym.borrow().name.clone(), func);
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
                                    return Ok(TcoStep::TailCall(*last_expr.clone(), env.clone()));
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
                        let args_vec = list_to_vec(rest)?;
                        if args_vec.len() % 2 != 0 {
                            return Ok(TcoStep::Done(Err(LispError::Generic(
                                "SETQ requires an even number of arguments".to_string(),
                            ))));
                        }
                        let mut last_val = LispVal::Nil;
                        for chunk in args_vec.chunks(2) {
                            let var = &chunk[0];
                            let val_expr = &chunk[1];
                            if let LispVal::Symbol(s) = var {
                                let v = eval(val_expr, env)?;
                                Environment::update(env, &s.borrow().name, v.clone());
                                last_val = v;
                            } else {
                                return Ok(TcoStep::Done(Err(LispError::Generic(
                                    "SETQ variable name must be a symbol".to_string(),
                                ))));
                            }
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
                    "LET" => {
                        let args = list_to_vec(rest)?;
                        if args.len() != 2 {
                            return Ok(TcoStep::Done(Err(LispError::Generic(
                                "let takes exactly two arguments".to_string(),
                            ))));
                        }
                        let bindings_vec = list_to_vec(&args[0])?;
                        let body = args[1].clone();

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
                    // let* evaluates bindings sequentially; each binding sees prior ones
                    "LET*" => {
                        let args = list_to_vec(rest)?;
                        if args.len() != 2 {
                            return Ok(TcoStep::Done(Err(LispError::Generic(
                                "let* takes exactly two arguments".to_string(),
                            ))));
                        }
                        let bindings_vec = list_to_vec(&args[0])?;
                        let body = args[1].clone();

                        let mut cur_env = Environment::new_child(env);
                        for binding in bindings_vec {
                            let pair = list_to_vec(&binding)?;
                            if pair.len() != 2 {
                                return Ok(TcoStep::Done(Err(LispError::Generic(
                                    "let* binding must be a pair".to_string(),
                                ))));
                            }
                            if let LispVal::Symbol(s) = &pair[0] {
                                let v = eval(&pair[1], &cur_env)?;
                                let next_env = Environment::new_child(&cur_env);
                                next_env.set(s.borrow().name.clone(), v);
                                cur_env = next_env;
                            } else {
                                return Ok(TcoStep::Done(Err(LispError::Generic(
                                    "let* binding name must be a symbol".to_string(),
                                ))));
                            }
                        }
                        Ok(TcoStep::TailCall(body, cur_env))
                    }
                    "VAU" | "$VAU" => {
                        // (vau (operands-param env-param) body...)
                        // operands-param receives the unevaluated operand list;
                        // env-param receives the caller's environment.
                        let args = list_to_vec(rest)?;
                        if args.len() < 1 {
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
                                    car: Box::new(form.clone()),
                                    cdr: Box::new(progn),
                                };
                            }
                            LispVal::Cons {
                                car: Box::new(progn_sym),
                                cdr: Box::new(progn),
                            }
                        };
                        Ok(TcoStep::Done(Ok(LispVal::Vau(crate::Vau {
                            operands_param: op_param,
                            env_param,
                            body: Box::new(body),
                            env: env.clone(),
                        }))))
                    }
                    _ => {
                        // Function call: evaluate the function head
                        let func = eval(first, env)?;
                        let args_list = list_to_vec(rest)?;

                        // Macro expansion: TCO — continue with the expanded form
                        if let LispVal::Macro(m) = &func {
                            let expanded = expand_macro(m, &args_list, env)?;
                            return Ok(TcoStep::TailCall(expanded, env.clone()));
                        }

                        // Vau application: bind operands (unevaluated) and caller env, TCO into body
                        if let LispVal::Vau(vau) = &func {
                            let new_env = Environment::new_child(&vau.env);
                            new_env.set(vau.operands_param.clone(), *rest.clone());
                            new_env.set(vau.env_param.clone(), LispVal::Environment(env.clone()));
                            return Ok(TcoStep::TailCall(*vau.body.clone(), new_env));
                        }

                        // Fexpr application: TCO — continue with fexpr body
                        if let LispVal::Fexpr(fexpr) = &func {
                            let new_env = Environment::new_child_with_dynamic(&fexpr.env, env);
                            if fexpr.params.len() == 1 {
                                // Single-param: bind entire unevaluated arg list to the one parameter.
                                new_env.set(fexpr.params[0].clone(), *rest.clone());
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
                                for (param, arg) in
                                    fexpr.params.iter().zip(unevaluated_args.into_iter())
                                {
                                    new_env.set(param.clone(), arg);
                                }
                            }
                            return Ok(TcoStep::TailCall(*fexpr.body.clone(), new_env));
                        }

                        // Evaluate arguments (non-tail)
                        let eval_args: Result<Vec<LispVal>, LispError> =
                            args_list.iter().map(|arg| eval(arg, env)).collect();
                        let eval_args = eval_args?;

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

                        // All other callables (builtins, natives): no TCO needed
                        Ok(TcoStep::Done(apply(&func, &eval_args, env)))
                    }
                }
            } else {
                // Non-symbol head: evaluate the head expression, then apply
                let func = eval(first, env)?;

                // Vau: must intercept BEFORE evaluating args
                if let LispVal::Vau(vau) = &func {
                    let new_env = Environment::new_child(&vau.env);
                    new_env.set(vau.operands_param.clone(), *rest.clone());
                    new_env.set(vau.env_param.clone(), LispVal::Environment(env.clone()));
                    return Ok(TcoStep::TailCall(*vau.body.clone(), new_env));
                }

                // Fexpr: must intercept BEFORE evaluating args
                if let LispVal::Fexpr(fexpr) = &func {
                    let new_env = Environment::new_child_with_dynamic(&fexpr.env, env);
                    if fexpr.params.len() == 1 {
                        new_env.set(fexpr.params[0].clone(), *rest.clone());
                    } else {
                        let unevaluated_args = list_to_vec(rest)?;
                        if unevaluated_args.len() != fexpr.params.len() {
                            return Ok(TcoStep::Done(Err(LispError::Generic(format!(
                                "fexpr expected {} arguments, got {}",
                                fexpr.params.len(),
                                unevaluated_args.len()
                            )))));
                        }
                        for (param, arg) in fexpr.params.iter().zip(unevaluated_args.into_iter()) {
                            new_env.set(param.clone(), arg);
                        }
                    }
                    return Ok(TcoStep::TailCall(*fexpr.body.clone(), new_env));
                }

                let args_list = list_to_vec(rest)?;
                let eval_args: Result<Vec<LispVal>, LispError> =
                    args_list.iter().map(|arg| eval(arg, env)).collect();
                let eval_args = eval_args?;

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
        let car_eval = quasiquote_eval(car, env)?;
        let cdr_eval = quasiquote_eval(cdr, env)?;
        Ok(LispVal::Cons {
            car: Box::new(car_eval),
            cdr: Box::new(cdr_eval),
        })
    } else {
        Ok(val.clone())
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
                    ))
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
                    ))
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
                    "IO capability is not enabled; call (enable-feature \"IO\") first".to_string(),
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
            if args.is_empty() {
                return Err(LispError::Generic("Error".to_string()));
            }
            let msg = if let LispVal::String(s) = &args[0] {
                s.clone()
            } else {
                crate::printer::print(&args[0])
            };
            Err(LispError::Generic(msg))
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
                Err(_) => Ok(LispVal::Nil),
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
                        car: Box::new(subst_helper(new, old, car)),
                        cdr: Box::new(subst_helper(new, old, cdr)),
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
                    {
                        if **pair_key == *key {
                            return Some(*pair_val.clone());
                        }
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
                            car: Box::new(sublis_helper(alist, car)),
                            cdr: Box::new(sublis_helper(alist, cdr)),
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
                        return Ok(*car.clone());
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
            // Arg order: (maplist list fn) — list first.
            // Lisp 1.5 manual has maplist[fn;x] (fn first), but our arg order
            // matches Common Lisp convention and is used consistently throughout
            // the stdlib. Intentional deviation from the 1.5 manual (issue #66).
            if args.len() != 2 {
                return Err(LispError::Generic(
                    "maplist requires exactly two arguments".to_string(),
                ));
            }
            let list = &args[0];
            let func = &args[1];
            let mut result = Vec::new();
            let mut current = list.clone();
            while let LispVal::Cons { car: _, cdr } = &current {
                let applied = apply(func, &[current.clone()], env)?;
                result.push(applied);
                current = *cdr.clone();
            }
            Ok(vec_to_list(result))
        }
        BuiltinFunc::Mapcar => {
            // Arg order: (mapcar list fn) — list first.
            // Lisp 1.5 manual has mapcar[fn;x] (fn first), but our arg order
            // matches Common Lisp convention and is used consistently throughout
            // the stdlib. Intentional deviation from the 1.5 manual (issue #66).
            if args.len() != 2 {
                return Err(LispError::Generic(
                    "mapcar requires exactly two arguments".to_string(),
                ));
            }
            let list = &args[0];
            let func = &args[1];
            let mut result = Vec::new();
            let mut current = list;
            while let LispVal::Cons { car, cdr } = current {
                let applied = apply(func, &[*car.clone()], env)?;
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
                    car: Box::new(args[1].clone()),
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
                    cdr: Box::new(args[1].clone()),
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
                Ok(*car.clone())
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
                    current = *cdr;
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
                // MOD uses floored division (different from remainder for negative numbers)
                Ok(LispVal::Number(x.rem_euclid(*y)))
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
                    ))
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
                    ))
                }
            };
            let mut current = pairs;
            while let LispVal::Cons { car, cdr } = current {
                if let LispVal::Cons {
                    car: sym,
                    cdr: rest,
                } = &**car
                {
                    if let LispVal::Symbol(s) = &**sym {
                        if let LispVal::Cons { car: val, cdr: _ } = &**rest {
                            s.borrow_mut().plist.insert(indicator.clone(), *val.clone());
                        }
                    }
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
