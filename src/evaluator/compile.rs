//! Compile → execute intermediate representation (Milestone 1).
//!
//! [`compile`] translates a lambda body into a [`Code`] IR tree once at
//! definition time.  [`exec`] runs it with an internal TCO trampoline that
//! preserves tail-call optimisation for chains of compiled lambdas.
//!
//! **Fallback rule**: anything the compiler does not yet handle is wrapped in
//! [`crate::Code::Interp`], which delegates transparently to the existing
//! tree-walking `eval`.  Correctness is therefore unconditional — a wider
//! `Interp` coverage just gives up some speed, never correctness.

use super::*;

// ─── helpers ─────────────────────────────────────────────────────────────────

/// Parse the CDR chain of a Lisp form into a fixed-length `Vec`, or return
/// `None` on error.  Used where falling back to `Code::Interp` is the caller's
/// responsibility.
fn safe_list_to_vec(list: &LispVal) -> Option<Vec<LispVal>> {
    list_to_vec(list).ok()
}

// ─── compile ─────────────────────────────────────────────────────────────────

/// Compile a Lisp form into a [`Code`] IR node wrapped in a shared pointer.
///
/// Conservative: any form or pattern that isn't explicitly recognised falls
/// through to [`crate::Code::Interp`].
pub(super) fn compile(form: &LispVal) -> Shared<crate::Code> {
    Shared::new(compile_inner(form))
}

fn compile_inner(form: &LispVal) -> crate::Code {
    use crate::Code;
    match form {
        // Self-evaluating atoms
        LispVal::Nil
        | LispVal::Number(_)
        | LispVal::Float(_)
        | LispVal::String(_)
        | LispVal::Char(_) => Code::Const(form.clone()),

        LispVal::Symbol(s) => {
            // Read the cached flags and drop the borrow before doing anything
            // that might re-borrow the same symbol (issue #156 pattern).
            let is_keyword = s.borrow().is_keyword;
            let has_special = s.borrow().special_form.is_some();
            if is_keyword {
                // Keywords self-evaluate.
                Code::Const(form.clone())
            } else if has_special {
                // A bare special-form name used as a value — very rare, fall back.
                Code::Interp(form.clone())
            } else {
                // Ordinary symbol → variable reference.
                Code::Var(s.clone())
            }
        }

        LispVal::Cons { car, cdr } => compile_cons(car, cdr, form),

        // Lambdas, builtins, etc. that appear as source forms are unusual;
        // the tree-walker handles them correctly so just fall back.
        _ => crate::Code::Interp(form.clone()),
    }
}

/// Compile a cons-cell form `(car . rest)`.
fn compile_cons(car: &Shared<LispVal>, rest: &Shared<LispVal>, form: &LispVal) -> crate::Code {
    use crate::Code;
    match car.as_ref() {
        LispVal::Symbol(s) => {
            // Read the special-form tag and immediately drop the borrow so
            // that any subsequent code can safely re-borrow the same symbol.
            let sf = s.borrow().special_form;
            match sf {
                Some(SpecialForm::Quote) => {
                    // (quote datum)
                    if let LispVal::Cons { car: datum, .. } = rest.as_ref() {
                        Code::Const(datum.as_ref().clone())
                    } else {
                        Code::Interp(form.clone())
                    }
                }
                Some(SpecialForm::If) => compile_if(rest, form),
                Some(SpecialForm::Progn) => compile_progn(rest, form),
                Some(SpecialForm::Cond) => compile_cond(rest, form),
                Some(SpecialForm::Let) => compile_let(rest, form),
                Some(SpecialForm::Setq) => compile_setq(rest, form),
                Some(SpecialForm::UnwindProtect) => compile_unwind_protect(rest, form),
                Some(SpecialForm::While) => compile_while(rest, form),
                Some(SpecialForm::For) => compile_for(rest, form),
                Some(_) => {
                    // All other special forms (lambda, defmacro, catch, prog, …):
                    // fall back to the tree-walker.
                    Code::Interp(form.clone())
                }
                None => {
                    // Ordinary symbol in head position → function call.
                    compile_call(car.as_ref(), rest, form)
                }
            }
        }
        // Non-symbol head (e.g. a lambda literal in call position): compile
        // as a call — exec will evaluate it and apply the result.
        other => compile_call(other, rest, form),
    }
}

