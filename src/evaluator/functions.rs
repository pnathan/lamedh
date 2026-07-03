use super::*;
use crate::environment::DynamicBinding;
#[inline(never)]
pub(super) fn make_lambda(
    params: &LispVal,
    body: &LispVal,
    env: &Shared<Environment>,
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
                    let rest_name = rest_p_sym.borrow().name.clone();
                    check_param_name(&rest_name, "lambda")?;
                    rest_param = Some(rest_name);
                    break; // No more params after &rest
                } else {
                    return Err(LispError::Generic(
                        "&rest must be followed by a symbol".to_string(),
                    ));
                }
            } else {
                let name = s.borrow().name.clone();
                check_param_name(&name, "lambda")?;
                params_vec.push(name);
            }
        } else {
            return Err(LispError::Generic(
                "lambda parameters must be symbols".to_string(),
            ));
        }
    }

    let param_ids: Vec<u32> = params_vec
        .iter()
        .map(|name| env.intern_symbol(name).borrow().id)
        .collect();
    let rest_param_id: Option<u32> = rest_param
        .as_ref()
        .map(|name| env.intern_symbol(name).borrow().id);
    // Compile the body once at definition time for the fast execute path.
    let compiled_body = super::compile::compile(body);
    Ok(LispVal::Lambda(Box::new(crate::Lambda {
        params: params_vec,
        rest_param,
        body: Box::new(body.clone()),
        env: env.clone(),
        param_ids,
        rest_param_id,
        compiled: Some(compiled_body),
    })))
}

#[inline(never)]
pub(super) fn make_fexpr(
    params: &LispVal,
    body: &LispVal,
    env: &Shared<Environment>,
) -> Result<LispVal, LispError> {
    let p_list = list_to_vec(params)?;
    let params_vec: Result<Vec<String>, _> = p_list
        .iter()
        .map(|p| {
            if let LispVal::Symbol(s) = p {
                let name = s.borrow().name.clone();
                check_param_name(&name, "fexpr")?;
                Ok(name)
            } else {
                Err(LispError::Generic(
                    "fexpr parameters must be symbols".to_string(),
                ))
            }
        })
        .collect();

    let params_vec = params_vec?;
    let param_ids: Vec<u32> = params_vec
        .iter()
        .map(|name| env.intern_symbol(name).borrow().id)
        .collect();
    Ok(LispVal::Fexpr(Box::new(crate::Fexpr {
        params: params_vec,
        body: Box::new(body.clone()),
        env: env.clone(),
        param_ids,
    })))
}

#[inline(never)]
pub(super) fn make_macro(
    params: &LispVal,
    body: &LispVal,
    env: &Shared<Environment>,
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
                    let rest_name = rest_p_sym.borrow().name.clone();
                    check_param_name(&rest_name, "macro")?;
                    rest_param = Some(rest_name);
                    break; // No more params after &rest
                } else {
                    return Err(LispError::Generic(
                        "&rest must be followed by a symbol".to_string(),
                    ));
                }
            } else {
                let name = s.borrow().name.clone();
                check_param_name(&name, "macro")?;
                params_vec.push(name);
            }
        } else {
            return Err(LispError::Generic(
                "macro parameters must be symbols".to_string(),
            ));
        }
    }

    let param_ids: Vec<u32> = params_vec
        .iter()
        .map(|name| env.intern_symbol(name).borrow().id)
        .collect();
    let rest_param_id: Option<u32> = rest_param
        .as_ref()
        .map(|name| env.intern_symbol(name).borrow().id);
    Ok(LispVal::Macro(Box::new(crate::Macro {
        params: params_vec,
        rest_param,
        body: Box::new(body.clone()),
        env: env.clone(),
        param_ids,
        rest_param_id,
    })))
}

