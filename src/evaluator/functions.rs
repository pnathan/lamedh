use super::*;
use crate::environment::DynamicBinding;
#[inline(never)]
pub(super) fn make_lambda(
    params: &LispVal,
    body: &LispVal,
    env: &Shared<Environment>,
) -> Result<LispVal, LispError> {
    // Compile the body once at definition time for the fast execute path.
    let compiled_body = super::compile::compile_lambda_body(params, body);
    build_lambda(params, body, compiled_body, env)
}

/// Construct a [`crate::Lambda`] from `params`/`body` and an already-compiled
/// body, capturing `env` as the closure's definition environment.
///
/// This is `make_lambda` minus the `compile` step: the compiled body is
/// supplied by the caller so that a lambda literal embedded in a compiled
/// function body (`Code::MakeLambda`) can reuse a body compiled once at the
/// enclosing definition's compile time instead of recompiling it on every
/// construction (issue #233).
/// Walk a parameter list that may be proper or dotted. A dotted symbol
/// tail is the classic rest-parameter shorthand — `(a b . more)` means
/// `(a b &rest more)`. Returns the proper prefix and the dotted tail
/// symbol, if any.
fn param_list_to_vec(
    params: &LispVal,
    what: &str,
) -> Result<(Vec<LispVal>, Option<LispVal>), LispError> {
    let mut vec = Vec::new();
    let mut current = params;
    while let LispVal::Cons { car, cdr } = current {
        vec.push(car.as_ref().clone());
        current = cdr;
    }
    match current {
        LispVal::Nil => Ok((vec, None)),
        LispVal::Symbol(_) => Ok((vec, Some(current.clone()))),
        other => Err(LispError::Generic(format!(
            "{what} parameter list must be a proper list or end in a rest \
             symbol, got tail {}",
            err_val(other)
        ))),
    }
}

#[inline(never)]
pub(super) fn build_lambda(
    params: &LispVal,
    body: &LispVal,
    compiled_body: Shared<crate::Code>,
    env: &Shared<Environment>,
) -> Result<LispVal, LispError> {
    let (p_list, dotted_tail) = param_list_to_vec(params, "lambda")?;
    let mut params_vec = Vec::new();
    // Ids come from the parameter symbol objects themselves (via
    // `binder_id`), not from re-interning the names — re-interning minted a
    // fresh id for gensym parameters, leaving the body's occurrence of the
    // same symbol unresolvable (issue #285).
    let mut param_ids: Vec<u32> = Vec::new();
    let mut rest_param = None;
    let mut rest_param_id: Option<u32> = None;
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
                    rest_param_id = Some(env.binder_id(rest_p_sym));
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
                param_ids.push(env.binder_id(s));
            }
        } else {
            return Err(LispError::Generic(
                "lambda parameters must be symbols".to_string(),
            ));
        }
    }
    if let Some(LispVal::Symbol(tail_sym)) = &dotted_tail {
        if rest_param.is_some() {
            return Err(LispError::Generic(
                "parameter list has both &rest and a dotted tail".to_string(),
            ));
        }
        let rest_name = tail_sym.borrow().name.clone();
        check_param_name(&rest_name, "lambda")?;
        rest_param = Some(rest_name);
        rest_param_id = Some(env.binder_id(tail_sym));
    }
    Ok(LispVal::Lambda(Box::new(crate::Lambda {
        params: params_vec,
        rest_param,
        body: Box::new(body.clone()),
        env: env.clone(),
        param_routing: crate::Shared::new(param_ids.clone()),
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
    let (p_list, dotted_tail) = param_list_to_vec(params, "fexpr")?;
    if dotted_tail.is_some() {
        return Err(LispError::Generic(
            "fexpr parameter list is (args env) — it takes no rest tail".to_string(),
        ));
    }
    // Names and ids collected together from the symbol objects — see the
    // issue #285 note in `build_lambda`.
    let pairs: Result<Vec<(String, u32)>, _> = p_list
        .iter()
        .map(|p| {
            if let LispVal::Symbol(s) = p {
                let name = s.borrow().name.clone();
                check_param_name(&name, "fexpr")?;
                Ok((name, env.binder_id(s)))
            } else {
                Err(LispError::Generic(
                    "fexpr parameters must be symbols".to_string(),
                ))
            }
        })
        .collect();

    let pairs = pairs?;
    let params_vec: Vec<String> = pairs.iter().map(|(n, _)| n.clone()).collect();
    let param_ids: Vec<u32> = pairs.iter().map(|(_, id)| *id).collect();
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
    let (p_list, dotted_tail) = param_list_to_vec(params, "macro")?;
    let mut params_vec = Vec::new();
    // Ids from the symbol objects via `binder_id` — see build_lambda (#285).
    let mut param_ids: Vec<u32> = Vec::new();
    let mut rest_param = None;
    let mut rest_param_id: Option<u32> = None;
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
                    rest_param_id = Some(env.binder_id(rest_p_sym));
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
                param_ids.push(env.binder_id(s));
            }
        } else {
            return Err(LispError::Generic(
                "macro parameters must be symbols".to_string(),
            ));
        }
    }
    if let Some(LispVal::Symbol(tail_sym)) = &dotted_tail {
        if rest_param.is_some() {
            return Err(LispError::Generic(
                "parameter list has both &rest and a dotted tail".to_string(),
            ));
        }
        let rest_name = tail_sym.borrow().name.clone();
        check_param_name(&rest_name, "macro")?;
        rest_param = Some(rest_name);
        rest_param_id = Some(env.binder_id(tail_sym));
    }
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
    // Backtrace frame slot for this (non-tail) evaluation boundary.
    let bt_prev = crate::evaluator::core::bt_enter();
    let r = run_trampoline_inner(initial, initial_env);
    crate::evaluator::core::bt_exit(bt_prev, r.is_ok());
    r
}

