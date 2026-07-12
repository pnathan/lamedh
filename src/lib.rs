//! # Lamedh — an embeddable Lisp 1.5 interpreter
//!
//! Lamedh (Hebrew: ל, "Lamed") is a complete, embeddable Lisp 1.5 interpreter
//! written in Rust.  It provides:
//!
//! - A tree-walking evaluator with trampolining tail-call optimisation (TCO)
//! - Lexical closures, macros, fexprs, and Kernel-style vau operatives
//! - Both lexical and dynamic (special) variable scoping
//! - An extensible type system via [`LispValExtension`]
//! - A capability-gated sandbox (filesystem, shell, stdin — all off by default)
//! - A full standard library embedded in the binary (no `.lisp` files needed)
//!
//! ## Quick start
//!
//! Add to `Cargo.toml`:
//!
//! ```toml
//! [dependencies]
//! lamedh = "0.2"
//! ```
//!
//! Evaluate Lisp expressions from Rust:
//!
//! ```rust,ignore
//! use lamedh::{eval_str, LispVal};
//! use lamedh::environment::Environment;
//!
//! lamedh::with_large_stack(|| {
//!     let env = Environment::with_stdlib();
//!
//!     // Evaluate a simple expression
//!     let val = eval_str("(+ 1 2 3)", &env).unwrap();
//!     assert_eq!(val, LispVal::Number(6));
//!
//!     // Define a function and call it
//!     eval_str("(defun factorial (n) (if (zerop n) 1 (* n (factorial (sub1 n)))))", &env).unwrap();
//!     let result = eval_str("(factorial 10)", &env).unwrap();
//!     assert_eq!(result, LispVal::Number(3628800));
//! });
//! ```
//!
//! ## Stack size
//!
//! The tree-walking evaluator recurses in Rust for non-tail calls.  The default
//! system thread stack (2–8 MiB) overflows before the recursion-depth guard
//! fires.  Always wrap the interpreter entry point in [`with_large_stack`], which
//! spawns a 512 MiB thread.  Because [`LispVal`] and [`environment::Environment`]
//! are `!Send`, create the environment *inside* the closure.
//!
//! ## Embedding pattern
//!
//! ```rust,ignore
//! use lamedh::{eval_str, LispError, LispVal};
//! use lamedh::environment::Environment;
//!
//! fn run_script(src: &str) -> Result<LispVal, LispError> {
//!     lamedh::with_large_stack(move || {
//!         let env = Environment::with_stdlib();
//!
//!         // Register a host function
//!         env.register_fn("greet", |args, _env| {
//!             let name = args[0].as_str_val()?;
//!             Ok(LispVal::from(format!("Hello, {name}!")))
//!         });
//!
//!         eval_str(src, &env)
//!     })
//! }
//! ```
//!
//! ## Sandboxing
//!
//! All potentially dangerous capabilities are disabled by default.  Use
//! [`environment::Environment::enable_feature`] to opt in:
//!
//! ```rust,ignore
//! // Allow scripts to call (shell "ls")
//! env.enable_feature("SHELL");
//!
//! // Allow read-only filesystem access: read-file, file-exists-p, directory-files, …
//! env.enable_feature("READ-FS");
//!
//! // Allow filesystem mutations: write-file, chmod, create-directory, delete-file, rename-file
//! env.enable_feature("CREATE-FS");
//!
//! // Allow temp-file creation: make-temp-file, make-temp-directory
//! env.enable_feature("TEMP-FS");
//!
//! // Allow (read) to consume stdin
//! env.enable_feature("IO");
//! ```
//!
//! ## Module structure
//!
//! | Module | Role |
//! |--------|------|
//! | [`environment`] | Variable binding, symbol interning, dynamic scoping |
//! | [`reader`] | nom-based s-expression parser |
//! | [`evaluator`] | Core eval loop with TCO, special forms, 100+ builtins |
//! | [`printer`] | Format [`LispVal`] back to readable text |
//! | [`optimizer`] | Constant-folding source-level optimizer |
//!
//! ## Language overview
//!
//! Lamedh is a Lisp 1.5 dialect with modern extensions:
//!
//! | Feature | Lisp form |
//! |---------|-----------|
//! | Variable binding | `(def x 42)`, `(setq x 99)` |
//! | Function definition | `(defun square (n) (* n n))` |
//! | Conditionals | `(if p a b)`, `(cond (p1 e1) (p2 e2))` |
//! | Local scope | `(let ((x 1) (y 2)) (+ x y))` |
//! | Closures | `(lambda (x) (* x x))` |
//! | Tail-recursive loops | `(defun loop (n) (if (zerop n) t (loop (sub1 n))))` |
//! | Macros | `(defmacro when (p &rest b) \`(if ,p (progn ,@b) nil))` |
//! | Fexprs | `(defexpr my-quote (args) (car args))` |
//! | Kernel vau | `(vau (ops e) (eval (car ops) e))` |
//! | Hash tables | `(let ((h (make-hash-table))) (set-bang h 'x 1) (gethash h 'x))` |
//! | Arrays | `(let ((a (array 5))) (store a 0 99) (fetch a 0))` |
//! | Property lists | `(putp 'foo "doc" "A foo.") (getp 'foo "doc")` |
//! | Records | `(defrecord point (x int64) (y int64)) (make-point 1 2)` |
//!
//! ## Cargo manifest
//!
//! ```toml
//! [package]
//! name        = "lamedh"
//! version     = "0.2.0"
//! edition     = "2024"
//! description = "An embeddable Lisp 1.5 interpreter written in Rust"
//! license     = "AGPL-3.0"
//!
//! [lib]
//! name = "lamedh"
//! path = "src/lib.rs"
//!
//! [dependencies]
//! nom = "=7.1.3"   # pinned reader dependency
//!
//! # Default typed-JIT backend. Disable default features for the
//! # dependency-light typed checker / closure interpreter path.
//! cranelift-jit      = { version = "0.133", optional = true }
//! cranelift-module   = { version = "0.133", optional = true }
//! cranelift-codegen  = { version = "0.133", optional = true }
//! cranelift-frontend = { version = "0.133", optional = true }
//!
//! [features]
//! default = ["jit"]
//! jit = [
//!     "dep:cranelift-jit",
//!     "dep:cranelift-module",
//!     "dep:cranelift-codegen",
//!     "dep:cranelift-frontend",
//! ]
//!
//! [workspace]
//! members         = ["cli"]
//! default-members = [".", "cli"]
//! # Benchmark comparison crates (benchmarks/*/rust) are excluded from the
//! # workspace and built directly by benchmarks/run_benchmarks.sh.
//! ```
//!
//! The dependency-light library build uses `nom` (parser combinators for the
//! reader). The default 0.2.x build also enables the typed JIT's Cranelift
//! backend; use `--no-default-features` to omit it. All I/O is gated behind
//! capability flags so embedders get a sandboxed interpreter by default.
//!
//! The companion binary crate `lamedh-cli` (in `cli/`) adds `rustyline` (REPL
//! line-editing) and `clap` (argument parsing) but does not affect the library.
//!
//! ## Standard library: Prelude vs. optional modules
//!
//! [`environment::Environment::with_stdlib`] loads every file below, in
//! order, all embedded in the binary at compile time (no `.lisp` files are
//! needed at runtime) — unchanged since before issue #256.
//! [`environment::Environment::with_prelude`] is a lighter alternative that
//! loads only the **Prelude** rows; pull in an **optional** row by name with
//! `(require 'name)` (see `lib/06-require.lisp`) once you have either kind
//! of environment. `with_stdlib` is defined as Prelude plus every optional
//! module, so it remains fully source- and behavior-compatible.
//!
//! | File | Tier | Requirable as | Key functions |
//! |------|------|---------------|--------------|
//! | `00-core.lisp` | Prelude | — | `DEFUN`, `PROG2`, `CSET`, `CSETQ` |
//! | `01-list.lisp` | Prelude | — | `APPEND`, `MEMBER`, `LENGTH`, `REVERSE`, `PAIRLIS`, `NULL`, `NCONC`, `COPY`, `SASSOC`, `MAPC`, `MAPCON` |
//! | `02-cxr.lisp` | Prelude | — | All 30 CAR/CDR compositions `CAAR`…`CDDDDR` |
//! | `03-meta.lisp` | Prelude | — | `DOCUMENTATION` |
//! | `04-predicates.lisp` | Prelude | — | `EQUAL` (structural equality) |
//! | `05-math.lisp` | Prelude | — | `ONEP`, `MINUSP`, `ADD1`, `SUB1`, `MAX`, `MIN`, `ABS` |
//! | `06-require.lisp` | Prelude | — | `REQUIRE`, `PROVIDE`, `REQUIRE-RELOAD`, `LOADED-MODULES`, `MODULE-INFO` |
//! | `07-shell.lisp` | optional | `shell` | `SH`, `SHELL-STDOUT`, `SHELL-STDERR`, `SHELL-EXIT-CODE`, `SHELL-OK-P` |
//! | `08-vau.lisp` | Prelude | — | Kernel-style derived forms: `$IF`, `$AND`, `$OR`, `$SEQUENCE` |
//! | `09-lisp15.lisp` | optional | `lisp15` | Lisp 1.5 appendix A: `PAIR`, `ATTRIB`, `PROP`, `FLAG`, `REMFLAG`, `MAP`, `SEARCH`, `RECIP`, `SELECT`, `TRACE` |
//! | `10-testing.lisp` | optional | `testing` | xUnit framework: `DEFTEST`, `ASSERT-EQUAL`, `ASSERT-TRUE`, `ASSERT-FALSE`, `ASSERT-NIL`, `RUN-TESTS`, `CLEAR-TESTS` |
//! | `11-optimizer-vau.lisp` | optional | `optimizer-vau` | Source optimizer: `OPTIMIZE-FORM`, `$OPT` |
//! | `12-control.lisp` … `18-format.lisp` | Prelude | — | Control flow, functional helpers, strings, sets/hash, conditions, arrays, `FORMAT` |
//! | `19-call-graph.lisp` | optional | `call-graph` | Call-graph analysis: `$CALL-GRAPH`, `CALL-GRAPH-CALLEES`, `CALL-GRAPH-CALLERS` |
//! | `20-condensation.lisp` | optional | `condensation` | Condensation: `DEFRECORD`/`DERIVE`, branded field access, sexpr change plane (`CONDENSE-DIFF`, `SEXPR-PATCH`, `EDIT!`) |
//! | `21-cl-compat.lisp` | Prelude | — | Common Lisp compat: `SETF`, `PUSH`/`POP`, `INCF`/`DECF`, `REMOVE`, `SUBSEQ`, `ELT`, `DEFPARAMETER`, ... |
//! | `22-guard.lisp` | optional | `guard` | Guard fences (#284) + capability processes (#140): `WITH-FUEL`, `WITH-CAPABILITIES`, `SANDBOXED`, `CAPABILITIES-NEEDED`, `SPAWN`/`SPAWN*`/`AWAIT` |
//! | `23-match.lisp` | optional | `match` | Structural pattern language: `PAT-MATCH`, `MATCH`, `DESTRUCTURING-BIND`, `SGREP`/`SGREP-FN` (issue #171), `REWRITE`, `INSTANTIATE` |
//! | `24-rules.lisp` | optional | `rules` | Rulebook optimizer: `DEFRULE`/`UNDEFRULE`/`LIST-RULES`/`APPLY-RULES` — optimization passes as pattern-language data |
//! | `25-variants.lisp` | optional | `variants` | Sum types: `DEFVARIANT`, exhaustive `VARIANT-CASE`, Option, Result |
//! | `26-instrument.lisp` | optional | `instrument` | `TRACE`/`UNTRACE`/`TIME`/`STEP-COUNT` (steps = the WITH-FUEL unit) |
//! | `27-modules.lisp` | optional | `modules` | Namespacing: `DEFMODULE`/`WITH-MODULE`/`IMPORT`, `MODULE:SYMBOL` names, custom capabilities |
//! | `28-types.lisp` | optional | `types` | The type table: verified declared schemes for builtins and stdlib functions |
//! | `29-protocols.lisp` | optional | `protocols` | THE dispatch system: typed protocols (`DEFPROTOCOL`/`DEFINSTANCE`, inference-selected instances) + conformance (`IMPLEMENTS!`/`IMPLEMENTS-P`) |
//! | `30-text.lisp` | optional | `text` | Explicit String ↔ UTF-8 `Array<Char>` boundary: `TEXT:STRING->UTF8`, `TEXT:UTF8->STRING` |
//! | `31-ports.lisp` | optional | `ports` | Synchronous binary ports: `PORTS:OPEN-INPUT`/`OPEN-OUTPUT`/`OPEN-APPEND`, `READ-BYTE!`/`READ-BYTES!`/`WRITE-BYTE!`/`WRITE-BYTES!`, `WITH-OPEN-PORT` |
//! | `32-base64.lisp` | optional | `base64` | `BASE64:ENCODE`/`DECODE`: `Array<Char>` bytes <-> Base64 `String`, `:STANDARD`/`:URL` alphabets, explicit padding |
//! | `33-hex.lisp` | optional | `hex` | `HEX:ENCODE`/`DECODE`: `Array<Char>` bytes <-> hexadecimal `String`, predictable-case encode, case-insensitive decode |
//! | `34-url.lisp` | optional | `url` | `URL:ENCODE-PATH-SEGMENT`/`ENCODE-QUERY-COMPONENT`/`DECODE`, `URL:PARSE`/`BUILD`, `URL:PARSE-QUERY`/`BUILD-QUERY` |
//! | `35-json.lisp` | optional | `json` | `JSON:PARSE`/`STRINGIFY`: object<->hash table, array<->`Array`, `true`/`false`/`null`<->`T`/`NIL`/`:NULL`, `JSON:NULL-P` |
//! | `36-mime.lisp` | optional | `mime` | `MIME:HEADERS-GET`/`GET-ALL`/`ADD`/`SET`/`REMOVE`/`NAMES` (case-insensitive, multi-value-safe), `MIME:PARSE-CONTENT-TYPE`/`BUILD-CONTENT-TYPE` |
//! | `97-doc-renderer.lisp` | optional | `doc-renderer` | REPL documentation renderer |
//! | `98-help-system.lisp` | optional | `help-system` | `(HELP)`, `(HELP 'fn)`, `(HELP 'categories)` |
//! | `99-help-data.lisp` | optional | `help-data` | Structured documentation database for all built-ins |
//!
//! Shell helpers (`07-shell.lisp`) require the `SHELL` capability to be granted
//! by the host (`env.enable_feature("SHELL")`) or CLI (`--capability SHELL`)
//! — independent of, and in addition to, whatever loads the file itself. The
//! testing framework (`10-testing.lisp`) is automatically available when you
//! call [`environment::Environment::with_stdlib`], or via `(require 'testing)`
//! on a lighter [`environment::Environment::with_prelude`] environment.
//!
//! Embedders can also register their own optional libraries — see
//! [`environment::Environment::register_module`] — and, with the `READ-FS`
//! capability, a host can configure disk directories `require` searches as a
//! last resort (see [`environment::Environment::add_module_search_path`]).

pub mod environment;
pub mod evaluator;
pub mod jit;
pub mod optimizer;
pub mod printer;
pub mod reader;

pub use evaluator::{DEFAULT_EVAL_DEPTH_LIMIT, eval_depth_limit, set_eval_depth_limit};

/// Stack size used by [`with_large_stack`]. The tree-walking interpreter uses
/// large stack frames, so deep recursion needs substantial headroom (the
/// recursion-depth guard fires before this is exhausted; see issue #61).
pub const INTERPRETER_STACK_SIZE: usize = 512 * 1024 * 1024;

/// Run `f` on a freshly spawned thread with a large stack ([`INTERPRETER_STACK_SIZE`]).
///
/// The default thread stack (often 2–8 MiB) is too small for the interpreter's
/// large frames to recurse meaningfully. The CLI and the test harness use this;
/// embedders running the interpreter from a small-stack thread should too (or
/// lower [`set_eval_depth_limit`]). Because `LispVal`/`Environment` are `!Send`,
/// create the environment *inside* `f`.
pub fn with_large_stack<F, T>(f: F) -> T
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
{
    std::thread::Builder::new()
        .stack_size(INTERPRETER_STACK_SIZE)
        .spawn(f)
        .expect("failed to spawn interpreter thread")
        .join()
        .expect("interpreter thread panicked")
}

use environment::Environment;
use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::hash::{Hash, Hasher};
use std::io::{BufRead, Read, Seek, SeekFrom, Write};
use std::net::{TcpListener, TcpStream, UdpSocket};
use std::path::{Path, PathBuf};

