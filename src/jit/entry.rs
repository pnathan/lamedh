//! Raw native entry points for NATIVE leaf `defun*` kernels (issue #424).
//!
//! A [`NativeFnHandle`] lets a host call JIT-compiled Lisp *directly* — no
//! boxing, no membrane type-conversion, no dispatch — for a per-sample hot
//! loop (e.g. marching cubes over a Lisp SDF). Extract one with
//! [`crate::native_entry`]; the handle pins the *specific* native edition
//! current at extraction time (an `Arc` snapshot), so it keeps running even
//! after the function is redefined. See [`crate::jit::Jit::native_entry`] for
//! the extraction rules (leaf-only, raw-scalar signature).
//!
//! ## Caveats (read before wiring one into a hot loop)
//!
//! - **No flag/condition propagation.** A raw-entry call does *not* raise
//!   `OVERFLOW`/`DIV0` or an out-of-bounds error into the Lisp condition
//!   system the way the [`crate::jit::Jit::call`] membrane does. Integer
//!   overflow, division by zero, and out-of-bounds/`code-char` conditions are
//!   recorded per-thread and readable via [`NativeFnHandle::last_error`] after
//!   a call — the caller is responsible for checking it when it matters.
//! - **Unmetered.** Raw entries bypass the fuel/step accounting entirely;
//!   there is no interruption budget. Only extract kernels you trust to
//!   terminate.
//! - **Thread-safe.** A handle is `Send + Sync`; calling the same handle
//!   concurrently from many threads on disjoint inputs is sound (each call
//!   builds its own per-call context). The `last_error` state is per-thread.

use super::native::NativeEdition;
use super::runtime::Ctx;
use super::types::Ty;
use core::ffi::c_void;
use std::cell::RefCell;
use std::sync::Arc;

thread_local! {
    /// The error (if any) recorded by the most recent raw-entry call *on this
    /// thread*. Set after every call; read by [`NativeFnHandle::last_error`].
    static LAST_ERROR: RefCell<Option<String>> = const { RefCell::new(None) };
}

/// The raw machine-word type of a native-entry parameter or return value — the
/// scalar shapes a raw entry point can pass without boxing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeTy {
    /// A 64-bit signed integer, carried as its own word.
    Int64,
    /// An IEEE-754 double, carried bit-cast into a word (`f64::to_bits`).
    Float64,
    /// A boolean, carried as `0`/`1`.
    Bool,
}

impl NativeTy {
    /// The raw-scalar view of a typed-JIT [`Ty`], or `None` for a compound or
    /// boxed type that cannot cross a raw entry point.
    pub(super) fn from_ty(ty: &Ty) -> Option<NativeTy> {
        match ty {
            Ty::Int64 => Some(NativeTy::Int64),
            Ty::Float64 => Some(NativeTy::Float64),
            Ty::Bool => Some(NativeTy::Bool),
            _ => None,
        }
    }

    /// The surface-syntax name (`"int64"`/`"float64"`/`"bool"`).
    pub fn name(self) -> &'static str {
        match self {
            NativeTy::Int64 => "int64",
            NativeTy::Float64 => "float64",
            NativeTy::Bool => "bool",
        }
    }
}

/// A raw native entry point for a NATIVE leaf `defun*` kernel (issue #424).
///
/// Pins one specific native edition (`Arc` snapshot): redefinition of the Lisp
/// function does **not** invalidate or redirect the handle — it keeps running
/// the edition it captured. Re-extract via [`crate::native_entry`] to pick up a
/// redefinition; compare [`NativeFnHandle::generation`] against
/// [`crate::jit::Jit::generation`] to detect staleness.
///
/// `Send + Sync`: safe to share and call concurrently (see the module docs and
/// the `NativeEdition` safety audit in `native.rs`).
#[derive(Clone)]
pub struct NativeFnHandle {
    name: String,
    edition: Arc<NativeEdition>,
    params: Vec<NativeTy>,
    ret: NativeTy,
    generation: u64,
}

impl std::fmt::Debug for NativeFnHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NativeFnHandle")
            .field("name", &self.name)
            .field("params", &self.params)
            .field("ret", &self.ret)
            .field("generation", &self.generation)
            .finish()
    }
}

impl NativeFnHandle {
    pub(super) fn new(
        name: String,
        edition: Arc<NativeEdition>,
        params: Vec<NativeTy>,
        ret: NativeTy,
        generation: u64,
    ) -> NativeFnHandle {
        NativeFnHandle {
            name,
            edition,
            params,
            ret,
            generation,
        }
    }