#[inline(never)]
pub(super) fn expand_macro(
    m: &crate::Macro,
    args: &[LispVal],
    _env: &Shared<Environment>,
) -> Result<LispVal, LispError> {
    let macro_env = Environment::new_child(&m.env);
    let has_dyn = macro_env.has_any_dynamic();
    let mut guards: Vec<DynamicBinding> = Vec::new();

    if let Some(rest_param_id) = m.rest_param_id {
        if args.len() < m.params.len() {
            return Err(LispError::Generic(format!(
                "macro expected at least {} arguments, got {}",
                m.params.len(),
                args.len()
            )));
        }
        for (id, arg) in m.param_ids.iter().zip(args.iter()) {
            if has_dyn
                && let Some(sym) = macro_env.symbol_by_id(*id)
                && sym.borrow().is_dynamic
            {
                guards.push(DynamicBinding::install(sym, arg.clone()));
                continue;
            }
            macro_env.set_id(*id, arg.clone());
        }
        let rest_args = vec_to_list(args[m.params.len()..].to_vec());
        if has_dyn
            && let Some(sym) = macro_env.symbol_by_id(rest_param_id)
            && sym.borrow().is_dynamic
        {
            guards.push(DynamicBinding::install(sym, rest_args));
        } else {
            macro_env.set_id(rest_param_id, rest_args);
        }
    } else {
        if m.params.len() != args.len() {
            return Err(LispError::Generic(format!(
                "macro expected {} arguments, got {}",
                m.params.len(),
                args.len()
            )));
        }
        for (id, arg) in m.param_ids.iter().zip(args) {
            if has_dyn
                && let Some(sym) = macro_env.symbol_by_id(*id)
                && sym.borrow().is_dynamic
            {
                guards.push(DynamicBinding::install(sym, arg.clone()));
                continue;
            }
            macro_env.set_id(*id, arg.clone());
        }
    }

    eval(&m.body, &macro_env)
    // guards drops here, restoring any dynamic bindings
}

/// Public entry point for evaluation. Acquires a depth-guard frame (issue #61)
/// Evaluate a single Lisp expression in `env`.
///
/// This is the primary entry point for evaluation.  It acquires a
/// recursion-depth guard and delegates to `run_trampoline`, which uses a
/// trampoline loop for tail-call optimisation: tail positions (`IF` branches,
/// `PROGN` last form, `LET` body, lambda bodies) are handled without growing
/// the Rust call stack.
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
pub fn eval(val: &LispVal, env: &Shared<Environment>) -> Result<LispVal, LispError> {
    // Bound recursion so deep/infinite recursion is a recoverable error rather
    // than a native stack overflow that aborts the whole process (issue #61).
    let _depth_guard = DepthGuard::enter()?;
    run_trampoline(Current::Val(val.clone()), env.clone())
}

/// Entry point for the compiled-code executor (`compile::exec`). Shares the
/// same trampoline as `eval` — see [`Current`] and [`TcoStep::ExecTail`] for
/// why: a compiled lambda's tail call must be able to reuse this loop even
/// when it crosses back into an uncompiled (`Code::Interp`) form, or TCO
/// breaks at the boundary (issue #200 M1', split-trampoline regression).
pub(super) fn exec_entry(
    code: Shared<crate::Code>,
    env: &Shared<Environment>,
) -> Result<LispVal, LispError> {
    let _depth_guard = DepthGuard::enter()?;
    run_trampoline(Current::Code(code), env.clone())
}

/// Represents the outcome of one iteration of the TCO trampoline.
/// Either we have a final value/error, or we have a tail call to continue with.
pub(super) enum TcoStep {
    /// Evaluation is complete; return this value.
    Done(Result<LispVal, LispError>),
    /// Tail call: evaluate `val` in `env` next, reusing this stack frame.
    TailCall(LispVal, Shared<Environment>),
    /// Tail call that also transfers dynamic-binding RAII guards to the
    /// trampoline loop.  The guards accumulate across tail calls and are
    /// restored as a LIFO stack when the trampoline finally returns a value.
    /// This lets TCO work correctly for tail-recursive functions that rebind
    /// dynamic (`*special*`) variables: each iteration pushes its guard onto
    /// the trampoline-owned vec, and the whole chain is unwound on return.
    TailCallWithGuards(LispVal, Shared<Environment>, Vec<DynamicBinding>),
    /// Tail call into the compiled-code executor: continue stepping this
    /// `Code` node in `env` on the *same* trampoline instead of recursing
    /// into a fresh `exec()`/`eval()` call. Without this, a compiled lambda
    /// tail-calling an uncompiled form (and back) grows the native stack and
    /// the eval-depth counter by one per iteration — see issue #200.
    ExecTail(Shared<crate::Code>, Shared<Environment>),
}