// ── Shared reference-counting and interior-mutability type aliases ─────────
//
// When the `arc-val` feature is enabled, reference-counted shared pointers
// become `Arc` and interior mutable cells become a thin `RwLock`-backed
// newtype with a `RefCell`-compatible API (`borrow` / `borrow_mut`).
// The default single-threaded build uses `Rc`/`RefCell` and is unchanged.
//
// Note: enabling `arc-val` provides atomic reference counts but does NOT
// automatically make `LispVal: Send + Sync` because `SharedState` still
// contains `Cell<bool>`.  This feature is a stepping stone toward full
// thread safety, not a complete solution on its own.

/// A reference-counted shared pointer.
///
/// `Rc<T>` by default; `Arc<T>` with the `arc-val` Cargo feature.
/// Use `Shared::new(v)`, `Shared::ptr_eq(a, b)`, and `.as_ptr()` so
/// the same code compiles under both builds.
#[cfg(not(feature = "arc-val"))]
pub type Shared<T> = std::rc::Rc<T>;
#[cfg(feature = "arc-val")]
pub type Shared<T> = std::sync::Arc<T>;

/// An interior-mutable cell with a `RefCell`-compatible API.
///
/// `RefCell<T>` by default; a thin `RwLock<T>` wrapper with the `arc-val`
/// feature.  Both variants expose `.borrow()` and `.borrow_mut()`.
#[cfg(not(feature = "arc-val"))]
pub type SharedCell<T> = std::cell::RefCell<T>;

/// `RwLock`-backed interior-mutable cell with a `RefCell`-compatible API.
/// Only present with the `arc-val` Cargo feature.
#[cfg(feature = "arc-val")]
pub struct SharedCell<T>(std::sync::RwLock<T>);

#[cfg(feature = "arc-val")]
impl<T> SharedCell<T> {
    /// Create a new cell wrapping `value`.  Mirrors `RefCell::new`.
    #[inline]
    pub fn new(value: T) -> Self {
        SharedCell(std::sync::RwLock::new(value))
    }
    /// Acquire a shared read lock.  Mirrors `RefCell::borrow`.  Panics if
    /// the lock is poisoned.
    #[inline]
    pub fn borrow(&self) -> std::sync::RwLockReadGuard<'_, T> {
        self.0.read().unwrap()
    }
    /// Acquire an exclusive write lock.  Mirrors `RefCell::borrow_mut`.
    /// Panics if the lock is poisoned.
    #[inline]
    pub fn borrow_mut(&self) -> std::sync::RwLockWriteGuard<'_, T> {
        self.0.write().unwrap()
    }
}

#[cfg(feature = "arc-val")]
impl<T: std::fmt::Debug> std::fmt::Debug for SharedCell<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.0.read() {
            Ok(g) => write!(f, "SharedCell({:?})", *g),
            Err(_) => write!(f, "SharedCell(<poisoned>)"),
        }
    }
}

#[cfg(feature = "arc-val")]
impl<T: Clone> Clone for SharedCell<T> {
    fn clone(&self) -> Self {
        SharedCell::new(self.0.read().unwrap().clone())
    }
}

#[cfg(feature = "arc-val")]
impl<T: PartialEq> PartialEq for SharedCell<T> {
    fn eq(&self, other: &Self) -> bool {
        *self.0.read().unwrap() == *other.0.read().unwrap()
    }
}

/// Trait for host-defined value types that can participate in the Lisp system.
///
/// Implement this trait on your Rust type and wrap it with
/// `LispVal::ext(value)` to make it available to Lisp code.
///
/// ```rust,ignore
/// use lamedh::{LispVal, LispValExtension, LispError};
///
/// struct MyPoint { x: f64, y: f64 }
///
/// impl LispValExtension for MyPoint {
///     fn type_name(&self) -> &str { "point" }
///     fn display(&self) -> String { format!("#<point {},{}>", self.x, self.y) }
///     fn eq_ext(&self, other: &dyn LispValExtension) -> bool {
///         other.as_any().downcast_ref::<MyPoint>()
///             .map_or(false, |p| p.x == self.x && p.y == self.y)
///     }
///     fn hash_ext(&self, state: &mut dyn std::hash::Hasher) {
///         self.x.to_bits().hash(state);
///         self.y.to_bits().hash(state);
///     }
///     fn as_any(&self) -> &dyn std::any::Any { self }
/// }
///
/// let pt = LispVal::ext(MyPoint { x: 1.0, y: 2.0 });
/// ```
pub trait LispValExtension: fmt::Debug {
    /// Short type tag shown in error messages and the printer.
    fn type_name(&self) -> &str;
    /// Human-readable representation (used by PRINT/PRIN1).
    fn display(&self) -> String;
    /// Value equality with another extension value.
    fn eq_ext(&self, other: &dyn LispValExtension) -> bool;
    /// Hash for use as a hash-table key.
    fn hash_ext(&self, state: &mut dyn Hasher);
    /// Downcast support — return `self` as `Any`.
    fn as_any(&self) -> &dyn std::any::Any;
}

/// Runtime error type for the Lisp interpreter.
///
/// Most user-visible errors are [`LispError::Generic`].  The other variants are
/// internal control-flow signals used by `PROG`/`RETURN`/`GO` and should never
/// escape a top-level [`eval_str`] call under normal circumstances; if they do,
/// treat them as bugs.
#[derive(Debug, Clone)]
pub enum LispError {
    /// A runtime error with a human-readable message.
    Generic(String),
    /// Carries a value out of a `PROG` form via `(RETURN val)`.
    /// Not a true error; caught by the `PROG` evaluator.
    Return(Box<LispVal>),
    /// Carries a label name out of a `PROG` form via `(GO label)`.
    /// Not a true error; caught by the `PROG` evaluator.
    Go(String),
    /// Carries a value out of a `CATCH` form via `(THROW tag value)`.
    /// Not a true error; caught by the matching `CATCH` (compared by tag value).
    Throw {
        tag: Box<LispVal>,
        value: Box<LispVal>,
    },
    /// Carries a value out of a `BLOCK` via `(RETURN-FROM name value)`.
    /// Not a true error; caught by the matching named `BLOCK`.
    ReturnFrom { name: String, value: Box<LispVal> },
    /// A signalled first-class condition value (typically a [`LispVal::Error`]).
    /// Raised by `(error ...)`; trapped by `ERRORSET` (→ NIL) and bound by the
    /// handler variable in `HANDLER-CASE`.
    Signaled(Box<LispVal>),
}

impl PartialEq for LispError {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (LispError::Generic(a), LispError::Generic(b)) => a == b,
            (LispError::Go(a), LispError::Go(b)) => a == b,
            (LispError::Return(v1), LispError::Return(v2)) => v1 == v2,
            (LispError::Signaled(a), LispError::Signaled(b)) => a == b,
            _ => false,
        }
    }
}

impl fmt::Display for LispError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            LispError::Generic(s) => write!(f, "Error: {s}"),
            LispError::Return(_) => write!(f, "Internal LispError: RETURN used outside of PROG."),
            LispError::Go(_) => write!(f, "Internal LispError: GO used outside of PROG."),
            LispError::Throw { .. } => {
                write!(f, "Internal LispError: THROW with no matching CATCH.")
            }
            LispError::ReturnFrom { .. } => {
                write!(f, "Internal LispError: RETURN-FROM with no matching BLOCK.")
            }
            LispError::Signaled(v) => match v.as_ref() {
                LispVal::Error(e) => write!(f, "Error: {}", e.message),
                other => write!(f, "Error: {}", crate::printer::print(other)),
            },
        }
    }
}

/// Discriminants for the built-in primitive functions.
///
/// Each variant corresponds to a Lisp function registered in
/// [`environment::Environment::new_with_builtins`].  The mapping from Lisp name
/// to variant is defined there; the actual implementation lives in
/// `evaluator::apply_builtin`.
///
/// You do not normally construct `BuiltinFunc` values directly — they are
/// created by the environment initialiser and stored as [`LispVal::Builtin`].
#[derive(Debug, Clone, PartialEq)]
pub enum BuiltinFunc {
    Plus,
    Minus,
    Multiply,
    Divide,
    Car,
    Cdr,
    Cons,
    Concat,
    Index,
    Eval,
    Eq,
    Not,
    NumericEquals,
    MakeHashTable,
    Get,
    Set,
    DeleteKey,
    CurrentEnvironment,
    Keys,
    Atom,
    Print,
    GetP,
    PutP,
    Stringp,
    Apply,
    LoadFile,
    Lessp,
    Greaterp,
    Zerop,
    Remainder,
    Expt,
    Numberp,
    // I/O functions
    Read,
    Prin1,
    Princ,
    Terpri,
    // Process control: (exit [code]) terminates the process
    Exit,
    // Error handling
    Error,
    Errorset,
    // List processing
    Subst,
    Sublis,
    Assoc,
    Maplist,
    Mapcar,
    Rplaca,
    Rplacd,
    // Bitwise operations
    Logor,
    Logand,
    Logxor,
    Leftshift,
    // Property list functions
    Remprop,
    Deflist,
    // Type predicates
    Fixp,
    Floatp,
    Charp,
    HashTablep,
    Symbolp,
    Boundp,
    Functionp,
    Macrop,
    // List functions
    List,
    Last,
    Nth,
    Nthcdr,
    Efface,
    // Numeric functions
    Mod,
    Plusp,
    Evenp,
    Oddp,
    Add1,
    Sub1,
    Random,
    RandomSeed,
    // Bitwise
    Ash,
    Lognot,
    Rot,
    // Function operations
    Funcall,
    Macroexpand,
    // String/Symbol functions
    Explode,
    Implode,
    Maknam,
    Gensym,
    Intern,
    Plist,
    // Float comparisons
    FloatEqual,
    FloatLessp,
    FloatGreaterp,
    // Condition flags
    SetFlag,
    ClearFlag,
    FlagSetP,
    ClearAllFlags,
    // Branded-rows field primitives (issue #308): representation-generic
    // record field access — the runtime half of the row type language.
    RecordRef,
    RecordWith,
    RecordDeclare,
    RecordNew,
    RecordBrand,
    RecordCompiledP,
    RecordFields,
    VariantDeclare,
    LastBacktrace,
    MonotonicMicros,
    ExplainCompile,
    SetValue,
    CapMaskAllowsP,
    Append,
    DeclareInstance,
    DeclareProtocolDispatch,
    // Introspective typing surface (rows port, #297 step 0): structured
    // checker verdicts, pure string->forms parsing, and declared schemes.
    SeeType,
    ReadString,
    DeclareType,
    // Kernel fuel (issue #284 Phase 2): per-thread step budget backstop.
    KernelFuelSet,
    KernelFuelRemaining,
    // Positioned reading (issue #171 phase 2a): parse a whole source string
    // into (form line col) triples for file:line tooling like SGREP-FILE.
    ReadAllPositioned,
    // Capabilities / features (read-only from Lisp; grant/revoke only from the host)
    FeatureEnabledP,
    Features,
    // Shell (gated behind the SHELL feature)
    Shell,
    // First-class environments
    MakeEnvironment,
    TheEnvironment,
    // Source optimizer
    Optimize,
    // Lisp 1.5 EVLIS
    Evlis,
    // Arrays (Lisp 1.5 Appendix A)
    MakeArray,
    ArrayFetch,
    ArrayStore,
    ArrayLength,
    Length,
    ListToArray,
    ArrayToList,
    Arrayp,
    Extensionp,
    ExtensionTypeName,
    // Lisp 1.5 EVCON
    Evcon,
    // I/O
    Spaces,
    // File I/O (gated behind READ-FS / CREATE-FS capability)
    ReadFile,
    ReadFileByte,
    ReadFileSection,
    WriteFile,
    // File metadata predicates (gated behind READ-FS capability)
    FileExistsP,
    DirectoryP,
    FileP,
    FileReadableP,
    FileWritableP,
    FileExecutableP,
    FileSize,
    DirectoryFiles,
    FileNewerP,
    // File mutation (gated behind CREATE-FS capability)
    Chmod,
    CreateDirectory,
    DeleteFile,
    RenameFile,
    // Temp filesystem (gated behind TEMP-FS capability)
    MakeTempFile,
    MakeTempDirectory,
    // Sorting (stable, non-destructive; takes a comparator predicate)
    Sort,
    // Math library (f64 transcendentals, float->int rounding, i64 integer math)
    Sqrt,
    Sin,
    Cos,
    Tan,
    Log,
    Exp,
    Floor,
    Ceiling,
    Round,
    Truncate,
    Gcd,
    Lcm,
    Isqrt,
    Signum,
    // String operations (kernel primitives not expressible in pure Lisp)
    StringLength,
    Substring,
    CharCode,
    CodeChar,
    MakeChar,
    StringToNumber,
    NumberToString,
    // Unicode-aware, locale-independent case fold (issue #254); backs the
    // case-insensitive string comparison family in lib/14-strings.lisp.
    StringCasefold,
    // Explicit String <-> Array<Char> UTF-8 boundary (issue #254); wrapped by
    // the TEXT module in lib/30-text.lisp.
    StringToUtf8,
    Utf8ToString,
    Utf8ToStringLossy,
    // Parse one s-expression from a string via the reader
    ReadFromString,
    // Value -> string rendering (backs FORMAT and friends)
    Prin1ToString,
    PrincToString,
    // First-class error/condition values
    MakeError,
    ErrorP,
    ErrorMessage,
    ErrorData,
    // Introspection
    Describe,
    SeeSource,
    Disassemble,
    // REQUIRE/PROVIDE module registry kernel support (issue #256): source
    // resolution (host-registered, then embedded) and evaluating an
    // in-memory module source string at the environment root. See
    // lib/06-require.lisp for the Lisp-layer policy these back.
    ModuleSourceLookup,
    ModuleSearchPaths,
    EvalModuleSource,
    // Binary ports (issue #255, epic #253): kernel substrate wrapped by the
    // PORTS module in lib/31-ports.lisp. Capability-gated construction
    // (READ-FS/CREATE-FS/IO); the binary operations below work uniformly
    // across every port kind once you have one.
    PortOpenInputFile,
    PortOpenOutputFile,
    PortOpenAppendFile,
    PortOpenInputBytes,
    PortOpenOutputBytes,
    PortOutputContents,
    PortStdin,
    PortStdout,
    PortStderr,
    PortReadByte,
    PortReadBytes,
    PortWriteByte,
    PortWriteBytes,
    PortFlush,
    PortClose,
    PortOpenP,
    PortInputP,
    PortOutputP,
    PortSeekableP,
    PortPosition,
    PortSeek,
    PortP,
    PortName,
    PortKind,
    // Networking (issue #258, epic #253): DNS/address, TCP, and UDP kernel
    // substrate wrapped by the NET/TCP/UDP modules (lib/37-net.lisp,
    // lib/38-tcp.lisp, lib/39-udp.lisp). Capability-gated construction
    // (NET-DNS/NET-CONNECT/NET-LISTEN) plus a host policy hook
    // (`Environment::set_net_policy`); see `src/evaluator/builtins_net.rs`.
    // A connected TCP stream is an ordinary Port (PortOpen*/PortRead*/...
    // above work on it unchanged); listeners and UDP sockets are
    // `LispVal::NetHandle`.
    NetResolve,
    NetLocalAddr,
    NetPeerAddr,
    TcpConnect,
    TcpListen,
    TcpAccept,
    TcpShutdown,
    TcpSetReadTimeout,
    TcpSetWriteTimeout,
    NetHandleClose,
    NetHandleOpenP,
    NetHandleP,
    NetHandleKind,
    NetHandleName,
    UdpBind,
    UdpConnect,
    UdpSendTo,
    UdpSend,
    UdpReceiveFrom,
    UdpSetTimeout,
    // OS integration (issue #260, epic #253): kernel substrate wrapped by
    // the OS/OS-LINUX modules in lib/41-os.lisp, lib/42-os-linux.lisp.
    OsArgs,
    OsExecutablePath,
    OsCwd,
    OsChdir,
    OsEnvGet,
    OsEnvList,
    OsEnvSet,
    OsEnvUnset,
    OsPid,
    OsPpid,
    OsHostname,
    OsNow,
    OsMonotonicNanos,
    OsSleep,
    OsPrngStep,
    OsRandomBytes,
    OsSpawn,
    OsProcessWait,
    OsProcessTryWait,
    OsProcessId,
    OsProcessKill,
    OsProcessTerminate,
    OsProcessOpenP,
    OsProcessP,
    OsSignal,
    OsLinuxStat,
    OsLinuxReadlink,
    // Concurrency primitives (gated behind the `concurrency` feature)
    #[cfg(feature = "concurrency")]
    MakeChannel,
    #[cfg(feature = "concurrency")]
    SpawnProcess,
    #[cfg(feature = "concurrency")]
    ChannelSend,
    #[cfg(feature = "concurrency")]
    ChannelRecv,
    #[cfg(feature = "concurrency")]
    ChannelRecvTimeout,
    #[cfg(feature = "concurrency")]
    CloneInterpreter,
}

