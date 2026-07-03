use super::*;
#[inline(never)]
pub(super) fn apply_apply(
    args: &[LispVal],
    env: &Shared<Environment>,
) -> Result<LispVal, LispError> {
    // CL-style spread: (apply f a b '(c d)) calls (f a b c d).  The last
    // argument must be a proper list; any arguments between the function and
    // that list are prepended to it (issue #245).
    if args.len() < 2 {
        return Err(LispError::Generic(
            "APPLY requires a function and an argument list".to_string(),
        ));
    }
    let func_arg = &args[0];
    let arg_list = &args[args.len() - 1];

    let func = match func_arg {
        LispVal::Symbol(s) => env
            .get(&s.borrow().name)
            .ok_or_else(|| LispError::Generic(format!("Function not found: {}", s.borrow().name))),
        _ => Ok(func_arg.clone()),
    }?;

    let mut unpacked_args: Vec<LispVal> = args[1..args.len() - 1].to_vec();
    match list_to_vec(arg_list) {
        Ok(vec) => unpacked_args.extend(vec),
        Err(_) => {
            return Err(LispError::Generic(format!(
                "APPLY: last argument must be a proper list, got {}",
                err_val(arg_list)
            )));
        }
    };

    match &func {
        LispVal::Macro(m) => {
            let expanded = expand_macro(m, &unpacked_args, env)?;
            eval(&expanded, env)
        }
        LispVal::Fexpr(f) => {
            let new_env = Environment::new_child_with_dynamic(&f.env, env);
            let has_dyn = new_env.has_any_dynamic();
            let mut _guards: Vec<DynamicBinding> = Vec::new();
            if f.params.len() == 1 {
                let fexpr_arg_list = vec_to_list(unpacked_args);
                if has_dyn
                    && let Some(sym) = new_env.symbol_by_id(f.param_ids[0])
                    && sym.borrow().is_dynamic
                {
                    _guards.push(DynamicBinding::install(sym, fexpr_arg_list));
                } else {
                    new_env.set(f.params[0].clone(), fexpr_arg_list);
                }
            } else {
                if unpacked_args.len() != f.params.len() {
                    return Err(LispError::Generic(format!(
                        "APPLY: fexpr expected {} arguments, got {}",
                        f.params.len(),
                        unpacked_args.len()
                    )));
                }
                for ((param, id), arg) in f.params.iter().zip(&f.param_ids).zip(unpacked_args) {
                    if has_dyn
                        && let Some(sym) = new_env.symbol_by_id(*id)
                        && sym.borrow().is_dynamic
                    {
                        _guards.push(DynamicBinding::install(sym, arg));
                        continue;
                    }
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

pub(super) fn is_truthy(val: &LispVal) -> bool {
    !matches!(val, LispVal::Nil)
}

#[inline(never)]
pub(super) fn apply_math_op(
    op: &BuiltinFunc,
    args: &[LispVal],
    env: &Shared<Environment>,
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
                other => Err(LispError::Generic(format!(
                    "Math functions only accept numbers, got {}",
                    err_val(other)
                ))),
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
                // IEEE 754 semantics: float division by zero yields ±inf (or
                // NaN for 0.0/0.0) rather than an error — the printer and
                // reader both round-trip inf/NaN (issue #245). Integer
                // division by zero below remains an error.
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
            other => Err(LispError::Generic(format!(
                "Math functions only accept numbers, got {}",
                err_val(other)
            ))),
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
pub(super) fn apply_list_op(op: &BuiltinFunc, args: &[LispVal]) -> Result<LispVal, LispError> {
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
                other => Err(LispError::Generic(format!(
                    "car requires a list, got {}",
                    err_val(other)
                ))),
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
                other => Err(LispError::Generic(format!(
                    "cdr requires a list, got {}",
                    err_val(other)
                ))),
            }
        }
        BuiltinFunc::Cons => {
            if args.len() != 2 {
                return Err(LispError::Generic(
                    "cons requires exactly two arguments".to_string(),
                ));
            }
            Ok(LispVal::Cons {
                car: Shared::new(args[0].clone()),
                cdr: Shared::new(args[1].clone()),
            })
        }
        _ => Err(LispError::Generic("Not a list operation".to_string())),
    }
}

#[inline(never)]
pub(super) fn apply_string_op(op: &BuiltinFunc, args: &[LispVal]) -> Result<LispVal, LispError> {
    match op {
        BuiltinFunc::Concat => {
            let strs: Result<Vec<String>, LispError> = args
                .iter()
                .map(|arg| match arg {
                    LispVal::String(s) => Ok(s.clone()),
                    other => Err(LispError::Generic(format!(
                        "concat only accepts strings, got {}",
                        err_val(other)
                    ))),
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
pub(super) fn as_f64(v: &LispVal, ctx: &str) -> Result<f64, LispError> {
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
pub(super) fn apply_math_lib(op: &BuiltinFunc, args: &[LispVal]) -> Result<LispVal, LispError> {
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

pub(super) fn gcd_i64(mut a: i64, mut b: i64) -> i64 {
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
pub(super) fn apply_string_lib(op: &BuiltinFunc, args: &[LispVal]) -> Result<LispVal, LispError> {
    let require_one = |name: &str| -> Result<(), LispError> {
        if args.len() == 1 {
            Ok(())
        } else {
            Err(LispError::Generic(format!(
                "{name} requires exactly one argument"
            )))
        }
    };
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
            require_one("char-code")?;
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
            require_one("code-char")?;
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
            require_one("make-char")?;
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
            require_one("string->number")?;
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
        BuiltinFunc::NumberToString => {
            require_one("number->string")?;
            match args.first() {
                Some(LispVal::Number(n)) => Ok(LispVal::String(n.to_string())),
                Some(LispVal::Float(f)) => Ok(LispVal::String(f.to_string())),
                _ => Err(LispError::Generic(
                    "number->string requires a number".to_string(),
                )),
            }
        }
        BuiltinFunc::Prin1ToString => {
            require_one("prin1-to-string")?;
            match args.first() {
                // Readable representation (strings are quoted), via the printer.
                Some(v) => Ok(LispVal::String(crate::printer::print(v))),
                None => unreachable!("arity checked above"),
            }
        }
        BuiltinFunc::PrincToString => {
            require_one("princ-to-string")?;
            match args.first() {
                // Human representation: a top-level string yields its raw contents,
                // mirroring PRINC; everything else uses the printer.
                Some(LispVal::String(s)) => Ok(LispVal::String(s.clone())),
                Some(v) => Ok(LispVal::String(crate::printer::print(v))),
                None => unreachable!("arity checked above"),
            }
        }
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
pub(super) fn apply_sort(
    args: &[LispVal],
    env: &Shared<Environment>,
) -> Result<LispVal, LispError> {
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
pub(super) fn apply_error_value_op(
    op: &BuiltinFunc,
    args: &[LispVal],
    env: &Shared<Environment>,
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
            Ok(LispVal::Error(Shared::new(crate::ErrorObj {
                message,
                data,
            })))
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
pub(super) fn apply_numeric_primitives(
    op: &BuiltinFunc,
    args: &[LispVal],
    env: &Shared<Environment>,
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
pub(super) fn apply_logical_op(
    op: &BuiltinFunc,
    args: &[LispVal],
    env: &Shared<Environment>,
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
pub(super) fn apply_hashtable_op(
    op: &BuiltinFunc,
    args: &[LispVal],
    env: &Shared<Environment>,
) -> Result<LispVal, LispError> {
    match op {
        BuiltinFunc::MakeHashTable => {
            if !args.is_empty() {
                return Err(LispError::Generic(
                    "make-hash-table takes no arguments".to_string(),
                ));
            }
            Ok(LispVal::HashTable(Shared::new(SharedCell::new(
                HashMap::new(),
            ))))
        }
        BuiltinFunc::Set => {
            if args.len() != 3 {
                return Err(LispError::Generic(
                    "sethash/set-bang takes exactly three arguments".to_string(),
                ));
            }
            if let LispVal::HashTable(h) = &args[0] {
                let key = args[1].clone();
                let val = args[2].clone();
                h.borrow_mut().insert(key, val);
                Ok(LispVal::Symbol(env.intern_symbol("T")))
            } else {
                Err(LispError::Generic(
                    "sethash/set-bang requires a hash table as its first argument".to_string(),
                ))
            }
        }
        BuiltinFunc::Get => {
            // Accepts (gethash table key) — the historical Lamedh order — or
            // CL's (gethash key table); a hash table is unambiguous in either
            // position (issue #246).
            if args.len() != 2 {
                return Err(LispError::Generic(
                    "gethash takes exactly two arguments".to_string(),
                ));
            }
            let (h, key) = match (&args[0], &args[1]) {
                (LispVal::HashTable(h), key) => (h, key),
                (key, LispVal::HashTable(h)) => (h, key),
                (other, _) => {
                    return Err(LispError::Generic(format!(
                        "gethash requires a hash table argument, got {} and {}",
                        err_val(other),
                        err_val(&args[1])
                    )));
                }
            };
            if let Some(val) = h.borrow().get(key) {
                Ok(val.clone())
            } else {
                Ok(LispVal::Nil)
            }
        }
        BuiltinFunc::DeleteKey => {
            // Accepts (delete-key! table key) or CL-style (remhash key table)
            // order, like GETHASH above (issue #246).
            if args.len() != 2 {
                return Err(LispError::Generic(
                    "delete-key/delete-key-bang takes exactly two arguments".to_string(),
                ));
            }
            let (h, key) = match (&args[0], &args[1]) {
                (LispVal::HashTable(h), key) => (h, key),
                (key, LispVal::HashTable(h)) => (h, key),
                (other, _) => {
                    return Err(LispError::Generic(format!(
                        "delete-key/delete-key-bang requires a hash table argument, got {} and {}",
                        err_val(other),
                        err_val(&args[1])
                    )));
                }
            };
            h.borrow_mut().remove(key);
            Ok(LispVal::Symbol(env.intern_symbol("T")))
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
            Ok(LispVal::HashTable(Shared::new(SharedCell::new(hash_map))))
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
pub(super) fn feature_name_arg(args: &[LispVal], who: &str) -> Result<String, LispError> {
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
pub(super) fn require_read_fs(env: &Shared<Environment>) -> Result<(), LispError> {
    if env.feature_enabled("READ-FS") {
        Ok(())
    } else {
        Err(LispError::Generic(
            "READ-FS capability is not enabled (grant it via --capability READ-FS or the host API)"
                .to_string(),
        ))
    }
}

pub(super) fn require_create_fs(env: &Shared<Environment>) -> Result<(), LispError> {
    if env.feature_enabled("CREATE-FS") {
        Ok(())
    } else {
        Err(LispError::Generic(
            "CREATE-FS capability is not enabled (grant it via --capability CREATE-FS or the host API)"
                .to_string(),
        ))
    }
}

pub(super) fn require_temp_fs(env: &Shared<Environment>) -> Result<(), LispError> {
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
pub(super) fn make_temp_path(prefix: &str, suffix: &str) -> std::path::PathBuf {
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
pub(super) fn apply_shell(
    args: &[LispVal],
    env: &Shared<Environment>,
) -> Result<LispVal, LispError> {
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
        car: Shared::new(LispVal::Number(code)),
        cdr: Shared::new(LispVal::Cons {
            car: Shared::new(LispVal::String(stdout)),
            cdr: Shared::new(LispVal::Cons {
                car: Shared::new(LispVal::String(stderr)),
                cdr: Shared::new(LispVal::Nil),
            }),
        }),
    })
}
