use super::*;
#[inline(never)]
pub(super) fn make_lambda(
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
pub(super) fn make_fexpr(
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
pub(super) fn make_macro(
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
pub(super) fn expand_macro(
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
pub(super) enum TcoStep {
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
pub(super) fn eval_impl(
    initial_val: LispVal,
    initial_env: Rc<Environment>,
) -> Result<LispVal, LispError> {
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
pub(super) fn make_typed_native(name: String) -> LispVal {
    LispVal::Native(Rc::new(
        move |args: &[LispVal], env: &Rc<Environment>| -> Result<LispVal, LispError> {
            let (ptys, ret) = env.jit_signature(&name).ok_or_else(|| {
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
                vals.push(lispval_to_typed(a, ty).map_err(LispError::Generic)?);
            }
            match env.jit_call(&name, &vals) {
                Some(Ok(v)) => Ok(typed_to_lispval(v, &ret, env)),
                Some(Err(e)) => Err(LispError::Generic(e)),
                None => Err(LispError::Generic(format!(
                    "typed function {name} is not defined"
                ))),
            }
        },
    ))
}

/// Build an **auto-typed** membrane entry: a function that tries the typed
/// (native) fast path and silently falls back to the original dynamic closure
/// when the arguments do not fit the inferred signature. This is what makes HM
/// firing under `defun` *transparent* — calls that match the inferred types run
/// compiled; everything else (wrong arity/shape, untyped values) behaves exactly
/// as the un-optimized function did.
pub(super) fn make_auto_typed_native(name: String, fallback: LispVal) -> LispVal {
    LispVal::Native(Rc::new(
        move |args: &[LispVal], env: &Rc<Environment>| -> Result<LispVal, LispError> {
            if let Some((ptys, ret)) = env.jit_signature(&name)
                && args.len() == ptys.len()
            {
                let mut vals = Vec::with_capacity(args.len());
                let mut fits = true;
                for (a, ty) in args.iter().zip(ptys.iter()) {
                    match lispval_to_typed(a, ty) {
                        Ok(v) => vals.push(v),
                        Err(_) => {
                            fits = false;
                            break;
                        }
                    }
                }
                if fits && let Some(Ok(v)) = env.jit_call(&name, &vals) {
                    return Ok(typed_to_lispval(v, &ret, env));
                }
            }
            // Fall back to the original dynamic definition.
            apply(&fallback, args, env)
        },
    ))
}

/// Try to type-compile the function bound to `name` (a plain lambda) and, on
/// success, swap its binding for an auto-typed membrane. Best-effort and silent:
/// any reason it cannot be typed (variadic, untyped body, ambiguous types) just
/// leaves the dynamic definition in place.
pub(super) fn optimize_function(name: &str, env: &Rc<Environment>) -> String {
    let Some(LispVal::Lambda(lam)) = env.get(name) else {
        return format!("{name} is not optimizable (not a lambda)");
    };
    // `&REST` functions are outside the monomorphic typed core.
    if lam.rest_param.is_some() {
        return format!("{name} is not optimizable (variadic &rest)");
    }
    // A multi-form body is a `(PROGN f1 f2 ...)`; unwrap it to the form list.
    let body_forms: Vec<LispVal> = match lam.body.as_ref() {
        LispVal::Cons { car, cdr } if matches!(car.as_ref(), LispVal::Symbol(s) if s.borrow().name == "PROGN") => {
            list_to_vec(cdr).unwrap_or_default()
        }
        other => vec![other.clone()],
    };
    if body_forms.is_empty() {
        return format!("{name} has an empty body");
    }
    // One pass: check (reports type errors even when nothing compiles), then
    // compile if compileable.
    match env.jit_analyze_untyped(name, &lam.params, &body_forms) {
        crate::jit::Analysis::Native(scheme) => {
            // Keep the original closure as the fallback for non-matching calls.
            env.set(
                name.to_string(),
                make_auto_typed_native(name.to_string(), LispVal::Lambda(lam)),
            );
            format!("{name} : {scheme}  [native]")
        }
        crate::jit::Analysis::Checked(scheme) => {
            format!("{name} : {scheme}  [checked, dynamic]")
        }
        crate::jit::Analysis::TypeError(e) => format!("type error in {name}: {e}"),
    }
}

/// Extract a plain function's parameter names and body forms (a `PROGN` is
/// unwrapped). `None` for anything that isn't a non-variadic lambda.
pub(super) fn lambda_params_body(
    name: &str,
    env: &Rc<Environment>,
) -> Option<(Vec<String>, Vec<LispVal>)> {
    let LispVal::Lambda(lam) = env.get(name)? else {
        return None;
    };
    if lam.rest_param.is_some() {
        return None;
    }
    let body_forms: Vec<LispVal> = match lam.body.as_ref() {
        LispVal::Cons { car, cdr } if matches!(car.as_ref(), LispVal::Symbol(s) if s.borrow().name == "PROGN") => {
            list_to_vec(cdr).unwrap_or_default()
        }
        other => vec![other.clone()],
    };
    if body_forms.is_empty() {
        return None;
    }
    Some((lam.params.clone(), body_forms))
}

/// Type-check the function bound to `name` (the non-compiled checker, #162) and
/// return a human-readable result: its inferred type scheme, or a type error.
pub(super) fn check_function(name: &str, env: &Rc<Environment>) -> String {
    // The live value-cell binding is authoritative: that is what a call actually
    // runs. If the function has been (re)defined as a *plain* lambda, a typed
    // registry entry of the same name is stale (e.g. an earlier `defun*`/
    // `defun-typed` that was overwritten by an untyped definition) and must not
    // be reported. Only when the live binding is not a plain lambda — a typed
    // native membrane, an auto-typed membrane, or absent — do we trust the typed
    // registry signature.
    let live_is_plain_lambda = matches!(env.get(name), Some(LispVal::Lambda(_)));
    if !live_is_plain_lambda && let Some((params, ret)) = env.jit_named_signature(name) {
        let param_str = params
            .iter()
            .map(|(n, t)| format!("({n} {})", crate::jit::ty_name(t)))
            .collect::<Vec<_>>()
            .join(" ");
        let compiled = matches!(env.jit_is_compiled(name), Some(true));
        let status = if compiled { "compiled" } else { "interpreted" };
        return format!(
            "{name} : ({param_str}) -> {} [{status}]",
            crate::jit::ty_name(&ret)
        );
    }
    match lambda_params_body(name, env) {
        Some((params, body)) => match env.jit_check_untyped(name, &params, &body) {
            Ok(scheme) => format!("{name} : {scheme}"),
            Err(e) => format!("type error in {name}: {e}"),
        },
        None => format!("{name} is not a checkable function (variadic or not a lambda)"),
    }
}

/// Coerce a `LispVal` to a typed [`crate::jit::Value`] for a parameter of type
/// `ty`. `int64` accepts `Number`; `float64` accepts `Float` or widens `Number`;
/// `bool` follows Lisp truthiness (`nil` is false, everything else true).
pub(super) fn char_byte_from_number(n: i64, context: &str) -> Result<u8, String> {
    u8::try_from(n).map_err(|_| format!("{context}: {n} out of range 0-255"))
}

pub(super) fn lispval_to_typed(
    lv: &LispVal,
    ty: &crate::jit::Ty,
) -> Result<crate::jit::Value, String> {
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
            LispVal::Number(n) => Ok(Value::Char(char_byte_from_number(*n, "char argument")?)),
            other => Err(format!("expected char argument, got {other:?}")),
        },
        // A `(array char)` parameter accepts a string as its UTF-8 bytes (the
        // #137 membrane); any array accepts a Lisp array, converted element-wise.
        Ty::Array(elem) => match lv {
            LispVal::String(s) if matches!(**elem, Ty::Char) => {
                Ok(Value::Array(s.bytes().map(Value::Char).collect()))
            }
            LispVal::Array(a) => {
                let items = a.borrow();
                let mut out = Vec::with_capacity(items.len());
                for it in items.iter() {
                    out.push(lispval_to_typed(it, elem)?);
                }
                Ok(Value::Array(out))
            }
            other => Err(format!("expected array argument, got {other:?}")),
        },
        Ty::Struct(def) => match lv {
            LispVal::Struct(obj) => {
                if obj.type_name != def.name {
                    return Err(format!(
                        "expected struct {}, got {}",
                        def.name, obj.type_name
                    ));
                }
                if obj.fields.len() != def.fields.len() {
                    return Err(format!(
                        "expected {} fields for struct, got {}",
                        def.fields.len(),
                        obj.fields.len()
                    ));
                }
                let mut out = Vec::with_capacity(obj.fields.len());
                for (it, (_, ft)) in obj.fields.iter().zip(def.fields.iter()) {
                    out.push(lispval_to_typed(it, ft)?);
                }
                Ok(Value::Struct(out))
            }
            other => Err(format!("expected struct {}, got {other:?}", def.name)),
        },
        // Only compileable types back a native edition, so non-compileable
        // types (#162) and unresolved variables never reach the membrane.
        _ => Err(format!(
            "type {} is not compileable at the typed membrane",
            crate::jit::ty_name(ty)
        )),
    }
}

/// Re-box a typed [`crate::jit::Value`] result as a `LispVal`, type-directed:
/// `bool` → `T`/`NIL`, `(array char)` → a string, other arrays → a Lisp array,
/// structs → a nominal typed struct value.
pub(super) fn typed_to_lispval(
    v: crate::jit::Value,
    ty: &crate::jit::Ty,
    env: &Rc<Environment>,
) -> LispVal {
    use crate::jit::{Ty, Value};
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
        Value::Array(items) => match ty {
            Ty::Array(elem) if matches!(**elem, Ty::Char) => {
                let bytes: Vec<u8> = items
                    .iter()
                    .map(|x| match x {
                        Value::Char(b) => *b,
                        Value::Int(n) => *n as u8,
                        _ => 0,
                    })
                    .collect();
                LispVal::String(String::from_utf8_lossy(&bytes).into_owned())
            }
            Ty::Array(elem) => {
                let lst: Vec<LispVal> = items
                    .into_iter()
                    .map(|x| typed_to_lispval(x, elem, env))
                    .collect();
                LispVal::Array(Rc::new(std::cell::RefCell::new(lst)))
            }
            _ => LispVal::Nil,
        },
        Value::Struct(fields) => match ty {
            Ty::Struct(def) => {
                let values: Vec<LispVal> = fields
                    .into_iter()
                    .zip(def.fields.iter())
                    .map(|(fv, (_, ft))| typed_to_lispval(fv, ft, env))
                    .collect();
                LispVal::Struct(Rc::new(StructObj {
                    type_name: def.name.clone(),
                    fields: values,
                }))
            }
            _ => LispVal::Nil,
        },
    }
}