/// Precomputed tag for special-form dispatch in `eval_step`.
///
/// Stored in [`Symbol::special_form`] and set **once** at intern time by
/// [`environment::SymbolTable::intern`].  The common case (ordinary function
/// calls) stores `None` and skips the entire string-compare match.  Each
/// variant corresponds to exactly one (or more aliased) arm in the special-form
/// match inside `eval_step`.
///
/// `Copy + Eq` so the tag can be read from `s.borrow().special_form` and
/// the borrow dropped before the arm body executes — avoiding the re-borrow
/// hazard from issue #156.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpecialForm {
    /// `QUOTE`
    Quote,
    /// `QUASIQUOTE`
    Quasiquote,
    /// `COND`
    Cond,
    /// `IF`
    If,
    /// `AND`
    And,
    /// `OR`
    Or,
    /// `UNWIND-PROTECT`
    UnwindProtect,
    /// `CATCH`
    Catch,
    /// `THROW`
    Throw,
    /// `BLOCK`
    Block,
    /// `RETURN-FROM`
    ReturnFrom,
    /// `HANDLER-CASE`
    HandlerCase,
    /// `DEF`
    Def,
    /// `DEFDYNAMIC` and its alias `DEFVAR`
    Defdynamic,
    /// `LAMBDA`
    Lambda,
    /// `FUNCTION`
    Function,
    /// `LABEL`
    Label,
    /// `DEFINE`
    Define,
    /// `DEFEXPR`
    Defexpr,
    /// `DEFMACRO`
    Defmacro,
    /// `DEFUN-TYPED`
    DefunTyped,
    /// `WITH-FUEL` — kernel-armed step budget with RAII restore (#320/#284)
    WithFuel,
    /// `WITH-CAPABILITIES` — dynamic-extent capability mask (#320)
    WithCapabilities,
    /// `DEFUN*`
    DefunStar,
    /// `JIT-OPTIMIZE`
    JitOptimize,
    /// `CHECK-TYPE`
    CheckType,
    /// `DEFSTRUCT-TYPED`
    DefstructTyped,
    /// `DECLARE-TYPED`
    DeclareTyped,
    /// `PROGN`
    Progn,
    /// `SETQ`
    Setq,
    /// `PROG`
    Prog,
    /// `RETURN`
    Return,
    /// `GO`
    Go,
    /// `FOR`
    For,
    /// `WHILE`
    While,
    /// `LET`
    Let,
    /// `LET*`
    LetStar,
    /// `MACRO` (anonymous macro constructor)
    Macro,
    /// `FEXPR` (anonymous fexpr constructor)
    Fexpr,
    /// `VAU` and its alias `$VAU`
    Vau,
}

/// An interned Lisp symbol.
///
/// Symbols are the atomic identifiers of Lisp programs — names like `FOO`,
/// `+`, `CAR`, or `*MY-VAR*`.  Every distinct name is stored exactly once in
/// the [`environment::SymbolTable`]; two occurrences of the same name share the
/// same `Rc<RefCell<Symbol>>` allocation, making [`LispVal`] `EQ` comparison
/// a pointer-equality test.
///
/// Each symbol carries a **property list** (`plist`) — a `HashMap` of
/// indicator→value pairs.  The standard library uses `"docstring"` to store
/// documentation accessible via `(documentation 'sym)`.  You can store
/// arbitrary metadata from Rust via [`environment::Environment::intern_symbol`]
/// and then mutate `sym.borrow_mut().plist`.
#[derive(Debug, Clone, PartialEq)]
pub struct Symbol {
    /// Uppercased canonical name.
    pub name: String,
    /// Arbitrary key/value metadata attached to this symbol.
    pub plist: HashMap<String, LispVal>,
    /// Global value cell (the canonical-namespace binding for this symbol).
    ///
    /// Root-level (global) bindings live here rather than in a `HashMap` on the
    /// root environment frame, so a global/function reference resolves by reading
    /// the cell on the symbol the AST already holds — O(1), no hash, no chain
    /// walk. Local bindings (params, `let`, `prog`, loop vars) still live in
    /// their frame's map. Because each interpreter chain has its own
    /// `SymbolTable`, these cells are naturally scoped per global namespace, so
    /// independent `make-environment` namespaces stay isolated.
    pub value: Option<LispVal>,
    /// Monotonic id assigned by [`environment::SymbolTable::intern`] or
    /// [`environment::SymbolTable::gensym`].  Used as the key in local
    /// environment frames (`HashMap<u32, LispVal>`) so that binding and lookup
    /// use integer hashing instead of string hashing/cloning.
    pub id: u32,
    /// Cached flag: `true` iff `name.starts_with(':')`.  Set once at intern
    /// time so the hot `eval_step` symbol arm avoids a string scan on every
    /// variable reference.
    pub is_keyword: bool,
    /// Cached flag: `true` once this symbol has been marked as a dynamic
    /// (special) variable via [`environment::Environment::mark_dynamic`].  Set
    /// via `borrow_mut()` in `mark_dynamic`; read via `borrow()` in
    /// `Environment::resolve`, replacing the `dynamic_vars` set probe on the
    /// hot evaluation path.
    pub is_dynamic: bool,
    /// Precomputed special-form tag.  `None` for ordinary symbols (the
    /// overwhelmingly common case — every user-defined function call pays zero
    /// cost for this field).  Set once at [`environment::SymbolTable::intern`]
    /// time; never changes afterwards.
    pub special_form: Option<SpecialForm>,
}

/// Compiled intermediate representation for a lambda body (Milestone 1).
///
/// [`Code`] is produced by `evaluator::compile::compile` at lambda-creation
/// time and stored in [`Lambda::compiled`].  `evaluator::compile::exec` runs
/// it with an internal TCO trampoline.  Any form that the compiler does not
/// yet handle is wrapped in [`Code::Interp`], which falls back to the
/// tree-walking evaluator transparently.
///
/// ## Variants handled (M1 + M2)
///
/// | Variant         | Lisp form |
/// |-----------------|-----------|
/// | `Const`         | literals, `(quote datum)` |
/// | `Var`           | symbol references |
/// | `If`            | `(if cond then [else])` |
/// | `Seq`           | `(progn f1 … fn)` |
/// | `Let`           | `(let ((v e) …) body…)` with lexical variables only |
/// | `Call`          | ordinary function calls (fixed-arity and `&rest`) |
/// | `SetVar`        | `(setq v1 e1 …)` |
/// | `UnwindProtect` | `(unwind-protect body cleanup…)` |
/// | `While`         | `(while cond body…)` |
/// | `For`           | `(for (var start end [step]) body…)` |
///
/// Dynamic-variable `let`, `catch`/`throw`, `block`/`return-from`, and
/// `prog`/`go`/`return` are deliberately left as `Code::Interp`: the
/// tree-walker fallback already handles them correctly (with full TCO, since
/// the M1' unified trampoline), and the payoff of compiling them is low
/// relative to the correctness risk (`prog`/`go` in particular doesn't map
/// onto this tree-shaped IR without a larger, M3-scale redesign).
/// | `Interp`| fallback to tree-walker |
#[derive(Debug)]
pub enum Code {
    /// A constant value — number, string, char, nil, or a quoted datum.
    Const(LispVal),
    /// A variable reference: resolve the symbol in the current environment.
    Var(Shared<SharedCell<Symbol>>),
    /// A compile-time-resolved lexical parameter read (issue #200 M3):
    /// `depth` frames up the parent chain, then `slots[slot]` — no symbol
    /// resolution, no hashing. `sym` is the soundness fallback: when the
    /// frame at `depth` carries no routing (a binding path not yet
    /// slot-converted, e.g. a tree-walker `apply`), or the symbol has been
    /// made dynamic mid-flight, execution falls back to full resolution —
    /// coverage affects speed, never correctness.
    LocalGet {
        depth: u16,
        slot: u16,
        sym: Shared<SharedCell<Symbol>>,
    },
    /// `(if cond then else)`.  The else branch is `Const(Nil)` when absent.
    If(Shared<Code>, Shared<Code>, Shared<Code>),
    /// `(progn f1 … fn)` — evaluate all, return the last.
    Seq(Vec<Shared<Code>>),
    /// `(let ((v1 e1) …) body…)` with only lexical (non-dynamic) bindings.
    ///
    /// Each init expression is evaluated in the *outer* environment and then
    /// bound in a fresh child environment, matching standard `let` semantics.
    /// Any clause that involves a dynamic variable falls back to `Code::Interp`
    /// at compile time so that `DynamicBinding` RAII guards are handled
    /// correctly by the tree-walking evaluator.
    Let {
        /// `(symbol_id, init_code)` pairs, evaluated in the outer env.
        bindings: Vec<(u32, Shared<Code>)>,
        /// Slot routing for the child frame (issue #200 M3 slice 2): the
        /// binder ids, one shared table per LET form so frame creation
        /// does not allocate it. Present iff the binders are distinct —
        /// degenerate duplicate-binder LETs keep the map path.
        routing: Option<Shared<Vec<u32>>>,
        /// Body evaluated in the child env (tail position).
        body: Shared<Code>,
    },
    /// A function call: evaluate callee and all args, then apply.
    ///
    /// `original` is the raw AST form.  It is used as a transparent fallback
    /// when the callee turns out to be a macro, fexpr, or vau operative at
    /// runtime — those forms need their arguments *unevaluated* and must be
    /// re-routed through the tree-walking `eval`.
    Call {
        callee: Shared<Code>,
        args: Vec<Shared<Code>>,
        /// Original AST form for the macro/fexpr/vau fallback path.
        original: LispVal,
    },
    /// `(setq v1 e1 v2 e2 …)` — evaluate each `ei` in order and store it into
    /// `vi` (created in the current environment if not already bound,
    /// matching the tree-walker). Returns the last value assigned.
    SetVar(Vec<(Shared<SharedCell<Symbol>>, Shared<Code>)>),
    /// `(unwind-protect body cleanup…)` — evaluate `body`, then always
    /// evaluate every `cleanup` form (even if `body` errored or performed a
    /// non-local exit), then propagate `body`'s result.
    UnwindProtect {
        body: Shared<Code>,
        cleanups: Vec<Shared<Code>>,
    },
    /// `(while cond body…)` — re-test `cond` before each iteration; body runs
    /// in the current environment (no per-iteration frame). Yields `NIL`.
    While {
        cond: Shared<Code>,
        body: Vec<Shared<Code>>,
    },
    /// `(for (var start end [step]) body…)` — inclusive integer range, one
    /// reused child frame, in-place counter mutation. Yields `NIL`.
    For {
        var_id: u32,
        start: Shared<Code>,
        end: Shared<Code>,
        step: Option<Shared<Code>>,
        body: Vec<Shared<Code>>,
    },
    /// A `(lambda (params…) body…)` literal appearing inside a compiled body.
    ///
    /// The body is compiled **once**, at the enclosing definition's compile
    /// time; each execution only constructs the [`Lambda`] value — capturing
    /// the current environment and interning the parameter ids — and reuses
    /// this pre-compiled body instead of recompiling it on every call
    /// (issue #233).
    MakeLambda {
        /// Raw parameter list form (e.g. `(x y &REST z)`), parsed at
        /// construction time against the running environment.
        params: LispVal,
        /// The lambda body expressions (the form's cdr after the params).
        body_forms: Vec<LispVal>,
        /// The body pre-compiled once at enclosing-compile time.
        compiled_body: Shared<Code>,
    },
    /// Fallback: call the tree-walking `eval` on the original AST form.
    Interp(LispVal),
}

/// A lexical closure created by `(lambda (params…) body…)` or `defun`.
///
/// When called, a new child environment is created whose lexical parent is
/// `env` (the definition site) and whose dynamic parent is the caller's
/// environment (so dynamic/special variables propagate correctly).  Parameters
/// are bound in this child env before `body` is evaluated.
///
/// If `rest_param` is `Some(name)`, any arguments beyond the fixed `params`
/// are collected into a list and bound to that name (the `&REST` convention).
#[derive(Debug, Clone)]
pub struct Lambda {
    /// Fixed parameter names, in order.
    pub params: Vec<String>,
    /// Optional variadic parameter name (`&REST rest`).
    pub rest_param: Option<String>,
    /// Body expression (typically `(PROGN ...)` for multi-form bodies).
    pub body: Box<LispVal>,
    /// Captured lexical environment (definition site).
    pub env: Shared<Environment>,
    /// Symbol ids for the fixed parameters (parallel to `params`).  Used as
    /// the binding key in local environment frames instead of cloning Strings.
    pub param_ids: Vec<u32>,
    /// Symbol id for the `&REST` parameter, if any.
    pub rest_param_id: Option<u32>,
    /// Shared slot-routing table for call frames (issue #200 M3): the
    /// fixed parameter ids, one `Rc` reused by every call so frame
    /// creation does not allocate a table. Derived artefact like
    /// `compiled` — excluded from `PartialEq`.
    pub param_routing: Shared<Vec<u32>>,
    /// Pre-compiled body for the fast execute path (Milestone 1).
    ///
    /// Set by `make_lambda` at definition time.  `None` for lambdas
    /// constructed outside the evaluator (tests, embedders).  Excluded from
    /// [`PartialEq`] and hash so that two otherwise-identical lambdas are
    /// still considered equal regardless of whether they carry a compiled body.
    pub compiled: Option<Shared<Code>>,
}

impl PartialEq for Lambda {
    fn eq(&self, other: &Self) -> bool {
        self.params == other.params
            && self.rest_param == other.rest_param
            && self.body == other.body
            && Shared::ptr_eq(&self.env, &other.env)
        // `compiled` is intentionally excluded — it is a derived artefact and
        // does not affect the semantic identity of the closure.
    }
}

/// A fexpr ("function expression") created by `(defexpr name (args-sym) body…)`.
///
/// Unlike a lambda, a fexpr receives its arguments **unevaluated** — the
/// entire argument list is passed as a single `LispVal` list bound to the
/// name in `params[0]`.  The fexpr body can then selectively call `(eval …)`
/// on whichever arguments it chooses.
///
/// Fexprs are the classic Lisp mechanism for user-defined special forms.
/// They differ from macros in that they compute a *value* directly rather
/// than returning code to be evaluated.
#[derive(Debug, Clone)]
pub struct Fexpr {
    /// Single-element vec containing the name for the unevaluated arg list.
    pub params: Vec<String>,
    /// Body expression.
    pub body: Box<LispVal>,
    /// Captured lexical environment.
    pub env: Shared<Environment>,
    /// Symbol ids for the parameters (parallel to `params`).
    pub param_ids: Vec<u32>,
}

impl PartialEq for Fexpr {
    fn eq(&self, other: &Self) -> bool {
        self.params == other.params
            && self.body == other.body
            && Shared::ptr_eq(&self.env, &other.env)
    }
}

/// A macro created by `(defmacro name (params…) body…)`.
///
/// Macros receive their arguments **unevaluated** and return a Lisp form
/// (the *expansion*) which is then evaluated in the caller's environment.
/// This two-phase design (expand-then-eval) distinguishes macros from fexprs.
///
/// Supports `&REST` for variadic parameter lists, enabling patterns like
/// `(defmacro when (test &rest body) \`(if ,test (progn ,@body) nil))`.
#[derive(Debug, Clone)]
pub struct Macro {
    /// Fixed parameter names.
    pub params: Vec<String>,
    /// Optional `&REST` parameter for remaining arguments.
    pub rest_param: Option<String>,
    /// Body expression; evaluating it must produce a valid Lisp form.
    pub body: Box<LispVal>,
    /// Captured lexical environment (definition site).
    pub env: Shared<Environment>,
    /// Symbol ids for the fixed parameters (parallel to `params`).
    pub param_ids: Vec<u32>,
    /// Symbol id for the `&REST` parameter, if any.
    pub rest_param_id: Option<u32>,
}

impl PartialEq for Macro {
    fn eq(&self, other: &Self) -> bool {
        self.params == other.params
            && self.rest_param == other.rest_param
            && self.body == other.body
            && Shared::ptr_eq(&self.env, &other.env)
    }
}

/// The function signature for host-registered (native) Lisp callables.
pub type NativeFn = dyn Fn(&[LispVal], &Shared<Environment>) -> Result<LispVal, LispError>;

/// A Kernel-style vau operative closure.
///
/// When called, `operands_param` is bound to the **unevaluated** operand list
/// and `env_param` is bound to the **caller's lexical environment** as a
/// `LispVal::Environment`.  The body is then evaluated in the extended closure
/// env, which gives full access to both.
#[derive(Debug, Clone)]
pub struct Vau {
    pub operands_param: String,
    pub env_param: String,
    pub body: Box<LispVal>,
    pub env: Shared<Environment>,
    /// Symbol id for the operands parameter.
    pub operands_param_id: u32,
    /// Symbol id for the environment parameter.
    pub env_param_id: u32,
}

impl PartialEq for Vau {
    fn eq(&self, other: &Self) -> bool {
        self.operands_param == other.operands_param
            && self.env_param == other.env_param
            && self.body == other.body
            && Shared::ptr_eq(&self.env, &other.env)
    }
}

