use super::*;
use crate::environment::DynamicBinding;
/// Wrap a slice of body forms into a single form for constructors that take one
/// body expression (`make_macro`/`make_fexpr`). Zero forms → `NIL`, one form →
/// itself, many → `(progn form...)`.
fn progn_wrap(forms: &[LispVal], env: &Shared<Environment>) -> LispVal {
    match forms {
        [] => LispVal::Nil,
        [single] => single.clone(),
        many => {
            let mut progn = LispVal::Nil;
            for form in many.iter().rev() {
                progn = LispVal::Cons {
                    car: Shared::new(form.clone()),
                    cdr: Shared::new(progn),
                };
            }
            LispVal::Cons {
                car: Shared::new(LispVal::Symbol(env.intern_symbol("PROGN"))),
                cdr: Shared::new(progn),
            }
        }
    }
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
pub(super) fn eval_for(rest: &LispVal, env: &Shared<Environment>) -> Result<TcoStep, LispError> {
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

    let (var_id, var_sym) = if let LispVal::Symbol(s) = &spec[0] {
        if let Err(e) = check_bindable(&s.borrow().name, "FOR") {
            return Ok(TcoStep::Done(Err(e)));
        }
        (s.borrow().id, s.clone())
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
    let is_dyn = var_sym.borrow().is_dynamic;
    let _dyn_guard = if is_dyn {
        Some(DynamicBinding::install(
            var_sym.clone(),
            LispVal::Number(start),
        ))
    } else {
        None
    };

    let mut i = start;
    loop {
        // Inclusive bound; direction depends on the sign of step.
        if (step > 0 && i > end) || (step < 0 && i < end) {
            break;
        }
        if is_dyn {
            var_sym.borrow_mut().value = Some(LispVal::Number(i));
        } else {
            loop_env.set_id(var_id, LispVal::Number(i));
        }
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
pub(super) fn eval_while(rest: &LispVal, env: &Shared<Environment>) -> Result<TcoStep, LispError> {
    if let LispVal::Cons {
        car: cond_expr,
        cdr: body_list,
    } = rest
    {
        let body_forms = list_to_vec(body_list)?;
        loop {
            let test = eval(cond_expr, env)?;
            if !is_truthy(&test) {
                break;
            }
            for form in &body_forms {
                eval(form, env)?;
            }
        }
        Ok(TcoStep::Done(Ok(LispVal::Nil)))
    } else {
        Ok(TcoStep::Done(Err(LispError::Generic(
            "while requires a condition and a body".to_string(),
        ))))
    }
}

/// Dispatch an already-evaluated Macro, Vau, or Fexpr with unevaluated args.
///
/// Shared by the tree-walker (`eval_application`) and the compiled-code
/// executor (`exec_step` Code::Call).  Keeping it in one place means:
///   - the dynamic-binding logic (#222) only exists once
///   - the compiled path no longer re-evaluates the operator expression when it
///     resolves to a macro/fexpr/vau, eliminating the double-evaluation bug
///     (#226)
pub(super) fn apply_unevaluated(
    func: &LispVal,
    rest: &LispVal,
    env: &Shared<Environment>,
) -> Result<TcoStep, LispError> {
    if let LispVal::Macro(m) = func {
        let args_list = list_to_vec(rest)?;
        let expanded = expand_macro(m, &args_list, env)?;
        return Ok(TcoStep::TailCall(expanded, env.clone()));
    }

    if let LispVal::Vau(vau) = func {
        let new_env = Environment::new_child(&vau.env);
        new_env.set_id(vau.operands_param_id, rest.clone());
        new_env.set_id(vau.env_param_id, LispVal::Environment(env.clone()));
        return Ok(TcoStep::TailCall(*vau.body.clone(), new_env));
    }

    if let LispVal::Fexpr(fexpr) = func {
        let new_env = Environment::new_child_with_dynamic(&fexpr.env, env);
        let has_dyn = new_env.has_any_dynamic();
        let mut guards: Vec<DynamicBinding> = Vec::new();
        if fexpr.param_ids.len() == 1 {
            let id = fexpr.param_ids[0];
            if has_dyn {
                if let Some(sym) = new_env.symbol_by_id(id) {
                    if sym.borrow().is_dynamic {
                        guards.push(DynamicBinding::install(sym, rest.clone()));
                    } else {
                        new_env.set_id(id, rest.clone());
                    }
                } else {
                    new_env.set_id(id, rest.clone());
                }
            } else {
                new_env.set_id(id, rest.clone());
            }
        } else {
            let unevaluated_args = list_to_vec(rest)?;
            if unevaluated_args.len() != fexpr.params.len() {
                return Ok(TcoStep::Done(Err(LispError::Generic(format!(
                    "fexpr expected {} arguments, got {}",
                    fexpr.params.len(),
                    unevaluated_args.len()
                )))));
            }
            for (id, arg) in fexpr.param_ids.iter().zip(unevaluated_args) {
                if has_dyn
                    && let Some(sym) = new_env.symbol_by_id(*id)
                    && sym.borrow().is_dynamic
                {
                    guards.push(DynamicBinding::install(sym, arg));
                    continue;
                }
                new_env.set_id(*id, arg);
            }
        }
        if guards.is_empty() {
            return Ok(TcoStep::TailCall(*fexpr.body.clone(), new_env));
        } else {
            return Ok(TcoStep::TailCallWithGuards(
                *fexpr.body.clone(),
                new_env,
                guards,
            ));
        }
    }

    Ok(TcoStep::Done(Err(LispError::Generic(format!(
        "apply_unevaluated: not a macro/fexpr/vau: {func:?}"
    )))))
}

/// Handle ordinary function application.
///
/// Called from `eval_step` for both symbol-headed calls (after the
/// special-form tag has been read and the borrow dropped) and non-symbol-head
/// calls. Centralises the Macro / Vau / Fexpr / Lambda / Builtin dispatch so
/// there is exactly one copy of this logic in the evaluator (previously it
/// was duplicated in the `_ =>` arm and in the `else` branch of the
/// symbol-check).
///
/// Safety note (issue #156): callers must ensure that **no borrow** of the
/// head-symbol `Shared<SharedCell<Symbol>>` is held when this function
/// runs, so that `apply_owned` (reached via the builtin/native path) can
/// safely call `borrow_mut()` on the same symbol if it appears as an
/// argument (e.g. `(putp 'putp ...)`).
#[inline]
fn eval_application(
    first: &LispVal,
    rest: &LispVal,
    env: &Shared<Environment>,
) -> Result<TcoStep, LispError> {
    let func = eval(first, env)?;

    // Macro / Vau / Fexpr: pass unevaluated args to the shared dispatcher.
    if matches!(
        func,
        LispVal::Macro(_) | LispVal::Vau(_) | LispVal::Fexpr(_)
    ) {
        return apply_unevaluated(&func, rest, env);
    }

    // Evaluated-argument callables (lambda/builtin/native): evaluate operands
    // straight off the cons chain — one allocation, no intermediate clones.
    let eval_args = eval_operands(rest, env)?;

    // Lambda application: TCO — set up new env and continue with body.
    if let LispVal::Lambda(lambda) = &func {
        // Backtrace: a named call in tail position claims this trampoline's
        // frame slot (pay-on-error tracing; success truncates it away).
        if let LispVal::Symbol(name) = first {
            crate::evaluator::core::bt_note_tail(name.borrow().id);
        }
        let new_env = Environment::new_child_with_dynamic(&lambda.env, env);
        let has_dyn = new_env.has_any_dynamic();
        let mut guards: Vec<DynamicBinding> = Vec::new();
        if let Some(rest_param_id) = lambda.rest_param_id {
            if eval_args.len() < lambda.params.len() {
                return Ok(TcoStep::Done(Err(LispError::Generic(format!(
                    "lambda expected at least {} arguments, got {}",
                    lambda.params.len(),
                    eval_args.len()
                )))));
            }
            // Move fixed args into the frame; remaining go to the rest list.
            let n_fixed = lambda.params.len();
            let mut eval_args = eval_args;
            for (id, arg) in lambda.param_ids.iter().zip(eval_args.drain(..n_fixed)) {
                if has_dyn
                    && let Some(sym) = new_env.symbol_by_id(*id)
                    && sym.borrow().is_dynamic
                {
                    guards.push(DynamicBinding::install(sym, arg));
                    continue;
                }
                new_env.set_id(*id, arg);
            }
            let rest_args = vec_to_list(eval_args.into_vec());
            if has_dyn
                && let Some(sym) = new_env.symbol_by_id(rest_param_id)
                && sym.borrow().is_dynamic
            {
                guards.push(DynamicBinding::install(sym, rest_args));
            } else {
                new_env.set_id(rest_param_id, rest_args);
            }
        } else {
            if lambda.params.len() != eval_args.len() {
                return Ok(TcoStep::Done(Err(LispError::Generic(format!(
                    "lambda expected {} arguments, got {}",
                    lambda.params.len(),
                    eval_args.len()
                )))));
            }
            // Move every arg directly into the frame — no clone.
            for (id, arg) in lambda.param_ids.iter().zip(eval_args) {
                if has_dyn
                    && let Some(sym) = new_env.symbol_by_id(*id)
                    && sym.borrow().is_dynamic
                {
                    guards.push(DynamicBinding::install(sym, arg));
                    continue;
                }
                new_env.set_id(*id, arg);
            }
        }
        if guards.is_empty() {
            // If a compiled body is available, hand off to it as a tail call on
            // the *same* trampoline (`TcoStep::ExecTail`) rather than invoking a
            // fresh `exec()` — that used to be a separate, non-tail Rust call
            // that grew the native stack and eval-depth counter on every
            // tree-walker → compiled crossing (issue #200 M1' regression).
            // Otherwise fall back to the standard TailCall so the outer
            // trampoline handles TCO for the tree-walking path.
            if let Some(compiled) = &lambda.compiled {
                return Ok(TcoStep::ExecTail(compiled.clone(), new_env));
            }
            return Ok(TcoStep::TailCall(*lambda.body.clone(), new_env));
        }
        // Dynamic params present: fall back to tree-walker so TailCallWithGuards
        // can carry the guards into the trampoline's DynamicGuardStack.
        return Ok(TcoStep::TailCallWithGuards(
            *lambda.body.clone(),
            new_env,
            guards,
        ));
    }

    // All other callables (builtins, natives): apply inline.
    // The caller guarantees no head-symbol borrow is open, so apply_owned
    // can safely call borrow_mut() if the head symbol appears as an argument
    // (issue #156 is resolved by the early borrow-drop in eval_step).
    Ok(TcoStep::Done(apply_owned(&func, eval_args, env)))
}

/// Perform one evaluation step. Returns `TcoStep::Done` for final results
/// and `TcoStep::TailCall` for tail positions (caller loops instead of recursing).
pub(super) fn eval_step(val: &LispVal, env: &Shared<Environment>) -> Result<TcoStep, LispError> {
    match val {
        LispVal::Nil => Ok(TcoStep::Done(Ok(LispVal::Nil))),
        LispVal::Symbol(s) => {
            if s.borrow().is_keyword {
                return Ok(TcoStep::Done(Ok(LispVal::Symbol(s.clone()))));
            }

            // Resolve straight from the interned symbol: global/function refs read
            // the symbol's value cell directly (no hash, no chain walk), locals
            // walk their frames. Only the cold unbound path formats the name.
            let value = env.resolve(s).ok_or_else(|| {
                LispError::Generic(format!("Unbound variable: {}", s.borrow().name))
            })?;

            // Compatibility path for values explicitly bound to a LABEL form.
            // Normal LABEL evaluation now returns a closure that closes over its
            // own name binding instead of storing a re-evaluable LABEL graph.
            //
            // This goes through the depth-counted `eval` rather than an uncounted
            // `TcoStep::TailCall`: an indirect circular LABEL such as
            // `(LABEL a (LABEL b a))` rewrites endlessly between LABEL forms
            // (a → (LABEL a …) → (LABEL b a) → a → …) and, as a bare tail call,
            // would spin in the trampoline forever without ever hitting the
            // eval-depth guard. Routing through `eval` bounds the cycle and
            // surfaces it as a `LispError` (issue #153). Legitimate LABEL
            // recursion carries its loop via lambda application, not this
            // head-resolution step, so its tail-call behavior is unaffected.
            // REGULARITY (0.3): the old "LABEL compat" hack — auto-evaluating
            // any LIST VALUE whose head is the symbol LABEL on variable read —
            // is gone. It made (list 'label ...) data explode and reserved
            // LABEL as a name everywhere. The LABEL special form itself (in
            // operator position) is untouched.
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
        | LispVal::Struct(_)
        | LispVal::Extension(_)
        | LispVal::Error(_)
        | LispVal::Port(_)
        | LispVal::NetHandle(_) => Ok(TcoStep::Done(Ok(val.clone()))),
        #[cfg(feature = "concurrency")]
        LispVal::Channel(_) => Ok(TcoStep::Done(Ok(val.clone()))),

        LispVal::Cons {
            car: first,
            cdr: rest,
        } => {
            if let LispVal::Symbol(s) = &**first {
                // Read the special-form tag as a `Copy` value.  The `Ref`
                // guard (RefCell) or read-lock guard (arc-val RwLock) is a
                // temporary whose lifetime ends at the semicolon — so no
                // borrow of `s` is held when the arm bodies execute.  This
                // eliminates the re-borrow hazard from issue #156 and lets
                // `eval_application` call `apply_owned` inline even when the
                // head symbol appears as an argument (e.g. `(putp 'putp …)`).
                let sf_tag = s.borrow().special_form;

                // Fast path: ordinary function call (the overwhelmingly common
                // case in hot loops).  Skips the entire special-form match and
                // the RefCell borrow entirely.
                if sf_tag.is_none() {
                    return eval_application(first, rest, env);
                }

                // Dispatch on the precomputed enum tag — no string compare.
                match sf_tag.unwrap() {
                    SpecialForm::Quote => {
                        if let LispVal::Cons { car, cdr } = &**rest
                            && **cdr == LispVal::Nil
                        {
                            return Ok(TcoStep::Done(Ok(car.as_ref().clone())));
                        }
                        Ok(TcoStep::Done(Err(LispError::Generic(
                            "quote takes exactly one argument".to_string(),
                        ))))
                    }
                    SpecialForm::Quasiquote => {
                        if let LispVal::Cons { car, cdr } = &**rest
                            && **cdr == LispVal::Nil
                        {
                            return Ok(TcoStep::Done(quasiquote_eval(car, env)));
                        }
                        Ok(TcoStep::Done(Err(LispError::Generic(
                            "quasiquote takes exactly one argument".to_string(),
                        ))))
                    }
                    SpecialForm::Cond => {
                        // Walk the clause list and each clause's body directly off
                        // the cons cells — no `list_to_vec` allocations. COND runs
                        // on the hot path (it is the body of most conditionals and
                        // tail-recursive loops), so the former per-eval Vec churn
                        // was pure overhead.
                        let mut clauses = rest.as_ref();
                        loop {
                            let (clause, rest_clauses) = match clauses {
                                LispVal::Nil => break,
                                LispVal::Cons { car, cdr } => (car.as_ref(), cdr.as_ref()),
                                _ => {
                                    return Ok(TcoStep::Done(Err(LispError::Generic(
                                        "cond clauses must be proper lists".to_string(),
                                    ))));
                                }
                            };
                            match clause {
                                // Empty clause `()` is not allowed.
                                LispVal::Nil => {
                                    return Ok(TcoStep::Done(Err(LispError::Generic(
                                        "cond clauses must be non-empty lists".to_string(),
                                    ))));
                                }
                                LispVal::Cons {
                                    car: test,
                                    cdr: body,
                                } => {
                                    let predicate_result = eval(test, env)?;
                                    if is_truthy(&predicate_result) {
                                        // `(test)` with no body returns the test value.
                                        let mut b = body.as_ref();
                                        if matches!(b, LispVal::Nil) {
                                            return Ok(TcoStep::Done(Ok(predicate_result)));
                                        }
                                        // Evaluate body forms, tail-calling the last.
                                        loop {
                                            match b {
                                                LispVal::Cons {
                                                    car: form,
                                                    cdr: next,
                                                } => {
                                                    if matches!(next.as_ref(), LispVal::Nil) {
                                                        return Ok(TcoStep::TailCall(
                                                            form.as_ref().clone(),
                                                            env.clone(),
                                                        ));
                                                    }
                                                    eval(form, env)?;
                                                    b = next.as_ref();
                                                }
                                                _ => {
                                                    return Ok(TcoStep::Done(Err(
                                                        LispError::Generic(
                                                            "cond clauses must be proper lists"
                                                                .to_string(),
                                                        ),
                                                    )));
                                                }
                                            }
                                        }
                                    }
                                }
                                // Non-nil atom clause: not a list.
                                _ => {
                                    return Ok(TcoStep::Done(Err(LispError::Generic(
                                        "cond clauses must be proper lists".to_string(),
                                    ))));
                                }
                            }
                            clauses = rest_clauses;
                        }
                        Ok(TcoStep::Done(Ok(LispVal::Nil)))
                    }
                    SpecialForm::If => {
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
                    SpecialForm::And => {
                        let mut last_val = LispVal::Symbol(env.intern_symbol("T"));
                        let forms = list_to_vec(rest)?;
                        for form in forms {
                            last_val = eval(&form, env)?;
                            if !is_truthy(&last_val) {
                                return Ok(TcoStep::Done(Ok(LispVal::Nil)));
                            }
                        }
                        Ok(TcoStep::Done(Ok(last_val)))
                    }
                    SpecialForm::Or => {
                        let forms = list_to_vec(rest)?;
                        for form in forms {
                            let v = eval(&form, env)?;
                            if is_truthy(&v) {
                                return Ok(TcoStep::Done(Ok(v)));
                            }
                        }
                        Ok(TcoStep::Done(Ok(LispVal::Nil)))
                    }
                    SpecialForm::UnwindProtect => {
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
                    SpecialForm::Catch => {
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
                    SpecialForm::Throw => {
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
                    SpecialForm::Block => {
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
                    SpecialForm::ReturnFrom => {
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
                    SpecialForm::HandlerCase => {
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
                        let bt_base = crate::evaluator::core::bt_depth();
                        let condition = match eval(&forms[0], env) {
                            Ok(v) => return Ok(TcoStep::Done(Ok(v))),
                            Err(LispError::Signaled(c)) => {
                                // Catching consumes the error's frames into
                                // LAST-BACKTRACE (readable in the handler).
                                crate::evaluator::core::bt_capture(bt_base, env);
                                *c
                            }
                            Err(LispError::Generic(msg)) => {
                                crate::evaluator::core::bt_capture(bt_base, env);
                                LispVal::Error(Shared::new(crate::ErrorObj {
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
                    SpecialForm::Def => {
                        let args = list_to_vec(rest)?;
                        if args.len() != 2 && args.len() != 3 {
                            return Ok(TcoStep::Done(Err(LispError::Generic(
                                "def takes two or three arguments".to_string(),
                            ))));
                        }
                        if let LispVal::Symbol(s) = &args[0] {
                            let name = s.borrow().name.clone();
                            if let Err(e) = check_bindable(&name, "DEF") {
                                return Ok(TcoStep::Done(Err(e)));
                            }
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
                    SpecialForm::Defdynamic => {
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
                        if let Err(e) = check_bindable(&name, "DEFDYNAMIC") {
                            return Ok(TcoStep::Done(Err(e)));
                        }

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

                        // Install initial value directly in the symbol's global value
                        // cell. With shallow binding the cell IS the authoritative
                        // store for the current dynamic value, so we must write there
                        // regardless of how deeply nested the call site is.
                        symbol.borrow_mut().value = Some(value);

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
                    SpecialForm::Lambda => {
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
                    SpecialForm::Function => {
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
                    SpecialForm::Label => {
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

                            if !matches!(
                                expr_val,
                                LispVal::Cons { car, .. }
                                    if matches!(
                                        car.as_ref(),
                                        LispVal::Symbol(lambda_sym)
                                            if lambda_sym.borrow().name == "LAMBDA"
                                    )
                            ) {
                                return Ok(TcoStep::Done(Err(LispError::Generic(
                                    "LABEL expression must be a LAMBDA expression".to_string(),
                                ))));
                            }

                            let new_env = Environment::new_child(env);
                            let func = eval(expr_val, &new_env)?;
                            match func {
                                LispVal::Lambda(_) => {
                                    new_env.set(name_sym.borrow().name.clone(), func.clone());
                                    Ok(TcoStep::Done(Ok(func)))
                                }
                                _ => Ok(TcoStep::Done(Err(LispError::Generic(
                                    "LABEL expression must evaluate to a function".to_string(),
                                )))),
                            }
                        } else {
                            Ok(TcoStep::Done(Err(LispError::Generic(
                                "LABEL name must be a symbol".to_string(),
                            ))))
                        }
                    }
                    SpecialForm::Define => {
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
                    SpecialForm::Defexpr | SpecialForm::Defmacro => {
                        let is_defexpr = matches!(sf_tag, Some(SpecialForm::Defexpr));
                        let form_name = if is_defexpr { "DEFEXPR" } else { "DEFMACRO" };
                        let args = list_to_vec(rest)?;
                        if args.len() < 3 || args.len() > 4 {
                            return Ok(TcoStep::Done(Err(LispError::Generic(format!(
                                "{} takes three or four arguments",
                                form_name
                            )))));
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
                            let func = if is_defexpr {
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
                            Ok(TcoStep::Done(Err(LispError::Generic(format!(
                                "{} requires a symbol as its first argument",
                                form_name
                            )))))
                        }
                    }
                    // Dynamic-extent guard fences (#320): kernel special
                    // forms with RAII restore. There is deliberately NO
                    // Lisp-callable way to widen either state — the save/
                    // restore happens in this Rust frame only.
                    SpecialForm::WithFuel => {
                        let forms = list_to_vec(rest)?;
                        if forms.is_empty() {
                            return Ok(TcoStep::Done(Err(LispError::Generic(
                                "with-fuel requires a budget and body".to_string(),
                            ))));
                        }
                        let budget = match eval(&forms[0], env) {
                            Ok(LispVal::Number(n)) if n >= 0 => n as u64,
                            Ok(other) => {
                                return Ok(TcoStep::Done(Err(LispError::Generic(format!(
                                    "with-fuel: budget must be a non-negative integer, got {}",
                                    crate::printer::print(&other)
                                )))));
                            }
                            Err(e) => return Ok(TcoStep::Done(Err(e))),
                        };
                        let prev = crate::evaluator::core::kernel_fuel_remaining();
                        let karmed = prev.map_or(budget, |p| p.min(budget));
                        crate::evaluator::core::set_kernel_fuel(Some(karmed));
                        crate::evaluator::core::fuel_fence_enter();
                        let mut result = Ok(LispVal::Nil);
                        for f in &forms[1..] {
                            result = eval(f, env);
                            if result.is_err() {
                                break;
                            }
                        }
                        // Restore the enclosing budget MINUS what this fence
                        // spent (None remaining = exhaustion disarmed the
                        // counter: everything spent). Runs on error paths too.
                        crate::evaluator::core::fuel_fence_leave();
                        let know = crate::evaluator::core::kernel_fuel_remaining();
                        let spent = karmed.saturating_sub(know.unwrap_or(0));
                        crate::evaluator::core::set_kernel_fuel(
                            prev.map(|p| p.saturating_sub(spent)),
                        );
                        Ok(TcoStep::Done(result))
                    }
                    SpecialForm::WithCapabilities => {
                        let forms = list_to_vec(rest)?;
                        if forms.is_empty() {
                            return Ok(TcoStep::Done(Err(LispError::Generic(
                                "with-capabilities requires a capability list and body".to_string(),
                            ))));
                        }
                        let requested = match eval(&forms[0], env) {
                            Ok(v) => v,
                            Err(e) => return Ok(TcoStep::Done(Err(e))),
                        };
                        let mut names: Vec<String> = Vec::new();
                        let mut cur = requested;
                        loop {
                            match cur {
                                LispVal::Nil => break,
                                LispVal::Cons { car, cdr } => {
                                    match car.as_ref() {
                                        LispVal::Symbol(sym) => {
                                            names.push(sym.borrow().name.clone())
                                        }
                                        other => {
                                            return Ok(TcoStep::Done(Err(LispError::Generic(
                                                format!(
                                                    "with-capabilities: capabilities must be symbols, got {}",
                                                    crate::printer::print(other)
                                                ),
                                            ))));
                                        }
                                    }
                                    cur = cdr.as_ref().clone();
                                }
                                _ => {
                                    return Ok(TcoStep::Done(Err(LispError::Generic(
                                        "with-capabilities: capability list must be a proper list"
                                            .to_string(),
                                    ))));
                                }
                            }
                        }
                        // Attenuation only: intersect with any enclosing mask.
                        let prev = crate::evaluator::core::cap_mask_get();
                        let new_mask = match &prev {
                            None => names,
                            Some(p) => names.into_iter().filter(|n| p.contains(n)).collect(),
                        };
                        let saved = crate::evaluator::core::cap_mask_set(Some(new_mask));
                        let mut result = Ok(LispVal::Nil);
                        for f in &forms[1..] {
                            result = eval(f, env);
                            if result.is_err() {
                                break;
                            }
                        }
                        crate::evaluator::core::cap_mask_set(saved);
                        Ok(TcoStep::Done(result))
                    }
                    SpecialForm::DefunTyped => {
                        // No-compile under an active fuel fence (#284/#320).
                        if crate::evaluator::core::kernel_fuel_remaining().is_some() {
                            return Ok(TcoStep::Done(Err(LispError::Generic(
                                "defun-typed is disabled under an active fuel fence (no-compile, issue #284)"
                                    .to_string(),
                            ))));
                        }
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
                                let sym = env.intern_symbol(&name);
                                sym.borrow_mut()
                                    .plist
                                    .insert("source-form".to_string(), form.clone());
                                // Invalidation hooks (#230)
                                sym.borrow_mut().plist.remove("pure-checked");
                                invalidate_call_graph(&name, env);
                                Ok(TcoStep::Done(Ok(LispVal::Symbol(sym))))
                            }
                            Err(e) => Ok(TcoStep::Done(Err(LispError::Generic(e)))),
                        }
                    }
                    SpecialForm::DefunStar => eval_defun_star(rest, env),
                    SpecialForm::JitOptimize => {
                        // No-compile under an active fuel fence (#284/#320):
                        // a native edition's internal loops never return to
                        // the metered trampoline, so compilation is refused
                        // while a budget is armed (dynamic extent).
                        if crate::evaluator::core::kernel_fuel_remaining().is_some() {
                            return Ok(TcoStep::Done(Ok(LispVal::Symbol(
                                env.intern_symbol("COMPILE-DISABLED-BY-GUARD"),
                            ))));
                        }
                        // Best-effort, transparent typed compilation of an
                        // already-defined (un-annotated) function: try HM
                        // inference over its lambda body; on success, rebind the
                        // name to an auto-typed membrane that fast-paths typed
                        // calls and falls back to the original closure. Never
                        // errors — a function that cannot be typed is left as is.
                        // Usable two ways: `(jit-optimize name)` on an already
                        // defined function, or `(jit-optimize (defun f ...))`
                        // (the inner form is evaluated and its result symbol is
                        // optimized). Distinct from the Lisp-layer `optimize`,
                        // which rewrites a quoted *form*.
                        let args = list_to_vec(rest)?;
                        let sym = match args.first() {
                            Some(LispVal::Symbol(s)) => Some(s.clone()),
                            Some(form) => match eval(form, env)? {
                                LispVal::Symbol(s) => Some(s),
                                _ => None,
                            },
                            None => None,
                        };
                        match sym {
                            Some(s) => {
                                let name = s.borrow().name.clone();
                                // Hard compile policy (issue #168): a
                                // no-compile declaration pins the function
                                // to the tree-walker — report, don't compile.
                                let no_compile = matches!(
                                    s.borrow().plist.get("no-compile"),
                                    Some(v) if !matches!(v, LispVal::Nil)
                                );
                                if no_compile {
                                    return Ok(TcoStep::Done(Ok(LispVal::String(format!(
                                        "{name}: compile disabled by declaration"
                                    )))));
                                }
                                let status = optimize_function(&name, env);
                                s.borrow_mut().plist.remove("pure-checked");
                                invalidate_call_graph(&name, env);
                                Ok(TcoStep::Done(Ok(LispVal::String(status))))
                            }
                            None => Ok(TcoStep::Done(Ok(LispVal::Nil))),
                        }
                    }
                    SpecialForm::CheckType => {
                        // Type-check a function name or an arbitrary expression.
                        //
                        // `(check-type name)` / `(check-type 'name)` — function lookup.
                        // `(check-type expr)` — first try to elaborate `expr` in
                        //   checker mode.  If the result is the gradual top type `any`
                        //   (meaning the elaborator hit an untyped operative and gave
                        //   up), fall back to evaluating `expr`; if it yields a symbol,
                        //   do a function lookup on that.  This makes
                        //   `(check-type (defun id (x) x))` work as before.
                        let cargs = list_to_vec(rest)?;
                        let arg = cargs.first();
                        let msg = match arg {
                            // Bare symbol → function lookup
                            Some(LispVal::Symbol(s)) => check_function(&s.borrow().name, env),
                            // Quoted symbol `'name` → function lookup
                            Some(LispVal::Cons { car, cdr }) if matches!(car.as_ref(), LispVal::Symbol(s) if s.borrow().name == "QUOTE") =>
                            {
                                // (quote name) where name is a symbol → function lookup
                                if let LispVal::Cons {
                                    car: inner,
                                    cdr: nil,
                                } = cdr.as_ref()
                                    && *nil.as_ref() == LispVal::Nil
                                    && let LispVal::Symbol(s) = inner.as_ref()
                                {
                                    check_function(&s.borrow().name, env)
                                } else {
                                    "check-type: malformed quoted form".to_string()
                                }
                            }
                            // Any other form: try checker elaboration.
                            // Falls back to eval→check_function when elaboration
                            // returns the gradual `any` (opaque operative call).
                            Some(expr) => match env.jit_check_expr(expr) {
                                Err(e) => format!("type error: {e}"),
                                Ok(t) if t != "any" => t,
                                Ok(_) => match eval(expr, env) {
                                    Ok(LispVal::Symbol(s)) => check_function(&s.borrow().name, env),
                                    _ => "any".to_string(),
                                },
                            },
                            None => "check-type: expected an argument".to_string(),
                        };
                        Ok(TcoStep::Done(Ok(LispVal::String(msg))))
                    }
                    SpecialForm::DefstructTyped => {
                        // Register a typed struct and install membrane entries for
                        // its generated accessors (make-NAME / NAME-FIELD /
                        // set-NAME-FIELD), so structs are usable from untyped Lisp.
                        let form = LispVal::Cons {
                            car: first.clone(),
                            cdr: rest.clone(),
                        };
                        match env.jit_define_struct(&form) {
                            Ok(names) => {
                                let mut installed = Vec::with_capacity(names.len());
                                for n in names {
                                    env.set(n.clone(), make_typed_native(n.clone()));
                                    installed.push(LispVal::Symbol(env.intern_symbol(&n)));
                                }
                                Ok(TcoStep::Done(Ok(vec_to_list(installed))))
                            }
                            Err(e) => Ok(TcoStep::Done(Err(LispError::Generic(e)))),
                        }
                    }
                    SpecialForm::DeclareTyped => {
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
                    SpecialForm::Progn => {
                        let forms = list_to_vec(rest)?;
                        if forms.is_empty() {
                            return Ok(TcoStep::Done(Ok(LispVal::Nil)));
                        }
                        for form in &forms[..forms.len() - 1] {
                            eval(form, env)?;
                        }
                        Ok(TcoStep::TailCall(
                            forms.last().expect("PROGN body is non-empty").clone(),
                            env.clone(),
                        ))
                    }
                    SpecialForm::Setq => {
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
                                // Release the read borrow before update: a global
                                // SETQ writes the symbol's value cell (borrow_mut).
                                let var_name = s.borrow().name.clone();
                                if let Err(e) = check_bindable(&var_name, "SETQ") {
                                    return Ok(TcoStep::Done(Err(e)));
                                }
                                let v = eval(val_expr, env)?;
                                // Symbol-aware update: a gensym target writes
                                // its own cell/frame entry, not an interned
                                // twin minted from the name (issue #285).
                                Environment::update_sym(env, s, v.clone());
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
                    SpecialForm::Prog => {
                        let args = list_to_vec(rest)?;
                        if args.is_empty() {
                            return Ok(TcoStep::Done(Err(LispError::Generic(
                                "PROG requires at least a var list".to_string(),
                            ))));
                        }

                        let var_list = list_to_vec(&args[0])?;
                        let body = &args[1..];

                        let prog_env = Environment::new_child(env);
                        let has_dyn = prog_env.has_any_dynamic();
                        let mut prog_guards: Vec<DynamicBinding> = Vec::new();

                        for var in var_list {
                            if let LispVal::Symbol(s) = var {
                                if let Err(e) = check_bindable(&s.borrow().name, "PROG") {
                                    return Ok(TcoStep::Done(Err(e)));
                                }
                                let sb = s.borrow();
                                if has_dyn && sb.is_dynamic {
                                    drop(sb);
                                    prog_guards.push(DynamicBinding::install(s, LispVal::Nil));
                                } else {
                                    prog_env.set_id(sb.id, LispVal::Nil);
                                }
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
                    SpecialForm::Return => {
                        let args = list_to_vec(rest)?;
                        if args.len() != 1 {
                            return Ok(TcoStep::Done(Err(LispError::Generic(
                                "RETURN takes exactly one argument".to_string(),
                            ))));
                        }
                        let retval = eval(&args[0], env)?;
                        Ok(TcoStep::Done(Err(LispError::Return(Box::new(retval)))))
                    }
                    SpecialForm::Go => {
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
                    SpecialForm::For => eval_for(rest, env),
                    SpecialForm::While => eval_while(rest, env),
                    SpecialForm::Let => {
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
                        //
                        // Dynamic variables use shallow binding (O(1) symbol-cell swap)
                        // rather than a local-frame entry. Each `DynamicBinding` guard
                        // saves the old cell value and restores it on drop, so all exit
                        // paths — normal return, `?` early return, THROW, GO, errors —
                        // correctly restore the previous dynamic binding.
                        let let_env = Environment::new_child(env);
                        let mut dynamic_guards: Vec<DynamicBinding> = Vec::new();
                        for (param, arg_expr) in params.iter().zip(arg_exprs.iter()) {
                            if let LispVal::Symbol(s) = param {
                                if let Err(e) = check_bindable(&s.borrow().name, "LET") {
                                    return Ok(TcoStep::Done(Err(e)));
                                }
                                let v = eval(arg_expr, env)?;
                                let sb = s.borrow();
                                if sb.is_dynamic {
                                    // Shallow binding: install value in the symbol cell.
                                    drop(sb);
                                    dynamic_guards.push(DynamicBinding::install(s.clone(), v));
                                } else {
                                    let id = sb.id;
                                    drop(sb);
                                    let_env.set_id(id, v);
                                }
                            } else {
                                return Ok(TcoStep::Done(Err(LispError::Generic(
                                    "let binding name must be a symbol".to_string(),
                                ))));
                            }
                        }
                        if dynamic_guards.is_empty() {
                            Ok(TcoStep::TailCall(body, let_env))
                        } else {
                            Ok(TcoStep::TailCallWithGuards(body, let_env, dynamic_guards))
                        }
                    }
                    // let* binds sequentially in a SINGLE frame: each binding is
                    // evaluated in that frame and then written into it, so later
                    // bindings see earlier ones without allocating a frame per
                    // binding (the difference from desugaring to nested LETs).
                    // Dynamic variables use shallow binding here too; later
                    // bindings in the same let* that reference a just-bound
                    // dynamic variable see the newly installed symbol-cell value.
                    SpecialForm::LetStar => {
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
                        let mut dynamic_guards: Vec<DynamicBinding> = Vec::new();
                        for binding in bindings_vec {
                            let pair = list_to_vec(&binding)?;
                            if pair.len() != 2 {
                                return Ok(TcoStep::Done(Err(LispError::Generic(
                                    "let* binding must be a pair".to_string(),
                                ))));
                            }
                            if let LispVal::Symbol(s) = &pair[0] {
                                if let Err(e) = check_bindable(&s.borrow().name, "LET*") {
                                    return Ok(TcoStep::Done(Err(e)));
                                }
                                let v = eval(&pair[1], &let_env)?;
                                let sb = s.borrow();
                                if sb.is_dynamic {
                                    drop(sb);
                                    dynamic_guards.push(DynamicBinding::install(s.clone(), v));
                                } else {
                                    let id = sb.id;
                                    drop(sb);
                                    let_env.set_id(id, v);
                                }
                            } else {
                                return Ok(TcoStep::Done(Err(LispError::Generic(
                                    "let* binding name must be a symbol".to_string(),
                                ))));
                            }
                        }
                        if dynamic_guards.is_empty() {
                            Ok(TcoStep::TailCall(body, let_env))
                        } else {
                            Ok(TcoStep::TailCallWithGuards(body, let_env, dynamic_guards))
                        }
                    }
                    SpecialForm::Macro => {
                        // Anonymous macro constructor: `(macro (params...) body...)`
                        // yields a Macro *value* (the symmetric completion of
                        // LAMBDA→Lambda and VAU→Vau). This is what lets `macrolet`
                        // live entirely in the Lisp layer as a let-over-constructor.
                        let args = list_to_vec(rest)?;
                        if args.is_empty() {
                            return Ok(TcoStep::Done(Err(LispError::Generic(
                                "macro requires a parameter list".to_string(),
                            ))));
                        }
                        let body = progn_wrap(&args[1..], env);
                        Ok(TcoStep::Done(make_macro(&args[0], &body, env)))
                    }
                    SpecialForm::Fexpr => {
                        // Anonymous fexpr constructor: `(fexpr (params...) body...)`
                        // yields a Fexpr value (symmetric with MACRO/VAU). Backs the
                        // Lisp-layer `fexprlet`.
                        let args = list_to_vec(rest)?;
                        if args.is_empty() {
                            return Ok(TcoStep::Done(Err(LispError::Generic(
                                "fexpr requires a parameter list".to_string(),
                            ))));
                        }
                        let body = progn_wrap(&args[1..], env);
                        Ok(TcoStep::Done(make_fexpr(&args[0], &body, env)))
                    }
                    SpecialForm::Vau => {
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
                        let (op_param, op_param_id) = if let LispVal::Symbol(s) = &param_list[0] {
                            let sb = s.borrow();
                            if let Err(e) = check_param_name(&sb.name, "vau") {
                                return Ok(TcoStep::Done(Err(e)));
                            }
                            (sb.name.clone(), sb.id)
                        } else {
                            return Ok(TcoStep::Done(Err(LispError::Generic(
                                "vau operands parameter must be a symbol".to_string(),
                            ))));
                        };
                        let (env_param, env_param_id) = if let LispVal::Symbol(s) = &param_list[1] {
                            let sb = s.borrow();
                            if let Err(e) = check_param_name(&sb.name, "vau") {
                                return Ok(TcoStep::Done(Err(e)));
                            }
                            (sb.name.clone(), sb.id)
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
                                    car: Shared::new(form.clone()),
                                    cdr: Shared::new(progn),
                                };
                            }
                            LispVal::Cons {
                                car: Shared::new(progn_sym),
                                cdr: Shared::new(progn),
                            }
                        };
                        Ok(TcoStep::Done(Ok(LispVal::Vau(Box::new(crate::Vau {
                            operands_param: op_param,
                            env_param,
                            body: Box::new(body),
                            env: env.clone(),
                            operands_param_id: op_param_id,
                            env_param_id,
                        })))))
                    }
                }
            } else {
                // Non-symbol head: evaluate and apply directly.
                eval_application(first, rest, env)
            }
        }
    }
}

// ---------------------------------------------------------------------------
// defun* — unified smart function definition (#170)
// ---------------------------------------------------------------------------
//
// Accepted parameter styles (all auto-detected):
//   classic arglist : (defun* name (a b)        body...)   ; like defun
//   classic + types : (defun* name ((a int64) b) body...)
//   flat bare       : (defun* name a b          body...)
//   flat typed      : (defun* name (a int64) (b int64) body...)
// An optional bare type keyword after the params is the return type, and an
// optional leading string is a docstring:
//   (defun* name "doc" (a int64) (b int64) int64 body...)
//
// Dispatch rules:
//   1. Parse params (each Some(ty) pinned / None inferred) + optional ret type.
//   2. Call jit_define_partial: seeds fresh vars for unspecified slots.
//   3. On success:  install a typed-native membrane + store source-form on plist.
//      On failure:  install a plain lambda (silent if no hints given; note if hints).
//
// Note policy — eprintln with `;` prefix so it reads as a Lisp comment:
//   • Inferred (any None slot) + compiled  →  "; defun* NAME : sig  [compiled]"
//   • Had hints + failed to compile        →  "; defun* NAME: could not compile (why)"
//   • All specified + compiled             →  silent (expected)
//   • No hints + failed to compile         →  silent (expected)

/// One parameter of a `defun*`: name + optional pinned type (`None` = inferred).
type StarParam = (String, Option<crate::jit::Ty>);

/// Parse one element of a *classic* arglist (`sym` or `(sym ty)` or `(sym)`).
fn classic_arglist_param(item: &LispVal) -> Result<StarParam, LispError> {
    match item {
        LispVal::Symbol(s) => Ok((s.borrow().name.clone(), None)),
        LispVal::Cons { car, cdr } => {
            if let LispVal::Symbol(s) = car.as_ref() {
                let ty = match cdr.as_ref() {
                    LispVal::Nil => None,
                    LispVal::Cons { car: ty_form, .. } => crate::jit::try_parse_ty_simple(ty_form),
                    _ => None,
                };
                Ok((s.borrow().name.clone(), ty))
            } else {
                Err(LispError::Generic(
                    "defun*: parameter name must be a symbol".to_string(),
                ))
            }
        }
        _ => Err(LispError::Generic(
            "defun*: malformed parameter".to_string(),
        )),
    }
}

/// Parse the parameter section of a `defun*`, starting at `items[start]`.
/// Returns the params and the index of the first post-parameter item (where the
/// optional return-type / body begins). Handles classic arglists, flat bare
/// symbols, and flat typed groups (see the module comment above).
fn parse_star_params(
    items: &[LispVal],
    start: usize,
) -> Result<(Vec<StarParam>, usize), LispError> {
    let mut params: Vec<StarParam> = Vec::new();
    let mut i = start;

    // Leading `()` or a non-param-shaped list in first position = classic arglist.
    match items.get(i) {
        // Empty arglist: (defun* f () body...)
        Some(LispVal::Nil) => return Ok((params, i + 1)),
        Some(item @ LispVal::Cons { car, cdr }) => {
            // Is this a single flat typed/inferred param — `(sym TYPE)` or `(sym)`?
            // If so, fall through to the flat loop; otherwise it's a classic arglist.
            let is_single_flat_param = matches!(car.as_ref(), LispVal::Symbol(_))
                && match cdr.as_ref() {
                    LispVal::Nil => true, // (sym)
                    LispVal::Cons { car: ty, cdr: nil } => {
                        *nil.as_ref() == LispVal::Nil
                            && crate::jit::try_parse_ty_simple(ty).is_some()
                    }
                    _ => false,
                };
            if !is_single_flat_param {
                // Classic arglist: this one list holds every parameter.
                for elem in list_to_vec(item)? {
                    params.push(classic_arglist_param(&elem)?);
                }
                return Ok((params, i + 1));
            }
        }
        _ => {}
    }

    // Flat style: collect consecutive `sym` / `(sym ty)` / `(sym)` until a bare
    // type keyword (return type) or a non-parameter form (body) is reached.
    while i < items.len() {
        match &items[i] {
            LispVal::Symbol(s) => {
                if crate::jit::try_parse_ty_simple(&items[i]).is_some() {
                    break; // bare type keyword → return-type position
                }
                params.push((s.borrow().name.clone(), None));
                i += 1;
            }
            LispVal::Cons { car, cdr } if matches!(car.as_ref(), LispVal::Symbol(_)) => {
                let sname = if let LispVal::Symbol(s) = car.as_ref() {
                    s.borrow().name.clone()
                } else {
                    unreachable!()
                };
                match cdr.as_ref() {
                    LispVal::Nil => {
                        params.push((sname, None)); // (sym)
                        i += 1;
                    }
                    LispVal::Cons {
                        car: ty_form,
                        cdr: nil,
                    } if *nil.as_ref() == LispVal::Nil
                        && crate::jit::try_parse_ty_simple(ty_form).is_some() =>
                    {
                        params.push((sname, crate::jit::try_parse_ty_simple(ty_form))); // (sym ty)
                        i += 1;
                    }
                    _ => break, // not a flat param → body
                }
            }
            _ => break, // number, string, nested call, … → body
        }
    }
    Ok((params, i))
}

/// Invalidate the call-graph entry for `name` so stale callee data doesn't
/// survive a redefinition (#230).  Removes the `$call-graph` hash entry and
/// pushes the name onto `$cg-pending` (both are Lisp-layer globals).
fn invalidate_call_graph(name: &str, env: &Shared<Environment>) {
    let name_sym = env.intern_symbol(name);
    let name_val = LispVal::Symbol(name_sym.clone());
    // remhash $call-graph name
    let cg_sym = env.intern_symbol("$CALL-GRAPH");
    let ht_opt = cg_sym.borrow().value.clone();
    if let Some(LispVal::HashTable(ht)) = ht_opt {
        ht.borrow_mut().remove(&name_val);
    }
    // push name onto $cg-pending
    let pending_sym = env.intern_symbol("$CG-PENDING");
    let old = pending_sym.borrow().value.clone().unwrap_or(LispVal::Nil);
    pending_sym.borrow_mut().value = Some(LispVal::Cons {
        car: Shared::new(name_val),
        cdr: Shared::new(old),
    });
}

pub(super) fn eval_defun_star(
    rest: &LispVal,
    env: &Shared<Environment>,
) -> Result<TcoStep, LispError> {
    let items = list_to_vec(rest)?;
    if items.is_empty() {
        return Err(LispError::Generic("defun*: missing name".to_string()));
    }

    // --- name ---
    let name_sym = match &items[0] {
        LispVal::Symbol(s) => s.clone(),
        _ => {
            return Err(LispError::Generic(
                "defun*: name must be a symbol".to_string(),
            ));
        }
    };
    let name = name_sym.borrow().name.clone();

    let mut i = 1usize;

    // --- optional docstring ---
    let mut docstring: Option<String> = None;
    if let Some(LispVal::String(s)) = items.get(i) {
        docstring = Some(s.clone());
        i += 1;
    }

    // --- params (classic arglist, flat bare, or flat typed; auto-detected) ---
    let (params, next) = parse_star_params(&items, i)?;
    i = next;

    // --- optional return-type annotation ---
    let mut ret_hint: Option<crate::jit::Ty> = None;
    if let Some(ty) = items.get(i).and_then(crate::jit::try_parse_ty_simple) {
        ret_hint = Some(ty);
        i += 1;
    }

    // --- body (must be non-empty) ---
    if i >= items.len() {
        return Err(LispError::Generic(format!("defun*: {name}: no body forms")));
    }
    let body_forms: Vec<LispVal> = items[i..].to_vec();

    // ---- decide what was specified vs. inferred ----
    let had_hints = params.iter().any(|(_, t)| t.is_some()) || ret_hint.is_some();
    let had_unspecified = params.iter().any(|(_, t)| t.is_none()) || ret_hint.is_none();

    // ---- attempt typed compilation ----
    match env.jit_define_partial(&name, &params, ret_hint, &body_forms) {
        Ok((_id, sig)) => {
            // Typed compilation succeeded.
            env.set(name.clone(), make_typed_native(name.clone()));
            let sym = env.intern_symbol(&name);
            // Build the source form for see-source introspection.
            let source_form = LispVal::Cons {
                car: Shared::new(LispVal::Symbol(env.intern_symbol("DEFUN*"))),
                cdr: Shared::new(vec_to_list(items)),
            };
            sym.borrow_mut()
                .plist
                .insert("source-form".to_string(), source_form);
            if let Some(doc) = docstring {
                sym.borrow_mut()
                    .plist
                    .insert("docstring".to_string(), LispVal::String(doc));
            }
            // Invalidation hooks: clear cached purity verdict and
            // stale call-graph entry so analyses stay sound (#230).
            sym.borrow_mut().plist.remove("pure-checked");
            invalidate_call_graph(&name, env);
            if had_unspecified {
                eprintln!("; defun* {name} : {sig}  [compiled]");
            }
            Ok(TcoStep::Done(Ok(LispVal::Symbol(sym))))
        }
        Err(reason) => {
            // Typed compilation failed — fall back to a plain lambda.
            // Build a lambda from the (unannotated) param names and body.
            let param_names: Vec<LispVal> = params
                .iter()
                .map(|(n, _)| LispVal::Symbol(env.intern_symbol(n)))
                .collect();
            let params_lv = vec_to_list(param_names);
            let body_val = if body_forms.len() == 1 {
                body_forms[0].clone()
            } else {
                let mut progn = vec![LispVal::Symbol(env.intern_symbol("PROGN"))];
                progn.extend(body_forms);
                vec_to_list(progn)
            };
            let lambda = make_lambda(&params_lv, &body_val, env)
                .map_err(|e| LispError::Generic(format!("defun* {name}: {e}")))?;
            env.set(name.clone(), lambda);
            let sym = env.intern_symbol(&name);
            if let Some(doc) = docstring {
                sym.borrow_mut()
                    .plist
                    .insert("docstring".to_string(), LispVal::String(doc));
            }
            sym.borrow_mut().plist.remove("pure-checked");
            invalidate_call_graph(&name, env);
            if had_hints {
                eprintln!("; defun* {name}: could not compile ({reason}); using untyped lambda");
            }
            Ok(TcoStep::Done(Ok(LispVal::Symbol(sym))))
        }
    }
}