fn run_trampoline_inner(
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
        // Kernel fuel (issue #284 Phase 2): one step per trampoline
        // iteration — covering every eval/exec entry and every TCO tail
        // step. Unarmed cost is a thread-local load + predictable branch.
        crate::evaluator::core::charge_kernel_fuel()?;
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
                    crate::jit::Value::Array(_)
                    | crate::jit::Value::Struct(_)
                    | crate::jit::Value::TypedArray(_) => {
                        unreachable!("flat scalar array write-back produced a compound element")
                    }
                })
                .collect();
            *rc.borrow_mut() = new_items;
        }
        // A `LispVal::TypedArray` argument was passed to the native call as a
        // raw pointer into its own buffer (`Value::to_word`'s `TypedArray`
        // arm) — the callee's `store`/`aset` already mutated it in place, so
        // (unlike `LispVal::Array` above) no copy-back is needed here.
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
                Some(Ok((v, updated, flags))) => {
                    // Set OVERFLOW before signalling a division error: the
                    // tree-walker evaluates left-to-right, so an inner overflow
                    // leaves the flag observable even when a later division by
                    // zero raises — match that ordering exactly (#228).
                    if flags.overflow {
                        env.set_flag("OVERFLOW");
                    }
                    if flags.div_by_zero {
                        return Err(LispError::Generic("Division by zero".to_string()));
                    }
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
            // Under an active fuel fence the native fast path is skipped
            // (#320): compiled internal loops never return to the metered
            // trampoline, so fenced calls take the interpreted fallback,
            // which charges per step like everything else.
            if crate::evaluator::core::kernel_fuel_remaining().is_some() {
                return apply(&fallback, args, env);
            }
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
                    && let Some(Ok((v, updated, flags))) =
                        env.jit_call_with_array_writeback(&name, &vals)
                {
                    // Set OVERFLOW before signalling a division error: the
                    // tree-walker evaluates left-to-right, so an inner overflow
                    // leaves the flag observable even when a later division by
                    // zero raises — match that ordering exactly (#228).
                    if flags.overflow {
                        env.set_flag("OVERFLOW");
                    }
                    if flags.div_by_zero {
                        return Err(LispError::Generic("Division by zero".to_string()));
                    }
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
        // One-door defun may already have compiled the name: report the
        // live typed signature instead of a useless "not a lambda".
        if let Some((params, ret)) = env.jit_named_signature(name) {
            let args = params
                .iter()
                .map(|(_, t)| crate::jit::ty_name(t))
                .collect::<Vec<_>>()
                .join(" ");
            return format!(
                "{name} : (-> ({args}) {})  [native, already compiled]",
                crate::jit::ty_name(&ret)
            );
        }
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
            // Keep the original closure as the fallback for non-matching
            // calls. Bind GLOBALLY: jit-optimize semantically rebinds the
            // function's definition — when invoked from inside a call frame
            // (one-door defun's $defun-auto-compile), a frame-local env.set
            // would bind the membrane into the transient frame and evaporate.
            // Preserve introspection (the historical objection to
            // auto-compiling defuns): record the original lambda source on
            // the plist, where see-source looks first.
            let source_form = {
                let mut parts = vec![LispVal::Symbol(env.intern_symbol("LAMBDA"))];
                let param_syms: Vec<LispVal> = lam
                    .params
                    .iter()
                    .map(|p| LispVal::Symbol(env.intern_symbol(p)))
                    .collect();
                parts.push(vec_to_list(param_syms));
                parts.push((*lam.body).clone());
                vec_to_list(parts)
            };
            env.intern_symbol(name)
                .borrow_mut()
                .plist
                .insert("source-form".to_string(), source_form);
            env.global_set(
                name,
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
            other => Err(format!("expected int64 argument, got {}", err_val(other))),
        },
        Ty::Float64 => match lv {
            LispVal::Float(f) => Ok(Value::Float(*f)),
            LispVal::Number(n) => Ok(Value::Float(*n as f64)),
            other => Err(format!("expected float64 argument, got {}", err_val(other))),
        },
        Ty::Bool => Ok(Value::Bool(!matches!(lv, LispVal::Nil))),
        Ty::Char => match lv {
            LispVal::Char(b) => Ok(Value::Char(*b)),
            LispVal::Number(n) => Ok(Value::Char(char_byte_from_number(*n, "char argument")?)),
            other => Err(format!("expected char argument, got {}", err_val(other))),
        },
        // A `(array char)` parameter accepts a string as its UTF-8 bytes (the
        // #137 membrane); any array accepts a Lisp array, converted element-wise.
        Ty::Array(elem) => match lv {
            LispVal::String(s) if matches!(**elem, Ty::Char) => {
                Ok(Value::Array(s.bytes().map(Value::Char).collect()))
            }
            // Zero-copy path: a `LispVal::TypedArray` whose element type
            // matches crosses as `Value::TypedArray`, letting
            // `Value::to_word` hand the native call a pointer straight into
            // its own buffer instead of copying element-by-element.
            LispVal::TypedArray(ta) if crate::jit::elem_ty_matches(ta.elem, elem) => {
                Ok(Value::TypedArray(ta.clone()))
            }
            LispVal::TypedArray(ta) => Err(format!(
                "expected array of {}, got typed array of {}",
                crate::jit::ty_name(elem),
                ta.elem
            )),
            LispVal::Array(a) => {
                let items = a.borrow();
                let mut out = Vec::with_capacity(items.len());
                for it in items.iter() {
                    out.push(lispval_to_typed(it, elem)?);
                }
                Ok(Value::Array(out))
            }
            other => Err(format!("expected array argument, got {}", err_val(other))),
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
            other => Err(format!(
                "expected struct {}, got {}",
                def.name,
                err_val(other)
            )),
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
        // `from_word` never produces this on a return path (it always
        // rebuilds a plain `Value::Array`), but it can flow through as a
        // pass-through argument value in other call shapes, so round-trip it
        // rather than asserting unreachable.
        Value::TypedArray(ta) => LispVal::TypedArray(ta),
    }
}

/// Structured checker verdict for `name`, as data (the `see-type` builtin):
///
/// - `(TYPED (-> (t ...) ret) COMPILED|INTERPRETED)` — registered typed function
/// - `(CHECKED scheme)` — plain lambda the checker accepts; `scheme` is the
///   inferred type as a readable form, e.g. `(FORALL (A) (-> (A A) BOOL))`
/// - `(TYPE-ERROR "msg")` — the checker rejects it
/// - `(DYNAMIC "reason")` — variadic, a builtin, or not a function
///
/// The classification burden (e.g. distinguishing informative from vacuous
/// schemes) lives in the Lisp layer; this only reports what the checker knows,
/// structurally, so no consumer has to parse the human-readable string.
/// Structured tier explanation for `name` (the `explain-compile` builtin):
/// an alist of (TIER . symbol) plus, when relevant, (SIGNATURE . form) /
/// (SCHEME . form) / (BLOCKER . string). The BLOCKER is the concrete,
/// actionable reason the definition is not natively compiled — e.g. the
/// exact type the codegen path rejected — so "why is this slow" is a
/// dialogue instead of a mystery.
pub(super) fn explain_compile_form(name: &str, env: &Shared<Environment>) -> LispVal {
    let sym = |s: &str| LispVal::Symbol(env.intern_symbol(s));
    let pair = |k: &str, v: LispVal| LispVal::Cons {
        car: Shared::new(sym(k)),
        cdr: Shared::new(v),
    };
    let report = |items: Vec<LispVal>| vec_to_list(items);

    // Already registered in the typed island?
    let live_is_plain_lambda = matches!(env.get(name), Some(LispVal::Lambda(_)));
    if !live_is_plain_lambda && let Some((params, ret)) = env.jit_named_signature(name) {
        let args = params
            .iter()
            .map(|(_, t)| crate::jit::ty_name(t))
            .collect::<Vec<_>>()
            .join(" ");
        let sig = format!("(-> ({args}) {})", crate::jit::ty_name(&ret));
        let sig_form = crate::reader::read(&sig, env).unwrap_or(LispVal::Nil);
        let compiled = matches!(env.jit_is_compiled(name), Some(true));
        return report(vec![
            pair(
                "TIER",
                sym(if compiled {
                    "COMPILED"
                } else {
                    "TYPED-INTERPRETED"
                }),
            ),
            pair("SIGNATURE", sig_form),
        ]);
    }

    // Pinned away from the compiler?
    let pinned = env
        .intern_symbol(name)
        .borrow()
        .plist
        .contains_key("no-compile");

    match lambda_params_body(name, env) {
        Some((params, body)) => {
            let checked = match env.jit_check_untyped(name, &params, &body) {
                Ok(scheme) => scheme,
                Err(e) => {
                    return report(vec![
                        pair("TIER", sym("TYPE-ERROR")),
                        pair("BLOCKER", LispVal::String(e)),
                    ]);
                }
            };
            let scheme_form =
                crate::reader::read(&checked, env).unwrap_or(LispVal::String(checked.clone()));
            let mut items = vec![pair("TIER", sym("CHECKED")), pair("SCHEME", scheme_form)];
            if pinned {
                items.push(pair(
                    "BLOCKER",
                    LispVal::String(
                        "pinned to the interpreter by (declare (no-compile)) / declaim".to_string(),
                    ),
                ));
                return report(items);
            }
            match env.jit_compile_reason(name, &params, &body) {
                Ok(()) => items.push(pair(
                    "BLOCKER",
                    LispVal::String(
                        "none — natively compileable; (jit-optimize name) installs it".to_string(),
                    ),
                )),
                Err(reason) => items.push(pair("BLOCKER", LispVal::String(reason))),
            }
            report(items)
        }
        None => report(vec![
            pair("TIER", sym("DYNAMIC")),
            pair(
                "BLOCKER",
                LispVal::String("variadic or not a plain lambda".to_string()),
            ),
        ]),
    }
}

pub(super) fn see_type_form(name: &str, env: &Shared<Environment>) -> LispVal {
    let sym = |s: &str| LispVal::Symbol(env.intern_symbol(s));
    let live_is_plain_lambda = matches!(env.get(name), Some(LispVal::Lambda(_)));
    if !live_is_plain_lambda && let Some((params, ret)) = env.jit_named_signature(name) {
        let args = params
            .iter()
            .map(|(_, t)| crate::jit::ty_name(t))
            .collect::<Vec<_>>()
            .join(" ");
        let sig = format!("(-> ({args}) {})", crate::jit::ty_name(&ret));
        let sig_form = crate::reader::read(&sig, env).unwrap_or(LispVal::Nil);
        let status = if matches!(env.jit_is_compiled(name), Some(true)) {
            "COMPILED"
        } else {
            "INTERPRETED"
        };
        return vec_to_list(vec![sym("TYPED"), sig_form, sym(status)]);
    }
    // A **declared** scheme (experimental rows): an axiom asserted by the Lisp
    // layer (e.g. a row-polymorphic concept accessor). Reported distinctly —
    // the checker trusts it at call sites but never derived it from the body.
    if let Some(rendered) = env.jit_declared_scheme(name) {
        let form = crate::reader::read(&rendered, env).unwrap_or(LispVal::String(rendered));
        return vec_to_list(vec![sym("DECLARED"), form]);
    }
    match lambda_params_body(name, env) {
        Some((params, body)) => match env.jit_check_untyped(name, &params, &body) {
            Ok(scheme) => {
                let form =
                    crate::reader::read(&scheme, env).unwrap_or(LispVal::String(scheme.clone()));
                vec_to_list(vec![sym("CHECKED"), form])
            }
            Err(e) => vec_to_list(vec![sym("TYPE-ERROR"), LispVal::String(e)]),
        },
        None => vec_to_list(vec![
            sym("DYNAMIC"),
            LispVal::String("variadic or not a plain lambda".to_string()),
        ]),
    }
}

/// The plist key `defun*` records an inference-failure reason under (loud
/// type inference): set on a fallback-to-lambda, cleared on a successful
/// typed (re)definition. See `eval_defun_star`.
pub(super) const WHY_NOT_TYPED_KEY: &str = "why-not-typed";

/// `(signature 'fn)` — the inferred type signature of a typed function as a
/// readable sexpr, e.g. `(INT64 INT64 -> INT64)`; `NIL` for an untyped
/// function (a plain lambda, or no such function at all). Loud type
/// inference (#134 follow-up): the live value-cell binding is authoritative,
/// exactly like `check_function`/`see_type_form` — a typed registry entry
/// left behind by an earlier `defun*`/`defun-typed` that was since
/// overwritten with a plain lambda must not be reported as still typed.
pub(super) fn signature_form(name: &str, env: &Shared<Environment>) -> LispVal {
    let live_is_plain_lambda = matches!(env.get(name), Some(LispVal::Lambda(_)));
    if live_is_plain_lambda {
        return LispVal::Nil;
    }
    match env.jit_named_signature(name) {
        Some((params, ret)) => {
            let args = params
                .iter()
                .map(|(_, t)| crate::jit::ty_name(t))
                .collect::<Vec<_>>()
                .join(" ");
            let sig = if args.is_empty() {
                format!("(-> {})", crate::jit::ty_name(&ret))
            } else {
                format!("({args} -> {})", crate::jit::ty_name(&ret))
            };
            crate::reader::read(&sig, env).unwrap_or(LispVal::Nil)
        }
        None => LispVal::Nil,
    }
}

/// `(compiled-p 'fn)` — the execution tier that will actually run: `NATIVE`
/// if a Cranelift native edition exists (`jit` feature only), `CLOSURE` if
/// only the portable closure edition does, `NIL` for a plain interpreted
/// function or a name with no typed registration at all. Same live-binding
/// authority rule as `signature_form`.
pub(super) fn compiled_p_form(name: &str, env: &Shared<Environment>) -> LispVal {
    let live_is_plain_lambda = matches!(env.get(name), Some(LispVal::Lambda(_)));
    if live_is_plain_lambda {
        return LispVal::Nil;
    }
    match env.jit_tier(name) {
        Some(crate::jit::Tier::Native) => LispVal::Symbol(env.intern_symbol("NATIVE")),
        Some(crate::jit::Tier::Closure) => LispVal::Symbol(env.intern_symbol("CLOSURE")),
        None => LispVal::Nil,
    }
}

/// `(why-not-typed 'fn)` — for a `defun*` that fell back to an untyped
/// lambda, the recorded inference-failure reason (a string); `NIL` if the
/// function is currently typed, or was never a `defun*` candidate (the
/// reason is only ever recorded by `eval_defun_star`'s fallback branch, and
/// cleared on a subsequent successful typed (re)definition).
pub(super) fn why_not_typed_form(name: &str, env: &Shared<Environment>) -> LispVal {
    // If the live binding is currently typed, there is nothing to explain —
    // even if a stale reason happens to still be sitting in the plist (e.g.
    // installed by `defun-typed`/`jit-optimize` rather than `defun*` after an
    // earlier `defun*` failure for the same name).
    let live_is_plain_lambda = matches!(env.get(name), Some(LispVal::Lambda(_)));
    if !live_is_plain_lambda && env.jit_named_signature(name).is_some() {
        return LispVal::Nil;
    }
    match env
        .intern_symbol(name)
        .borrow()
        .plist
        .get(WHY_NOT_TYPED_KEY)
    {
        Some(LispVal::String(s)) => LispVal::String(s.clone()),
        _ => LispVal::Nil,
    }
}