// NOTE: the crate-level doc block is at the very top of this file (//! lines).
// Additional sections are appended here so that rustdoc renders them in order.

/// The universal value type of the Lamedh Lisp interpreter.
///
/// Every Lisp object — numbers, strings, lists, functions, hash tables — is
/// represented as a `LispVal`.  The variants map directly to the types defined
/// in the Lisp 1.5 manual, extended with modern additions.
///
/// ## Memory model
///
/// - **Cons cells** use `Rc<LispVal>` (not `Box`) so sharing a list head is
///   O(1) refcount bump.  Cons cells are immutable; `RPLACA`/`RPLACD` return
///   new cells rather than mutating in place.
/// - **Symbols** are interned: two occurrences of `FOO` share one
///   `Rc<RefCell<Symbol>>`.  [`LispVal::eq`] for symbols uses pointer equality,
///   matching the Lisp `EQ` predicate.
/// - **Hash tables**, **arrays**, **environments**, and **natives** all use `Rc`
///   for shared ownership.
///
/// ## Truthiness
///
/// [`LispVal::Nil`] is the only falsy value.  Everything else — including `0`,
/// `""`, and empty string — is truthy.  Use [`LispVal::is_truthy`] in Rust code
/// and `(not x)` in Lisp code.
///
/// ## Conversion
///
/// Use the `From<T>` impls for Rust→Lisp conversion and `TryFrom<LispVal>` for
/// Lisp→Rust:
///
/// ```rust,ignore
/// let n: LispVal = LispVal::from(42i64);         // Number(42)
/// let s: LispVal = LispVal::from("hello");        // String("hello")
/// let lst = LispVal::list([1i64, 2, 3]);          // (1 2 3)
///
/// let x: i64 = i64::try_from(n)?;                // 42
/// let v: Vec<LispVal> = Vec::try_from(lst)?;     // [Number(1), Number(2), Number(3)]
/// ```
pub enum LispVal {
    /// An interned symbol such as `FOO`, `+`, or `*MY-VAR*`.
    Symbol(Shared<SharedCell<Symbol>>),
    /// A 64-bit signed integer.
    Number(i64),
    /// A single byte (character, 0–255). Produced by char literals (`'a'` →
    /// `Char(97)`). Coerces to `Number` in arithmetic, like C integer
    /// promotion. Distinct from `Number`: `charp` tests for this type;
    /// `fixp` does not match it.
    Char(u8),
    /// A 64-bit IEEE 754 floating-point number.
    Float(f64),
    /// A UTF-8 string literal.
    String(String),
    /// A built-in primitive function.  See [`BuiltinFunc`].
    Builtin(BuiltinFunc),
    /// A lexical closure.  See [`Lambda`].
    ///
    /// Boxed: closures are far larger than the other variants, so storing them
    /// inline would make every `LispVal` (even a `Number`) as wide as a
    /// `Lambda`. Boxing keeps `LispVal` small, which is a large win because the
    /// evaluator clones and moves `LispVal`s constantly (see the perf work in
    /// the boxing pass). The same reasoning applies to `Fexpr`/`Macro`/`Vau`.
    Lambda(Box<Lambda>),
    /// A fexpr (unevaluated-argument function).  See [`Fexpr`].
    Fexpr(Box<Fexpr>),
    /// A macro (code-returning function).  See [`Macro`].
    Macro(Box<Macro>),
    /// A Kernel-style vau operative.  See [`Vau`].
    Vau(Box<Vau>),
    /// A cons cell.  Children use [`Shared`] (not `Box`) so cloning a list is
    /// an O(1) refcount bump instead of a deep copy — cons cells are immutable
    /// in this implementation (`rplaca`/`rplacd` return new cells), so
    /// structural sharing is sound.
    ///
    /// **This immutability is load-bearing, not just a circular-list guard**
    /// (issue #114). Because clones share the same `Shared<LispVal>` children,
    /// any future op that mutates a cons cell *in place* (a real in-place
    /// `RPLACA`/`RPLACD`, a `SETF`-of-`CAR`, a destructured-slot setter, …)
    /// would silently mutate every value sharing that sub-structure — parser
    /// output, captured closure bodies, quasiquote templates, macro-expansion
    /// inputs. If mutating list ops are ever added, do not mutate through the
    /// shared pointer: use copy-on-write at the mutation point (e.g.
    /// `Rc::make_mut`), rebuild the spine being mutated, or gate destructive
    /// ops behind a distinct `Rc<RefCell<…>>` cell type.
    Cons {
        car: Shared<LispVal>,
        cdr: Shared<LispVal>,
    },
    /// The empty list / boolean false.  `()` and `NIL` are the same object.
    Nil,
    /// A mutable hash table, used as `(make-hash-table)`.  Keys and values are
    /// arbitrary `LispVal`s.
    HashTable(Shared<SharedCell<HashMap<LispVal, LispVal>>>),
    /// A host-registered Rust closure callable from Lisp.  See
    /// [`environment::Environment::register_fn`].
    Native(Shared<NativeFn>),
    /// A first-class environment.  Obtained via `(the-environment)` or
    /// `(make-environment)`.  Can be passed to `(eval expr env)`.
    Environment(Shared<Environment>),
    /// A 0-indexed mutable vector (Lisp 1.5 `array`).  Created by `(array n)`;
    /// accessed with `(fetch a i)` / `(store a i v)`.
    Array(Shared<SharedCell<Vec<LispVal>>>),
    /// A typed, nominal struct value crossing the typed membrane.
    Struct(Shared<StructObj>),
    /// A host-defined extension value.  Use [`LispVal::ext`] to construct.
    /// See [`LispValExtension`].
    Extension(Shared<dyn LispValExtension>),
    /// A first-class error/condition value: a message plus an optional data
    /// payload (a cons list of "irritants", or `Nil`).  Constructed with
    /// `(make-error msg data)`, signalled by `(error ...)`, and bound by the
    /// handler variable in `(handler-case ...)`.  Boxed behind a [`Shared`]
    /// pointer so the `LispVal` stays small.
    Error(Shared<ErrorObj>),
    /// A synchronous binary I/O handle (issue #255, epic #253): a file,
    /// in-memory byte buffer, standard stream, or host-wrapped
    /// reader/writer. Opaque from Lisp — operated on only through the
    /// `PORT-*` kernel primitives (`src/evaluator/builtins_ports.rs`),
    /// wrapped by the `PORTS` module (`lib/31-ports.lisp`). Compares by
    /// identity, like `Array`/`HashTable`/`Environment`. See [`PortObj`].
    ///
    /// A connected TCP stream (issue #258, epic #253) is represented as an
    /// ordinary `Port` too (`PortState::TcpStream`), so every `PORTS`
    /// operation (`read-byte!`, `write-bytes!`, `close!`, `port-p`, ...)
    /// works on it unchanged -- see `TCP-CONNECT*`/`TCP-ACCEPT*` in
    /// `src/evaluator/builtins_net.rs`.
    Port(Shared<PortObj>),
    /// A network listener or datagram socket (issue #258, epic #253):
    /// opaque owned resources that are NOT byte streams (unlike a connected
    /// TCP stream, which is a [`LispVal::Port`]) -- a `TcpListener` accepts
    /// connections, a `UdpSocket` sends/receives whole datagrams. Wrapped by
    /// the `TCP`/`UDP` modules (`lib/37-net.lisp`/`38-tcp.lisp`/`39-udp.lisp`).
    /// Opaque from Lisp -- operated on only through the `TCP-*`/`UDP-*`/
    /// `NET-HANDLE-*` kernel primitives (`src/evaluator/builtins_net.rs`).
    /// Compares by identity, like [`LispVal::Port`]. See [`NetHandleObj`].
    NetHandle(Shared<NetHandleObj>),
    /// A spawned child process (issue #260, epic #253): an opaque owned
    /// handle over `std::process::Child`. Never a bare PID -- operated on
    /// only through the `OS-PROCESS-*` kernel primitives
    /// (`src/evaluator/builtins_os.rs`), wrapped by the `OS` module
    /// (`lib/41-os.lisp`). Its stdin/stdout/stderr pipes (when requested as
    /// `:PIPE`) are ordinary [`LispVal::Port`]s (issue #255), reusing
    /// [`LispVal::wrap_reader`]/[`LispVal::wrap_writer`]. Compares by
    /// identity, like [`LispVal::Port`]/[`LispVal::NetHandle`]. See
    /// [`ChildObj`].
    OsChild(Shared<ChildObj>),
    /// A message-passing channel (only present under the `concurrency` feature).
    ///
    /// Created with `(make-channel)`.  Both ends are bundled together so that
    /// one side can be cloned off and used from another evaluation context.
    /// Values cross the channel boundary as printer-serialised strings and are
    /// re-read by the receiver.
    #[cfg(feature = "concurrency")]
    Channel(std::sync::Arc<ChannelObj>),
}

/// The payload of a [`LispVal::Error`].
///
/// Conceptually a small map of `message` (a `String`) and `data` (a cons cell
/// or `Nil`).  Modelled as a struct with those two fields rather than a literal
/// `HashMap` so the common accessors are direct field reads.
#[derive(Debug, Clone)]
pub struct ErrorObj {
    /// Human-readable error message.
    pub message: String,
    /// Structured payload: a cons list of irritants, or `Nil`.
    pub data: LispVal,
}

/// Backing storage for a [`PortObj`]. Never exposed to Lisp directly —
/// `PortObj`'s methods are the only way in or out, so a raw file descriptor
/// never crosses the Lisp boundary (issue #255's embedding requirement).
enum PortState {
    /// Closed (either explicitly, or default-constructed). Every operation
    /// except `close`/`port-open-p` errors on this state.
    Closed,
    File(fs::File),
    MemoryInput(std::io::Cursor<Vec<u8>>),
    /// A growable output buffer; not seekable (append-only), matching the
    /// simplest reading of "output byte-array port" in the issue.
    MemoryOutput(Vec<u8>),
    Stdin(std::io::Stdin),
    Stdout(std::io::Stdout),
    Stderr(std::io::Stderr),
    /// A host-registered arbitrary byte source, installed via
    /// [`LispVal::wrap_reader`] without exposing a raw fd to Lisp.
    HostReader(Box<dyn Read>),
    /// A host-registered arbitrary byte sink, installed via
    /// [`LispVal::wrap_writer`].
    HostWriter(Box<dyn Write>),
    /// A connected TCP stream (issue #258, epic #253) -- `TCP-CONNECT*` or
    /// `TCP-ACCEPT*` in `src/evaluator/builtins_net.rs`. Not seekable. The
    /// only `PortState` with TCP-specific out-of-band operations
    /// (`shutdown`/timeouts/peer address); see [`PortObj::tcp_peer_addr`]
    /// and friends below.
    TcpStream(TcpStream),
}

impl fmt::Debug for PortState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let tag = match self {
            PortState::Closed => "Closed",
            PortState::File(_) => "File",
            PortState::MemoryInput(_) => "MemoryInput",
            PortState::TcpStream(_) => "TcpStream",
            PortState::MemoryOutput(_) => "MemoryOutput",
            PortState::Stdin(_) => "Stdin",
            PortState::Stdout(_) => "Stdout",
            PortState::Stderr(_) => "Stderr",
            PortState::HostReader(_) => "HostReader",
            PortState::HostWriter(_) => "HostWriter",
        };
        write!(f, "{tag}")
    }
}

/// The payload of a [`LispVal::Port`] (issue #255, epic #253): a synchronous
/// binary I/O handle over a file, an in-memory byte buffer, a standard
/// stream, or a host-wrapped reader/writer.
///
/// **Deterministic ownership.** The documented contract is an explicit
/// close (`(ports:close! p)`, or `(ports:with-open-port (p ...) ...)` which
/// closes on every exit path via `UNWIND-PROTECT`). Rust's ordinary `Drop`
/// — the underlying `File`/etc. closes automatically when the last
/// [`Shared`] reference to this `PortObj` is dropped, since `state` owns it
/// with no `mem::forget` anywhere — is a last-resort safety net, not the
/// primary cleanup path; nothing here needs a custom `Drop` impl because
/// `fs::File` and friends already close themselves on drop.
///
/// **Capabilities.** `PortObj` itself performs no capability checks; the
/// kernel primitives that construct one (`src/evaluator/builtins_ports.rs`)
/// check `READ-FS`/`CREATE-FS`/`IO` before ever calling these constructors,
/// exactly like the existing file builtins in `src/evaluator/apply.rs`.
#[derive(Debug)]
pub struct PortObj {
    /// Diagnostic resource kind, e.g. `"file"`, `"memory"`, `"stdin"`.
    pub kind: &'static str,
    /// Diagnostic name, e.g. a file path or `"<stdin>"`. Not necessarily
    /// unique; used only for `print`/error messages.
    pub name: String,
    readable: bool,
    writable: bool,
    seekable: bool,
    state: SharedCell<PortState>,
}

impl PortObj {
    fn new(
        kind: &'static str,
        name: impl Into<String>,
        readable: bool,
        writable: bool,
        seekable: bool,
        state: PortState,
    ) -> Shared<PortObj> {
        Shared::new(PortObj {
            kind,
            name: name.into(),
            readable,
            writable,
            seekable,
            state: SharedCell::new(state),
        })
    }

    /// True unless the port has been closed (explicitly or via `Drop`'s
    /// state transition — there is no separate case for the latter; closing
    /// always goes through the same `PortState::Closed` transition).
    pub fn is_open(&self) -> bool {
        !matches!(*self.state.borrow(), PortState::Closed)
    }

    pub fn is_readable(&self) -> bool {
        self.readable
    }

    pub fn is_writable(&self) -> bool {
        self.writable
    }

    pub fn is_seekable(&self) -> bool {
        self.seekable
    }

    fn require_open(&self) -> std::io::Result<()> {
        if self.is_open() {
            Ok(())
        } else {
            Err(std::io::Error::other("port is closed"))
        }
    }

    /// Read exactly one byte, or `Ok(None)` at EOF.
    pub fn read_byte(&self) -> std::io::Result<Option<u8>> {
        self.require_open()?;
        if !self.readable {
            return Err(std::io::Error::other("port is not readable"));
        }
        let mut buf = [0u8; 1];
        let n = self.read_into(&mut buf)?;
        Ok(if n == 0 { None } else { Some(buf[0]) })
    }

    /// Read up to `max` bytes. Returns fewer than `max` (including zero) at
    /// EOF or on a partial read — never `None`; EOF and "no bytes available
    /// right now" are both an empty/short `Vec`, matching `Read::read`.
    pub fn read_bytes(&self, max: usize) -> std::io::Result<Vec<u8>> {
        self.require_open()?;
        if !self.readable {
            return Err(std::io::Error::other("port is not readable"));
        }
        let mut buf = vec![0u8; max];
        let n = self.read_into(&mut buf)?;
        buf.truncate(n);
        Ok(buf)
    }

    fn read_into(&self, buf: &mut [u8]) -> std::io::Result<usize> {
        let mut state = self.state.borrow_mut();
        match &mut *state {
            PortState::File(f) => f.read(buf),
            PortState::MemoryInput(c) => c.read(buf),
            PortState::Stdin(s) => s.lock().read(buf),
            PortState::HostReader(r) => r.read(buf),
            PortState::TcpStream(s) => s.read(buf),
            _ => Err(std::io::Error::other("port is not readable")),
        }
    }

    /// Write `data`; returns the number of bytes written (may be short of
    /// `data.len()` for a partial write).
    pub fn write_bytes(&self, data: &[u8]) -> std::io::Result<usize> {
        self.require_open()?;
        if !self.writable {
            return Err(std::io::Error::other("port is not writable"));
        }
        let mut state = self.state.borrow_mut();
        match &mut *state {
            PortState::File(f) => f.write(data),
            PortState::MemoryOutput(v) => {
                v.extend_from_slice(data);
                Ok(data.len())
            }
            PortState::Stdout(s) => s.lock().write(data),
            PortState::Stderr(s) => s.lock().write(data),
            PortState::HostWriter(w) => w.write(data),
            PortState::TcpStream(s) => s.write(data),
            _ => Err(std::io::Error::other("port is not writable")),
        }
    }

    /// Flush buffered writes. A no-op (not an error) on ports with nothing
    /// to flush (readers, already-closed-is-caught-above, memory buffers).
    pub fn flush(&self) -> std::io::Result<()> {
        self.require_open()?;
        let mut state = self.state.borrow_mut();
        match &mut *state {
            PortState::File(f) => f.flush(),
            PortState::Stdout(s) => s.lock().flush(),
            PortState::Stderr(s) => s.lock().flush(),
            PortState::HostWriter(w) => w.flush(),
            PortState::TcpStream(s) => s.flush(),
            _ => Ok(()),
        }
    }

    /// Close the port. Idempotent: closing an already-closed port is a
    /// silent no-op, never an error (issue #255's documented contract).
    pub fn close(&self) {
        *self.state.borrow_mut() = PortState::Closed;
    }

