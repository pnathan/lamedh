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
use crate::{
    BuiltinFunc, LispError, LispVal, Shared, SharedCell, StructObj, environment::Environment,
};
use std::cell::Cell;
use std::collections::HashMap;

mod apply;
mod builtins_core;
mod builtins_extra;
mod builtins_tail;
mod core;
mod functions;
mod introspection;
mod quasiquote;
mod special_forms;

#[cfg(test)]
mod tests;

use self::apply::*;
use self::builtins_core::*;
use self::builtins_extra::*;
use self::builtins_tail::*;
use self::core::*;
use self::functions::*;
use self::introspection::*;
use self::quasiquote::*;
use self::special_forms::*;

pub use self::core::{DEFAULT_EVAL_DEPTH_LIMIT, eval_depth_limit, set_eval_depth_limit};
pub use self::functions::eval;