/// Compile `(if cond then [else])`.
fn compile_if(rest: &Shared<LispVal>, form: &LispVal) -> crate::Code {
    use crate::Code;
    let forms = match safe_list_to_vec(rest) {
        Some(v) => v,
        None => return Code::Interp(form.clone()),
    };
    match forms.len() {
        2 => {
            let cond = compile(&forms[0]);
            let then = compile(&forms[1]);
            let els = Shared::new(Code::Const(LispVal::Nil));
            Code::If(cond, then, els)
        }
        3 => {
            let cond = compile(&forms[0]);
            let then = compile(&forms[1]);
            let els = compile(&forms[2]);
            Code::If(cond, then, els)
        }
        // Wrong number of sub-forms — tree-walker will give the right error.
        _ => Code::Interp(form.clone()),
    }
}

/// Compile `(progn f1 … fn)`.
fn compile_progn(rest: &Shared<LispVal>, form: &LispVal) -> crate::Code {
    use crate::Code;
    let forms = match safe_list_to_vec(rest) {
        Some(v) => v,
        None => return Code::Interp(form.clone()),
    };
    if forms.is_empty() {
        Code::Const(LispVal::Nil)
    } else {
        Code::Seq(forms.iter().map(compile).collect())
    }
}

/// Compile `(cond (t1 r1…) (t2 r2…) …)` to a chain of nested `Code::If` nodes.
///
/// Each clause `(condition result…)` becomes one level of `Code::If`.
/// Clauses with a single form `(condition)` — which return the condition value
/// when truthy — cannot be compiled without evaluating the condition twice, so
/// the whole COND form falls back to `Code::Interp` if any such clause is
/// encountered.
fn compile_cond(rest: &Shared<LispVal>, form: &LispVal) -> crate::Code {
    let clauses = match safe_list_to_vec(rest) {
        Some(v) => v,
        None => return crate::Code::Interp(form.clone()),
    };
    compile_cond_clauses(&clauses, form)
}

fn compile_cond_clauses(clauses: &[LispVal], form: &LispVal) -> crate::Code {
    use crate::Code;
    if clauses.is_empty() {
        // No matching clause → NIL (same as the tree-walker).
        return Code::Const(LispVal::Nil);
    }

    let clause_forms = match safe_list_to_vec(&clauses[0]) {
        Some(v) if !v.is_empty() => v,
        _ => return Code::Interp(form.clone()),
    };

    // Clause `(test)` with no result: return the test value when truthy.
    // We'd have to evaluate the condition twice (once to test, once to return)
    // or introduce a temporary binding; fall back to the tree-walker instead.
    if clause_forms.len() == 1 {
        return Code::Interp(form.clone());
    }

    let cond_code = compile(&clause_forms[0]);
    // Result is either a single form or a sequence (last in tail position).
    let result_code = if clause_forms.len() == 2 {
        compile(&clause_forms[1])
    } else {
        Shared::new(Code::Seq(clause_forms[1..].iter().map(compile).collect()))
    };
    let else_code = Shared::new(compile_cond_clauses(&clauses[1..], form));

    Code::If(cond_code, result_code, else_code)
}

/// Compile `(let ((v1 e1) …) body…)` for lexical (non-dynamic) variables.
///
/// Init expressions are evaluated in the outer environment (standard `let`
/// semantics).  Any binding that refers to a dynamic variable causes the whole
/// form to fall back to `Code::Interp` so the tree-walker's `DynamicBinding`
/// RAII guards are applied correctly.
fn compile_let(rest: &Shared<LispVal>, form: &LispVal) -> crate::Code {
    use crate::Code;

    let args = match safe_list_to_vec(rest) {
        Some(v) if v.len() >= 2 => v,
        // Missing binding list or body → fall back (tree-walker gives a good error).
        _ => return Code::Interp(form.clone()),
    };

    let binding_forms = match safe_list_to_vec(&args[0]) {
        Some(v) => v,
        None => return Code::Interp(form.clone()),
    };

    let mut bindings: Vec<(u32, Shared<Code>)> = Vec::with_capacity(binding_forms.len());
    for binding in &binding_forms {
        let pair = match safe_list_to_vec(binding) {
            Some(v) if v.len() == 2 => v,
            _ => return Code::Interp(form.clone()),
        };
        if let LispVal::Symbol(s) = &pair[0] {
            let sb = s.borrow();
            if sb.is_dynamic {
                // Dynamic bindings require RAII guards — fall back.
                return Code::Interp(form.clone());
            }
            let id = sb.id;
            drop(sb);
            bindings.push((id, compile(&pair[1])));
        } else {
            return Code::Interp(form.clone());
        }
    }

    // Body: single form is compiled directly; multiple forms become a Seq so
    // the last is in tail position.
    let body = if args.len() == 2 {
        compile(&args[1])
    } else {
        Shared::new(Code::Seq(args[1..].iter().map(compile).collect()))
    };

    Code::Let { bindings, body }
}