    /// Current byte offset, for seekable ports only.
    pub fn position(&self) -> std::io::Result<u64> {
        self.require_open()?;
        if !self.seekable {
            return Err(std::io::Error::other("port does not support position/seek"));
        }
        let mut state = self.state.borrow_mut();
        match &mut *state {
            PortState::File(f) => f.stream_position(),
            PortState::MemoryInput(c) => Ok(c.position()),
            _ => Err(std::io::Error::other("port does not support position/seek")),
        }
    }

    /// Seek to an absolute byte offset from the start, for seekable ports
    /// only. Returns the new position.
    pub fn seek_to(&self, offset: u64) -> std::io::Result<u64> {
        self.require_open()?;
        if !self.seekable {
            return Err(std::io::Error::other("port does not support position/seek"));
        }
        let mut state = self.state.borrow_mut();
        match &mut *state {
            PortState::File(f) => f.seek(SeekFrom::Start(offset)),
            PortState::MemoryInput(c) => {
                c.set_position(offset);
                Ok(offset)
            }
            _ => Err(std::io::Error::other("port does not support position/seek")),
        }
    }

    /// The accumulated bytes of an output byte-array port. Errors for any
    /// other port kind.
    pub fn output_contents(&self) -> std::io::Result<Vec<u8>> {
        match &*self.state.borrow() {
            PortState::MemoryOutput(v) => Ok(v.clone()),
            _ => Err(std::io::Error::other(
                "port is not an output byte-array port",
            )),
        }
    }

    // ── TCP-specific operations (issue #258, epic #253) ───────────────────
    //
    // These four methods only make sense for `PortState::TcpStream`; every
    // other port kind errors. They live on `PortObj` rather than a free
    // function in `builtins_net.rs` for the same reason the byte operations
    // above do: representation access (`std::net::TcpStream`'s methods)
    // stays next to the `PortState` that owns it.

    fn with_tcp_stream<T>(
        &self,
        op: &str,
        f: impl FnOnce(&TcpStream) -> std::io::Result<T>,
    ) -> std::io::Result<T> {
        self.require_open()?;
        match &*self.state.borrow() {
            PortState::TcpStream(s) => f(s),
            _ => Err(std::io::Error::other(format!(
                "{op}: not a TCP stream port"
            ))),
        }
    }

    /// The remote address this TCP stream is connected to.
    pub fn tcp_peer_addr(&self) -> std::io::Result<std::net::SocketAddr> {
        self.with_tcp_stream("peer-addr", |s| s.peer_addr())
    }

    /// The local address this TCP stream is bound to.
    pub fn tcp_local_addr(&self) -> std::io::Result<std::net::SocketAddr> {
        self.with_tcp_stream("local-addr", |s| s.local_addr())
    }

    /// Shut down the read half, write half, or both, per
    /// [`std::net::Shutdown`]. Does not close the port -- matches the ticket's
    /// "shutdown read/write/both" as an operation distinct from `close!`.
    pub fn tcp_shutdown(&self, how: std::net::Shutdown) -> std::io::Result<()> {
        self.with_tcp_stream("shutdown", |s| s.shutdown(how))
    }

    /// Set (or, with `None`, clear) the read timeout.
    pub fn tcp_set_read_timeout(&self, dur: Option<std::time::Duration>) -> std::io::Result<()> {
        self.with_tcp_stream("set-read-timeout", |s| s.set_read_timeout(dur))
    }

    /// Set (or, with `None`, clear) the write timeout.
    pub fn tcp_set_write_timeout(&self, dur: Option<std::time::Duration>) -> std::io::Result<()> {
        self.with_tcp_stream("set-write-timeout", |s| s.set_write_timeout(dur))
    }

    // ── Constructors ────────────────────────────────────────────────────

    /// Wrap a connected [`TcpStream`] as a duplex binary [`LispVal::Port`]
    /// (issue #258, epic #253). Not seekable. `name` is diagnostic only
    /// (typically the peer address). Capability checks
    /// (`NET-CONNECT`/`NET-LISTEN`) happen in `builtins_net.rs` before this
    /// is called, matching every other `PortObj` constructor's split.
    ///
    /// This is deliberately the same `LispVal::Port` representation as every
    /// other port, not a new variant: it is the seam a future TLS layer can
    /// wrap without changing the port API (`ports:*` and any TLS wrapper
    /// would both take/return this same `Port` shape).
    pub(crate) fn tcp_stream(name: impl Into<String>, stream: TcpStream) -> Shared<PortObj> {
        PortObj::new(
            "tcp-stream",
            name,
            true,
            true,
            false,
            PortState::TcpStream(stream),
        )
    }

    pub(crate) fn open_input_file(path: &str) -> std::io::Result<Shared<PortObj>> {
        let f = fs::File::open(path)?;
        Ok(PortObj::new(
            "file",
            path,
            true,
            false,
            true,
            PortState::File(f),
        ))
    }

    pub(crate) fn open_output_file(path: &str, append: bool) -> std::io::Result<Shared<PortObj>> {
        let f = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .append(append)
            .truncate(!append)
            .open(path)?;
        Ok(PortObj::new(
            "file",
            path,
            false,
            true,
            true,
            PortState::File(f),
        ))
    }

    pub(crate) fn open_memory_input(bytes: Vec<u8>) -> Shared<PortObj> {
        PortObj::new(
            "memory",
            "<memory-input>",
            true,
            false,
            true,
            PortState::MemoryInput(std::io::Cursor::new(bytes)),
        )
    }

    pub(crate) fn open_memory_output() -> Shared<PortObj> {
        PortObj::new(
            "memory",
            "<memory-output>",
            false,
            true,
            false,
            PortState::MemoryOutput(Vec::new()),
        )
    }

    pub(crate) fn stdin_port() -> Shared<PortObj> {
        PortObj::new(
            "stdin",
            "<stdin>",
            true,
            false,
            false,
            PortState::Stdin(std::io::stdin()),
        )
    }

    pub(crate) fn stdout_port() -> Shared<PortObj> {
        PortObj::new(
            "stdout",
            "<stdout>",
            false,
            true,
            false,
            PortState::Stdout(std::io::stdout()),
        )
    }

    pub(crate) fn stderr_port() -> Shared<PortObj> {
        PortObj::new(
            "stderr",
            "<stderr>",
            false,
            true,
            false,
            PortState::Stderr(std::io::stderr()),
        )
    }
}

// ---------------------------------------------------------------------------
// Network resources (issue #258, epic #253): listeners and datagram sockets.
// ---------------------------------------------------------------------------

/// Backing storage for a [`NetHandleObj`]. Never exposed to Lisp directly,
/// mirroring [`PortState`].
enum NetHandleState {
    /// Closed (explicitly, via `NET-HANDLE-CLOSE*`). Every operation except
    /// close/open-p errors on this state -- same documented contract as
    /// [`PortObj::close`].
    Closed,
    TcpListener(TcpListener),
    UdpSocket(UdpSocket),
}

impl fmt::Debug for NetHandleState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let tag = match self {
            NetHandleState::Closed => "Closed",
            NetHandleState::TcpListener(_) => "TcpListener",
            NetHandleState::UdpSocket(_) => "UdpSocket",
        };
        write!(f, "{tag}")
    }
}

/// The payload of a [`LispVal::NetHandle`] (issue #258, epic #253): a TCP
/// listener or a UDP socket. Unlike a connected TCP stream (a
/// [`LispVal::Port`]), neither of these is a byte stream -- a listener
/// yields new connections via `accept`, a UDP socket sends/receives whole
/// datagrams -- so they get their own opaque representation instead of
/// reusing `PortState`, following the same "representation access is Rust,
/// policy is Lisp" split as [`PortObj`].
///
/// **Deterministic ownership.** Same documented contract as `PortObj`:
/// explicit close (`(tcp:close-listener! l)` / `(udp:close! s)`) is the
/// primary cleanup path; Rust's ordinary `Drop` (`TcpListener`/`UdpSocket`
/// close their fd automatically once the last [`Shared`] reference is
/// dropped) is a last-resort safety net, not the primary path.
///
/// **Capabilities.** Like `PortObj`, this performs no capability checks
/// itself; `src/evaluator/builtins_net.rs` checks `NET-LISTEN` before ever
/// constructing one, plus consults the host policy hook
/// ([`environment::Environment::set_net_policy`]) so a granted capability
/// can be scoped to specific hosts/ports.
#[derive(Debug)]
pub struct NetHandleObj {
    /// Diagnostic resource kind: `"tcp-listener"` or `"udp-socket"`.
    pub kind: &'static str,
    /// Diagnostic name, e.g. the bound local address. Not necessarily
    /// unique; used only for `print`/error messages.
    pub name: String,
    state: SharedCell<NetHandleState>,
}

impl NetHandleObj {
    fn new(kind: &'static str, name: impl Into<String>, state: NetHandleState) -> Shared<Self> {
        Shared::new(NetHandleObj {
            kind,
            name: name.into(),
            state: SharedCell::new(state),
        })
    }

    pub(crate) fn tcp_listener(name: impl Into<String>, listener: TcpListener) -> Shared<Self> {
        NetHandleObj::new("tcp-listener", name, NetHandleState::TcpListener(listener))
    }

    pub(crate) fn udp_socket(name: impl Into<String>, socket: UdpSocket) -> Shared<Self> {
        NetHandleObj::new("udp-socket", name, NetHandleState::UdpSocket(socket))
    }

    /// True unless the handle has been closed.
    pub fn is_open(&self) -> bool {
        !matches!(*self.state.borrow(), NetHandleState::Closed)
    }

    fn require_open(&self) -> std::io::Result<()> {
        if self.is_open() {
            Ok(())
        } else {
            Err(std::io::Error::other("network handle is closed"))
        }
    }

    /// Close the handle. Idempotent: closing an already-closed handle is a
    /// silent no-op, never an error (matches `PortObj::close`'s contract).
    pub fn close(&self) {
        *self.state.borrow_mut() = NetHandleState::Closed;
    }

    /// The address this handle is bound to.
    pub fn local_addr(&self) -> std::io::Result<std::net::SocketAddr> {
        self.require_open()?;
        match &*self.state.borrow() {
            NetHandleState::TcpListener(l) => l.local_addr(),
            NetHandleState::UdpSocket(s) => s.local_addr(),
            NetHandleState::Closed => unreachable!("checked by require_open"),
        }
    }

    /// Accept one incoming TCP connection, returning the new stream (wrapped
    /// as an ordinary [`LispVal::Port`] by the caller) and the peer address.
    /// Errors (not panics) if this handle is a UDP socket.
    pub fn tcp_accept(&self) -> std::io::Result<(TcpStream, std::net::SocketAddr)> {
        self.require_open()?;
        match &*self.state.borrow() {
            NetHandleState::TcpListener(l) => l.accept(),
            _ => Err(std::io::Error::other("not a TCP listener")),
        }
    }

    /// Run `f` with the underlying `UdpSocket`. Errors if this handle is a
    /// TCP listener. Centralizes the open-check + variant-match every UDP
    /// operation in `builtins_net.rs` needs.
    pub fn with_udp_socket<T>(
        &self,
        f: impl FnOnce(&UdpSocket) -> std::io::Result<T>,
    ) -> std::io::Result<T> {
        self.require_open()?;
        match &*self.state.borrow() {
            NetHandleState::UdpSocket(s) => f(s),
            _ => Err(std::io::Error::other("not a UDP socket")),
        }
    }
}

// ---------------------------------------------------------------------------
// OS integration (issue #260, epic #253): spawned child processes and
// signal delivery.
// ---------------------------------------------------------------------------

/// Backing storage for a [`ChildObj`]. Never exposed to Lisp directly,
/// mirroring [`PortState`]/[`NetHandleState`].
enum ChildState {
    /// The child has not yet been reaped: `wait!`/`try-wait` may still block
    /// or poll it, and `kill!`/`terminate!` may still signal it.
    Running(std::process::Child),
    /// The child has exited and been reaped (by `wait!`/`try-wait!`
    /// observing exit, or the Drop backstop's best-effort non-blocking
    /// reap); its exit status is cached here. Every operation except
    /// `open-p`/re-reading the cached status is a documented no-op/error on
    /// this state, mirroring `PortObj::close`'s "double-close is a no-op"
    /// contract.
    Reaped(ChildExitStatus),
}

impl fmt::Debug for ChildState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let tag = match self {
            ChildState::Running(_) => "Running",
            ChildState::Reaped(_) => "Reaped",
        };
        write!(f, "{tag}")
    }
}

/// A child process's exit status, normalized off `std::process::ExitStatus`
/// into plain typed data (`src/evaluator/builtins_os.rs` turns this into a
/// structured alist) instead of exposing the platform `ExitStatus` type.
#[derive(Debug, Clone, Copy)]
pub struct ChildExitStatus {
    /// The process's exit code, if it exited normally (as opposed to being
    /// terminated by a signal).
    pub code: Option<i32>,
    /// The signal number that terminated the process, if any (Unix only;
    /// via `std::os::unix::process::ExitStatusExt`).
    pub signal: Option<i32>,
    /// True iff the process exited with status 0.
    pub success: bool,
}

impl From<std::process::ExitStatus> for ChildExitStatus {
    #[cfg(unix)]
    fn from(status: std::process::ExitStatus) -> Self {
        use std::os::unix::process::ExitStatusExt;
        ChildExitStatus {
            code: status.code(),
            signal: status.signal(),
            success: status.success(),
        }
    }

    // Signal termination is a POSIX concept; on a non-Unix target there is
    // no such thing to report (issue #260 is explicitly scoped to
    // Linux/POSIX, but this keeps the dependency-light core compilable
    // elsewhere rather than hard-breaking the build).
    #[cfg(not(unix))]
    fn from(status: std::process::ExitStatus) -> Self {
        ChildExitStatus {
            code: status.code(),
            signal: None,
            success: status.success(),
        }
    }
}

/// The payload of a [`LispVal::OsChild`] (issue #260, epic #253): a spawned
/// `std::process::Child`. Like [`NetHandleObj`], not a byte stream itself --
/// its stdio pipes (when requested) are separate [`LispVal::Port`]s wrapping
/// `ChildStdin`/`ChildStdout`/`ChildStderr` via [`LispVal::wrap_writer`]/
/// [`LispVal::wrap_reader`] -- so it gets its own opaque representation.
///
/// **Deterministic ownership.** Explicit `wait!`/`try-wait!` (which reap the
/// process) or `kill!`/`terminate!` (which signal it; the caller must still
/// `wait!` to reap it, exactly like POSIX) are the primary cleanup path.
/// **Drop backstop**: when the last reference goes away, a best-effort
/// non-blocking `try_wait` reaps the child if it has *already* exited
/// (avoiding a zombie); a still-running child is deliberately left running
/// rather than killed -- matching common daemon-spawning idioms and
/// documented here and in `lib/41-os.lisp`, not a silent surprise kill.
///
/// **Capabilities.** Like `PortObj`/`NetHandleObj`, this performs no
/// capability checks itself; `src/evaluator/builtins_os.rs` checks
/// `OS-PROCESS` before ever constructing one (plus consults the host policy
/// hook, [`environment::Environment::set_os_policy`]), and `OS-SIGNAL`
/// before signaling any PID not held as an owned handle. Continued use of an
/// already-spawned child (`wait!`/`try-wait!`/`kill!`/`terminate!`) performs
/// no further capability check -- the epic's "a successfully returned handle
/// is authority to continue" rule.
#[derive(Debug)]
pub struct ChildObj {
    /// Diagnostic name: the program path as spawned. Not necessarily unique;
    /// used only for `print`/error messages.
    pub name: String,
    /// The child's OS PID, captured at spawn time and retained even after
    /// the child is reaped (unlike a live-only `Child::id()` read) so
    /// diagnostics/logging can always report it.
    pub pid: u32,
    state: SharedCell<ChildState>,
}

impl ChildObj {
    pub(crate) fn new(name: impl Into<String>, child: std::process::Child) -> Shared<Self> {
        let pid = child.id();
        Shared::new(ChildObj {
            name: name.into(),
            pid,
            state: SharedCell::new(ChildState::Running(child)),
        })
    }

    /// True unless the child has been reaped (`wait!`/`try-wait!` observed
    /// its exit, or the Drop backstop did).
    pub fn is_open(&self) -> bool {
        matches!(&*self.state.borrow(), ChildState::Running(_))
    }

    /// Block until the child exits, then reap and cache its status.
    /// Idempotent: calling this again after the child is already reaped
    /// returns the cached status instead of erroring.
    pub fn wait(&self) -> std::io::Result<ChildExitStatus> {
        let mut state = self.state.borrow_mut();
        match &mut *state {
            ChildState::Running(c) => {
                let status: ChildExitStatus = c.wait()?.into();
                *state = ChildState::Reaped(status);
                Ok(status)
            }
            ChildState::Reaped(status) => Ok(*status),
        }
    }