/// Which representation the trampoline is currently stepping. A single loop
/// drives both the tree-walker (`Val`) and the compiled executor (`Code`) so
/// that a tail call crossing between them costs no native stack depth.
enum Current {
    Val(LispVal),
    Code(Shared<crate::Code>),
}

/// Internal trampoline evaluator. Runs a loop that reuses the current Rust
/// stack frame for tail calls, achieving proper TCO without consuming extra
/// native stack depth for each Lisp tail-recursive call — whether the tail
/// position holds a raw AST form or a compiled `Code` node.
///
/// All non-tail recursive calls (e.g. evaluating an IF condition, evaluating
/// function arguments) still go through the public `eval()`/`exec()` so that
/// the depth guard is correctly applied to non-tail frames.
fn run_trampoline(
    initial: Current,
    initial_env: Shared<Environment>,
) -> Result<LispVal, LispError> {
    let mut current: Current = initial;
    let mut current_env: Shared<Environment> = initial_env;
    // Dynamic-binding guards accumulated across tail calls (e.g. a tail-
    // recursive function that rebinds a `*special*` each iteration). They are
    // restored on *every* exit path by `DynamicGuardStack`'s Drop:
    //   - `Done`: dropped after the result value is produced,
    //   - error/THROW via `?`: dropped as the stack unwinds.
    // Restoration is LIFO (last installed → first restored), which a plain
    // `Vec` drop would get wrong for nested same-symbol rebindings. `Val` and
    // `Code` steps share this one stack, so a dynamic binding installed by a
    // tree-walker `TailCallWithGuards` stays live across a subsequent
    // `ExecTail` into compiled code, exactly as it would within pure
    // tree-walker recursion.
    let mut guards = DynamicGuardStack(Vec::new());

    loop {
        // Each iteration computes a TcoStep, then either returns or loops.
        // All borrows of `current`/`current_env` are scoped inside this block
        // so they are released before we potentially assign to them.
        let step = {
            let env = &current_env;
            match &current {
                Current::Val(val) => eval_step(val, env),
                Current::Code(code) => exec_step(code, env),
            }
        }?;

        match step {
            TcoStep::Done(result) => return result,
            TcoStep::TailCall(new_val, new_env) => {
                current = Current::Val(new_val);
                current_env = new_env;
            }
            TcoStep::TailCallWithGuards(new_val, new_env, new_guards) => {
                current = Current::Val(new_val);
                current_env = new_env;
                // Collapse to O(distinct symbols): if this symbol already has a
                // guard in the stack, the existing (older) guard holds the real
                // pre-loop saved value. The new guard saved the previous
                // iteration's installed value — drop it without restoring.
                for mut g in new_guards {
                    let id = g.symbol_id();
                    if guards.0.iter().any(|existing| existing.symbol_id() == id) {
                        // Drop the saved LispVal to avoid leaking it, then
                        // suppress the Drop impl (which would restore the
                        // intermediate value we don't want).
                        drop(g.take_saved());
                        std::mem::forget(g);
                    } else {
                        guards.0.push(g);
                    }
                }
            }
            TcoStep::ExecTail(new_code, new_env) => {
                current = Current::Code(new_code);
                current_env = new_env;
            }
        }
    }
}

/// Owns the dynamic-binding guards accumulated by the trampoline loop and
/// restores them in LIFO order (last installed → first restored) on *any* exit
/// path: normal return, deferred `Apply`, or error/THROW unwinding via `?`.
/// A plain `Vec<DynamicBinding>` drop restores front-to-back, which corrupts
/// nested same-symbol rebindings (e.g. `(let ((*x* 1)) (let ((*x* 2)) ...))`),
/// so we pop from the back explicitly. The `Vec` does not allocate until the
/// first guard is pushed, keeping the common no-dynamics path zero-cost.
struct DynamicGuardStack(Vec<DynamicBinding>);

