//! Typed JIT prototype: pre-runtime monomorphic type checking + closure compilation.
//!
//! Working slice of `docs/typed-jit-design.md` that needs no native-code backend
//! (no external deps; Cranelift slots in behind the same [`TypedFn`] interface
//! later, as a `jit` cargo feature).
//!
//! ## What works
//! - **Type membrane + inference.** `(defun-typed (name ret) ((arg ty)...)
//!   body...)` is elaborated by a bidirectional checker that runs *before*
//!   runtime and rejects ill-typed definitions. Elaboration *is* type checking
//!   (Turnstile-style): `Cx::elab` returns the typed [`Core`] and its [`Ty`].
//!   Type agreement is decided by HM-lite **unification** (`infer`): explicit
//!   annotations are principal-type pins, and a `let-typed` binding may omit its
//!   type to have it **inferred** from the initializer. Every type is `resolve`d
//!   to a concrete scalar before a definition is accepted (issue #135), the
//!   substrate the array/string element types (#137/#138) monomorphize on.
//! - **Basic compile.** `compile` lowers the typed core to a tree of closures
//!   over *unboxed* machine words. Runtime values are raw `u64`s: `int64` is the
//!   word, `float64` is `f64::to_bits`, `bool` is `0`/`1`. The static type tells
//!   each node how to read its word, so there is no tag and no `Rc` in the hot
//!   path.
//! - **Calls + recursion.** A [`Jit`] registry gives every function a stable id;
//!   calls go through the registry cell (design policy (a)), so self-recursion,
//!   cross-function calls, and — via [`Jit::declare`] — mutual recursion all work,
//!   and redefining a callee is just an edition swap.
//! - **Interpret-or-compiled dispatch + redefinition.** A call picks the compiled
//!   edition if present, else interprets; a call pins (`Rc`-clones) the edition it
//!   runs, so a swapped-out edition survives until in-flight callers return (the
//!   `Arc`/`ArcSwap` upgrade is #108).
//!
//! Scalar core: `int64`/`float64`/`bool`/`char` (= `u8`/`byte`); `+ - * / mod`
//! and comparisons `< > <= >= = /=` (operand-type directed), `and`/`or`/`not`,
//! `if`, `let-typed`, sequencing, and calls. `char` is an unboxed byte
//! (`0..=255` in a `u64`): it compares as an integer, converts to/from `int64`
//! via `char-code` / `code-char`, and crosses the membrane as a `LispVal::Char`
//! or an in-range `LispVal::Number` at input.
//!
//! Compound types (#137/#138): `(array T)` (element `T` **inferred**, never
//! annotated) and typed `struct`s. Both are a pointer to a flat `[len, e0, …]`
//! `u64` buffer rooted in the per-call arena (`Ctx`); access is bounds-checked
//! and panic-free. A `(array char)` is a string. `(array T)` ↔ `LispVal::Array`
//! / `LispVal::String`, `struct` ↔ nominal `LispVal::Struct`, at the membrane.
//!
//! Inference (#135): HM-lite unification + occurs-check + resolve (`infer`)
//! drives every type to a concrete monomorphic representation before codegen.
//! [`Jit::infer_untyped`] types a *fully un-annotated* function when its body is
//! an inferable typed island — HM firing under `defun` (via `jit-optimize`).
//!
//! Integer arithmetic wraps and integer `/`,`mod` by zero yield `0` (no panics);
//! this diverges from the checked tree-walker and is revisited with #67.

use crate::{LispVal, Shared, SharedCell, StructObj, TypedArrayObj};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

#[cfg(feature = "jit")]
mod native;

#[cfg(feature = "jit")]
pub mod entry;

mod elaboration;
mod infer;
mod parse;
mod registry;
mod runtime;
mod types;

use infer::Infer;

use self::elaboration::*;
use self::parse::*;
use self::runtime::*;
use self::types::*;

pub use self::registry::{Jit, TypedFn};
pub use self::runtime::{TraceStep, core_node_count, verify_core};
pub use self::types::{
    Analysis, BinOp, CmpOp, Core, JitFlags, NumKind, StructDef, Tier, Ty, Value, WritebackResult,
    elem_ty_matches, is_compileable, ty_name,
};

/// Try to parse a surface type form without a struct registry — handles scalar
/// keywords (`int64`, `float64`, `bool`, `char`, `u8`, `byte`) and explicit
/// `(array T)` forms. Used by `defun*` to distinguish a type annotation from
/// the start of a body form. Returns `None` for anything not recognised.
pub fn try_parse_ty_simple(form: &crate::LispVal) -> Option<Ty> {
    match form {
        crate::LispVal::Symbol(s) => Ty::parse(&s.borrow().name),
        crate::LispVal::Cons { car, cdr } => {
            // Only recognise (array T) where T is itself a simple type.
            if let (
                crate::LispVal::Symbol(h),
                crate::LispVal::Cons {
                    car: elem,
                    cdr: nil,
                },
            ) = (car.as_ref(), cdr.as_ref())
                && h.borrow().name == "ARRAY"
                && *nil.as_ref() == crate::LispVal::Nil
            {
                return try_parse_ty_simple(elem).map(|t| Ty::Array(Box::new(t)));
            }
            None
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests;