    /// Non-blocking poll: `Ok(None)` if still running, `Ok(Some(status))`
    /// (reaping it) if it has exited. Idempotent like [`ChildObj::wait`].
    pub fn try_wait(&self) -> std::io::Result<Option<ChildExitStatus>> {
        let mut state = self.state.borrow_mut();
        match &mut *state {
            ChildState::Running(c) => match c.try_wait()? {
                Some(exit) => {
                    let status: ChildExitStatus = exit.into();
                    *state = ChildState::Reaped(status);
                    Ok(Some(status))
                }
                None => Ok(None),
            },
            ChildState::Reaped(status) => Ok(Some(*status)),
        }
    }

    /// Send `SIGKILL` (hard kill) via `std::process::Child::kill`, the one
    /// signal std exposes without FFI. Errors with `:CLOSED` (via the
    /// caller) if already reaped. Does not reap -- call `wait!`/`try-wait!`
    /// afterward, exactly like POSIX `kill` + `waitpid`.
    pub fn kill(&self) -> std::io::Result<()> {
        match &mut *self.state.borrow_mut() {
            ChildState::Running(c) => c.kill(),
            ChildState::Reaped(_) => Err(std::io::Error::other("process is closed")),
        }
    }

    /// Send `SIGTERM` (graceful termination) via [`send_signal`]. See
    /// [`ChildObj::kill`] for the reaping contract.
    pub fn terminate(&self) -> std::io::Result<()> {
        match &*self.state.borrow() {
            ChildState::Running(_) => send_signal(self.pid as i32, SIGTERM),
            ChildState::Reaped(_) => Err(std::io::Error::other("process is closed")),
        }
    }
}

impl Drop for ChildObj {
    /// Drop backstop (issue #260's "deterministic close + Drop backstop"):
    /// best-effort, non-blocking reap of an already-exited child so it does
    /// not linger as a zombie just because the Lisp handle was garbage
    /// collected/dropped without an explicit `wait!`/`try-wait!`. A child
    /// that is *still running* when the last reference drops is
    /// deliberately left running, not killed -- see the struct doc comment.
    fn drop(&mut self) {
        if let ChildState::Running(c) = &mut *self.state.borrow_mut() {
            let _ = c.try_wait();
        }
    }
}

/// `kill(2)`'s well-known signal numbers on Linux/most POSIX systems,
/// exposed as typed names (issue #260's "typed signal names/numbers")
/// instead of Lisp code ever needing to know raw numbers.
pub const SIGHUP: i32 = 1;
pub const SIGINT: i32 = 2;
pub const SIGQUIT: i32 = 3;
pub const SIGKILL: i32 = 9;
pub const SIGPIPE: i32 = 13;
pub const SIGALRM: i32 = 14;
pub const SIGTERM: i32 = 15;
pub const SIGCHLD: i32 = 17;
pub const SIGCONT: i32 = 18;
pub const SIGSTOP: i32 = 19;
pub const SIGUSR1: i32 = 10;
pub const SIGUSR2: i32 = 12;

/// Look up a typed signal name (case-insensitive, with or without the
/// leading "SIG") against the fixed table above. `None` for anything not in
/// the table -- this interface deliberately does not accept raw signal
/// numbers from Lisp (issue #253's "no raw syscall numbers" ruling extends
/// to signal numbers: every signal Lisp can send has a name).
pub fn signal_by_name(name: &str) -> Option<i32> {
    let upper = name
        .trim_start_matches(':')
        .trim_start_matches("SIG")
        .to_ascii_uppercase();
    Some(match upper.as_str() {
        "HUP" => SIGHUP,
        "INT" => SIGINT,
        "QUIT" => SIGQUIT,
        "KILL" => SIGKILL,
        "PIPE" => SIGPIPE,
        "ALRM" => SIGALRM,
        "TERM" => SIGTERM,
        "CHLD" => SIGCHLD,
        "CONT" => SIGCONT,
        "STOP" => SIGSTOP,
        "USR1" => SIGUSR1,
        "USR2" => SIGUSR2,
        _ => return None,
    })
}

// `kill(2)` is a named, fixed, POSIX libc function -- not the "raw syscall
// numbers" issue #253/#260 rule out (that ruling targets an unrestricted
// `(syscall number ...)` Lisp primitive; this is one hard-coded, typed,
// internal Rust helper, the same technique `std::process::Child::kill`
// itself uses internally on Unix). No new crate dependency: `libc` is
// already dynamically linked into every Rust binary on Linux, this just
// declares one of its well-known symbols. Lisp never sees a raw signal
// number or a general FFI primitive -- only [`signal_by_name`]'s fixed
// table and the typed `OS-PROCESS-TERMINATE*`/`OS-SIGNAL*` kernel
// primitives (`src/evaluator/builtins_os.rs`).
#[cfg(unix)]
unsafe extern "C" {
    fn kill(pid: i32, sig: i32) -> i32;
}

/// Send POSIX signal `sig` to `pid` via `kill(2)`. See the `unsafe extern`
/// block above for why this is in scope for issue #260 despite "no raw
/// syscalls"/"no arbitrary FFI". Not available on a non-Unix target (issue
/// #260 is explicitly scoped to Linux/POSIX); kept `#[cfg(not(unix))]`-safe
/// so the dependency-light core stays compilable elsewhere rather than
/// hard-breaking the build.
#[cfg(unix)]
pub(crate) fn send_signal(pid: i32, sig: i32) -> std::io::Result<()> {
    // SAFETY: `kill(2)` takes two plain integers and returns a plain
    // integer; no pointers cross the FFI boundary, so there is no aliasing/
    // provenance/lifetime concern. `-1` signals failure with `errno` set,
    // which `std::io::Error::last_os_error()` reads immediately afterward
    // (nothing else on this thread touches `errno` in between).
    let rc = unsafe { kill(pid, sig) };
    if rc == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

#[cfg(not(unix))]
pub(crate) fn send_signal(_pid: i32, _sig: i32) -> std::io::Result<()> {
    Err(std::io::Error::other(
        "signal delivery is not supported on this platform",
    ))
}

/// An OS operation subject to the host policy hook (issue #260, epic #253):
/// [`environment::Environment::set_os_policy`]. Mirrors [`NetOperation`]'s
/// role for networking.
#[derive(Debug, Clone)]
pub enum OsOperation<'a> {
    /// Spawning a child process (`OS-SPAWN*`).
    Spawn {
        program: &'a str,
        args: &'a [String],
        cwd: Option<&'a str>,
    },
    /// Sending a signal to a PID not held as an owned child handle
    /// (`OS-SIGNAL*`).
    Signal { pid: i64, signal: i32 },
}

/// A host-installed policy callback consulted by `builtins_os.rs` before
/// spawning or signaling, in addition to (not instead of) the
/// `OS-PROCESS`/`OS-SIGNAL` capability checks. See
/// [`environment::Environment::set_os_policy`].
pub(crate) type OsPolicyFn = dyn Fn(&OsOperation) -> bool;

/// A network operation subject to the host policy hook (issue #258, epic
/// #253): [`environment::Environment::set_net_policy`]. Coarser than the
/// three capabilities (`NET-DNS`/`NET-CONNECT`/`NET-LISTEN`) it complements
/// -- UDP `send-to`/`connect` are policy-checked as `Connect`, UDP `bind` as
/// `Listen` -- since the policy question ("is this host/port allowed") is
/// the same shape for TCP and UDP.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetOperation {
    /// Explicit hostname resolution (`NET-RESOLVE*`).
    Resolve,
    /// Outbound connection: `TCP-CONNECT*`, `UDP-CONNECT*`, `UDP-SEND-TO*`.
    Connect,
    /// Binding/listening for inbound traffic: `TCP-LISTEN*`, `UDP-BIND*`.
    Listen,
}

/// A host-installed policy callback consulted by `builtins_net.rs` before
/// resolving/connecting/binding, in addition to (not instead of) the
/// coarse-grained capability check. Lets an embedder scope a granted
/// capability to specific hosts/ports -- e.g. an HTTP-client grant that
/// should not become unrestricted SSRF authority. See
/// [`environment::Environment::set_net_policy`].
pub(crate) type NetPolicyFn = dyn Fn(NetOperation, &str, u16) -> bool;

impl LispVal {
    /// Wrap a host `Read` stream as an input [`LispVal::Port`], for
    /// embedders that want to hand Lisp code a byte source (a pipe, a
    /// decompressor, a network stream, ...) without exposing a raw file
    /// descriptor (issue #255's embedding requirement). The resulting port
    /// is not seekable; `kind`/`name` are diagnostic only.
    pub fn wrap_reader(
        name: impl Into<String>,
        kind: &'static str,
        reader: Box<dyn Read>,
    ) -> LispVal {
        LispVal::Port(PortObj::new(
            kind,
            name,
            true,
            false,
            false,
            PortState::HostReader(reader),
        ))
    }

    /// Wrap a host `Write` sink as an output [`LispVal::Port`]. See
    /// [`LispVal::wrap_reader`].
    pub fn wrap_writer(
        name: impl Into<String>,
        kind: &'static str,
        writer: Box<dyn Write>,
    ) -> LispVal {
        LispVal::Port(PortObj::new(
            kind,
            name,
            false,
            true,
            false,
            PortState::HostWriter(writer),
        ))
    }
}

/// The payload of a [`LispVal::Channel`] (only present under the `concurrency` feature).
///
/// A channel allows single-producer / single-consumer message passing between
/// two Lisp evaluation contexts.  Values are serialised via the printer on the
/// send side and deserialised via the reader on the receive side, avoiding the
/// requirement for [`LispVal`] to be `Send`.
///
/// The receiver is wrapped in a [`std::sync::Mutex`] so that the whole
/// `ChannelObj` can be placed behind an [`std::sync::Arc`] and cloned cheaply
/// (the Mutex guards exclusive access to `recv`).
#[cfg(feature = "concurrency")]
#[derive(Debug)]
pub struct ChannelObj {
    /// The sending half of the underlying `mpsc` channel.
    /// `Sender<String>` is `Clone + Send`, so this can be duplicated freely.
    pub sender: std::sync::mpsc::Sender<String>,
    /// The receiving half.  `mpsc::Receiver` is neither `Clone` nor `Sync`,
    /// so we wrap it in a `Mutex` to allow the `ChannelObj` to live behind `Arc`.
    pub receiver: std::sync::Mutex<std::sync::mpsc::Receiver<String>>,
}

/// The payload of a [`LispVal::Struct`].
#[derive(Debug, Clone)]
pub struct StructObj {
    /// Uppercased typed struct name, e.g. `POINT`.
    pub type_name: String,
    /// Field values in declaration order.
    pub fields: Vec<LispVal>,
}

impl LispVal {
    /// Wrap a host value implementing [`LispValExtension`] into a `LispVal`.
    pub fn ext<T: LispValExtension + 'static>(v: T) -> Self {
        LispVal::Extension(Shared::new(v))
    }
}

impl fmt::Debug for LispVal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LispVal::Symbol(s) => write!(f, "Symbol({:?})", s.borrow().name),
            LispVal::Number(n) => write!(f, "Number({n})"),
            LispVal::Char(b) => write!(f, "Char({b})"),
            LispVal::Float(v) => write!(f, "Float({v})"),
            LispVal::String(s) => write!(f, "String({s:?})"),
            LispVal::Builtin(b) => write!(f, "Builtin({b:?})"),
            LispVal::Lambda(_) => write!(f, "Lambda(...)"),
            LispVal::Fexpr(_) => write!(f, "Fexpr(...)"),
            LispVal::Macro(_) => write!(f, "Macro(...)"),
            LispVal::Vau(_) => write!(f, "Vau(...)"),
            LispVal::Cons { car, cdr } => write!(f, "Cons({car:?}, {cdr:?})"),
            LispVal::Nil => write!(f, "Nil"),
            LispVal::HashTable(_) => write!(f, "HashTable(...)"),
            LispVal::Native(_) => write!(f, "Native(...)"),
            LispVal::Environment(_) => write!(f, "Environment(...)"),
            LispVal::Array(a) => write!(f, "Array(len={})", a.borrow().len()),
            LispVal::Struct(s) => write!(f, "Struct(type={}, fields={:?})", s.type_name, s.fields),
            LispVal::Extension(e) => write!(f, "Extension({})", e.type_name()),
            LispVal::Error(e) => write!(f, "Error({:?}, {:?})", e.message, e.data),
            LispVal::Port(p) => write!(f, "Port(kind={}, name={:?})", p.kind, p.name),
            LispVal::NetHandle(h) => {
                write!(f, "NetHandle(kind={}, name={:?})", h.kind, h.name)
            }
            LispVal::OsChild(c) => write!(f, "OsChild(name={:?})", c.name),
            #[cfg(feature = "concurrency")]
            LispVal::Channel(_) => write!(f, "Channel(...)"),
        }
    }
}

impl Clone for LispVal {
    fn clone(&self) -> Self {
        match self {
            LispVal::Symbol(s) => LispVal::Symbol(s.clone()),
            LispVal::Number(n) => LispVal::Number(*n),
            LispVal::Char(b) => LispVal::Char(*b),
            LispVal::Float(f) => LispVal::Float(*f),
            LispVal::String(s) => LispVal::String(s.clone()),
            LispVal::Builtin(b) => LispVal::Builtin(b.clone()),
            LispVal::Lambda(l) => LispVal::Lambda(l.clone()),
            LispVal::Fexpr(x) => LispVal::Fexpr(x.clone()),
            LispVal::Macro(m) => LispVal::Macro(m.clone()),
            LispVal::Vau(v) => LispVal::Vau(v.clone()),
            LispVal::Cons { car, cdr } => LispVal::Cons {
                car: car.clone(),
                cdr: cdr.clone(),
            },
            LispVal::Nil => LispVal::Nil,
            LispVal::HashTable(h) => LispVal::HashTable(h.clone()),
            LispVal::Native(f) => LispVal::Native(f.clone()),
            LispVal::Environment(e) => LispVal::Environment(e.clone()),
            LispVal::Array(a) => LispVal::Array(a.clone()),
            LispVal::Struct(s) => LispVal::Struct(s.clone()),
            LispVal::Extension(e) => LispVal::Extension(e.clone()),
            LispVal::Error(e) => LispVal::Error(e.clone()),
            LispVal::Port(p) => LispVal::Port(p.clone()),
            LispVal::NetHandle(h) => LispVal::NetHandle(h.clone()),
            LispVal::OsChild(c) => LispVal::OsChild(c.clone()),
            #[cfg(feature = "concurrency")]
            LispVal::Channel(c) => LispVal::Channel(std::sync::Arc::clone(c)),
        }
    }
}

fn lisp_float_eq(a: f64, b: f64) -> bool {
    a == b || (a.is_nan() && b.is_nan())
}

fn lisp_float_hash_bits(f: f64) -> u64 {
    if f.is_nan() {
        f64::NAN.to_bits()
    } else if f == 0.0 {
        0.0f64.to_bits()
    } else {
        f.to_bits()
    }
}

impl PartialEq for LispVal {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (LispVal::Symbol(a), LispVal::Symbol(b)) => Shared::ptr_eq(a, b),
            (LispVal::Number(a), LispVal::Number(b)) => a == b,
            (LispVal::Char(a), LispVal::Char(b)) => a == b,
            (LispVal::Float(a), LispVal::Float(b)) => lisp_float_eq(*a, *b),
            (LispVal::String(a), LispVal::String(b)) => a == b,
            (
                LispVal::Cons {
                    car: car1,
                    cdr: cdr1,
                },
                LispVal::Cons {
                    car: car2,
                    cdr: cdr2,
                },
            ) => car1 == car2 && cdr1 == cdr2,
            (LispVal::Nil, LispVal::Nil) => true,
            (LispVal::HashTable(a), LispVal::HashTable(b)) => Shared::ptr_eq(a, b),
            (LispVal::Builtin(a), LispVal::Builtin(b)) => a == b,
            (LispVal::Lambda(a), LispVal::Lambda(b)) => a == b,
            (LispVal::Fexpr(a), LispVal::Fexpr(b)) => a == b,
            (LispVal::Macro(a), LispVal::Macro(b)) => a == b,
            (LispVal::Vau(a), LispVal::Vau(b)) => a == b,
            (LispVal::Native(a), LispVal::Native(b)) => Shared::ptr_eq(a, b),
            (LispVal::Environment(a), LispVal::Environment(b)) => Shared::ptr_eq(a, b),
            (LispVal::Array(a), LispVal::Array(b)) => Shared::ptr_eq(a, b),
            (LispVal::Struct(a), LispVal::Struct(b)) => {
                a.type_name == b.type_name && a.fields == b.fields
            }
            (LispVal::Extension(a), LispVal::Extension(b)) => a.eq_ext(b.as_ref()),
            (LispVal::Error(a), LispVal::Error(b)) => a.message == b.message && a.data == b.data,
            // Ports are opaque and compare by identity (issue #255), like
            // Array/HashTable/Native/Environment.
            (LispVal::Port(a), LispVal::Port(b)) => Shared::ptr_eq(a, b),
            // Network handles are likewise opaque and compare by identity
            // (issue #258).
            (LispVal::NetHandle(a), LispVal::NetHandle(b)) => Shared::ptr_eq(a, b),
            // Child-process handles are likewise opaque and compare by
            // identity (issue #260).
            (LispVal::OsChild(a), LispVal::OsChild(b)) => Shared::ptr_eq(a, b),
            #[cfg(feature = "concurrency")]
            (LispVal::Channel(a), LispVal::Channel(b)) => std::sync::Arc::ptr_eq(a, b),
            _ => false,
        }
    }
}

