// Typed-JIT spike — ILLUSTRATIVE ONLY.
//
// This file is intentionally NOT part of the `lamedh` crate (it lives under
// docs/spike/ so `cargo build`/`cargo clippy` stay green). It sketches the
// `infer -> Cranelift` path and, more importantly, the FunctionCell dispatch
// that makes REPL redefinition safe. Types like `cranelift_*` and `LispVal`
// are referenced as they would be wired up, not as a compiling unit.
//
// See docs/typed-jit-design.md for the prose.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
// In the real crate: use arc_swap::ArcSwap; here we name it abstractly.
use arc_swap::ArcSwap;

use crate::LispVal;

// ---------------------------------------------------------------------------
// 1. Types proven by the pre-runtime HM membrane. The FnType IS the ABI.
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Ty {
    Int64,
    Float64,
    // ... Bool, Str, Cons(a), Fn(.. -> ..), and `Any` for the gradual edge.
    Any,
}

#[derive(Clone, PartialEq, Eq, Debug)]
struct FnType {
    params: Vec<Ty>,
    ret: Ty,
}

/// The typed core the membrane emits. Small, applicative, no fexpr/eval.
/// (Real impl: lower straight from this; sketch keeps it tiny.)
enum Core {
    LitI64(i64),
    Var(u32),                 // de Bruijn / slot index, resolved at type time
    Add(Box<Core>, Box<Core>),
    Mul(Box<Core>, Box<Core>),
    // If(cond, then, else), Let(init, body), Call(cell, args), ...
}

// ---------------------------------------------------------------------------
// 2. An executable edition. Drop = munmap. Reclaimed, never eagerly freed.
// ---------------------------------------------------------------------------

/// Boxed ABI: the convention used at the membrane and by interpreted callers.
type NativeEntry = extern "C" fn(args: *const LispVal, argc: usize) -> LispVal;

struct ExecutableMemory {
    ptr: *mut u8,
    len: usize,
}
impl Drop for ExecutableMemory {
    fn drop(&mut self) {
        // unsafe { libc::munmap(self.ptr.cast(), self.len) };
        // Only runs once the last Arc<CompiledCode> referencing it is gone,
        // i.e. after every in-flight call into this edition has returned.
    }
}

struct CompiledCode {
    entry: NativeEntry,        // boxed ABI (LispVal in / LispVal out) at the edge
    // fast_entry: Option<...>, // island-internal unboxed convention for typed->typed
    ty: FnType,                // edition's signature
    generation: u64,           // matches the cell generation it was built for
    _pages: ExecutableMemory,  // keeps the code mapped while this Arc is alive
}

// ---------------------------------------------------------------------------
// 3. The indirection cell. One per function name. Callers go THROUGH it, so a
//    (re)compile is a single atomic store and the old edition stays valid for
//    frames already running it.
// ---------------------------------------------------------------------------

struct FunctionCell {
    ty: FnType,                                  // current signature
    interp: Core,                                // source of truth, ALWAYS runnable
    compiled: ArcSwap<Option<Arc<CompiledCode>>>,// lock-free hot-swap
    generation: AtomicU64,                       // bumped on every (re)definition
}

impl FunctionCell {
    /// A function CALL is a runtime dispatch: compiled edition if present and
    /// current, else interpret. One cheap atomic load; the loaded Arc pins the
    /// edition for the duration of the call.
    fn call(&self, args: &[LispVal]) -> LispVal {
        let cur = self.compiled.load(); // Guard<Option<Arc<CompiledCode>>>
        match &**cur {
            Some(code) => (code.entry)(args.as_ptr(), args.len()),
            None => interp_typed_core(&self.interp, args),
        }
        // `cur` (and thus the Arc) drops here, AFTER the native call returns:
        // that is what keeps a just-replaced edition's pages mapped until the
        // last in-flight caller is done.
    }

    /// Eager/AOT compile: types are PROVEN, so no call-count warmup is needed.
    /// Install None first (interpreter serves immediately), then store the
    /// edition when ready — synchronously for small bodies, or off-thread.
    fn (re)define(&self, new_ty: FnType, new_core: Core) {
        // pseudo: self.ty = new_ty; self.interp = new_core;
        let gen = self.generation.fetch_add(1, Ordering::AcqRel) + 1;
        self.compiled.store(Arc::new(None));     // fall back to interp instantly
        // ... HM already ran at the macro layer and rejected ill-typed defs ...
        let edition = jit_compile(&self.interp, &self.ty, gen); // §4
        self.compiled.store(Arc::new(Some(Arc::new(edition))));
        // Any thread still inside the previous edition holds its own Arc; its
        // ExecutableMemory::drop (munmap) runs only when that Arc count hits 0.
    }
}

fn interp_typed_core(_core: &Core, _args: &[LispVal]) -> LispVal {
    // The existing tree-walker, specialized to the typed core. Always correct;
    // this is the deopt target and the pre-JIT serving path.
    unimplemented!()
}

// ---------------------------------------------------------------------------
// 4. Lower typed core -> Cranelift IR -> executable edition.
//    HM made this trivial: int64 -> i64, no tags, no boxing inside the island.
// ---------------------------------------------------------------------------

fn jit_compile(core: &Core, ty: &FnType, generation: u64) -> CompiledCode {
    // use cranelift_codegen::ir::{types, AbiParam, InstBuilder, ...};
    // use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext};
    // use cranelift_jit::{JITBuilder, JITModule};
    //
    // 1. Build a signature from `ty`: each Int64 param -> AbiParam::new(I64).
    //    NOTE: the *public* entry uses the boxed NativeEntry ABI; it unboxes
    //    args (LispVal::Number -> i64) at the prologue and re-boxes the result
    //    at the epilogue. That prologue/epilogue IS the contract membrane (§2).
    // 2. emit(core): recurse, leaving the SSA value of each node:
    //      LitI64(n) -> builder.ins().iconst(I64, n)
    //      Add(a,b)  -> builder.ins().iadd(emit(a), emit(b))
    //      Mul(a,b)  -> builder.ins().imul(emit(a), emit(b))
    //      Var(i)    -> the i-th (already unboxed) block param
    //      Call(c,_) -> load c.cell.entry and `call_indirect` (policy (a),
    //                   §3.4): always correct across redefinition.
    // 3. module.finalize_definitions(); get the raw code ptr; wrap in
    //    ExecutableMemory; transmute to NativeEntry.
    let _ = (core, ty, generation);
    unimplemented!("wire cranelift-jit here")
}

// ---------------------------------------------------------------------------
// 5. The spike's acceptance, as tests would assert:
//
//   (defun-typed (sq int64) ((x int64)) (* x x))
//     -> infer: sq : int64 -> int64
//     -> Core::Mul(Var(0), Var(0))
//     -> jit_compile -> edition E0
//     -> cell.call(&[Number(7)]) == Number(49)              // == tree-walker
//     -> tight loop of (sq k): ~50x the interpreter
//
//   redefinition safety:
//     thread A: long-running cell.call (edition E0 on stack)
//     thread B: cell.redefine(... (* x x x) ...)  -> edition E1
//     - A finishes against E0 (valid pages), returns old-shaped result
//     - E0's Arc count -> 0 -> ExecutableMemory::drop -> munmap (no UAF)
//     - subsequent cell.call hits E1
// ---------------------------------------------------------------------------