/// Compile `(setq v1 e1 v2 e2 …)`.
///
/// Each `vi` must be a bare symbol; a malformed (odd-length, or non-symbol
/// variable) form falls back so the tree-walker reports the right error.
/// `Environment::update` already handles dynamic vs. lexical resolution (and
/// creates the variable in the current environment if unbound), matching the
/// tree-walker's `SETQ` semantics exactly — no dynamic-binding RAII is needed
/// here because `setq` mutates an existing cell rather than installing one.
fn compile_setq(rest: &Shared<LispVal>, form: &LispVal) -> crate::Code {
    use crate::Code;
    let forms = match safe_list_to_vec(rest) {
        Some(v) if v.len() % 2 == 0 => v,
        // Odd arg count or dotted list — tree-walker gives the right error.
        _ => return Code::Interp(form.clone()),
    };
    let mut pairs = Vec::with_capacity(forms.len() / 2);
    for pair in forms.chunks_exact(2) {
        match &pair[0] {
            LispVal::Symbol(s) => pairs.push((s.clone(), compile(&pair[1]))),
            _ => return Code::Interp(form.clone()),
        }
    }
    Code::SetVar(pairs)
}

/// Compile `(unwind-protect body cleanup…)`.
fn compile_unwind_protect(rest: &Shared<LispVal>, form: &LispVal) -> crate::Code {
    use crate::Code;
    let forms = match safe_list_to_vec(rest) {
        Some(v) if !v.is_empty() => v,
        _ => return Code::Interp(form.clone()),
    };
    let body = compile(&forms[0]);
    let cleanups = forms[1..].iter().map(compile).collect();
    Code::UnwindProtect { body, cleanups }
}

/// Compile `(while cond body…)`.
fn compile_while(rest: &Shared<LispVal>, form: &LispVal) -> crate::Code {
    use crate::Code;
    let forms = match safe_list_to_vec(rest) {
        Some(v) if !v.is_empty() => v,
        _ => return Code::Interp(form.clone()),
    };
    let cond = compile(&forms[0]);
    let body = forms[1..].iter().map(compile).collect();
    Code::While { cond, body }
}

/// Compile `(for (var start end [step]) body…)`.
fn compile_for(rest: &Shared<LispVal>, form: &LispVal) -> crate::Code {
    use crate::Code;
    let args = match safe_list_to_vec(rest) {
        Some(v) if !v.is_empty() => v,
        _ => return Code::Interp(form.clone()),
    };
    let spec = match safe_list_to_vec(&args[0]) {
        Some(v) if v.len() == 3 || v.len() == 4 => v,
        _ => return Code::Interp(form.clone()),
    };
    let var_id = match &spec[0] {
        LispVal::Symbol(s) => s.borrow().id,
        _ => return Code::Interp(form.clone()),
    };
    let start = compile(&spec[1]);
    let end = compile(&spec[2]);
    let step = if spec.len() == 4 {
        Some(compile(&spec[3]))
    } else {
        None
    };
    let body = args[1..].iter().map(compile).collect();
    Code::For {
        var_id,
        start,
        end,
        step,
        body,
    }
}

/// Compile a generic function call `(head arg1 … argN)`.
fn compile_call(head: &LispVal, rest: &Shared<LispVal>, form: &LispVal) -> crate::Code {
    use crate::Code;
    let arg_forms = match safe_list_to_vec(rest) {
        Some(v) => v,
        // Dotted argument list — the tree-walker will signal the right error.
        None => return Code::Interp(form.clone()),
    };
    let callee = compile(head);
    let args = arg_forms.iter().map(compile).collect();
    // Store the original form so exec can fall back to eval when the callee
    // turns out to be a macro, fexpr, or vau (which need unevaluated args).
    Code::Call {
        callee,
        args,
        original: form.clone(),
    }
}