impl Eq for LispVal {}

impl Hash for LispVal {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self {
            LispVal::Symbol(s) => Shared::as_ptr(s).hash(state),
            LispVal::Number(n) => n.hash(state),
            LispVal::Char(b) => b.hash(state),
            LispVal::Float(f) => lisp_float_hash_bits(*f).hash(state),
            LispVal::String(s) => s.hash(state),
            LispVal::Cons { car, cdr } => {
                car.hash(state);
                cdr.hash(state);
            }
            LispVal::Nil => 0.hash(state),
            LispVal::HashTable(h) => {
                Shared::as_ptr(h).hash(state);
            }
            LispVal::Native(f) => {
                Shared::as_ptr(f).hash(state);
            }
            LispVal::Environment(e) => {
                Shared::as_ptr(e).hash(state);
            }
            LispVal::Array(a) => {
                Shared::as_ptr(a).hash(state);
            }
            LispVal::Struct(s) => {
                s.type_name.hash(state);
                s.fields.hash(state);
            }
            LispVal::Extension(e) => {
                e.hash_ext(state);
            }
            LispVal::Error(e) => {
                e.message.hash(state);
                e.data.hash(state);
            }
            LispVal::Port(p) => {
                Shared::as_ptr(p).hash(state);
            }
            LispVal::NetHandle(h) => {
                Shared::as_ptr(h).hash(state);
            }
            LispVal::OsChild(c) => {
                Shared::as_ptr(c).hash(state);
            }
            LispVal::Builtin(_)
            | LispVal::Lambda(_)
            | LispVal::Fexpr(_)
            | LispVal::Macro(_)
            | LispVal::Vau(_) => {
                // Functions are not hashable by value.
            }
            #[cfg(feature = "concurrency")]
            LispVal::Channel(c) => std::sync::Arc::as_ptr(c).hash(state),
        }
    }
}

// ---------------------------------------------------------------------------
// From<T> for LispVal — infallible conversions from Rust primitives
// ---------------------------------------------------------------------------

impl From<i64> for LispVal {
    fn from(n: i64) -> Self {
        LispVal::Number(n)
    }
}

impl From<f64> for LispVal {
    fn from(f: f64) -> Self {
        LispVal::Float(f)
    }
}

impl From<bool> for LispVal {
    fn from(b: bool) -> Self {
        if b {
            LispVal::Symbol(Shared::new(SharedCell::new(Symbol {
                name: "T".to_string(),
                plist: HashMap::new(),
                value: None,
                id: 0, // un-interned; never used as a binding key
                is_keyword: false,
                is_dynamic: false,
                special_form: None,
            })))
        } else {
            LispVal::Nil
        }
    }
}

impl From<String> for LispVal {
    fn from(s: String) -> Self {
        LispVal::String(s)
    }
}

impl From<&str> for LispVal {
    fn from(s: &str) -> Self {
        LispVal::String(s.to_string())
    }
}

impl From<Vec<LispVal>> for LispVal {
    fn from(v: Vec<LispVal>) -> Self {
        v.into_iter()
            .rev()
            .fold(LispVal::Nil, |cdr, car| LispVal::Cons {
                car: Shared::new(car),
                cdr: Shared::new(cdr),
            })
    }
}

// ---------------------------------------------------------------------------
// TryFrom<LispVal> for Rust primitives — fallible extractions
// ---------------------------------------------------------------------------

/// Render a value for inclusion in an error message, truncated so a huge list
/// or string cannot flood the output (issue #247). Mirrors
/// `evaluator::core::err_val`, which is private to the evaluator module and
/// so not reachable from here.
fn err_val_display(v: &LispVal) -> String {
    let s = printer::print(v);
    if s.chars().count() > 60 {
        let head: String = s.chars().take(57).collect();
        format!("{head}...")
    } else {
        s
    }
}

impl TryFrom<LispVal> for i64 {
    type Error = LispError;
    fn try_from(val: LispVal) -> Result<Self, Self::Error> {
        match val {
            LispVal::Number(n) => Ok(n),
            other => Err(LispError::Generic(format!(
                "expected integer, got {}",
                err_val_display(&other)
            ))),
        }
    }
}

impl TryFrom<LispVal> for f64 {
    type Error = LispError;
    fn try_from(val: LispVal) -> Result<Self, Self::Error> {
        match val {
            LispVal::Float(f) => Ok(f),
            LispVal::Number(n) => Ok(n as f64),
            other => Err(LispError::Generic(format!(
                "expected number, got {}",
                err_val_display(&other)
            ))),
        }
    }
}

impl TryFrom<LispVal> for bool {
    type Error = LispError;
    fn try_from(val: LispVal) -> Result<Self, Self::Error> {
        Ok(val.is_truthy())
    }
}

impl TryFrom<LispVal> for String {
    type Error = LispError;
    fn try_from(val: LispVal) -> Result<Self, Self::Error> {
        match val {
            LispVal::String(s) => Ok(s),
            other => Err(LispError::Generic(format!(
                "expected string, got {}",
                err_val_display(&other)
            ))),
        }
    }
}

impl TryFrom<LispVal> for Vec<LispVal> {
    type Error = LispError;
    fn try_from(val: LispVal) -> Result<Self, Self::Error> {
        val.as_list_vec()
    }
}

// ---------------------------------------------------------------------------
// LispVal helpers
// ---------------------------------------------------------------------------

impl LispVal {
    /// Build a Lisp list from any iterator of items convertible to LispVal.
    pub fn list<I, T>(iter: I) -> Self
    where
        I: IntoIterator<Item = T>,
        T: Into<LispVal>,
    {
        let items: Vec<LispVal> = iter.into_iter().map(Into::into).collect();
        LispVal::from(items)
    }

    /// Extract the integer value, or return an error.
    pub fn as_number(&self) -> Result<i64, LispError> {
        match self {
            LispVal::Number(n) => Ok(*n),
            other => Err(LispError::Generic(format!(
                "expected integer, got {}",
                err_val_display(other)
            ))),
        }
    }

    /// Extract the float value (also accepts integers, coercing to f64).
    pub fn as_float(&self) -> Result<f64, LispError> {
        match self {
            LispVal::Float(f) => Ok(*f),
            LispVal::Number(n) => Ok(*n as f64),
            other => Err(LispError::Generic(format!(
                "expected number, got {}",
                err_val_display(other)
            ))),
        }
    }

    /// Extract the string value, or return an error.
    pub fn as_str_val(&self) -> Result<&str, LispError> {
        match self {
            LispVal::String(s) => Ok(s.as_str()),
            other => Err(LispError::Generic(format!(
                "expected string, got {}",
                err_val_display(other)
            ))),
        }
    }

    /// Collect a proper Lisp list into a Vec, or return an error if not a list.
    pub fn as_list_vec(&self) -> Result<Vec<LispVal>, LispError> {
        let mut result = Vec::new();
        let mut current = self;
        loop {
            match current {
                LispVal::Nil => break,
                LispVal::Cons { car, cdr } => {
                    result.push(car.as_ref().clone());
                    current = cdr;
                }
                other => {
                    return Err(LispError::Generic(format!(
                        "not a proper list: dotted pair ending in {}",
                        err_val_display(other)
                    )));
                }
            }
        }
        Ok(result)
    }

    /// Lisp truthiness: everything except Nil is truthy.
    pub fn is_truthy(&self) -> bool {
        !matches!(self, LispVal::Nil)
    }
}

// ---------------------------------------------------------------------------
// Embedded standard library
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Embedded standard library
// ---------------------------------------------------------------------------

/// The standard library sources embedded at compile time, in load order.
///
/// Each entry is `(filename, source)`. The filename is used only for error messages.
const STDLIB_SOURCES: &[(&str, &str)] = &[
    ("00-core.lisp", include_str!("../lib/00-core.lisp")),
    ("01-list.lisp", include_str!("../lib/01-list.lisp")),
    ("02-cxr.lisp", include_str!("../lib/02-cxr.lisp")),
    ("03-meta.lisp", include_str!("../lib/03-meta.lisp")),
    (
        "04-predicates.lisp",
        include_str!("../lib/04-predicates.lisp"),
    ),
    ("05-math.lisp", include_str!("../lib/05-math.lisp")),
    ("06-require.lisp", include_str!("../lib/06-require.lisp")),
    ("07-shell.lisp", include_str!("../lib/07-shell.lisp")),
    ("08-vau.lisp", include_str!("../lib/08-vau.lisp")),
    ("09-lisp15.lisp", include_str!("../lib/09-lisp15.lisp")),
    ("10-testing.lisp", include_str!("../lib/10-testing.lisp")),
    (
        "11-optimizer-vau.lisp",
        include_str!("../lib/11-optimizer-vau.lisp"),
    ),
    ("12-control.lisp", include_str!("../lib/12-control.lisp")),
    (
        "13-functional.lisp",
        include_str!("../lib/13-functional.lisp"),
    ),
    ("14-strings.lisp", include_str!("../lib/14-strings.lisp")),
    (
        "15-sets-hash.lisp",
        include_str!("../lib/15-sets-hash.lisp"),
    ),
    (
        "16-conditions.lisp",
        include_str!("../lib/16-conditions.lisp"),
    ),
    ("17-arrays.lisp", include_str!("../lib/17-arrays.lisp")),
    ("18-format.lisp", include_str!("../lib/18-format.lisp")),
    (
        "19-call-graph.lisp",
        include_str!("../lib/19-call-graph.lisp"),
    ),
    (
        "20-condensation.lisp",
        include_str!("../lib/20-condensation.lisp"),
    ),
    (
        "21-cl-compat.lisp",
        include_str!("../lib/21-cl-compat.lisp"),
    ),
    ("22-guard.lisp", include_str!("../lib/22-guard.lisp")),
    ("23-match.lisp", include_str!("../lib/23-match.lisp")),
    ("24-rules.lisp", include_str!("../lib/24-rules.lisp")),
    ("25-variants.lisp", include_str!("../lib/25-variants.lisp")),
    (
        "26-instrument.lisp",
        include_str!("../lib/26-instrument.lisp"),
    ),
    ("27-modules.lisp", include_str!("../lib/27-modules.lisp")),
    ("28-types.lisp", include_str!("../lib/28-types.lisp")),
    (
        "29-protocols.lisp",
        include_str!("../lib/29-protocols.lisp"),
    ),
    ("30-text.lisp", include_str!("../lib/30-text.lisp")),
    ("31-ports.lisp", include_str!("../lib/31-ports.lisp")),
    ("32-base64.lisp", include_str!("../lib/32-base64.lisp")),
    ("33-hex.lisp", include_str!("../lib/33-hex.lisp")),
    ("34-url.lisp", include_str!("../lib/34-url.lisp")),
    ("35-json.lisp", include_str!("../lib/35-json.lisp")),
    ("36-mime.lisp", include_str!("../lib/36-mime.lisp")),
    ("37-net.lisp", include_str!("../lib/37-net.lisp")),
    ("38-tcp.lisp", include_str!("../lib/38-tcp.lisp")),
    ("39-udp.lisp", include_str!("../lib/39-udp.lisp")),
    ("40-http.lisp", include_str!("../lib/40-http.lisp")),
    ("41-os.lisp", include_str!("../lib/41-os.lisp")),
    ("42-os-linux.lisp", include_str!("../lib/42-os-linux.lisp")),
    (
        "97-doc-renderer.lisp",
        include_str!("../lib/97-doc-renderer.lisp"),
    ),
    (
        "98-help-system.lisp",
        include_str!("../lib/98-help-system.lisp"),
    ),
    (
        "99-help-data.lisp",
        include_str!("../lib/99-help-data.lisp"),
    ),
];

/// The Prelude: the stable general-purpose language vocabulary loaded by
/// [`Environment::with_prelude`] (issue #256, epic #253). This is the ticket's
/// explicit Prelude list ("00-core through 05-math; 08-vau; 12-control through
/// 18-format; 21-cl-compat") plus `06-require.lisp` -- REQUIRE/PROVIDE must be
/// Prelude vocabulary too, since pulling in any optional library by name is
/// the entire point of the split (see that file's header comment). Every file
/// here is also part of [`STDLIB_SOURCES`] (duplicated `include_str!`, not
/// shared storage) so [`load_stdlib`] stays completely independent of this
/// list and therefore behavior-unchanged.
const PRELUDE_SOURCES: &[(&str, &str)] = &[
    ("00-core.lisp", include_str!("../lib/00-core.lisp")),
    ("01-list.lisp", include_str!("../lib/01-list.lisp")),
    ("02-cxr.lisp", include_str!("../lib/02-cxr.lisp")),
    ("03-meta.lisp", include_str!("../lib/03-meta.lisp")),
    (
        "04-predicates.lisp",
        include_str!("../lib/04-predicates.lisp"),
    ),
    ("05-math.lisp", include_str!("../lib/05-math.lisp")),
    ("06-require.lisp", include_str!("../lib/06-require.lisp")),
    ("08-vau.lisp", include_str!("../lib/08-vau.lisp")),
    ("12-control.lisp", include_str!("../lib/12-control.lisp")),
    (
        "13-functional.lisp",
        include_str!("../lib/13-functional.lisp"),
    ),
    ("14-strings.lisp", include_str!("../lib/14-strings.lisp")),
    (
        "15-sets-hash.lisp",
        include_str!("../lib/15-sets-hash.lisp"),
    ),
    (
        "16-conditions.lisp",
        include_str!("../lib/16-conditions.lisp"),
    ),
    ("17-arrays.lisp", include_str!("../lib/17-arrays.lisp")),
    ("18-format.lisp", include_str!("../lib/18-format.lisp")),
    (
        "21-cl-compat.lisp",
        include_str!("../lib/21-cl-compat.lisp"),
    ),
];

/// The embedded tier of REQUIRE's module resolution (issue #256): every
/// currently-optional library file, as `(module_name, filename, source)`.
/// `module_name` is what Lisp code passes to `(require 'name)` -- derived
/// from the filename by dropping the numeric prefix and `.lisp` extension,
/// uppercased. [`with_prelude`](Environment::with_prelude) does not evaluate
/// these; [`load_stdlib`] evaluates them all (via [`STDLIB_SOURCES`], not
/// this list) for `with_stdlib`'s unchanged eager load, then marks each one
/// REQUIRE-loaded so `(require 'name)` afterward is a correct no-op instead
/// of a redundant re-evaluation.
///
/// Not every file here is independently requirable in true isolation --
/// some optional libraries call into others' functions from inside function
/// bodies (resolved lazily at call time, so the file still *loads* cleanly)
/// without themselves declaring `(require ...)` for that use; auditing and
/// declaring the full transitive dependency graph for the *pre-existing*
/// optional libraries is out of scope for #256 (the real *load-time*
/// forward references among them: 30-text.lisp's own `(require 'modules)`;
/// 99-help-data.lisp's own `(require 'help-system)`; and #257's five codec
/// modules, each `(require 'modules)` and `base64`/`hex`/`json`/`url` also
/// `(require 'text)` -- all already declared in those files).
const OPTIONAL_MODULES: &[(&str, &str, &str)] = &[
    (
        "SHELL",
        "07-shell.lisp",
        include_str!("../lib/07-shell.lisp"),
    ),
    (
        "LISP15",
        "09-lisp15.lisp",
        include_str!("../lib/09-lisp15.lisp"),
    ),
    (
        "TESTING",
        "10-testing.lisp",
        include_str!("../lib/10-testing.lisp"),
    ),
    (
        "OPTIMIZER-VAU",
        "11-optimizer-vau.lisp",
        include_str!("../lib/11-optimizer-vau.lisp"),
    ),
    (
        "CALL-GRAPH",
        "19-call-graph.lisp",
        include_str!("../lib/19-call-graph.lisp"),
    ),
    (
        "CONDENSATION",
        "20-condensation.lisp",
        include_str!("../lib/20-condensation.lisp"),
    ),
    (
        "GUARD",
        "22-guard.lisp",
        include_str!("../lib/22-guard.lisp"),
    ),
    (
        "MATCH",
        "23-match.lisp",
        include_str!("../lib/23-match.lisp"),
    ),
    (
        "RULES",
        "24-rules.lisp",
        include_str!("../lib/24-rules.lisp"),
    ),
    (
        "VARIANTS",
        "25-variants.lisp",
        include_str!("../lib/25-variants.lisp"),
    ),
    (
        "INSTRUMENT",
        "26-instrument.lisp",
        include_str!("../lib/26-instrument.lisp"),
    ),
    (
        "MODULES",
        "27-modules.lisp",
        include_str!("../lib/27-modules.lisp"),
    ),
    (
        "TYPES",
        "28-types.lisp",
        include_str!("../lib/28-types.lisp"),
    ),
    (
        "PROTOCOLS",
        "29-protocols.lisp",
        include_str!("../lib/29-protocols.lisp"),
    ),
    ("TEXT", "30-text.lisp", include_str!("../lib/30-text.lisp")),
    (
        "PORTS",
        "31-ports.lisp",
        include_str!("../lib/31-ports.lisp"),
    ),
    (
        "BASE64",
        "32-base64.lisp",
        include_str!("../lib/32-base64.lisp"),
    ),
    ("HEX", "33-hex.lisp", include_str!("../lib/33-hex.lisp")),
    ("URL", "34-url.lisp", include_str!("../lib/34-url.lisp")),
    ("JSON", "35-json.lisp", include_str!("../lib/35-json.lisp")),
    ("MIME", "36-mime.lisp", include_str!("../lib/36-mime.lisp")),
    ("NET", "37-net.lisp", include_str!("../lib/37-net.lisp")),
    ("TCP", "38-tcp.lisp", include_str!("../lib/38-tcp.lisp")),
    ("UDP", "39-udp.lisp", include_str!("../lib/39-udp.lisp")),
    ("HTTP", "40-http.lisp", include_str!("../lib/40-http.lisp")),
    ("OS", "41-os.lisp", include_str!("../lib/41-os.lisp")),
    (
        "OS-LINUX",
        "42-os-linux.lisp",
        include_str!("../lib/42-os-linux.lisp"),
    ),
    (
        "DOC-RENDERER",
        "97-doc-renderer.lisp",
        include_str!("../lib/97-doc-renderer.lisp"),
    ),
    (
        "HELP-SYSTEM",
        "98-help-system.lisp",
        include_str!("../lib/98-help-system.lisp"),
    ),
    (
        "HELP-DATA",
        "99-help-data.lisp",
        include_str!("../lib/99-help-data.lisp"),
    ),
];