impl Drop for DynamicGuardStack {
    fn drop(&mut self) {
        while let Some(g) = self.0.pop() {
            drop(g);
        }
    }
}

/// Build the `Native` membrane entry for a typed function `name`.
/// Issue #216: `STORE`/`ASET` promise in-place mutation visible to every
/// reference to an array, but the typed runtime's arena buffer is a *copy*
/// of a `LispVal::Array` argument's contents (`Value::to_word`'s `Array`
/// arm always allocates a fresh buffer). Without this, a `store` inside a
/// `defun-typed` body silently never reached the caller's array, even
/// across repeated calls on the same object. This writes each flat-scalar-
/// array argument's post-call contents back into the *original*
/// `LispVal::Array`'s backing `RefCell` in place (preserving its `Rc`
/// identity — every other reference to the same array object sees the
/// update, matching ordinary interpreted `store`'s contract).
///
/// Scope, matching `Jit::call_with_array_writeback`'s own documented
/// boundary: only top-level, flat (non-nested) scalar-element arrays are
/// written back. A `LispVal::String` passed where `(array char)` was
/// declared type-checks the same as a genuine array here, but `String` has
/// no interior mutability at all — skipped, not silently corrupted. If the
/// same array object is passed as two distinct arguments, this is
/// last-writer-wins in argument order (matching classic value-result/
/// copy-in-copy-out semantics, not true aliasing) — a documented,
/// intentional divergence from in-place mutation for that specific case,
/// not a bug.
fn apply_array_writeback(
    args: &[LispVal],
    updated: Vec<Option<crate::jit::Value>>,
    env: &Shared<Environment>,
) {
    for (orig, upd) in args.iter().zip(updated) {
        if let (LispVal::Array(rc), Some(crate::jit::Value::Array(items))) = (orig, upd) {
            // The element type isn't needed here: `typed_to_lispval` only
            // needs it to distinguish `(array char)` (-> String) from a
            // genuine element array, and `orig` being `LispVal::Array`
            // already proves this wasn't the string case (see
            // `lispval_to_typed`'s `Ty::Array` arm).
            let new_items: Vec<LispVal> = items
                .into_iter()
                .map(|it| match it {
                    crate::jit::Value::Int(n) => LispVal::Number(n),
                    crate::jit::Value::Float(f) => LispVal::Float(f),
                    crate::jit::Value::Bool(b) => {
                        if b {
                            LispVal::Symbol(env.intern_symbol("T"))
                        } else {
                            LispVal::Nil
                        }
                    }
                    crate::jit::Value::Char(b) => LispVal::Char(b),
                    // Excluded by `is_flat_scalar_array`: only scalar
                    // elements reach a flat array's write-back.
                    crate::jit::Value::Array(_) | crate::jit::Value::Struct(_) => {
                        unreachable!("flat scalar array write-back produced a compound element")
                    }
                })
                .collect();
            *rc.borrow_mut() = new_items;
        }
    }
}

pub(super) fn make_typed_native(name: String) -> LispVal {
    LispVal::Native(Shared::new(
        move |args: &[LispVal], env: &Shared<Environment>| -> Result<LispVal, LispError> {
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
            match env.jit_call_with_array_writeback(&name, &vals) {
                Some(Ok((v, updated))) => {
                    apply_array_writeback(args, updated, env);
                    Ok(typed_to_lispval(v, &ret, env))
                }
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
    LispVal::Native(Shared::new(
        move |args: &[LispVal], env: &Shared<Environment>| -> Result<LispVal, LispError> {
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
                if fits
                    && let Some(Ok((v, updated))) = env.jit_call_with_array_writeback(&name, &vals)
                {
                    apply_array_writeback(args, updated, env);
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
pub(super) fn optimize_function(name: &str, env: &Shared<Environment>) -> String {
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
    env: &Shared<Environment>,
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
pub(super) fn check_function(name: &str, env: &Shared<Environment>) -> String {
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
    env: &Shared<Environment>,
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
                LispVal::Array(Shared::new(SharedCell::new(lst)))
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
                LispVal::Struct(Shared::new(StructObj {
                    type_name: def.name.clone(),
                    fields: values,
                }))
            }
            _ => LispVal::Nil,
        },
    }
}
