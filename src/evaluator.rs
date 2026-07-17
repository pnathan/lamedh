//! Core evaluation engine: special forms, builtin primitives, and TCO trampoline.
//!
//! ## Architecture
//!
//! Evaluation is split into three layers:
//!
//! 1. **[`eval`]** / **`exec_entry`** — public entry points for the
//!    tree-walker and the compiled `Code` executor respectively.  Each
//!    acquires a recursion-depth guard and delegates to the shared
//!    `run_trampoline`.
//! 2. **`run_trampoline`** — a single loop that steps either `eval_step` (raw
//!    AST) or `exec_step` (compiled `Code`), decided by a `Current` enum. When
//!    a step returns `TcoStep::TailCall`/`TailCallWithGuards` the loop
//!    continues with a new AST expression; `TcoStep::ExecTail` continues with
//!    a compiled `Code` node instead — without growing the Rust stack either
//!    way (trampolining TCO). `TcoStep::Done` exits the loop. Unifying the two
//!    representations into one loop matters because a tail call can cross
//!    between them (a compiled lambda tail-calling an uncompiled form, or vice
//!    versa); two independent trampolines would make each crossing a plain,
//!    stack-growing Rust call (issue #200 M1' — this was a real regression).
//! 3. **`eval_step`** / **`exec_step`** — the actual pattern-match on the AST
//!    node or `Code` node.  Each returns either a finished value or a
//!    tail-call request.
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
use crate::{
    BuiltinFunc, ElemTy, LispError, LispVal, PortObj, Shared, SharedCell, SpecialForm, StructObj,
    TypedArrayObj,
    environment::{DynamicBinding, Environment},
};
use std::cell::Cell;
use std::collections::HashMap;

mod apply;
mod builtins_core;
mod builtins_extra;
mod builtins_net;
mod builtins_os;
mod builtins_ports;
mod builtins_regex;
mod builtins_tail;
mod builtins_tls;
mod compile;
pub(crate) mod core;
mod functions;
mod introspection;
mod quasiquote;
mod special_forms;

// The static checker (src/check.rs) mirrors `defun*`'s auto-detected
// parameter grammar; exporting the evaluator's own parser keeps the two
// permanently in agreement.
pub(crate) use self::special_forms::parse_star_params;

#[cfg(feature = "concurrency")]
mod builtins_concurrency;

#[cfg(test)]
mod tests;

use self::apply::*;
use self::builtins_core::*;
use self::builtins_extra::*;
use self::builtins_net::*;
use self::builtins_os::*;
use self::builtins_ports::*;
use self::builtins_tail::*;
use self::builtins_tls::*;
use self::compile::*;
use self::core::*;
use self::functions::*;
use self::introspection::*;
use self::quasiquote::*;
use self::special_forms::*;

#[cfg(feature = "concurrency")]
use self::builtins_concurrency::*;

pub use self::core::{
    DEFAULT_EVAL_DEPTH_LIMIT, eval_depth_limit, kernel_fuel_remaining, set_eval_depth_limit,
    set_kernel_fuel,
};
pub use self::functions::eval;

/// Crate-internal entry point for the host fast-call API (issue #423):
/// [`crate::call_function`]/[`crate::FnHandle::call`] in `lib.rs`. Applies an
/// already-resolved `func` to already-evaluated `args`, reusing the exact
/// application path `(funcall func args...)` takes — [`apply`], in
/// `evaluator::apply` — including driving any TCO trampolining inside the
/// callee's body to completion (a `Lambda` body's own tail calls loop inside
/// `apply`'s `eval` call; this function does not re-implement any of that).
///
/// Rejects `Macro`/`Vau`/`Fexpr`: their calling convention takes
/// *unevaluated* argument forms (they receive ASTs and choose what to
/// evaluate), which a fast-call API — given only already-evaluated
/// [`LispVal`]s and no source text — cannot supply. `name` is the lookup
/// name the caller resolved `func` from, used only for this error message.
pub(crate) fn apply_evaluated(
    name: &str,
    func: &LispVal,
    args: &[LispVal],
    env: &Shared<Environment>,
) -> Result<LispVal, LispError> {
    let kind = match func {
        LispVal::Macro(_) => Some("a macro"),
        LispVal::Vau(_) => Some("an operative (VAU)"),
        LispVal::Fexpr(_) => Some("a fexpr"),
        _ => None,
    };
    if let Some(kind) = kind {
        return Err(LispError::Generic(format!(
            "{name} is {kind}; fast-call passes evaluated arguments — use eval"
        )));
    }
    apply(func, args, env)
}