/// (source, origin) for `name` (already uppercased) via the first two tiers
/// of REQUIRE's resolution order: sources the host registered directly on
/// `env`, then sources embedded in the binary ([`OPTIONAL_MODULES`]). The
/// third tier (disk, capability-gated) is implemented in Lisp
/// (`$require-resolve-disk` in `lib/06-require.lisp`) since it is ordinary
/// host-side-effecting work the existing `FILE-EXISTS-P`/`READ-FILE`
/// builtins already do correctly.
pub(crate) fn module_source_lookup(
    name: &str,
    env: &Shared<Environment>,
) -> Option<(String, &'static str)> {
    let upper = name.to_uppercase();
    if let Some(src) = env.registered_module_source(&upper) {
        return Some((src, "registered"));
    }
    OPTIONAL_MODULES
        .iter()
        .find(|(n, _, _)| *n == upper)
        .map(|(_, _, src)| (src.to_string(), "embedded"))
}

/// Parse and evaluate every top-level form in `src` into `env`.
///
/// Unlike [`load_file`], `src` is an in-memory string (a registered or
/// embedded module's source), not a path, so there is no include-cycle
/// bookkeeping here -- `require`'s own loading-stack (in
/// `lib/06-require.lisp`) already detects module cycles at a higher level.
/// `name` is used only to prefix error messages, mirroring `load_file`'s
/// `path:line:column: ...` shape.
///
/// Callers evaluating on behalf of `require` (see the `$EVAL-MODULE-SOURCE`
/// builtin) pass [`Environment::root_of`]`(env)` so that `defun`/`def`
/// inside a required module land as ordinary global bindings regardless of
/// the lexical frame `require` itself happens to run in.
pub fn eval_module_source(
    name: &str,
    src: &str,
    env: &Shared<Environment>,
) -> Result<(), LispError> {
    let stripped = reader::strip_shebang(src);
    let mut rest = stripped;
    loop {
        rest = reader::skip_ws(rest);
        let form_offset = stripped.len() - rest.len();
        match reader::read_next_with_depth_limit(rest, env, env.reader_depth_limit()) {
            Ok(None) => return Ok(()),
            Ok(Some((expr, rem))) => match evaluator::eval(&expr, env) {
                Ok(_) => rest = rem,
                Err(LispError::Generic(msg)) => {
                    let (line, col) = reader::position_of(stripped, form_offset);
                    return Err(LispError::Generic(format!("{name}:{line}:{col}: {msg}")));
                }
                Err(other) => return Err(other),
            },
            Err((offset, detail)) => {
                let anchor = reader::error_anchor(stripped, form_offset, offset, &detail);
                let (line, col) = reader::position_of(stripped, anchor);
                return Err(LispError::Generic(format!(
                    "{name}:{line}:{col}: parse error: {detail}"
                )));
            }
        }
    }
}

/// Evaluate the embedded standard library into `env`.
///
/// This is called by [`Environment::with_stdlib`]; call it directly if you want
/// to load the stdlib into an environment you already have.
///
/// Behavior-unchanged (issue #256): this evaluates exactly the same files in
/// exactly the same order as before REQUIRE/PROVIDE existed. The only
/// addition is bookkeeping, interleaved file-by-file: immediately after an
/// `OPTIONAL_MODULES` file's forms finish evaluating, it is marked
/// REQUIRE-loaded (bypassing resolution/eval/PROVIDE-checking -- see
/// `$require-mark-loaded!`). This must happen inline, not in one pass after
/// the whole loop, because some stdlib files themselves call `(require
/// 'name)` for an earlier optional file (30-text.lisp requires 'modules;
/// 99-help-data.lisp requires 'help-system) -- without interleaved marking
/// that `require` call would redundantly re-evaluate the dependency a
/// second time instead of seeing it already loaded.
pub fn load_stdlib(env: &Shared<Environment>) -> Result<(), LispError> {
    for (filename, src) in STDLIB_SOURCES {
        let exprs = reader::read_all(src, env)
            .map_err(|e| LispError::Generic(format!("stdlib parse error in {filename}: {e}")))?;
        for expr in exprs {
            evaluator::eval(&expr, env)
                .map_err(|e| LispError::Generic(format!("stdlib eval error in {filename}: {e}")))?;
        }
        if let Some((name, _, _)) = OPTIONAL_MODULES.iter().find(|(_, f, _)| f == filename) {
            let mark =
                format!("($require-mark-loaded! (quote {name}) \"embedded (stdlib {filename})\")");
            eval_str(&mark, env).map_err(|e| {
                LispError::Generic(format!(
                    "stdlib require-registry bookkeeping failed for {filename}: {e}"
                ))
            })?;
        }
    }
    Ok(())
}

/// Evaluate the Prelude (see `PRELUDE_SOURCES`) into `env`.
///
/// This is called by [`Environment::with_prelude`]; call it directly if you
/// want to load just the Prelude into an environment you already have.
pub fn load_prelude(env: &Shared<Environment>) -> Result<(), LispError> {
    for (filename, src) in PRELUDE_SOURCES {
        let exprs = reader::read_all(src, env)
            .map_err(|e| LispError::Generic(format!("prelude parse error in {filename}: {e}")))?;
        for expr in exprs {
            evaluator::eval(&expr, env).map_err(|e| {
                LispError::Generic(format!("prelude eval error in {filename}: {e}"))
            })?;
        }
    }
    Ok(())
}

/// Embedder API (issue #256): trigger `(require 'name)` from Rust, without
/// writing Lisp source. Equivalent to `env.require_module(name)` in the
/// epic's stated embedding surface; a free function taking `env` explicitly
/// to match this crate's existing convention ([`load_file`], [`load_stdlib`]).
pub fn require_module(name: &str, env: &Shared<Environment>) -> Result<LispVal, LispError> {
    eval_str(&format!("(require (quote {name}))"), env)
}

/// Embedder API (issue #256): the module names currently REQUIRE-loaded in
/// `env`, in no particular order. Equivalent to `env.loaded_modules()` in
/// the epic's stated embedding surface.
pub fn loaded_modules(env: &Shared<Environment>) -> Vec<String> {
    match eval_str("(loaded-modules)", env) {
        Ok(list) => list
            .as_list_vec()
            .unwrap_or_default()
            .into_iter()
            .filter_map(|v| match v {
                LispVal::Symbol(s) => Some(s.borrow().name.clone()),
                _ => None,
            })
            .collect(),
        Err(_) => Vec::new(),
    }
}

/// Evaluate a single s-expression string, returning the typed value.
///
/// The input must contain exactly one form; use [`eval_all`] for programs with
/// multiple top-level forms.
pub fn eval_str(src: &str, env: &Shared<Environment>) -> Result<LispVal, LispError> {
    // Reader errors are already self-describing ("parse error at line L,
    // column C: ..."), so no extra prefix is added here. The nesting-depth
    // limit comes from the environment (issue #270): the eval entry points
    // are documented to run under `with_large_stack`, where `with_stdlib`'s
    // generous limit is safe.
    let form = reader::read_with_depth_limit(src, env, env.reader_depth_limit())
        .map_err(LispError::Generic)?;
    evaluator::eval(&form, env)
}

/// Evaluate all s-expressions in `src` and return the results.
///
/// Expressions are evaluated in order; if any expression fails the error is
/// returned immediately and subsequent expressions are not evaluated.
pub fn eval_all(src: &str, env: &Shared<Environment>) -> Result<Vec<LispVal>, LispError> {
    let forms = reader::read_all_with_depth_limit(src, env, env.reader_depth_limit())
        .map_err(LispError::Generic)?;
    forms
        .iter()
        .map(|form| evaluator::eval(form, env))
        .collect()
}

/// Evaluate a single line for REPL display, returning a printable string.
///
/// This is a thin wrapper over [`eval_str`] for use in the REPL; host code
/// should prefer [`eval_str`] or [`eval_all`] to get typed results.
pub fn eval_line(line: &str, env: &Shared<Environment>) -> String {
    match eval_str(line, env) {
        Ok(result) => printer::print(&result),
        Err(e) => format_error_with_backtrace(&e, env),
    }
}

/// Format an evaluation error for a TOPLEVEL report: the error message plus
/// the backtrace of named frames the unwind left behind (innermost first),
/// consuming those frames. Errors with no named frames (a direct toplevel
/// mistake) format exactly as before.
pub fn format_error_with_backtrace(e: &LispError, env: &Shared<Environment>) -> String {
    let trace = evaluator::core::bt_capture(0, env);
    let msg = format!("{e}");
    if trace.is_empty() {
        msg
    } else {
        format!("{msg}\n  in: {}", trace.join(" \u{2190} "))
    }
}

fn proper_list_to_vec(list: &LispVal, context: &str) -> Result<Vec<LispVal>, LispError> {
    let mut out = Vec::new();
    let mut current = list;
    while let LispVal::Cons { car, cdr } = current {
        out.push(car.as_ref().clone());
        current = cdr;
    }
    if *current != LispVal::Nil {
        return Err(LispError::Generic(format!(
            "{context}: arguments must be a proper list, got tail {}",
            err_val_display(current)
        )));
    }
    Ok(out)
}

fn include_target(form: &LispVal) -> Result<Option<String>, LispError> {
    let LispVal::Cons { car, cdr } = form else {
        return Ok(None);
    };
    let LispVal::Symbol(sym) = car.as_ref() else {
        return Ok(None);
    };
    if sym.borrow().name != "INCLUDE" {
        return Ok(None);
    }

    let args = proper_list_to_vec(cdr, "include")?;
    if args.len() != 1 {
        return Err(LispError::Generic(
            "include requires exactly one string path".to_string(),
        ));
    }
    match &args[0] {
        LispVal::String(path) => Ok(Some(path.clone())),
        other => Err(LispError::Generic(format!(
            "INCLUDE: expected a string path, got {}",
            err_val_display(other)
        ))),
    }
}

fn resolve_include_path(parent_file: &Path, include: &str) -> PathBuf {
    let include_path = Path::new(include);
    if include_path.is_absolute() {
        return include_path.to_path_buf();
    }
    parent_file
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(include_path)
}

fn load_file_inner(
    path: &Path,
    env: &Shared<Environment>,
    include_stack: &mut Vec<PathBuf>,
) -> Result<(), LispError> {
    let cycle_key = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    if include_stack.iter().any(|p| p == &cycle_key) {
        let mut chain = include_stack
            .iter()
            .map(|p| p.display().to_string())
            .collect::<Vec<_>>();
        chain.push(cycle_key.display().to_string());
        return Err(LispError::Generic(format!(
            "include cycle detected: {}",
            chain.join(" -> ")
        )));
    }

    let display_path = path.display().to_string();
    let content = fs::read_to_string(path)
        .map_err(|e| LispError::Generic(format!("Failed to read file {display_path}: {e}")))?;

    // Parse and evaluate incrementally (issue #239): forms before an error
    // still take effect, and both parse and eval errors report the file,
    // line, and column of the offending form instead of discarding the whole
    // file with no position.
    include_stack.push(cycle_key);
    let result = (|| {
        let src = reader::strip_shebang(&content);
        let mut rest = src;
        loop {
            rest = reader::skip_ws(rest);
            let form_offset = src.len() - rest.len();
            match reader::read_next_with_depth_limit(rest, env, env.reader_depth_limit()) {
                Ok(None) => return Ok(()),
                Ok(Some((expr, rem))) => {
                    let step = if let Some(include) = include_target(&expr)? {
                        let include_path = resolve_include_path(path, &include);
                        load_file_inner(&include_path, env, include_stack)
                    } else {
                        evaluator::eval(&expr, env).map(|_| ())
                    };
                    match step {
                        Ok(()) => rest = rem,
                        Err(LispError::Generic(msg)) => {
                            let (line, col) = reader::position_of(src, form_offset);
                            return Err(LispError::Generic(format!(
                                "{display_path}:{line}:{col}: {msg}"
                            )));
                        }
                        Err(other) => return Err(other),
                    }
                }
                Err((offset, detail)) => {
                    let anchor = reader::error_anchor(src, form_offset, offset, &detail);
                    let (line, col) = reader::position_of(src, anchor);
                    return Err(LispError::Generic(format!(
                        "{display_path}:{line}:{col}: parse error: {detail}"
                    )));
                }
            }
        }
    })();
    include_stack.pop();
    result
}

/// Load and evaluate a single Lisp source file.
///
/// Reads the file at `path`, parses all top-level expressions, and evaluates
/// them in `env` in order.  The `READ-FS` capability must be enabled in `env`
/// for the Lisp builtin `(LOAD-FILE path)` to call this; Rust host code can
/// call it directly regardless of feature flags.
///
/// # Errors
///
/// Returns the first parse or evaluation error encountered, prefixed with
/// `path:line:column`.  Forms **before** the error have already been
/// evaluated and stay in effect; subsequent forms are not evaluated.
pub fn load_file(path: &str, env: &Shared<Environment>) -> Result<(), LispError> {
    load_file_inner(Path::new(path), env, &mut Vec::new())
}

/// Load and evaluate all `*.lisp` files in a directory, sorted by name.
///
/// Files are sorted by filename, so numeric prefixes (`00-core.lisp`,
/// `01-list.lisp`, …) control load order.  Subdirectories are ignored.
///
/// # Errors
///
/// Returns the first error from any file; subsequent files are not loaded.
pub fn load_directory(path: &str, env: &Shared<Environment>) -> Result<(), LispError> {
    let entries = std::fs::read_dir(path)
        .map_err(|e| LispError::Generic(format!("Failed to read directory {path}: {e}")))?;

    let mut files: Vec<_> = entries
        .filter_map(|entry| entry.ok())
        .filter(|entry| {
            let path = entry.path();
            path.is_file() && path.extension().is_some_and(|ext| ext == "lisp")
        })
        .collect();

    files.sort_by_key(|entry| entry.file_name());

    for entry in files {
        load_file(&entry.path().to_string_lossy(), env)?;
    }
    Ok(())
}

/// Run a minimal read-eval-print loop over `in_stream`/`out_stream`.
///
/// Reads one line at a time, evaluates it, and writes the result.  Used by
/// tests and simple embeddings that don't need rustyline's history and
/// completion.  For the interactive REPL, see the `lamedh-cli` crate.
pub fn repl_loop<R: BufRead, W: Write>(
    in_stream: &mut R,
    out_stream: &mut W,
) -> std::io::Result<()> {
    let env = Environment::new_with_builtins();
    loop {
        let mut line = String::new();
        let bytes_read = in_stream.read_line(&mut line)?;

        if bytes_read == 0 {
            // End of stream
            break;
        }

        let line = line.trim_end();
        if line.is_empty() {
            continue;
        }

        let output = eval_line(line, &env);
        writeln!(out_stream, "{output}")?;
        out_stream.flush()?;
    }
    Ok(())
}