// ─── exec ────────────────────────────────────────────────────────────────────

/// Execute a compiled [`Code`] tree with a TCO trampoline.
///
/// This is a thin entry point around the *same* trampoline `eval()` uses
/// (see [`super::functions::exec_entry`]) — the two share one loop so that a
/// tail call crossing between compiled `Code` and an uncompiled `Code::Interp`
/// form (and back) costs no native stack depth. Splitting them into
/// independent trampolines was the root cause of issue #200 M1's TCO
/// regression: each crossing was a plain (non-tail) Rust call, so both the
/// native stack and the eval-depth counter grew per iteration.
///
/// Each call to `exec` counts as one depth frame (via [`DepthGuard`]) so that
/// non-tail-recursive compiled lambdas are subject to the same recursion-depth
/// limit as the tree-walker. Tail calls — whether they stay in `Code` or hand
/// off to a raw AST form — do *not* add additional depth because they stay on
/// the same trampoline instance.
pub(super) fn exec(
    code: &Shared<crate::Code>,
    env: &Shared<Environment>,
) -> Result<LispVal, LispError> {
    exec_entry(code.clone(), env)
}

/// Perform one exec trampoline step.
pub(super) fn exec_step(
    code: &crate::Code,
    env: &Shared<Environment>,
) -> Result<TcoStep, LispError> {
    use crate::Code;
    match code {
        Code::Const(v) => Ok(TcoStep::Done(Ok(v.clone()))),

        Code::Var(sym) => Ok(TcoStep::Done(env.resolve(sym).ok_or_else(|| {
            LispError::Generic(format!("unbound variable: {}", sym.borrow().name))
        }))),

        // Fallback: hand the original AST form to the tree-walker side of the
        // *same* trampoline. This must be a tail hand-off (not a plain `eval`
        // call) because `Interp` can legitimately sit in tail position (e.g. a
        // dynamic `let` the compiler couldn't lower) — see issue #200 M1'.
        Code::Interp(form) => Ok(TcoStep::TailCall(form.clone(), env.clone())),

        Code::If(cond, then, els) => {
            // Condition is non-tail — evaluate it via exec so compiled
            // sub-expressions still benefit from the fast path.
            let test = exec(cond, env)?;
            let branch = if test.is_truthy() {
                then.clone()
            } else {
                els.clone()
            };
            // Then/else branch is tail — loop.
            Ok(TcoStep::ExecTail(branch, env.clone()))
        }

        Code::Seq(forms) => {
            match forms.len() {
                0 => Ok(TcoStep::Done(Ok(LispVal::Nil))),
                1 => {
                    // Single form — it is the tail.
                    Ok(TcoStep::ExecTail(forms[0].clone(), env.clone()))
                }
                n => {
                    // Evaluate all but the last (non-tail).
                    for f in &forms[..n - 1] {
                        exec(f, env)?;
                    }
                    // Last form is the tail.
                    Ok(TcoStep::ExecTail(forms[n - 1].clone(), env.clone()))
                }
            }
        }

        Code::Let { bindings, body } => {
            // Evaluate all init expressions in the *outer* env (standard let
            // semantics: bindings do not see each other).
            let let_env = Environment::new_child(env);
            for (id, init_code) in bindings {
                let val = exec(init_code, env)?;
                let_env.set_id(*id, val);
            }
            // Body is in tail position — loop with the child env.
            Ok(TcoStep::ExecTail(body.clone(), let_env))
        }

        Code::SetVar(pairs) => {
            // Not a tail position: SETQ's result is the last assigned value,
            // an ordinary (non-tail) expression result.
            let mut last = LispVal::Nil;
            for (sym, init) in pairs {
                let val = exec(init, env)?;
                // Drop the borrow before `update` (which may itself borrow
                // the same symbol, e.g. via intern) — issue #156 pattern.
                let name = sym.borrow().name.clone();
                Environment::update(env, &name, val.clone());
                last = val;
            }
            Ok(TcoStep::Done(Ok(last)))
        }

        Code::UnwindProtect { body, cleanups } => {
            // BODY is non-tail: cleanups must always run after it, even if it
            // errors or performs a non-local exit (matches `eval`'s handling).
            let result = exec(body, env);
            for cleanup in cleanups {
                let _ = exec(cleanup, env);
            }
            Ok(TcoStep::Done(result))
        }

        Code::While { cond, body } => {
            loop {
                if !exec(cond, env)?.is_truthy() {
                    break;
                }
                for f in body {
                    exec(f, env)?;
                }
            }
            Ok(TcoStep::Done(Ok(LispVal::Nil)))
        }

        Code::For {
            var_id,
            start,
            end,
            step,
            body,
        } => {
            let as_int = |v: &LispVal, who: &str| -> Result<i64, LispError> {
                match v {
                    LispVal::Number(n) => Ok(*n),
                    other => Err(LispError::Generic(format!(
                        "for {who} must be an integer, got {other:?}"
                    ))),
                }
            };
            let start_i = as_int(&exec(start, env)?, "start")?;
            let end_i = as_int(&exec(end, env)?, "end")?;
            let step_i = match step {
                Some(s) => as_int(&exec(s, env)?, "step")?,
                None => 1,
            };
            if step_i == 0 {
                return Ok(TcoStep::Done(Err(LispError::Generic(
                    "for step must be non-zero".to_string(),
                ))));
            }

            let loop_env = Environment::new_child(env);
            let mut i = start_i;
            loop {
                // Inclusive bound; direction depends on the sign of step.
                if (step_i > 0 && i > end_i) || (step_i < 0 && i < end_i) {
                    break;
                }
                loop_env.set_id(*var_id, LispVal::Number(i));
                for f in body {
                    exec(f, &loop_env)?;
                }
                match i.checked_add(step_i) {
                    Some(n) => i = n,
                    None => break,
                }
            }
            Ok(TcoStep::Done(Ok(LispVal::Nil)))
        }

        Code::Call {
            callee,
            args,
            original,
        } => {
            // Evaluate callee (non-tail).
            let func = exec(callee, env)?;

            // Macros, fexprs, and vau operatives need their arguments
            // *unevaluated*.  The operator (`func`) is already evaluated above;
            // dispatch it directly with the unevaluated operand tail so we
            // don't re-evaluate the operator expression (#226).
            if matches!(
                func,
                LispVal::Macro(_) | LispVal::Fexpr(_) | LispVal::Vau(_)
            ) {
                let rest = match original {
                    LispVal::Cons { cdr, .. } => (**cdr).clone(),
                    _ => LispVal::Nil,
                };
                return apply_unevaluated(&func, &rest, env);
            }

            // Evaluate all arguments (non-tail).
            let mut eval_args: Vec<LispVal> = Vec::with_capacity(args.len());
            for a in args {
                eval_args.push(exec(a, env)?);
            }

            // TCO for compiled lambdas (fixed-arity or `&rest`): skip the Rust
            // frame entirely by setting up the child env and looping.
            if let LispVal::Lambda(ref lambda) = func
                && let Some(ref compiled_body) = lambda.compiled
            {
                match lambda.rest_param_id {
                    None if lambda.params.len() == eval_args.len() => {
                        let new_env = Environment::new_child_with_dynamic(&lambda.env, env);
                        for (id, val) in lambda.param_ids.iter().zip(eval_args) {
                            new_env.set_id(*id, val);
                        }
                        return Ok(TcoStep::ExecTail(compiled_body.clone(), new_env));
                    }
                    Some(rest_param_id) if eval_args.len() >= lambda.params.len() => {
                        let new_env = Environment::new_child_with_dynamic(&lambda.env, env);
                        let n_fixed = lambda.params.len();
                        let mut eval_args = eval_args;
                        for (id, val) in lambda.param_ids.iter().zip(eval_args.drain(..n_fixed)) {
                            new_env.set_id(*id, val);
                        }
                        new_env.set_id(rest_param_id, vec_to_list(eval_args));
                        return Ok(TcoStep::ExecTail(compiled_body.clone(), new_env));
                    }
                    // Arity mismatch — fall through to `apply` for the error.
                    _ => {}
                }
            }

            // Fall back to `apply` for builtins, natives, uncompiled lambdas,
            // and arity mismatches.
            Ok(TcoStep::Done(apply(&func, &eval_args, env)))
        }
    }
}