    /// The (uppercased) name of the function this handle was extracted from.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The parameter types, in order.
    pub fn params(&self) -> &[NativeTy] {
        &self.params
    }

    /// The return type.
    pub fn ret(&self) -> NativeTy {
        self.ret
    }

    /// The generation of the native edition this handle pinned. Compare against
    /// [`crate::jit::Jit::generation`] for `name` to detect a redefinition:
    /// when they differ, the live function has been recompiled and this handle
    /// is running an older snapshot (still valid, just stale).
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// The error recorded by the most recent [`call_words`](Self::call_words)
    /// (or typed fast path) *on the current thread*, or `None` if that call
    /// completed cleanly. See the module docs: raw entries do not propagate
    /// these into the Lisp condition system.
    pub fn last_error(&self) -> Option<String> {
        LAST_ERROR.with(|c| c.borrow().clone())
    }

    /// The generic escape hatch: invoke the kernel with raw argument words and
    /// return its raw result word. The caller is responsible for encoding each
    /// argument per its [`params`](Self::params) type (an `int64` as its own
    /// word, a `float64` via [`f64::to_bits`], a `bool` as `0`/`1`) and
    /// decoding the result per [`ret`](Self::ret). Panics if `args.len()` does
    /// not match the parameter count.
    pub fn call_words(&self, args: &[u64]) -> u64 {
        assert_eq!(
            args.len(),
            self.params.len(),
            "native entry `{}`: expected {} argument word(s), got {}",
            self.name,
            self.params.len(),
            args.len()
        );
        // A fresh per-call leaf context on this thread's stack — no shared
        // state, so concurrent calls are independent.
        let ctx = Ctx::leaf();
        let ctx_ptr = &ctx as *const Ctx as *const c_void;
        // SAFETY: `ctx` is a live, valid `Ctx` that outlives the call; a leaf
        // edition reads only the flag/arena/error state of it (never `funcs`).
        // `args` has exactly the parameter count the native prologue reads.
        let raw = unsafe { self.edition.call(args, ctx_ptr) };
        let err = ctx.native_error();
        LAST_ERROR.with(|c| *c.borrow_mut() = err);
        raw
    }

    /// Typed fast path: a 3-`float64` → `float64` kernel (the SDF sampler
    /// shape). Debug-asserts the signature matches — the shape was already
    /// validated at extraction, so this only guards a caller wiring the wrong
    /// fast path to a handle.
    pub fn call_f3(&self, x: f64, y: f64, z: f64) -> f64 {
        debug_assert!(
            self.params == [NativeTy::Float64, NativeTy::Float64, NativeTy::Float64]
                && self.ret == NativeTy::Float64,
            "native entry `{}`: call_f3 shape mismatch (params {:?}, ret {:?})",
            self.name,
            self.params,
            self.ret
        );
        f64::from_bits(self.call_words(&[x.to_bits(), y.to_bits(), z.to_bits()]))
    }

    /// Typed fast path: a 1-`float64` → `float64` kernel.
    pub fn call_f1(&self, x: f64) -> f64 {
        debug_assert!(
            self.params == [NativeTy::Float64] && self.ret == NativeTy::Float64,
            "native entry `{}`: call_f1 shape mismatch (params {:?}, ret {:?})",
            self.name,
            self.params,
            self.ret
        );
        f64::from_bits(self.call_words(&[x.to_bits()]))
    }

    /// Typed fast path: a 1-`int64` → `int64` kernel.
    pub fn call_i1(&self, x: i64) -> i64 {
        debug_assert!(
            self.params == [NativeTy::Int64] && self.ret == NativeTy::Int64,
            "native entry `{}`: call_i1 shape mismatch (params {:?}, ret {:?})",
            self.name,
            self.params,
            self.ret
        );
        self.call_words(&[x as u64]) as i64
    }

    /// Typed fast path: a 2-`int64` → `int64` kernel.
    pub fn call_i2(&self, a: i64, b: i64) -> i64 {
        debug_assert!(
            self.params == [NativeTy::Int64, NativeTy::Int64] && self.ret == NativeTy::Int64,
            "native entry `{}`: call_i2 shape mismatch (params {:?}, ret {:?})",
            self.name,
            self.params,
            self.ret
        );
        self.call_words(&[a as u64, b as u64]) as i64
    }
}
