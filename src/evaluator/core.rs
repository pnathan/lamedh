use super::*;
use smallvec::SmallVec;
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
pub(super) struct DepthGuard;

impl DepthGuard {
    pub(super) fn enter() -> Result<DepthGuard, LispError> {
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
pub(super) fn list_to_vec(list: &LispVal) -> Result<Vec<LispVal>, LispError> {
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
pub(super) fn vec_to_list(vec: Vec<LispVal>) -> LispVal {
    vec.into_iter()
        .rev()
        .fold(LispVal::Nil, |cdr, car| LispVal::Cons {
            car: Shared::new(car),
            cdr: Shared::new(cdr),
        })
}

pub(super) fn proper_list_len(list: &LispVal) -> Result<usize, LispError> {
    let mut len = 0;
    let mut current = list;
    loop {
        match current {
            LispVal::Nil => return Ok(len),
            LispVal::Cons { cdr, .. } => {
                len += 1;
                current = cdr;
            }
            _ => {
                return Err(LispError::Generic(
                    "length: argument must be a proper list".to_string(),
                ));
            }
        }
    }
}

/// Evaluate a call's operands directly off the cons chain.
///
/// This is the hot path for ordinary function/lambda/builtin application. It
/// replaces the older `list_to_vec(rest)` + `.iter().map(eval).collect()` pair,
/// which allocated **two** vectors per call (one of cloned argument *expressions*,
/// one of evaluated results) and cloned every argument expression. Walking the
/// cons cells and evaluating each `car` in place does the same work with a
/// single allocation and no expression clones while still rejecting malformed
/// dotted argument lists.
pub(super) fn eval_operands(
    rest: &LispVal,
    env: &Shared<Environment>,
) -> Result<SmallVec<[LispVal; 4]>, LispError> {
    let mut out = SmallVec::new();
    let mut cur = rest;
    while let LispVal::Cons { car, cdr } = cur {
        out.push(eval(car, env)?);
        cur = cdr;
    }
    if *cur != LispVal::Nil {
        return Err(LispError::Generic(
            "function call arguments must be a proper list".to_string(),
        ));
    }
    Ok(out)
}

/// Render a value for inclusion in an error message, truncated so a huge
/// list or string cannot flood the output (issue #247).
pub(super) fn err_val(v: &LispVal) -> String {
    let s = crate::printer::print(v);
    if s.chars().count() > 60 {
        let head: String = s.chars().take(57).collect();
        format!("{head}...")
    } else {
        s
    }
}

/// Reject binding/assignment targets that must stay constant.
///
/// `T` is the canonical truth constant and keywords are self-evaluating;
/// rebinding either silently corrupts truth tests far from the mistake
/// (issue #237). `NIL` needs no check here — it reads as the `Nil` value,
/// not a symbol, so it can never reach a binder as a name.
pub(super) fn check_bindable(name: &str, context: &str) -> Result<(), LispError> {
    if name == "T" {
        Err(LispError::Generic(format!(
            "{context}: cannot rebind the constant T"
        )))
    } else if name.starts_with(':') {
        Err(LispError::Generic(format!(
            "{context}: cannot bind the keyword {name}"
        )))
    } else {
        Ok(())
    }
}

/// Reject unsupported lambda-list keywords (`&OPTIONAL`, `&KEY`, ...) so they
/// fail loudly at definition time instead of silently becoming parameters
/// literally named `&OPTIONAL` (issue #242). Callers handle `&REST` before
/// this check, so any `&`-name reaching here is unsupported.
pub(super) fn check_param_name(name: &str, kind: &str) -> Result<(), LispError> {
    if name.starts_with('&') {
        return Err(LispError::Generic(format!(
            "{name} is not supported in {kind} parameter lists (only &REST is)"
        )));
    }
    check_bindable(name, kind)
}

/// Wrap a body of one or more forms into a single evaluable expression.
///
/// A lone form is returned as-is; several forms are wrapped in `(PROGN ...)`
/// so the existing `PROGN` trampoline sequences them and keeps TCO on the last
/// form. Used by the binding special forms (`LET`/`LET*`) to accept multi-form
/// bodies. `forms` is expected to be non-empty; an empty slice yields `(PROGN)`,
/// which evaluates to `NIL`.
pub(super) fn wrap_body_forms(forms: &[LispVal], env: &Shared<Environment>) -> LispVal {
    if forms.len() == 1 {
        forms[0].clone()
    } else {
        let mut list = Vec::with_capacity(forms.len() + 1);
        list.push(LispVal::Symbol(env.intern_symbol("PROGN")));
        list.extend_from_slice(forms);
        vec_to_list(list)
    }
}
