//! Variable binding, symbol interning, dynamic scoping, and capability management.
//!
//! An [`Environment`] is a frame in the interpreter's variable-binding chain.
//! Each frame holds a `HashMap<String, LispVal>` of local bindings and a
//! pointer to a lexical parent frame.  When a function is called, a new child
//! frame is created whose lexical parent is the closure's captured environment
//! and whose *dynamic* parent is the caller's frame (for special-variable
//! propagation).
//!
//! ## Symbol interning
//!
//! All symbol names are stored once in the global [`SymbolTable`].  Two
//! occurrences of the name `"FOO"` share the same `Rc<RefCell<Symbol>>`
//! allocation.  This makes `EQ` (pointer equality) an O(1) test.
//!
//! ## Scoping
//!
//! - **Lexical** (default): lookup walks `parent` pointers — the chain of
//!   frames from the closure's definition site to the global frame.
//! - **Dynamic** (opt-in): variables marked via [`Environment::mark_dynamic`]
//!   use *shallow binding* — the current value is always in the symbol's
//!   global value cell.  `DEFDYNAMIC`/`DEFVAR` or `*EARMUFF*` naming signal
//!   dynamic intent; `DynamicBinding` RAII guards save/restore on every
//!   binding site.  No dynamic-parent chain walk is needed or performed.
//!
//! ## Capabilities
//!
//! Dangerous operations (`SHELL`, `READ-FS`, `CREATE-FS`, `TEMP-FS`, `IO`,
//! `NET-DNS`, `NET-CONNECT`, `NET-LISTEN`, `OS-ENV`, `OS-ENV-WRITE`,
//! `OS-PROCESS`, `OS-SIGNAL`) are gated behind feature
//! flags that are all **off by default**.  Call [`Environment::enable_feature`]
//! to opt in.  Because `SharedState` is shared across the whole chain, a
//! feature enabled anywhere is visible everywhere.

use crate::{
    BuiltinFunc, LispError, LispVal, NetOperation, Shared, SharedCell, SpecialForm, Symbol,
};
use std::cell::Cell;
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::hash::{BuildHasherDefault, Hasher};
use std::path::PathBuf;

thread_local! {
    /// Per-thread prototype worlds behind [`Environment::with_stdlib`] /
    /// [`Environment::with_prelude`]: built once per thread by the real
    /// loader, then deep-copy-forked (never handed out, never mutated) so
    /// every subsequent call costs milliseconds instead of a full stdlib
    /// evaluation. Thread-local rather than process-global because the
    /// default build's `Shared`/`SharedCell` are `Rc`/`RefCell` — the object
    /// graph is not `Send`, so worlds cannot cross threads.
    static STDLIB_PROTOTYPE: std::cell::RefCell<Option<Shared<Environment>>> =
        const { std::cell::RefCell::new(None) };
    static PRELUDE_PROTOTYPE: std::cell::RefCell<Option<Shared<Environment>>> =
        const { std::cell::RefCell::new(None) };
}

/// RAII guard for a shallow dynamic (special) variable binding.
///
/// On construction the current value in the symbol's global value cell is saved
/// and the new value is installed. On drop — which fires on every exit path
/// including `?` early returns, `THROW`, `RETURN-FROM`, PROG `GO`/`RETURN`,
/// errors, and panics — the saved value is restored. This implements the
/// save/install/restore protocol required by shallow binding so that dynamic
/// scoping works correctly without walking the dynamic-parent chain.
pub struct DynamicBinding {
    symbol: Shared<SharedCell<Symbol>>,
    saved: Option<LispVal>,
}

impl DynamicBinding {
    /// Save the current value of `sym`'s global value cell and install `new_val`.
    pub fn install(sym: Shared<SharedCell<Symbol>>, new_val: LispVal) -> Self {
        let saved = sym.borrow().value.clone();
        sym.borrow_mut().value = Some(new_val);
        DynamicBinding { symbol: sym, saved }
    }

    pub fn symbol_id(&self) -> u32 {
        self.symbol.borrow().id
    }

    /// Extract the saved value without restoring it.  Used by the trampoline's
    /// guard-dedup logic to drop the intermediate LispVal before `mem::forget`
    /// so it is not leaked.
    pub fn take_saved(&mut self) -> Option<LispVal> {
        self.saved.take()
    }
}

impl Drop for DynamicBinding {
    fn drop(&mut self) {
        self.symbol.borrow_mut().value = self.saved.take();
    }
}

/// A small, fast, non-cryptographic hasher (FxHash-style) used for the
/// per-frame variable-binding map.
///
/// The default `HashMap` uses SipHash, which is DoS-resistant but slow for the
/// short symbol-name keys that dominate variable lookup — and lookup runs on
/// every variable reference in the interpreter's hot path. Binding keys are
/// internal interpreter data (symbol names), not attacker-controlled in a way
/// that makes hash-flooding a concern, so trading SipHash for a multiply-rotate
/// hash is a safe, large win on lookup-heavy code.
#[derive(Default)]
struct FxHasher {
    hash: u64,
}

const FX_SEED: u64 = 0x51_7c_c1_b7_27_22_0a_95;

impl FxHasher {
    #[inline]
    fn add(&mut self, word: u64) {
        self.hash = (self.hash.rotate_left(5) ^ word).wrapping_mul(FX_SEED);
    }
}

impl Hasher for FxHasher {
    #[inline]
    fn write(&mut self, mut bytes: &[u8]) {
        while bytes.len() >= 8 {
            let mut buf = [0u8; 8];
            buf.copy_from_slice(&bytes[..8]);
            self.add(u64::from_le_bytes(buf));
            bytes = &bytes[8..];
        }
        if !bytes.is_empty() {
            let mut buf = [0u8; 8];
            buf[..bytes.len()].copy_from_slice(bytes);
            self.add(u64::from_le_bytes(buf));
        }
    }

    #[inline]
    fn write_u32(&mut self, n: u32) {
        self.add(n as u64);
    }

    #[inline]
    fn write_u64(&mut self, n: u64) {
        self.add(n);
    }

    #[inline]
    fn finish(&self) -> u64 {
        self.hash
    }
}

/// `HashMap` specialised to the fast [`FxHasher`] for variable bindings.
/// Keys are symbol ids (`u32`) — integer hashing is faster than string hashing
/// and avoids the String clone that the old `HashMap<String, LispVal>` required
/// on every bind operation.
type BindingMap = HashMap<u32, LispVal, BuildHasherDefault<FxHasher>>;

/// Global symbol table shared by all environments in an interpreter session.
///
/// Maintains a `HashMap<String, Rc<RefCell<Symbol>>>` so that every distinct
/// symbol name maps to exactly one heap allocation.  Pointer equality of the
/// `Rc` handles is the Lisp `EQ` predicate for symbols.
///
/// Uninterned symbols (created by [`SymbolTable::gensym`]) are stored in
/// `by_id` so they can be used as binding keys in local frames, but they are
/// *not* stored in the `symbols` name-map — they are guaranteed unique but
/// not sharable by name.
#[derive(Debug, Clone, PartialEq)]
pub struct SymbolTable {
    symbols: HashMap<String, Shared<SharedCell<Symbol>>>,
    gensym_counter: u64,
    /// Monotonic counter for assigning symbol ids.
    next_id: u32,
    /// Reverse map: `by_id[id]` is the symbol with that id.  Includes both
    /// interned and gensym symbols.
    by_id: Vec<Shared<SharedCell<Symbol>>>,
}

impl Default for SymbolTable {
    fn default() -> Self {
        Self::new()
    }
}

impl SymbolTable {
    /// Create an empty symbol table.
    pub fn new() -> Self {
        SymbolTable {
            symbols: HashMap::new(),
            gensym_counter: 0,
            next_id: 0,
            by_id: Vec::new(),
        }
    }

    /// Create a fresh uninterned symbol with a unique name (`G0000`, `G0001`, …).
    ///
    /// Gensym symbols get a real id and are pushed to `by_id` so they can be
    /// used as binding keys in local frames. They are *not* stored in the
    /// `symbols` name-map, so two `gensym` calls always return distinct
    /// [`Shared`] pointers even if the names collide.
    pub fn gensym(&mut self) -> Shared<SharedCell<Symbol>> {
        let name = format!("G{:04}", self.gensym_counter);
        self.gensym_counter += 1;
        let id = self.next_id;
        self.next_id += 1;
        let symbol = Shared::new(SharedCell::new(Symbol {
            name,
            plist: HashMap::new(),
            value: None,
            id,
            is_keyword: false, // gensym names never start with ':'
            is_dynamic: false,
            special_form: None, // gensym names are never special forms
        }));
        self.by_id.push(symbol.clone());
        symbol
    }

    pub fn all_symbols(&self) -> Vec<Shared<SharedCell<Symbol>>> {
        self.symbols.values().cloned().collect()
    }

    /// Return the interned symbol for `name` if it already exists, without
    /// creating one. Used to read a symbol's global value cell by name on the
    /// (cold) name-based lookup paths.
    pub fn get_symbol(&self, name: &str) -> Option<Shared<SharedCell<Symbol>>> {
        self.symbols.get(name).cloned()
    }

    /// Look up a symbol by its numeric id (the reverse of [`Self::intern`]).
    /// Returns `None` if the id is out of range (should not happen in practice).
    pub fn symbol_by_id(&self, id: u32) -> Option<Shared<SharedCell<Symbol>>> {
        self.by_id.get(id as usize).cloned()
    }

    /// Return the interned [`Shared`] for `name`, creating a new `Symbol` if needed.
    ///
    /// The name must already be uppercased; callers are responsible for
    /// normalisation (the environment does this via [`Environment::intern_symbol`]).
    pub fn intern(&mut self, name: &str) -> Shared<SharedCell<Symbol>> {
        if let Some(symbol) = self.symbols.get(name) {
            return symbol.clone();
        }

        // Compute the special-form tag once at intern time.  Ordinary symbols
        // (the overwhelming majority) get `None` and skip the string match in
        // `eval_step` entirely.  The `Copy` tag is read with a borrow that
        // ends at the `let` statement, before any arm body executes.
        let special_form = match name {
            "QUOTE" => Some(SpecialForm::Quote),
            "QUASIQUOTE" => Some(SpecialForm::Quasiquote),
            "COND" => Some(SpecialForm::Cond),
            "IF" => Some(SpecialForm::If),
            "AND" => Some(SpecialForm::And),
            "OR" => Some(SpecialForm::Or),
            "UNWIND-PROTECT" => Some(SpecialForm::UnwindProtect),
            "CATCH" => Some(SpecialForm::Catch),
            "THROW" => Some(SpecialForm::Throw),
            "BLOCK" => Some(SpecialForm::Block),
            "RETURN-FROM" => Some(SpecialForm::ReturnFrom),
            "HANDLER-CASE" => Some(SpecialForm::HandlerCase),
            "DEF" => Some(SpecialForm::Def),
            "DEFDYNAMIC" | "DEFVAR" => Some(SpecialForm::Defdynamic),
            "LAMBDA" => Some(SpecialForm::Lambda),
            "FUNCTION" => Some(SpecialForm::Function),
            "LABEL" => Some(SpecialForm::Label),
            "DEFINE" => Some(SpecialForm::Define),
            "DEFEXPR" => Some(SpecialForm::Defexpr),
            "DEFMACRO" => Some(SpecialForm::Defmacro),
            "DEFUN-TYPED" => Some(SpecialForm::DefunTyped),
            "WITH-FUEL" => Some(SpecialForm::WithFuel),
            "WITH-CAPABILITIES" => Some(SpecialForm::WithCapabilities),
            "DEFUN*" => Some(SpecialForm::DefunStar),
            "JIT-OPTIMIZE" => Some(SpecialForm::JitOptimize),
            "CHECK-TYPE" => Some(SpecialForm::CheckType),
            "DEFSTRUCT-TYPED" => Some(SpecialForm::DefstructTyped),
            "DECLARE-TYPED" => Some(SpecialForm::DeclareTyped),
            "PROGN" => Some(SpecialForm::Progn),
            "SETQ" => Some(SpecialForm::Setq),
            "PROG" => Some(SpecialForm::Prog),
            "RETURN" => Some(SpecialForm::Return),
            "GO" => Some(SpecialForm::Go),
            "FOR" => Some(SpecialForm::For),
            "WHILE" => Some(SpecialForm::While),
            "LET" => Some(SpecialForm::Let),
            "LET*" => Some(SpecialForm::LetStar),
            "MACRO" => Some(SpecialForm::Macro),
            "FEXPR" => Some(SpecialForm::Fexpr),
            "VAU" | "$VAU" => Some(SpecialForm::Vau),
            _ => None,
        };
        let id = self.next_id;
        self.next_id += 1;
        let symbol = Shared::new(SharedCell::new(Symbol {
            is_keyword: name.starts_with(':'),
            is_dynamic: false,
            special_form,
            id,
            name: name.to_string(),
            plist: HashMap::new(),
            value: None,
        }));
        self.symbols.insert(name.to_string(), symbol.clone());
        self.by_id.push(symbol.clone());
        symbol
    }
}

/// Interpreter state that is shared globally across every environment in a
/// chain (and across lexical/dynamic links): the symbol table, condition
/// flags, the set of dynamic variable names, and enabled capabilities.
///
/// These were previously four independent `Rc<RefCell<...>>` fields on
/// `Environment`. Bundling them behind a single `Rc` means creating a child
/// frame clones **one** `Rc` handle instead of four — a per-call win that
/// matters because a frame is allocated on every user-defined function call
/// (see issue #111, Tier-1 item D1).
#[derive(Debug)]
struct SharedState {
    symbols: SharedCell<SymbolTable>,
    condition_flags: SharedCell<HashMap<String, bool, BuildHasherDefault<FxHasher>>>,
    /// Set of variable names that are marked as dynamic (special variables).
    /// Uses [`FxHasher`] (not the default SipHash): `resolve` probes this set on
    /// every variable reference once any dynamic var exists, so the cheaper hash
    /// is on the hot path.
    dynamic_vars: SharedCell<HashSet<String, BuildHasherDefault<FxHasher>>>,
    /// Fast-path flag: `true` once any variable has ever been marked dynamic.
    /// While this is `false`, variable resolution can skip the dynamic-set
    /// membership probe entirely (a `HashSet` borrow + string hash that every
    /// reference would otherwise pay). Most programs register zero dynamic
    /// variables, so this keeps the common case to a single `Cell` read.
    has_dynamic: Cell<bool>,
    /// Set of enabled capabilities/features (e.g. "SHELL"). Off by default; the
    /// host or a Lisp program must opt in. This is the foundation for
    /// sandboxing (see issue #64).
    features: SharedCell<HashSet<String>>,
    /// Registry of typed (`defun-typed`) functions. Shared across the whole
    /// environment chain so a typed definition made at the REPL is visible
    /// everywhere and its compiled edition persists across calls.
    jit: SharedCell<crate::jit::Jit>,
    /// Maximum s-expression nesting depth the evaluator-facing reader entry
    /// points (`load_file`, `eval_str`/`eval_all`, `read-from-string`) will
    /// parse. Defaults to the conservative [`crate::reader::DEFAULT_READER_DEPTH`]
    /// (512, sized for stacks >= ~4 MiB); [`Environment::with_stdlib`] raises
    /// it to 50,000 because those entry points are documented to run on the
    /// 512 MiB [`crate::with_large_stack`] thread. Tune with
    /// [`Environment::set_reader_depth_limit`] (issue #270).
    reader_depth_limit: Cell<usize>,
    /// Host-registered module sources for `require` (issue #256): module name
    /// (uppercased) -> Lisp source text, set only via
    /// [`Environment::register_module`]. Checked before the compiled-in
    /// embedded module table, per the resolution order documented in
    /// `lib/06-require.lisp`. Shared across the whole environment chain, like
    /// `features` — registering a module anywhere makes it requirable
    /// everywhere in this interpreter instance.
    module_sources: SharedCell<HashMap<String, String>>,
    /// Disk directories `require` searches (its third, capability-gated,
    /// resolution tier) for `<path>/<downcased-name>.lisp`. Empty by default.
    /// Deliberately Rust-only to mutate ([`Environment::add_module_search_path`]
    /// / [`Environment::clear_module_search_paths`]): Lisp can only *read* the
    /// list (`$MODULE-SEARCH-PATHS`), so a host constrains disk resolution
    /// without exposing that authority to sandboxed Lisp code.
    module_search_paths: SharedCell<Vec<PathBuf>>,
    /// Host policy hook for networking (issue #258, epic #253), consulted by
    /// `src/evaluator/builtins_net.rs` in addition to (not instead of) the
    /// `NET-DNS`/`NET-CONNECT`/`NET-LISTEN` capability checks. `None` (the
    /// default) allows every operation once its capability is granted.
    /// Deliberately Rust-only to install ([`Environment::set_net_policy`]):
    /// Lisp cannot install or inspect a policy, so a host scopes a granted
    /// capability (e.g. restricting an HTTP-client grant's destinations)
    /// without exposing that authority to sandboxed Lisp code -- same shape
    /// as `module_search_paths`. Shared across the whole environment chain,
    /// like `features`.
    net_policy: SharedCell<Option<NetPolicy>>,
    /// Host policy hook for OS integration (issue #260, epic #253), consulted
    /// by `src/evaluator/builtins_os.rs` in addition to (not instead of) the
    /// `OS-PROCESS`/`OS-SIGNAL` capability checks -- same shape and purpose as
    /// `net_policy`: an embedder can scope a granted capability (e.g.
    /// restrict spawn to a specific executable/argv/cwd, or restrict signals
    /// to a specific target PID range) without exposing that authority to
    /// sandboxed Lisp code. `None` (the default) allows every operation once
    /// its capability is granted.
    os_policy: SharedCell<Option<OsPolicy>>,
    /// Host embedder opt-in for `tls:connect-insecure!` (issue #365, epic
    /// #253), gated behind the `net-tls` cargo feature. Default-deny
    /// (`false`): even though `tls:connect-insecure!` is an explicitly named
    /// Lisp-facing API (never a silent flag on the ordinary `tls:connect`),
    /// it also refuses to run unless the *host* has separately called
    /// [`Environment::set_allow_insecure_tls`] -- Lisp code alone can never
    /// disable certificate verification, mirroring `net_policy`'s "Rust-only
    /// to install" shape one level further (a policy callback can only
    /// *narrow* an already-granted capability; this flag is a second,
    /// independent gate an embedder must explicitly widen).
    #[cfg(feature = "net-tls")]
    allow_insecure_tls: Cell<bool>,
}

/// Wrapper around the boxed policy callback so `SharedState` can keep
/// deriving `Debug` (closures don't implement it).
#[derive(Clone)]
struct NetPolicy(Shared<crate::NetPolicyFn>);

impl fmt::Debug for NetPolicy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "NetPolicy(<callback>)")
    }
}

/// Wrapper around the boxed OS policy callback, mirroring [`NetPolicy`].
#[derive(Clone)]
struct OsPolicy(Shared<crate::OsPolicyFn>);

impl fmt::Debug for OsPolicy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "OsPolicy(<callback>)")
    }
}

impl SharedState {
    fn new() -> Self {
        SharedState {
            symbols: SharedCell::new(SymbolTable::new()),
            condition_flags: SharedCell::new(HashMap::default()),
            dynamic_vars: SharedCell::new(HashSet::default()),
            has_dynamic: Cell::new(false),
            features: SharedCell::new(HashSet::new()),
            jit: SharedCell::new(crate::jit::Jit::new()),
            reader_depth_limit: Cell::new(crate::reader::DEFAULT_READER_DEPTH),
            module_sources: SharedCell::new(HashMap::new()),
            module_search_paths: SharedCell::new(Vec::new()),
            net_policy: SharedCell::new(None),
            os_policy: SharedCell::new(None),
            #[cfg(feature = "net-tls")]
            allow_insecure_tls: Cell::new(false),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Environment {
    parent: Option<Shared<Environment>>,
    // Per-frame local bindings. Held inline (not behind a `Shared`): each frame
    // owns its own map and never shares it with another `Environment`, so the
    // extra reference-count allocation the wrapper required was pure per-call
    // overhead on the hot path. Identity in `PartialEq` is recovered from the
    // field's address (unique per `Environment`).
    bindings: SharedCell<BindingMap>,
    /// Slot routing for compiled binder frames (issue #200 M3): when
    /// present, `routing[i]` is the symbol id that owns `slots[i]`. Every
    /// id-based access on this frame (`set_id`, `resolve`, the update
    /// walks) consults the table first, so a dynamic shadow-write (e.g. a
    /// macro expansion's `def` of a parameter name) hits the *same*
    /// storage a compiled `Code::LocalGet` reads — writes cannot bypass
    /// reads, which is what makes lexical addressing sound here where the
    /// reverted GlobalGet was not. `None` on ordinary map-only frames.
    routing: Option<Shared<Vec<u32>>>,
    /// Slot values for `routing` (parallel array). Empty when unrouted.
    slots: SharedCell<Vec<LispVal>>,
    /// Globally-shared interpreter state (symbols, flags, dynamic vars,
    /// features). Shared across the whole environment chain via a single
    /// [`Shared`] pointer.
    shared: Shared<SharedState>,
}

impl PartialEq for Environment {
    fn eq(&self, other: &Self) -> bool {
        let parents_equal = match (&self.parent, &other.parent) {
            (Some(p1), Some(p2)) => Shared::ptr_eq(p1, p2),
            (None, None) => true,
            _ => false,
        };
        parents_equal
            // `bindings` is now inline, so its field address uniquely identifies
            // this `Environment` (equivalent to the former `Rc` pointer compare).
            && std::ptr::eq(&self.bindings, &other.bindings)
            && Shared::ptr_eq(&self.shared, &other.shared)
    }
}

impl Default for Environment {
    fn default() -> Self {
        Self::new()
    }
}

impl Environment {
    /// Create a root environment with no parent and no builtins.
    ///
    /// Prefer [`Environment::new_with_builtins`] or [`Environment::with_stdlib`]
    /// for a usable interpreter environment.
    pub fn new() -> Self {
        Environment {
            parent: None,
            bindings: SharedCell::new(BindingMap::default()),
            routing: None,
            slots: SharedCell::new(Vec::new()),
            shared: Shared::new(SharedState::new()),
        }
    }

    /// Create a new child environment for lexical scoping.
    pub fn new_child(parent: &Shared<Environment>) -> Shared<Environment> {
        Shared::new(Environment {
            parent: Some(parent.clone()),
            bindings: SharedCell::new(BindingMap::default()),
            routing: None,
            slots: SharedCell::new(Vec::new()),
            shared: parent.shared.clone(),
        })
    }

    /// Create a child call frame with slot storage (issue #200 M3): the
    /// parameter values land directly in `slots`, owned per `routing`'s
    /// symbol ids. Later bindings of *other* ids on this frame go to the
    /// ordinary map; bindings of routed ids are redirected into their slot.
    pub fn new_child_with_slots(
        parent: &Shared<Environment>,
        routing: Shared<Vec<u32>>,
        slots: Vec<LispVal>,
    ) -> Shared<Environment> {
        debug_assert_eq!(routing.len(), slots.len());
        Shared::new(Environment {
            parent: Some(parent.clone()),
            bindings: SharedCell::new(BindingMap::default()),
            routing: Some(routing),
            slots: SharedCell::new(slots),
            shared: parent.shared.clone(),
        })
    }

    /// Create a new child environment for function application.
    ///
    /// The lexical parent is `lexical_parent` (the captured closure environment).
    /// Dynamic scoping uses shallow binding — the `DynamicBinding` RAII guard
    /// saves/restores the symbol's global value cell at each binding site —
    /// so no dynamic-parent pointer is stored or walked.
    pub fn new_child_with_dynamic(
        lexical_parent: &Shared<Environment>,
        _caller_env: &Shared<Environment>,
    ) -> Shared<Environment> {
        Environment::new_child(lexical_parent)
    }

    /// Create a root environment with all 100+ built-in primitives registered.
    ///
    /// This does **not** load the Lisp standard library (`defun`, `append`,
    /// etc.).  Use [`Environment::with_stdlib`] for a fully-featured environment.
    ///
    /// All capability flags (`SHELL`, `READ-FS`, `CREATE-FS`, `TEMP-FS`, `IO`,
    /// `NET-DNS`, `NET-CONNECT`, `NET-LISTEN`, `OS-ENV`, `OS-ENV-WRITE`,
    /// `OS-PROCESS`, `OS-SIGNAL`) are disabled by default.
    pub fn new_with_builtins() -> Shared<Environment> {
        let env = Shared::new(Environment::new());
        let t_symbol = env.intern_symbol("T");
        env.set("T".to_string(), LispVal::Symbol(t_symbol));

        // Lisp 1.5 spec functions
        env.set("PLUS".to_string(), LispVal::Builtin(BuiltinFunc::Plus));
        env.set(
            "DIFFERENCE".to_string(),
            LispVal::Builtin(BuiltinFunc::Minus),
        );
        env.set("TIMES".to_string(), LispVal::Builtin(BuiltinFunc::Multiply));
        env.set(
            "QUOTIENT".to_string(),
            LispVal::Builtin(BuiltinFunc::Divide),
        );

        // Common operator aliases
        env.set("+".to_string(), LispVal::Builtin(BuiltinFunc::Plus));
        env.set("-".to_string(), LispVal::Builtin(BuiltinFunc::Minus));
        env.set("*".to_string(), LispVal::Builtin(BuiltinFunc::Multiply));
        env.set("/".to_string(), LispVal::Builtin(BuiltinFunc::Divide));
        env.set("CAR".to_string(), LispVal::Builtin(BuiltinFunc::Car));
        env.set("CDR".to_string(), LispVal::Builtin(BuiltinFunc::Cdr));
        env.set("CONS".to_string(), LispVal::Builtin(BuiltinFunc::Cons));
        env.set("EQ".to_string(), LispVal::Builtin(BuiltinFunc::Eq));
        env.set("ATOM".to_string(), LispVal::Builtin(BuiltinFunc::Atom));
        env.set("PRINT".to_string(), LispVal::Builtin(BuiltinFunc::Print));

        // Extensions
        env.set("CONCAT".to_string(), LispVal::Builtin(BuiltinFunc::Concat));
        env.set("INDEX".to_string(), LispVal::Builtin(BuiltinFunc::Index));
        env.set("EVAL".to_string(), LispVal::Builtin(BuiltinFunc::Eval));
        env.set("EVLIS".to_string(), LispVal::Builtin(BuiltinFunc::Evlis));
        env.set("NOT".to_string(), LispVal::Builtin(BuiltinFunc::Not));
        env.set(
            "EQUAL-NUMBER".to_string(),
            LispVal::Builtin(BuiltinFunc::NumericEquals),
        );
        env.set(
            "=".to_string(),
            LispVal::Builtin(BuiltinFunc::NumericEquals),
        );
        env.set(
            "MAKE-HASH-TABLE".to_string(),
            LispVal::Builtin(BuiltinFunc::MakeHashTable),
        );
        // GETHASH for hash-table lookup; GET is the Lisp 1.5 plist lookup (= GETP)
        env.set("GETHASH".to_string(), LispVal::Builtin(BuiltinFunc::Get));
        env.set("GET".to_string(), LispVal::Builtin(BuiltinFunc::GetP));
        env.set(
            "HASH-TABLE-P".to_string(),
            LispVal::Builtin(BuiltinFunc::HashTablep),
        );
        env.set("SET-BANG".to_string(), LispVal::Builtin(BuiltinFunc::Set));
        env.set("SETHASH".to_string(), LispVal::Builtin(BuiltinFunc::Set));
        // REGULARITY (0.3 census): one name — REMHASH (collection first);
        // delete-key / delete-key-bang removed.
        env.set(
            "REMHASH".to_string(),
            LispVal::Builtin(BuiltinFunc::DeleteKey),
        );
        env.set(
            "CURRENT-ENVIRONMENT".to_string(),
            LispVal::Builtin(BuiltinFunc::CurrentEnvironment),
        );
        env.set("KEYS".to_string(), LispVal::Builtin(BuiltinFunc::Keys));
        env.set(
            "SEXPR-RENAME".to_string(),
            LispVal::Builtin(BuiltinFunc::SexprRename),
        );
        env.set("GETP".to_string(), LispVal::Builtin(BuiltinFunc::GetP));
        env.set("PUTP".to_string(), LispVal::Builtin(BuiltinFunc::PutP));
        env.set(
            "STRINGP".to_string(),
            LispVal::Builtin(BuiltinFunc::Stringp),
        );
        env.set("APPLY".to_string(), LispVal::Builtin(BuiltinFunc::Apply));
        env.set(
            "LOAD-FILE".to_string(),
            LispVal::Builtin(BuiltinFunc::LoadFile),
        );
        // REQUIRE/PROVIDE module registry kernel support (issue #256): the
        // Lisp layer (`lib/06-require.lisp`) owns resolution order, load-once
        // bookkeeping, cycle detection, and PROVIDE-checking; these three
        // builtins supply only what needs representation access —
        // host-registered/embedded source lookup, the read-only search-path
        // accessor, and evaluating an in-memory source string at the
        // environment root.
        env.set(
            "$MODULE-SOURCE-LOOKUP".to_string(),
            LispVal::Builtin(BuiltinFunc::ModuleSourceLookup),
        );
        env.set(
            "$MODULE-SEARCH-PATHS".to_string(),
            LispVal::Builtin(BuiltinFunc::ModuleSearchPaths),
        );
        env.set(
            "$EVAL-MODULE-SOURCE".to_string(),
            LispVal::Builtin(BuiltinFunc::EvalModuleSource),
        );

        env.set(
            "NUMBERP".to_string(),
            LispVal::Builtin(BuiltinFunc::Numberp),
        );
        // Arithmetic Primitives
        env.set("<".to_string(), LispVal::Builtin(BuiltinFunc::Lessp));
        env.set(">".to_string(), LispVal::Builtin(BuiltinFunc::Greaterp));
        env.set("LESSP".to_string(), LispVal::Builtin(BuiltinFunc::Lessp));
        env.set(
            "GREATERP".to_string(),
            LispVal::Builtin(BuiltinFunc::Greaterp),
        );
        env.set("ZEROP".to_string(), LispVal::Builtin(BuiltinFunc::Zerop));
        env.set(
            "REMAINDER".to_string(),
            LispVal::Builtin(BuiltinFunc::Remainder),
        );
        env.set("EXPT".to_string(), LispVal::Builtin(BuiltinFunc::Expt));

        // I/O functions
        env.set("READ".to_string(), LispVal::Builtin(BuiltinFunc::Read));
        env.set("PRIN1".to_string(), LispVal::Builtin(BuiltinFunc::Prin1));
        env.set("PRINC".to_string(), LispVal::Builtin(BuiltinFunc::Princ));
        env.set("TERPRI".to_string(), LispVal::Builtin(BuiltinFunc::Terpri));

        // Process control
        env.set("EXIT".to_string(), LispVal::Builtin(BuiltinFunc::Exit));

        // Error handling
        env.set("ERROR".to_string(), LispVal::Builtin(BuiltinFunc::Error));
        env.set(
            "ERRORSET".to_string(),
            LispVal::Builtin(BuiltinFunc::Errorset),
        );

        // List processing
        env.set("SUBST".to_string(), LispVal::Builtin(BuiltinFunc::Subst));
        env.set("SUBLIS".to_string(), LispVal::Builtin(BuiltinFunc::Sublis));
        env.set("ASSOC".to_string(), LispVal::Builtin(BuiltinFunc::Assoc));
        env.set(
            "MAPLIST".to_string(),
            LispVal::Builtin(BuiltinFunc::Maplist),
        );
        env.set("MAPCAR".to_string(), LispVal::Builtin(BuiltinFunc::Mapcar));
        env.set("RPLACA".to_string(), LispVal::Builtin(BuiltinFunc::Rplaca));
        env.set("RPLACD".to_string(), LispVal::Builtin(BuiltinFunc::Rplacd));

        // Bitwise operations
        // REGULARITY (0.3): the name every Lisp reader expects is CL's LOGIOR.
        env.set("LOGIOR".to_string(), LispVal::Builtin(BuiltinFunc::Logor));
        env.set("LOGAND".to_string(), LispVal::Builtin(BuiltinFunc::Logand));
        env.set("LOGXOR".to_string(), LispVal::Builtin(BuiltinFunc::Logxor));
        env.set(
            "LEFTSHIFT".to_string(),
            LispVal::Builtin(BuiltinFunc::Leftshift),
        );

        // Property list functions
        env.set(
            "REMPROP".to_string(),
            LispVal::Builtin(BuiltinFunc::Remprop),
        );
        env.set(
            "DEFLIST".to_string(),
            LispVal::Builtin(BuiltinFunc::Deflist),
        );

        // Type predicates
        env.set("FIXP".to_string(), LispVal::Builtin(BuiltinFunc::Fixp));
        env.set("FLOATP".to_string(), LispVal::Builtin(BuiltinFunc::Floatp));
        env.set("CHARP".to_string(), LispVal::Builtin(BuiltinFunc::Charp));
        env.set(
            "SYMBOLP".to_string(),
            LispVal::Builtin(BuiltinFunc::Symbolp),
        );
        env.set("BOUNDP".to_string(), LispVal::Builtin(BuiltinFunc::Boundp));
        env.set(
            "FUNCTIONP".to_string(),
            LispVal::Builtin(BuiltinFunc::Functionp),
        );
        env.set("MACROP".to_string(), LispVal::Builtin(BuiltinFunc::Macrop));

        // List functions
        env.set("LIST".to_string(), LispVal::Builtin(BuiltinFunc::List));
        env.set("LAST".to_string(), LispVal::Builtin(BuiltinFunc::Last));
        env.set("NTH".to_string(), LispVal::Builtin(BuiltinFunc::Nth));
        env.set("NTHCDR".to_string(), LispVal::Builtin(BuiltinFunc::Nthcdr));
        env.set("EFFACE".to_string(), LispVal::Builtin(BuiltinFunc::Efface));
        env.set("DELETE".to_string(), LispVal::Builtin(BuiltinFunc::Efface)); // Alias

        // Numeric functions
        env.set("MOD".to_string(), LispVal::Builtin(BuiltinFunc::Mod));
        env.set("PLUSP".to_string(), LispVal::Builtin(BuiltinFunc::Plusp));
        env.set("EVENP".to_string(), LispVal::Builtin(BuiltinFunc::Evenp));
        env.set("ODDP".to_string(), LispVal::Builtin(BuiltinFunc::Oddp));
        env.set("ADD1".to_string(), LispVal::Builtin(BuiltinFunc::Add1));
        env.set("SUB1".to_string(), LispVal::Builtin(BuiltinFunc::Sub1));
        env.set("1+".to_string(), LispVal::Builtin(BuiltinFunc::Add1)); // Alias
        env.set("1-".to_string(), LispVal::Builtin(BuiltinFunc::Sub1)); // Alias
        env.set("RANDOM".to_string(), LispVal::Builtin(BuiltinFunc::Random));
        env.set(
            "RANDOM-SEED!".to_string(),
            LispVal::Builtin(BuiltinFunc::RandomSeed),
        );

        // Bitwise operations
        env.set("ASH".to_string(), LispVal::Builtin(BuiltinFunc::Ash));
        env.set("LOGNOT".to_string(), LispVal::Builtin(BuiltinFunc::Lognot));
        env.set("ROT".to_string(), LispVal::Builtin(BuiltinFunc::Rot));

        // Function operations
        env.set(
            "FUNCALL".to_string(),
            LispVal::Builtin(BuiltinFunc::Funcall),
        );
        env.set(
            "MACROEXPAND".to_string(),
            LispVal::Builtin(BuiltinFunc::Macroexpand),
        );

        // String/Symbol functions
        env.set(
            "EXPLODE".to_string(),
            LispVal::Builtin(BuiltinFunc::Explode),
        );
        env.set(
            "IMPLODE".to_string(),
            LispVal::Builtin(BuiltinFunc::Implode),
        );
        env.set("MAKNAM".to_string(), LispVal::Builtin(BuiltinFunc::Maknam));
        env.set("GENSYM".to_string(), LispVal::Builtin(BuiltinFunc::Gensym));
        env.set("INTERN".to_string(), LispVal::Builtin(BuiltinFunc::Intern));
        env.set("PLIST".to_string(), LispVal::Builtin(BuiltinFunc::Plist));

        // PUT as alias for PUTP (classic Lisp 1.5 name)
        env.set("PUT".to_string(), LispVal::Builtin(BuiltinFunc::PutP));

        // Float comparisons
        env.set(
            "FLOAT-EQUAL".to_string(),
            LispVal::Builtin(BuiltinFunc::FloatEqual),
        );
        env.set(
            "FLOAT-LESSP".to_string(),
            LispVal::Builtin(BuiltinFunc::FloatLessp),
        );
        env.set(
            "FLOAT-GREATERP".to_string(),
            LispVal::Builtin(BuiltinFunc::FloatGreaterp),
        );

        // Condition flags
        env.set(
            "SET-FLAG".to_string(),
            LispVal::Builtin(BuiltinFunc::SetFlag),
        );
        env.set(
            "CLEAR-FLAG".to_string(),
            LispVal::Builtin(BuiltinFunc::ClearFlag),
        );
        env.set(
            "FLAG-SET-P".to_string(),
            LispVal::Builtin(BuiltinFunc::FlagSetP),
        );
        env.set(
            "CLEAR-ALL-FLAGS".to_string(),
            LispVal::Builtin(BuiltinFunc::ClearAllFlags),
        );
        env.set(
            "RECORD-REF".to_string(),
            LispVal::Builtin(BuiltinFunc::RecordRef),
        );
        env.set(
            "RECORD-DECLARE".to_string(),
            LispVal::Builtin(BuiltinFunc::RecordDeclare),
        );
        env.set(
            "RECORD-NEW".to_string(),
            LispVal::Builtin(BuiltinFunc::RecordNew),
        );
        env.set(
            "RECORD-BRAND".to_string(),
            LispVal::Builtin(BuiltinFunc::RecordBrand),
        );
        env.set(
            "RECORD-COMPILED-P".to_string(),
            LispVal::Builtin(BuiltinFunc::RecordCompiledP),
        );
        env.set(
            "RECORD-FIELDS".to_string(),
            LispVal::Builtin(BuiltinFunc::RecordFields),
        );
        env.set(
            "VARIANT-DECLARE".to_string(),
            LispVal::Builtin(BuiltinFunc::VariantDeclare),
        );
        env.set(
            "LAST-BACKTRACE".to_string(),
            LispVal::Builtin(BuiltinFunc::LastBacktrace),
        );
        env.set(
            "MONOTONIC-MICROS".to_string(),
            LispVal::Builtin(BuiltinFunc::MonotonicMicros),
        );
        env.set(
            "EXPLAIN-COMPILE".to_string(),
            LispVal::Builtin(BuiltinFunc::ExplainCompile),
        );
        env.set("SET".to_string(), LispVal::Builtin(BuiltinFunc::SetValue));
        env.set(
            "CAPABILITY-MASK-ALLOWS-P".to_string(),
            LispVal::Builtin(BuiltinFunc::CapMaskAllowsP),
        );
        env.set("APPEND".to_string(), LispVal::Builtin(BuiltinFunc::Append));
        env.set(
            "DECLARE-INSTANCE!".to_string(),
            LispVal::Builtin(BuiltinFunc::DeclareInstance),
        );
        env.set(
            "DECLARE-PROTOCOL-DISPATCH!".to_string(),
            LispVal::Builtin(BuiltinFunc::DeclareProtocolDispatch),
        );
        env.set(
            "RECORD-WITH".to_string(),
            LispVal::Builtin(BuiltinFunc::RecordWith),
        );
        env.set(
            "SEE-TYPE".to_string(),
            LispVal::Builtin(BuiltinFunc::SeeType),
        );
        env.set(
            "READ-STRING".to_string(),
            LispVal::Builtin(BuiltinFunc::ReadString),
        );
        env.set(
            "DECLARE-TYPE!".to_string(),
            LispVal::Builtin(BuiltinFunc::DeclareType),
        );
        env.set(
            "SIGNATURE".to_string(),
            LispVal::Builtin(BuiltinFunc::Signature),
        );
        env.set(
            "COMPILED-P".to_string(),
            LispVal::Builtin(BuiltinFunc::CompiledP),
        );
        env.set(
            "WHY-NOT-TYPED".to_string(),
            LispVal::Builtin(BuiltinFunc::WhyNotTyped),
        );
        // Kernel fuel (issue #284 Phase 2). WITH-FUEL fences shadow the
        // setter inside guarded code; hosts and top-level scripts may use it
        // directly.
        env.set(
            "KERNEL-FUEL-SET!".to_string(),
            LispVal::Builtin(BuiltinFunc::KernelFuelSet),
        );
        env.set(
            "KERNEL-FUEL-REMAINING".to_string(),
            LispVal::Builtin(BuiltinFunc::KernelFuelRemaining),
        );
        // Positioned reading (issue #171 phase 2a). Pure parsing — takes a
        // string, no capability; file access stays behind READ-FS.
        env.set(
            "READ-ALL-POSITIONED".to_string(),
            LispVal::Builtin(BuiltinFunc::ReadAllPositioned),
        );

        // Capabilities / features — read-only from Lisp.
        // Grant/revoke capabilities only from the host API (env.enable_feature)
        // or the CLI (--capability).  Lisp code may introspect but not self-escalate.
        env.set(
            "FEATURE-ENABLED-P".to_string(),
            LispVal::Builtin(BuiltinFunc::FeatureEnabledP),
        );
        env.set(
            "FEATURES".to_string(),
            LispVal::Builtin(BuiltinFunc::Features),
        );

        // SHELL: gated behind the SHELL capability (off by default)
        env.set("SHELL".to_string(), LispVal::Builtin(BuiltinFunc::Shell));

        // First-class environments
        env.set(
            "THE-ENVIRONMENT".to_string(),
            LispVal::Builtin(BuiltinFunc::TheEnvironment),
        );
        env.set(
            "MAKE-ENVIRONMENT".to_string(),
            LispVal::Builtin(BuiltinFunc::MakeEnvironment),
        );
        // Source optimizer
        env.set(
            "OPTIMIZE".to_string(),
            LispVal::Builtin(BuiltinFunc::Optimize),
        );

        // Arrays (Lisp 1.5)
        env.set(
            "ARRAY".to_string(),
            LispVal::Builtin(BuiltinFunc::MakeArray),
        );
        env.set(
            "FETCH".to_string(),
            LispVal::Builtin(BuiltinFunc::ArrayFetch),
        );
        env.set(
            "STORE".to_string(),
            LispVal::Builtin(BuiltinFunc::ArrayStore),
        );
        env.set(
            "ARRAY-LENGTH*".to_string(),
            LispVal::Builtin(BuiltinFunc::ArrayLength),
        );
        env.set("$LENGTH".to_string(), LispVal::Builtin(BuiltinFunc::Length));
        env.set(
            "$LIST->ARRAY".to_string(),
            LispVal::Builtin(BuiltinFunc::ListToArray),
        );
        env.set(
            "$ARRAY->LIST".to_string(),
            LispVal::Builtin(BuiltinFunc::ArrayToList),
        );
        env.set("ARRAYP".to_string(), LispVal::Builtin(BuiltinFunc::Arrayp));
        // CHARP is registered with the type predicates; MAKE-CHAR with the
        // char/string ops (near CODE-CHAR).
        env.set(
            "EXTENSION-P".to_string(),
            LispVal::Builtin(BuiltinFunc::Extensionp),
        );
        env.set(
            "EXTENSION-TYPE".to_string(),
            LispVal::Builtin(BuiltinFunc::ExtensionTypeName),
        );

        // EVCON: evaluate clauses (Lisp 1.5 Appendix A)
        env.set("EVCON".to_string(), LispVal::Builtin(BuiltinFunc::Evcon));
        // SPACES: print N spaces (Lisp 1.5 I/O)
        env.set("SPACES".to_string(), LispVal::Builtin(BuiltinFunc::Spaces));

        // File I/O (gated behind READ-FS capability)
        env.set(
            "READ-FILE".to_string(),
            LispVal::Builtin(BuiltinFunc::ReadFile),
        );
        env.set(
            "READ-FILE-BYTE".to_string(),
            LispVal::Builtin(BuiltinFunc::ReadFileByte),
        );
        env.set(
            "READ-FILE-SECTION".to_string(),
            LispVal::Builtin(BuiltinFunc::ReadFileSection),
        );
        env.set(
            "READ-FILE-SECTION-LOSSY".to_string(),
            LispVal::Builtin(BuiltinFunc::ReadFileSectionLossy),
        );
        env.set(
            "READ-FILE-SECTION-BYTES".to_string(),
            LispVal::Builtin(BuiltinFunc::ReadFileSectionBytes),
        );
        env.set(
            "WRITE-FILE".to_string(),
            LispVal::Builtin(BuiltinFunc::WriteFile),
        );

        // File metadata predicates (gated behind READ-FS capability)
        env.set(
            "FILE-EXISTS-P".to_string(),
            LispVal::Builtin(BuiltinFunc::FileExistsP),
        );
        env.set(
            "DIRECTORY-P".to_string(),
            LispVal::Builtin(BuiltinFunc::DirectoryP),
        );
        env.set("FILE-P".to_string(), LispVal::Builtin(BuiltinFunc::FileP));
        env.set(
            "FILE-READABLE-P".to_string(),
            LispVal::Builtin(BuiltinFunc::FileReadableP),
        );
        env.set(
            "FILE-WRITABLE-P".to_string(),
            LispVal::Builtin(BuiltinFunc::FileWritableP),
        );
        env.set(
            "FILE-EXECUTABLE-P".to_string(),
            LispVal::Builtin(BuiltinFunc::FileExecutableP),
        );
        env.set(
            "FILE-SIZE".to_string(),
            LispVal::Builtin(BuiltinFunc::FileSize),
        );
        env.set(
            "DIRECTORY-FILES".to_string(),
            LispVal::Builtin(BuiltinFunc::DirectoryFiles),
        );
        env.set(
            "FILE-NEWER-P".to_string(),
            LispVal::Builtin(BuiltinFunc::FileNewerP),
        );

        // File mutation (gated behind CREATE-FS capability)
        env.set("CHMOD".to_string(), LispVal::Builtin(BuiltinFunc::Chmod));
        env.set(
            "CREATE-DIRECTORY".to_string(),
            LispVal::Builtin(BuiltinFunc::CreateDirectory),
        );
        env.set(
            "DELETE-FILE".to_string(),
            LispVal::Builtin(BuiltinFunc::DeleteFile),
        );
        env.set(
            "RENAME-FILE".to_string(),
            LispVal::Builtin(BuiltinFunc::RenameFile),
        );

        // Temp filesystem (gated behind TEMP-FS capability)
        env.set(
            "MAKE-TEMP-FILE".to_string(),
            LispVal::Builtin(BuiltinFunc::MakeTempFile),
        );
        env.set(
            "MAKE-TEMP-DIRECTORY".to_string(),
            LispVal::Builtin(BuiltinFunc::MakeTempDirectory),
        );

        // Note: PLUS/DIFFERENCE/TIMES/QUOTIENT/LESSP/GREATERP/REMAINDER
        // are registered above with the other Lisp 1.5 spec functions.

        // Arrays (Lisp 1.5 Appendix A). The primitives existed and were
        // dispatched but were never bound to names; the manual documents
        // ARRAY/FETCH/STORE/ARRAY-LENGTH*, with MAKE-ARRAY/ARRAY-FETCH*/ARRAY-STORE*
        // as longer aliases.
        env.set(
            "ARRAY".to_string(),
            LispVal::Builtin(BuiltinFunc::MakeArray),
        );
        env.set(
            "MAKE-ARRAY".to_string(),
            LispVal::Builtin(BuiltinFunc::MakeArray),
        );
        env.set(
            "FETCH".to_string(),
            LispVal::Builtin(BuiltinFunc::ArrayFetch),
        );
        env.set(
            "ARRAY-FETCH*".to_string(),
            LispVal::Builtin(BuiltinFunc::ArrayFetch),
        );
        // Common-Lisp-style alias (issue #214): the typed JIT's elaborator
        // already treats AREF/ASET as synonyms for FETCH/STORE
        // (`src/jit/elaboration.rs`'s dispatch), so a `defun-typed` using
        // them compiled and ran fine while the identical body, run
        // interpreted (e.g. after `defun*`'s documented silent fallback to
        // a plain lambda), errored "Unbound variable" -- the elaborator
        // recognized a name the evaluator had no definition for at all.
        env.set(
            "AREF".to_string(),
            LispVal::Builtin(BuiltinFunc::ArrayFetch),
        );
        env.set(
            "STORE".to_string(),
            LispVal::Builtin(BuiltinFunc::ArrayStore),
        );
        env.set(
            "ARRAY-STORE*".to_string(),
            LispVal::Builtin(BuiltinFunc::ArrayStore),
        );
        env.set(
            "ASET".to_string(),
            LispVal::Builtin(BuiltinFunc::ArrayStore),
        );
        env.set(
            "ARRAY-LENGTH*".to_string(),
            LispVal::Builtin(BuiltinFunc::ArrayLength),
        );
        // Elementwise SIMD array ops: the typed JIT compiles these to a
        // vectorized native loop (`Core::ArrayMap2`); this registration is
        // the tree-walker's own (scalar, wrapping) reference implementation,
        // reached when the call is interpreted rather than compiled.
        env.set(
            "ARRAY-ADD!".to_string(),
            LispVal::Builtin(BuiltinFunc::ArrayAddBang),
        );
        env.set(
            "ARRAY-SUB!".to_string(),
            LispVal::Builtin(BuiltinFunc::ArraySubBang),
        );
        env.set(
            "ARRAY-MUL!".to_string(),
            LispVal::Builtin(BuiltinFunc::ArrayMulBang),
        );
        // SIMD integer array reductions: the typed JIT compiles these to a
        // vectorized native loop (`Core::ArraySum`/`Core::ArrayDot`); this
        // registration is the tree-walker's own (scalar, wrapping) reference
        // implementation, reached when the call is interpreted rather than
        // compiled. Wrapping int64 addition is associative, so the SIMD
        // reduction and this sequential fold agree bit-for-bit.
        env.set(
            "ARRAY-SUM".to_string(),
            LispVal::Builtin(BuiltinFunc::ArraySum),
        );
        env.set(
            "ARRAY-DOT".to_string(),
            LispVal::Builtin(BuiltinFunc::ArrayDot),
        );
        env.set("$LENGTH".to_string(), LispVal::Builtin(BuiltinFunc::Length));
        env.set(
            "$LIST->ARRAY".to_string(),
            LispVal::Builtin(BuiltinFunc::ListToArray),
        );
        env.set(
            "$ARRAY->LIST".to_string(),
            LispVal::Builtin(BuiltinFunc::ArrayToList),
        );

        // Flat typed arrays (issue: JIT zero-copy array membrane): a
        // `(typed-array n elem-type)` array stores elements as raw `u64`
        // words from creation, in the same layout as the JIT's call-arena
        // buffers, so it can cross into a typed function without a copy
        // when the element types agree. `aref`/`aset`/`array-length*`/
        // `length`/`arrayp` all accept it alongside a plain `array`.
        env.set(
            "TYPED-ARRAY".to_string(),
            LispVal::Builtin(BuiltinFunc::MakeTypedArray),
        );
        env.set(
            "TYPED-ARRAY-P".to_string(),
            LispVal::Builtin(BuiltinFunc::TypedArrayp),
        );

        // Sorting (stable, non-destructive; takes a comparator predicate)
        env.set("SORT".to_string(), LispVal::Builtin(BuiltinFunc::Sort));

        // Math library (implemented in Rust: f64 transcendentals, float->int
        // rounding, i64 integer math)
        env.set("SQRT".to_string(), LispVal::Builtin(BuiltinFunc::Sqrt));
        env.set("FLOAT".to_string(), LispVal::Builtin(BuiltinFunc::Float));
        env.set("SIN".to_string(), LispVal::Builtin(BuiltinFunc::Sin));
        env.set("COS".to_string(), LispVal::Builtin(BuiltinFunc::Cos));
        env.set("TAN".to_string(), LispVal::Builtin(BuiltinFunc::Tan));
        env.set("LOG".to_string(), LispVal::Builtin(BuiltinFunc::Log));
        env.set("EXP".to_string(), LispVal::Builtin(BuiltinFunc::Exp));
        env.set("FLOOR".to_string(), LispVal::Builtin(BuiltinFunc::Floor));
        env.set(
            "CEILING".to_string(),
            LispVal::Builtin(BuiltinFunc::Ceiling),
        );
        env.set("ROUND".to_string(), LispVal::Builtin(BuiltinFunc::Round));
        env.set(
            "TRUNCATE".to_string(),
            LispVal::Builtin(BuiltinFunc::Truncate),
        );
        env.set("GCD".to_string(), LispVal::Builtin(BuiltinFunc::Gcd));
        env.set("LCM".to_string(), LispVal::Builtin(BuiltinFunc::Lcm));
        env.set("ISQRT".to_string(), LispVal::Builtin(BuiltinFunc::Isqrt));
        env.set("SIGNUM".to_string(), LispVal::Builtin(BuiltinFunc::Signum));

        // String operations (kernel primitives; the convenience layer is in Lisp)
        env.set(
            "STRING-LENGTH*".to_string(),
            LispVal::Builtin(BuiltinFunc::StringLength),
        );
        env.set(
            "SUBSTRING".to_string(),
            LispVal::Builtin(BuiltinFunc::Substring),
        );
        env.set(
            "CHAR-CODE".to_string(),
            LispVal::Builtin(BuiltinFunc::CharCode),
        );
        env.set(
            "CODE-CHAR".to_string(),
            LispVal::Builtin(BuiltinFunc::CodeChar),
        );
        env.set(
            "MAKE-CHAR".to_string(),
            LispVal::Builtin(BuiltinFunc::MakeChar),
        );
        env.set(
            "STRING->NUMBER".to_string(),
            LispVal::Builtin(BuiltinFunc::StringToNumber),
        );
        env.set(
            "NUMBER->STRING".to_string(),
            LispVal::Builtin(BuiltinFunc::NumberToString),
        );
        env.set(
            "STRING-CASEFOLD*".to_string(),
            LispVal::Builtin(BuiltinFunc::StringCasefold),
        );
        env.set(
            "STRING->UTF8*".to_string(),
            LispVal::Builtin(BuiltinFunc::StringToUtf8),
        );
        env.set(
            "UTF8->STRING*".to_string(),
            LispVal::Builtin(BuiltinFunc::Utf8ToString),
        );
        env.set(
            "UTF8->STRING-LOSSY*".to_string(),
            LispVal::Builtin(BuiltinFunc::Utf8ToStringLossy),
        );
        env.set(
            "READ-FROM-STRING".to_string(),
            LispVal::Builtin(BuiltinFunc::ReadFromString),
        );
        env.set(
            "PRIN1-TO-STRING".to_string(),
            LispVal::Builtin(BuiltinFunc::Prin1ToString),
        );
        env.set(
            "PRINC-TO-STRING".to_string(),
            LispVal::Builtin(BuiltinFunc::PrincToString),
        );

        // First-class error/condition values
        env.set(
            "MAKE-ERROR".to_string(),
            LispVal::Builtin(BuiltinFunc::MakeError),
        );
        env.set("ERROR-P".to_string(), LispVal::Builtin(BuiltinFunc::ErrorP));
        env.set(
            "ERROR-MESSAGE".to_string(),
            LispVal::Builtin(BuiltinFunc::ErrorMessage),
        );
        env.set(
            "ERROR-DATA".to_string(),
            LispVal::Builtin(BuiltinFunc::ErrorData),
        );

        // Binary ports (issue #255, epic #253): kernel substrate wrapped by
        // the PORTS module in lib/31-ports.lisp. Flat "*"-suffixed names,
        // like the STRING->UTF8*/UTF8->STRING* substrate for TEXT.
        env.set(
            "PORT-OPEN-INPUT-FILE*".to_string(),
            LispVal::Builtin(BuiltinFunc::PortOpenInputFile),
        );
        env.set(
            "PORT-OPEN-OUTPUT-FILE*".to_string(),
            LispVal::Builtin(BuiltinFunc::PortOpenOutputFile),
        );
        env.set(
            "PORT-OPEN-APPEND-FILE*".to_string(),
            LispVal::Builtin(BuiltinFunc::PortOpenAppendFile),
        );
        env.set(
            "PORT-OPEN-INPUT-BYTES*".to_string(),
            LispVal::Builtin(BuiltinFunc::PortOpenInputBytes),
        );
        env.set(
            "PORT-OPEN-OUTPUT-BYTES*".to_string(),
            LispVal::Builtin(BuiltinFunc::PortOpenOutputBytes),
        );
        env.set(
            "PORT-OUTPUT-CONTENTS*".to_string(),
            LispVal::Builtin(BuiltinFunc::PortOutputContents),
        );
        env.set(
            "PORT-STDIN*".to_string(),
            LispVal::Builtin(BuiltinFunc::PortStdin),
        );
        env.set(
            "PORT-STDOUT*".to_string(),
            LispVal::Builtin(BuiltinFunc::PortStdout),
        );
        env.set(
            "PORT-STDERR*".to_string(),
            LispVal::Builtin(BuiltinFunc::PortStderr),
        );
        env.set(
            "PORT-READ-BYTE*".to_string(),
            LispVal::Builtin(BuiltinFunc::PortReadByte),
        );
        env.set(
            "PORT-READ-BYTES*".to_string(),
            LispVal::Builtin(BuiltinFunc::PortReadBytes),
        );
        env.set(
            "PORT-WRITE-BYTE*".to_string(),
            LispVal::Builtin(BuiltinFunc::PortWriteByte),
        );
        env.set(
            "PORT-WRITE-BYTES*".to_string(),
            LispVal::Builtin(BuiltinFunc::PortWriteBytes),
        );
        env.set(
            "PORT-FLUSH*".to_string(),
            LispVal::Builtin(BuiltinFunc::PortFlush),
        );
        env.set(
            "PORT-CLOSE*".to_string(),
            LispVal::Builtin(BuiltinFunc::PortClose),
        );
        env.set(
            "PORT-OPEN-P*".to_string(),
            LispVal::Builtin(BuiltinFunc::PortOpenP),
        );
        env.set(
            "PORT-INPUT-P*".to_string(),
            LispVal::Builtin(BuiltinFunc::PortInputP),
        );
        env.set(
            "PORT-OUTPUT-P*".to_string(),
            LispVal::Builtin(BuiltinFunc::PortOutputP),
        );
        env.set(
            "PORT-SEEKABLE-P*".to_string(),
            LispVal::Builtin(BuiltinFunc::PortSeekableP),
        );
        env.set(
            "PORT-POSITION*".to_string(),
            LispVal::Builtin(BuiltinFunc::PortPosition),
        );
        env.set(
            "PORT-SEEK*".to_string(),
            LispVal::Builtin(BuiltinFunc::PortSeek),
        );
        env.set("PORT-P*".to_string(), LispVal::Builtin(BuiltinFunc::PortP));
        env.set(
            "PORT-NAME*".to_string(),
            LispVal::Builtin(BuiltinFunc::PortName),
        );
        env.set(
            "PORT-KIND*".to_string(),
            LispVal::Builtin(BuiltinFunc::PortKind),
        );

        // Networking (issue #258, epic #253): kernel substrate wrapped by
        // the NET/TCP/UDP modules in lib/37-net.lisp, lib/38-tcp.lisp,
        // lib/39-udp.lisp. Flat "*"-suffixed names, like the PORT-* substrate
        // above.
        env.set(
            "NET-RESOLVE*".to_string(),
            LispVal::Builtin(BuiltinFunc::NetResolve),
        );
        env.set(
            "NET-LOCAL-ADDR*".to_string(),
            LispVal::Builtin(BuiltinFunc::NetLocalAddr),
        );
        env.set(
            "NET-PEER-ADDR*".to_string(),
            LispVal::Builtin(BuiltinFunc::NetPeerAddr),
        );
        env.set(
            "TCP-CONNECT*".to_string(),
            LispVal::Builtin(BuiltinFunc::TcpConnect),
        );
        env.set(
            "TCP-LISTEN*".to_string(),
            LispVal::Builtin(BuiltinFunc::TcpListen),
        );
        env.set(
            "TCP-ACCEPT*".to_string(),
            LispVal::Builtin(BuiltinFunc::TcpAccept),
        );
        env.set(
            "TCP-SHUTDOWN*".to_string(),
            LispVal::Builtin(BuiltinFunc::TcpShutdown),
        );
        env.set(
            "TCP-SET-READ-TIMEOUT*".to_string(),
            LispVal::Builtin(BuiltinFunc::TcpSetReadTimeout),
        );
        env.set(
            "TCP-SET-WRITE-TIMEOUT*".to_string(),
            LispVal::Builtin(BuiltinFunc::TcpSetWriteTimeout),
        );
        env.set(
            "NET-HANDLE-CLOSE*".to_string(),
            LispVal::Builtin(BuiltinFunc::NetHandleClose),
        );
        env.set(
            "NET-HANDLE-OPEN-P*".to_string(),
            LispVal::Builtin(BuiltinFunc::NetHandleOpenP),
        );
        env.set(
            "NET-HANDLE-P*".to_string(),
            LispVal::Builtin(BuiltinFunc::NetHandleP),
        );
        env.set(
            "NET-HANDLE-KIND*".to_string(),
            LispVal::Builtin(BuiltinFunc::NetHandleKind),
        );
        env.set(
            "NET-HANDLE-NAME*".to_string(),
            LispVal::Builtin(BuiltinFunc::NetHandleName),
        );
        env.set(
            "UDP-BIND*".to_string(),
            LispVal::Builtin(BuiltinFunc::UdpBind),
        );
        env.set(
            "UDP-CONNECT*".to_string(),
            LispVal::Builtin(BuiltinFunc::UdpConnect),
        );
        env.set(
            "UDP-SEND-TO*".to_string(),
            LispVal::Builtin(BuiltinFunc::UdpSendTo),
        );
        env.set(
            "UDP-SEND*".to_string(),
            LispVal::Builtin(BuiltinFunc::UdpSend),
        );
        env.set(
            "UDP-RECEIVE-FROM*".to_string(),
            LispVal::Builtin(BuiltinFunc::UdpReceiveFrom),
        );
        env.set(
            "UDP-SET-TIMEOUT*".to_string(),
            LispVal::Builtin(BuiltinFunc::UdpSetTimeout),
        );

        // TLS (issue #365, epic #253): kernel substrate wrapped by the TLS
        // module in lib/43-tls.lisp, behind the off-by-default `net-tls`
        // cargo feature. Registered unconditionally (see BuiltinFunc's own
        // doc comment) so `(require 'tls)` and every `tls:*` name always
        // resolve; with the feature compiled out every one of these except
        // TLS-AVAILABLE-P* signals a structured `:category :tls-unavailable`
        // error instead of doing any work.
        env.set(
            "TLS-AVAILABLE-P*".to_string(),
            LispVal::Builtin(BuiltinFunc::TlsAvailableP),
        );
        env.set(
            "TLS-WRAP-CLIENT*".to_string(),
            LispVal::Builtin(BuiltinFunc::TlsWrapClient),
        );
        env.set(
            "TLS-WRAP-CLIENT-INSECURE*".to_string(),
            LispVal::Builtin(BuiltinFunc::TlsWrapClientInsecure),
        );
        env.set(
            "TLS-WRAP-SERVER*".to_string(),
            LispVal::Builtin(BuiltinFunc::TlsWrapServer),
        );
        env.set(
            "TLS-ALPN-PROTOCOL*".to_string(),
            LispVal::Builtin(BuiltinFunc::TlsAlpnProtocol),
        );
        env.set(
            "TLS-PEER-CERTIFICATES*".to_string(),
            LispVal::Builtin(BuiltinFunc::TlsPeerCertificates),
        );
        env.set(
            "TLS-PEER-CERTIFICATE-SUMMARY*".to_string(),
            LispVal::Builtin(BuiltinFunc::TlsPeerCertificateSummary),
        );
        env.set(
            "TLS-SNI-HOSTNAME*".to_string(),
            LispVal::Builtin(BuiltinFunc::TlsSniHostname),
        );

        // OS integration (issue #260, epic #253): kernel substrate wrapped by
        // the OS/OS-LINUX modules in lib/41-os.lisp, lib/42-os-linux.lisp.
        // Flat "*"-suffixed names, like the PORT-*/NET-*/TCP-*/UDP-*
        // substrate above.
        env.set(
            "OS-ARGS*".to_string(),
            LispVal::Builtin(BuiltinFunc::OsArgs),
        );
        env.set(
            "OS-EXECUTABLE-PATH*".to_string(),
            LispVal::Builtin(BuiltinFunc::OsExecutablePath),
        );
        env.set("OS-CWD*".to_string(), LispVal::Builtin(BuiltinFunc::OsCwd));
        env.set(
            "OS-CHDIR*".to_string(),
            LispVal::Builtin(BuiltinFunc::OsChdir),
        );
        env.set(
            "OS-ENV-GET*".to_string(),
            LispVal::Builtin(BuiltinFunc::OsEnvGet),
        );
        env.set(
            "OS-ENV-LIST*".to_string(),
            LispVal::Builtin(BuiltinFunc::OsEnvList),
        );
        env.set(
            "OS-ENV-SET*".to_string(),
            LispVal::Builtin(BuiltinFunc::OsEnvSet),
        );
        env.set(
            "OS-ENV-UNSET*".to_string(),
            LispVal::Builtin(BuiltinFunc::OsEnvUnset),
        );
        env.set("OS-PID*".to_string(), LispVal::Builtin(BuiltinFunc::OsPid));
        env.set(
            "OS-PPID*".to_string(),
            LispVal::Builtin(BuiltinFunc::OsPpid),
        );
        env.set(
            "OS-HOSTNAME*".to_string(),
            LispVal::Builtin(BuiltinFunc::OsHostname),
        );
        env.set("OS-NOW*".to_string(), LispVal::Builtin(BuiltinFunc::OsNow));
        env.set(
            "OS-MONOTONIC-NANOS*".to_string(),
            LispVal::Builtin(BuiltinFunc::OsMonotonicNanos),
        );
        env.set(
            "OS-SLEEP*".to_string(),
            LispVal::Builtin(BuiltinFunc::OsSleep),
        );
        env.set(
            "OS-PRNG-STEP*".to_string(),
            LispVal::Builtin(BuiltinFunc::OsPrngStep),
        );
        env.set(
            "OS-RANDOM-BYTES*".to_string(),
            LispVal::Builtin(BuiltinFunc::OsRandomBytes),
        );
        env.set(
            "OS-SPAWN*".to_string(),
            LispVal::Builtin(BuiltinFunc::OsSpawn),
        );
        env.set(
            "OS-PROCESS-WAIT*".to_string(),
            LispVal::Builtin(BuiltinFunc::OsProcessWait),
        );
        env.set(
            "OS-PROCESS-TRY-WAIT*".to_string(),
            LispVal::Builtin(BuiltinFunc::OsProcessTryWait),
        );
        env.set(
            "OS-PROCESS-ID*".to_string(),
            LispVal::Builtin(BuiltinFunc::OsProcessId),
        );
        env.set(
            "OS-PROCESS-KILL*".to_string(),
            LispVal::Builtin(BuiltinFunc::OsProcessKill),
        );
        env.set(
            "OS-PROCESS-TERMINATE*".to_string(),
            LispVal::Builtin(BuiltinFunc::OsProcessTerminate),
        );
        env.set(
            "OS-PROCESS-OPEN-P*".to_string(),
            LispVal::Builtin(BuiltinFunc::OsProcessOpenP),
        );
        env.set(
            "OS-PROCESS-P*".to_string(),
            LispVal::Builtin(BuiltinFunc::OsProcessP),
        );
        env.set(
            "OS-SIGNAL*".to_string(),
            LispVal::Builtin(BuiltinFunc::OsSignal),
        );
        env.set(
            "OS-LINUX-STAT*".to_string(),
            LispVal::Builtin(BuiltinFunc::OsLinuxStat),
        );
        env.set(
            "OS-LINUX-READLINK*".to_string(),
            LispVal::Builtin(BuiltinFunc::OsLinuxReadlink),
        );

        // Regex primitives (lib/44-regex.lisp)
        env.set(
            "REGEX-COMPILE*".to_string(),
            LispVal::Builtin(BuiltinFunc::RegexCompile),
        );
        env.set(
            "REGEX-P*".to_string(),
            LispVal::Builtin(BuiltinFunc::RegexP),
        );
        env.set(
            "REGEX-PATTERN*".to_string(),
            LispVal::Builtin(BuiltinFunc::RegexPattern),
        );
        env.set(
            "REGEX-ESCAPE*".to_string(),
            LispVal::Builtin(BuiltinFunc::RegexEscape),
        );
        env.set(
            "REGEX-IS-MATCH*".to_string(),
            LispVal::Builtin(BuiltinFunc::RegexIsMatch),
        );
        env.set(
            "REGEX-FIND*".to_string(),
            LispVal::Builtin(BuiltinFunc::RegexFind),
        );
        env.set(
            "REGEX-FIND-ALL*".to_string(),
            LispVal::Builtin(BuiltinFunc::RegexFindAll),
        );
        env.set(
            "REGEX-CAPTURES*".to_string(),
            LispVal::Builtin(BuiltinFunc::RegexCaptures),
        );
        env.set(
            "REGEX-CAPTURES-NAMED*".to_string(),
            LispVal::Builtin(BuiltinFunc::RegexCapturesNamed),
        );
        env.set(
            "REGEX-REPLACE*".to_string(),
            LispVal::Builtin(BuiltinFunc::RegexReplace),
        );
        env.set(
            "REGEX-REPLACE-ALL*".to_string(),
            LispVal::Builtin(BuiltinFunc::RegexReplaceAll),
        );
        env.set(
            "REGEX-SPLIT*".to_string(),
            LispVal::Builtin(BuiltinFunc::RegexSplit),
        );

        // Introspection
        env.set(
            "DESCRIBE".to_string(),
            LispVal::Builtin(BuiltinFunc::Describe),
        );
        env.set(
            "SEE-SOURCE".to_string(),
            LispVal::Builtin(BuiltinFunc::SeeSource),
        );
        env.set(
            "DISASSEMBLE".to_string(),
            LispVal::Builtin(BuiltinFunc::Disassemble),
        );

        // Concurrency primitives (gated behind the `concurrency` feature)
        #[cfg(feature = "concurrency")]
        {
            env.set(
                "MAKE-CHANNEL".to_string(),
                LispVal::Builtin(BuiltinFunc::MakeChannel),
            );
            env.set(
                "SPAWN-THREAD".to_string(),
                LispVal::Builtin(BuiltinFunc::SpawnProcess),
            );
            env.set(
                "CHANNEL-SEND".to_string(),
                LispVal::Builtin(BuiltinFunc::ChannelSend),
            );
            env.set(
                "CHANNEL-RECV".to_string(),
                LispVal::Builtin(BuiltinFunc::ChannelRecv),
            );
            env.set(
                "CHANNEL-RECV-TIMEOUT".to_string(),
                LispVal::Builtin(BuiltinFunc::ChannelRecvTimeout),
            );
            env.set(
                "CLONE-INTERPRETER".to_string(),
                LispVal::Builtin(BuiltinFunc::CloneInterpreter),
            );
        }

        env
    }

    /// Create a new environment with all builtins **and** the embedded standard
    /// library pre-loaded. This is the recommended entry point for host code
    /// that wants a fully-featured Lisp interpreter without shipping .lisp files.
    ///
    /// The first call on a thread parses and evaluates the embedded stdlib
    /// once into a private, never-escaping prototype world; every call
    /// (including the first) then returns a deep-copy fork of that prototype
    /// via [`Environment::fork_world`] — a fresh symbol table, fresh global
    /// value cells, fresh closures/containers — in a few milliseconds
    /// instead of re-evaluating ~45 library files. Each returned environment
    /// is a fully isolated world: definitions, redefinitions, dynamic
    /// variables, property lists, condition flags, and capability grants in
    /// one are never visible in another, exactly as with the pre-fork
    /// loader. Symbol `EQ` identity holds within each returned world (as it
    /// always has; distinct `with_stdlib` worlds have always had distinct
    /// symbol interning).
    ///
    /// Use [`Environment::with_stdlib_fresh`] to bypass the per-thread
    /// prototype (e.g. in a process that builds exactly one environment).
    ///
    /// Panics if the embedded stdlib fails to parse or evaluate (which would be
    /// a compile-time bug, not a runtime condition).
    pub fn with_stdlib() -> Shared<Environment> {
        STDLIB_PROTOTYPE.with(|slot| {
            let mut proto = slot.borrow_mut();
            if proto.is_none() {
                *proto = Some(Self::with_stdlib_fresh());
            }
            let p = proto.as_ref().expect("prototype was just installed");
            // The prototype never escapes this thread-local, so nothing can
            // have mutated it; a fork is observably identical to a fresh
            // load. Fall back to the full load if the world ever becomes
            // unforkable (it cannot for the embedded stdlib — see
            // `fork_world` — but the fallback keeps this future-proof).
            Environment::fork_world(p).unwrap_or_else(|_| Self::with_stdlib_fresh())
        })
    }

    /// Build a stdlib environment by actually parsing and evaluating the
    /// embedded sources — the pre-caching behavior of
    /// [`Environment::with_stdlib`], which now serves a deep-copy fork of a
    /// per-thread prototype instead of re-evaluating ~45 files per call.
    ///
    /// Use this when you want exactly one fully-loaded environment and do
    /// not want a prototype copy retained in thread-local storage for the
    /// rest of the thread's life (e.g. a one-environment-per-process CLI),
    /// or when you need to measure/exercise the real loader. The resulting
    /// environment is indistinguishable from a `with_stdlib()` fork.
    pub fn with_stdlib_fresh() -> Shared<Environment> {
        let env = Self::new_with_builtins();
        // The interpreter entry points that consume this environment
        // (`load_file`, `eval_str`/`eval_all`, `read-from-string`) are
        // documented to run on the 512 MiB `with_large_stack` thread, so the
        // reader depth limit can be far more generous than the conservative
        // library default of 512. At the measured ~6.4 KB of native stack per
        // nesting level (debug build; see `reader::DEFAULT_READER_DEPTH`),
        // 50,000 levels is ~320 MB — inside the 512 MiB stack, and ~1.7x
        // under the empirically observed crash boundary of ~84,000-90,000
        // levels on that thread (issue #270).
        env.set_reader_depth_limit(50_000);
        crate::load_stdlib(&env).expect("embedded stdlib should always load cleanly");
        env
    }

    /// Create a new environment with all builtins **and** the Prelude —
    /// the stable general-purpose language vocabulary — pre-loaded, but
    /// *without* the optional embedded libraries `with_stdlib` also loads
    /// (issue #256, epic #253; see `PRELUDE_SOURCES` in `src/lib.rs` for the
    /// exact file list and `lib/06-require.lisp` for the rationale). Pull in
    /// an optional library by name with `(require 'name)`; see
    /// [`Environment::register_module`] to add your own.
    ///
    /// This is lighter than [`Environment::with_stdlib`] (no shell/testing/
    /// optimizer/call-graph/condensation/pattern-matching/help database/...
    /// vocabulary until requested) but is otherwise fully-featured: every
    /// kernel builtin is registered, exactly as with `with_stdlib`.
    ///
    /// Like [`Environment::with_stdlib`], calls after the first on a thread
    /// are served as a cheap deep-copy fork of a per-thread prototype (see
    /// [`Environment::fork_world`]); use [`Environment::with_prelude_fresh`]
    /// to bypass the prototype.
    ///
    /// Panics if the embedded Prelude fails to parse or evaluate (which would
    /// be a compile-time bug, not a runtime condition).
    pub fn with_prelude() -> Shared<Environment> {
        PRELUDE_PROTOTYPE.with(|slot| {
            let mut proto = slot.borrow_mut();
            if proto.is_none() {
                *proto = Some(Self::with_prelude_fresh());
            }
            let p = proto.as_ref().expect("prototype was just installed");
            Environment::fork_world(p).unwrap_or_else(|_| Self::with_prelude_fresh())
        })
    }

    /// Build a Prelude environment by actually evaluating the embedded
    /// Prelude sources — the pre-caching behavior of
    /// [`Environment::with_prelude`]. Same trade-off as
    /// [`Environment::with_stdlib_fresh`].
    pub fn with_prelude_fresh() -> Shared<Environment> {
        let env = Self::new_with_builtins();
        env.set_reader_depth_limit(50_000);
        crate::load_prelude(&env).expect("embedded prelude should always load cleanly");
        env
    }

    /// Create a sandboxed environment with all builtins registered but all
    /// dangerous capabilities disabled.
    ///
    /// All potentially dangerous feature flags (`SHELL`, `READ-FS`, `CREATE-FS`,
    /// `TEMP-FS`, `IO`) are off by default in every environment, so this is
    /// semantically equivalent to `new_with_builtins()`.  The explicit name
    /// communicates intent clearly to embedders: scripts loaded into this
    /// environment cannot access the filesystem, spawn subprocesses, or read
    /// from stdin unless the host explicitly calls `enable_feature`.
    ///
    /// # Example
    /// ```rust,ignore
    /// let env = Environment::new_sandboxed();
    /// assert!(!env.feature_enabled("SHELL"));
    /// assert!(!env.feature_enabled("READ-FS"));
    /// assert!(!env.feature_enabled("IO"));
    /// ```
    pub fn new_sandboxed() -> Shared<Environment> {
        Self::new_with_builtins()
    }

    /// Intern `name` (uppercased) into the global symbol table.
    ///
    /// Returns the shared `Rc<RefCell<Symbol>>` for this name, creating a new
    /// entry if the name has not been seen before.
    pub fn intern_symbol(&self, name: &str) -> Shared<SharedCell<Symbol>> {
        self.shared.symbols.borrow_mut().intern(name)
    }

    /// Look up the symbol with the given id in the global symbol table.
    /// Returns `None` if the id is not registered (should not happen for
    /// ids produced by `intern`/`gensym`).
    pub fn symbol_by_id(&self, id: u32) -> Option<Shared<SharedCell<Symbol>>> {
        self.shared.symbols.borrow().symbol_by_id(id)
    }

    /// Fast-path check: `true` once any variable has been marked dynamic.
    /// Binding sites use this to skip the `symbol_by_id` + `is_dynamic` probe
    /// in the common case where no dynamic variables exist at all.
    #[inline]
    pub fn has_any_dynamic(&self) -> bool {
        self.shared.has_dynamic.get()
    }

    /// Whether the symbol with id `id` is a dynamic (special) variable.
    /// Used by the compiled TCO fast path to check a lambda's own params
    /// rather than bailing on the world-global
    /// [`Environment::has_any_dynamic`] flag alone (which is permanently
    /// true once the stdlib defines its first dynamic variable).
    pub fn symbol_id_is_dynamic(&self, id: u32) -> bool {
        self.shared
            .symbols
            .borrow()
            .symbol_by_id(id)
            .is_some_and(|s| s.borrow().is_dynamic)
    }

    /// Generate a fresh uninterned symbol.  Equivalent to `(gensym)` in Lisp.
    pub fn gensym(&self) -> Shared<SharedCell<Symbol>> {
        self.shared.symbols.borrow_mut().gensym()
    }

    pub fn all_symbols(&self) -> Vec<Shared<SharedCell<Symbol>>> {
        self.shared.symbols.borrow().all_symbols()
    }

    /// Return the (uppercased, canonical) names of every interned symbol.
    ///
    /// Convenience wrapper over [`Environment::all_symbols`] for callers that
    /// only need names — e.g. REPL tab-completion — and don't want to deal
    /// with borrowing each symbol cell themselves.
    pub fn all_symbol_names(&self) -> Vec<String> {
        self.shared
            .symbols
            .borrow()
            .all_symbols()
            .iter()
            .map(|sym| sym.borrow().name.clone())
            .collect()
    }

    /// Return `true` if `name` is bound anywhere in the lexical chain.
    pub fn is_bound(&self, name: &str) -> bool {
        self.get(name).is_some()
    }

    /// Return the names of every symbol that is actually **bound** (has a
    /// value visible from this environment — a global value-cell binding, a
    /// local frame binding, or a dynamic binding), as opposed to merely
    /// interned.
    ///
    /// Used to build did-you-mean suggestions on unbound-symbol errors (see
    /// [`crate::teaching_errors`]): suggesting an interned-but-unbound name
    /// (e.g. a symbol that only ever appeared as quoted data) would be
    /// useless noise. This is an O(n) scan over the symbol table plus one
    /// `resolve()` per symbol, which is only acceptable because callers only
    /// reach it on the (cold) error-construction path.
    pub fn bound_symbol_names(&self) -> Vec<String> {
        self.shared
            .symbols
            .borrow()
            .all_symbols()
            .iter()
            .filter(|sym| self.resolve(sym).is_some())
            .map(|sym| sym.borrow().name.clone())
            .collect()
    }

    /// `true` if this is the root (global) frame, whose variable storage is the
    /// per-symbol value cells rather than a `HashMap`.
    #[inline]
    fn is_root(&self) -> bool {
        self.parent.is_none()
    }

    /// Write a global binding into the symbol value cell, by name.
    pub(crate) fn global_set(&self, name: &str, val: LispVal) {
        let sym = self.shared.symbols.borrow_mut().intern(name);
        sym.borrow_mut().value = Some(val);
    }

    /// Bind `id` (a symbol's numeric id) to `val` in this local environment frame.
    ///
    /// Only valid on non-root frames; the root frame stores bindings in per-symbol
    /// value cells, not in the binding map.  Panics in debug mode if called on the
    /// root frame.
    #[inline]
    pub fn set_id(&self, id: u32, val: LispVal) {
        debug_assert!(!self.is_root(), "set_id called on root frame");
        if let Some(rt) = &self.routing
            && let Some(pos) = rt.iter().position(|r| *r == id)
        {
            self.slots.borrow_mut()[pos] = val;
            return;
        }
        self.bindings.borrow_mut().insert(id, val);
    }

    /// Update an existing binding of `id` on THIS frame (slot or map),
    /// returning whether one was found. Shared by the SETQ walks so slot
    /// frames and map frames update through one code path.
    fn frame_update_existing(&self, id: u32, val: &LispVal) -> bool {
        if let Some(rt) = &self.routing
            && let Some(pos) = rt.iter().position(|r| *r == id)
        {
            self.slots.borrow_mut()[pos] = val.clone();
            return true;
        }
        if self.bindings.borrow().contains_key(&id) {
            self.bindings.borrow_mut().insert(id, val.clone());
            return true;
        }
        false
    }

    /// Direct lexical-address read for `Code::LocalGet` (issue #200 M3):
    /// `depth` frames up the parent chain, then `slots[slot]`. `None` when
    /// the chain is shorter than `depth` or the target frame carries no
    /// routing / no such slot — the executor then falls back to full
    /// resolution.
    #[inline]
    pub(crate) fn local_get(&self, depth: u16, slot: u16) -> Option<LispVal> {
        let mut frame = self;
        for _ in 0..depth {
            frame = frame.parent.as_deref()?;
        }
        frame.routing.as_ref()?;
        frame.slots.borrow().get(slot as usize).cloned()
    }

    /// Read `id` from THIS frame only (slot or map), cloning the value.
    #[inline]
    pub(crate) fn frame_get(&self, id: u32) -> Option<LispVal> {
        if let Some(rt) = &self.routing
            && let Some(pos) = rt.iter().position(|r| *r == id)
        {
            return Some(self.slots.borrow()[pos].clone());
        }
        self.bindings.borrow().get(&id).cloned()
    }

    /// Hot-path variable resolution from the interned symbol the AST holds.
    ///
    /// Local frames are probed by symbol id (u32 integer key); the root binding
    /// is read from the symbol's value cell directly — O(1), no hash lookup.
    /// Respects dynamic scoping when any dynamic variable exists.
    pub fn resolve(&self, sym: &Shared<SharedCell<Symbol>>) -> Option<LispVal> {
        let s = sym.borrow();
        if self.shared.has_dynamic.get() && s.is_dynamic {
            // Shallow binding: the current dynamic value is always in the symbol's
            // value cell — O(1), no dynamic-parent chain walk required.
            // is_dynamic is a cached flag set by mark_dynamic(), so this avoids
            // the dynamic_vars HashSet probe on the hot evaluation path.
            return s.value.clone();
        }
        // Lexical: walk local frames by id; the root binding is the value cell
        // of the *current namespace's* canonical symbol for this name.
        //
        // Cross-namespace fix (issue #223): when a lambda is created inside a
        // foreign `(make-environment)`, `param_ids` are interned under that
        // environment's SymbolTable, but the AST body symbols retain the
        // *original* table's ids.  Detect the mismatch once here and remap to
        // the canonical id so local-frame lookups use the right key.
        let id = {
            let table = self.shared.symbols.borrow();
            match table.symbol_by_id(s.id) {
                Some(canon) if Shared::ptr_eq(&canon, sym) => s.id,
                _ => match table.get_symbol(&s.name) {
                    Some(canon) => canon.borrow().id,
                    None => s.id,
                },
            }
        };
        let mut frame = self;
        loop {
            if frame.is_root() {
                let table = frame.shared.symbols.borrow();
                // Fast path: in the common single-namespace case the AST symbol
                // IS this table's canonical symbol (its id indexes back to the
                // same `Rc`), so read the cell directly — O(1), no name hash.
                match table.symbol_by_id(id) {
                    Some(canon) if Shared::ptr_eq(&canon, sym) => return s.value.clone(),
                    // First-class environments: `sym` was interned in a different
                    // namespace/table, so resolve the name against THIS table's
                    // canonical symbol and read *its* cell (not the caller's).
                    _ => {
                        return table
                            .get_symbol(&s.name)
                            .and_then(|c| c.borrow().value.clone());
                    }
                }
            }
            if let Some(val) = frame.frame_get(id) {
                return Some(val);
            }
            frame = frame.parent.as_deref().unwrap();
        }
    }

    /// Lexical variable lookup by name.  Walks the `parent` chain; does **not**
    /// check the dynamic-parent chain.  Use [`Environment::get_var`] for the
    /// scoping-aware lookup that respects dynamic variables.
    ///
    /// This is the cold path (name-based).  The hot path goes through
    /// [`Environment::resolve`] which uses the symbol id from the AST node
    /// directly without re-interning the name.
    pub fn get(&self, name: &str) -> Option<LispVal> {
        // Find the interned symbol, then use resolve for the actual lookup so
        // we share the same id-based frame walk as the hot path.
        let sym = self.shared.symbols.borrow().get_symbol(name)?;
        self.resolve(&sym)
    }

    /// Bind `name` to `val` in this environment frame (not in any parent).
    ///
    /// At the root frame this writes the symbol's global value cell; in a child
    /// frame it writes the frame's local map using the symbol id as the key.
    /// Use [`Environment::update`] to modify an existing binding that may live
    /// in a parent frame.
    pub fn set(&self, name: String, val: LispVal) {
        if self.is_root() {
            self.global_set(&name, val);
        } else {
            let id = self.shared.symbols.borrow_mut().intern(&name).borrow().id;
            self.bindings.borrow_mut().insert(id, val);
        }
    }

    /// Register a host Rust closure as a callable Lisp function named `name`.
    ///
    /// The name is uppercased to match Lisp convention. After registration,
    /// `(NAME arg1 arg2 ...)` calls the closure with evaluated arguments.
    pub fn register_fn<F>(&self, name: &str, f: F)
    where
        F: Fn(&[LispVal], &Shared<Environment>) -> Result<LispVal, LispError> + 'static,
    {
        self.set(name.to_uppercase(), LispVal::Native(Shared::new(f)));
    }

    /// Type-check and compile a `(defun-typed ...)` form into the shared typed
    /// registry. Returns the function's (uppercased) name on success.
    pub fn jit_define(&self, form: &LispVal) -> Result<String, String> {
        let id = self.shared.jit.borrow_mut().define(form)?;
        self.shared
            .jit
            .borrow()
            .name_of(id)
            .ok_or_else(|| "jit: defined function has no name".to_string())
    }

    /// Forward-declare a typed signature from a `(declare-typed ...)` form so
    /// mutually-recursive functions can reference each other. Returns the name.
    pub fn jit_declare(&self, form: &LispVal) -> Result<String, String> {
        self.shared.jit.borrow_mut().declare_form(form)
    }

    /// Define a typed struct from a `(defstruct-typed ...)` form. Returns the
    /// generated accessor function names (`make-NAME`, `NAME-FIELD`, …) so the
    /// caller can install their membrane entries.
    pub fn jit_define_struct(&self, form: &LispVal) -> Result<Vec<String>, String> {
        self.shared.jit.borrow_mut().define_struct(form)
    }

    /// Best-effort: attempt to type and natively compile an un-annotated function
    /// (HM firing under `defun`). Returns `true` if it became a typed edition.
    pub fn jit_infer_untyped(&self, name: &str, params: &[String], body: &[LispVal]) -> bool {
        self.shared
            .jit
            .borrow_mut()
            .infer_untyped(name, params, body)
            .is_ok()
    }

    /// Type-check an un-annotated function without compiling it (the non-compiled
    /// checker, #162). Returns its generalized type as a printable scheme, or a
    /// type error.
    /// Dry-run codegen verdict: Ok(()) = would compile natively; Err(msg) =
    /// the concrete blocker. Installs nothing (explain-compile).
    pub fn jit_compile_reason(
        &self,
        name: &str,
        params: &[String],
        body: &[LispVal],
    ) -> Result<(), String> {
        self.shared
            .jit
            .borrow_mut()
            .compile_reason(name, params, body)
    }

    pub fn jit_check_untyped(
        &self,
        name: &str,
        params: &[String],
        body: &[LispVal],
    ) -> Result<String, String> {
        let resolver = |n: &str| self.checker_lambda_source(n);
        self.shared
            .jit
            .borrow_mut()
            .check_untyped(name, params, body, Some(&resolver))
    }

    /// Type-check a single expression and return its inferred type as a string.
    pub fn jit_check_expr(&self, expr: &LispVal) -> Result<String, String> {
        let resolver = |n: &str| self.checker_lambda_source(n);
        self.shared
            .jit
            .borrow_mut()
            .check_expr(expr, Some(&resolver))
    }

    /// Register a **declared** type scheme (experimental rows, `declare-type!`)
    /// for `name` from its surface form. Returns the rendered scheme.
    pub fn jit_declare_scheme(&self, name: &str, form: &LispVal) -> Result<String, String> {
        self.shared.jit.borrow_mut().declare_scheme(name, form)
    }

    /// Register a record type without typed functions (issue #308 stage B).
    pub fn jit_declare_record(&self, name: &str, field_specs: &LispVal) -> Result<(), String> {
        self.shared
            .jit
            .borrow_mut()
            .declare_record(name, field_specs)
    }

    /// Whether record `name`'s fields are all natively storable.
    pub fn jit_record_compileable(&self, name: &str) -> Option<bool> {
        self.shared.jit.borrow().record_compileable(name)
    }

    /// Declare a parametric record/ctor (0.3 HM generics).
    pub fn jit_declare_generic_record(
        &self,
        name: &str,
        params: &[String],
        field_specs: &LispVal,
    ) -> Result<(), String> {
        self.shared
            .jit
            .borrow_mut()
            .declare_generic_record(name, params, field_specs)
    }

    /// Declare a parametric variant (0.3 HM generics).
    pub fn jit_declare_generic_variant(
        &self,
        name: &str,
        arity: usize,
        ctors: Vec<String>,
    ) -> Result<(), String> {
        self.shared
            .jit
            .borrow_mut()
            .declare_generic_variant(name, arity, ctors)
    }

    /// Register a protocol INSTANCE scheme (0.3 typed protocols).
    pub fn jit_declare_instance(&self, name: &str, form: &LispVal) -> Result<String, String> {
        self.shared.jit.borrow_mut().declare_instance(name, form)
    }

    /// Set a protocol's dispatch argument position (fn-first protocols
    /// like `map` dispatch on 1; the default is 0).
    pub fn jit_declare_protocol_dispatch(&self, name: &str, idx: usize) {
        self.shared
            .jit
            .borrow_mut()
            .declare_protocol_dispatch(name, idx)
    }

    /// Declare a sum type (variant) in the typed registry (#312).
    pub fn jit_declare_variant(&self, name: &str, ctors: Vec<String>) -> Result<(), String> {
        self.shared.jit.borrow_mut().declare_variant(name, ctors)
    }

    /// Constructor brands of the registered variant `name`, if any.
    pub fn jit_variant_ctors(&self, name: &str) -> Option<Vec<String>> {
        self.shared.jit.borrow().variant_ctors(name)
    }

    /// Ordered field names of the registered typed struct `name` (issue
    /// #308: consulted by the `record-ref`/`record-with` primitives).
    pub fn jit_struct_field_names(&self, name: &str) -> Option<Vec<String>> {
        self.shared.jit.borrow().struct_field_names(name)
    }

    /// The rendered declared scheme for `name`, if any.
    pub fn jit_declared_scheme(&self, name: &str) -> Option<String> {
        self.shared.jit.borrow().declared_scheme_name(name)
    }

    /// (params, body forms) of `name` when it is bound to a plain fixed-arity
    /// lambda — the checker's lambda source for on-demand call-site checking
    /// (#308). Variadic lambdas and non-lambdas yield `None`.
    fn checker_lambda_source(&self, name: &str) -> Option<(Vec<String>, Vec<LispVal>)> {
        let LispVal::Lambda(lam) = self.get(name)? else {
            return None;
        };
        if lam.rest_param.is_some() {
            return None;
        }
        let body_forms: Vec<LispVal> = match lam.body.as_ref() {
            LispVal::Cons { car, cdr } if matches!(car.as_ref(), LispVal::Symbol(s) if s.borrow().name == "PROGN") =>
            {
                let mut out = Vec::new();
                let mut cur = cdr.as_ref();
                while let LispVal::Cons { car, cdr } = cur {
                    out.push(car.as_ref().clone());
                    cur = cdr.as_ref();
                }
                out
            }
            other => vec![other.clone()],
        };
        if body_forms.is_empty() {
            return None;
        }
        Some((lam.params.clone(), body_forms))
    }

    /// One-pass analyze: check, then compile if compileable (#162 stage 4).
    pub fn jit_analyze_untyped(
        &self,
        name: &str,
        params: &[String],
        body: &[LispVal],
    ) -> crate::jit::Analysis {
        let resolver = |n: &str| self.checker_lambda_source(n);
        self.shared
            .jit
            .borrow_mut()
            .analyze_untyped(name, params, body, Some(&resolver))
    }

    /// `(param types, return type)` of a registered typed function, if any.
    pub fn jit_signature(&self, name: &str) -> Option<(Vec<crate::jit::Ty>, crate::jit::Ty)> {
        self.shared.jit.borrow().signature(name)
    }

    /// Like `jit_signature` but includes parameter names.
    pub fn jit_named_signature(
        &self,
        name: &str,
    ) -> Option<(Vec<(String, crate::jit::Ty)>, crate::jit::Ty)> {
        self.shared.jit.borrow().named_signature(name)
    }

    /// Compile a function with **partial type hints** (the `defun*` back-end).
    /// `None` in a param slot or for `ret_hint` means "infer this type". Returns
    /// `(id, sig_string)` on success; rolls back and returns `Err` on failure.
    pub fn jit_define_partial(
        &self,
        name: &str,
        params: &[(String, Option<crate::jit::Ty>)],
        ret_hint: Option<crate::jit::Ty>,
        body: &[LispVal],
    ) -> Result<(usize, String), String> {
        self.shared
            .jit
            .borrow_mut()
            .define_partial(name, params, ret_hint, body)
    }

    /// Whether a registered typed function currently has a compiled edition.
    /// `None` if no typed function by that name exists.
    pub fn jit_is_compiled(&self, name: &str) -> Option<bool> {
        let jit = self.shared.jit.borrow();
        jit.get(name).map(|f| f.is_compiled())
    }

    /// The execution tier a registered typed function will actually run on
    /// (`NATIVE`/`CLOSURE`) — the `compiled-p` introspection builtin's
    /// back-end. `None` if no defined typed function by that name exists.
    pub fn jit_tier(&self, name: &str) -> Option<crate::jit::Tier> {
        self.shared.jit.borrow().tier(name)
    }

    /// Render the typed-core IR of a registered ("jotted") function as a
    /// human-readable pseudo-assembly listing. `None` if no such typed function
    /// exists.
    pub fn jit_disassemble(&self, name: &str) -> Option<String> {
        self.shared.jit.borrow().disassemble(name)
    }

    /// Call a typed function with already-converted [`crate::jit::Value`]s,
    /// crossing the membrane. `None` if no such typed function exists.
    pub fn jit_call(
        &self,
        name: &str,
        args: &[crate::jit::Value],
    ) -> Option<Result<crate::jit::Value, String>> {
        let jit = self.shared.jit.borrow();
        jit.id(name)?;
        Some(jit.call(name, args))
    }

    /// Like [`Environment::jit_call`], but also reads back the post-call
    /// contents of any flat-scalar-array argument (issue #216), so the
    /// caller can write a mutation back into its own backing store — see
    /// `Jit::call_with_array_writeback`.
    pub fn jit_call_with_array_writeback(
        &self,
        name: &str,
        args: &[crate::jit::Value],
    ) -> Option<crate::jit::WritebackResult> {
        let jit = self.shared.jit.borrow();
        jit.id(name)?;
        Some(jit.call_with_array_writeback(name, args))
    }

    /// The id a binder (lambda/fexpr/macro parameter, SETQ target, …) should
    /// key its frame entry under for `sym` — the binding-side mirror of
    /// [`Environment::resolve`]'s canonicalization (issues #223/#262, #285):
    ///
    /// - If `sym` is canonical in this environment's symbol table (its id
    ///   indexes back to the same `Rc` — true for ordinary interned symbols
    ///   and for gensyms, which live in `by_id` but not the name map), use
    ///   its own id. This is what lets a `gensym` work as a binder: the body
    ///   occurrence is the same object and resolves to the same id (#285).
    /// - Otherwise the symbol came from a foreign table (first-class
    ///   environments, #223): intern its *name* here and use the canonical
    ///   id, matching the name-remap `resolve` performs on lookups.
    pub fn binder_id(&self, sym: &Shared<SharedCell<Symbol>>) -> u32 {
        let (own_id, name) = {
            let s = sym.borrow();
            (s.id, s.name.clone())
        };
        let canonical_here = {
            let table = self.shared.symbols.borrow();
            matches!(table.symbol_by_id(own_id), Some(canon) if Shared::ptr_eq(&canon, sym))
        };
        if canonical_here {
            own_id
        } else {
            self.intern_symbol(&name).borrow().id
        }
    }

    /// [`Environment::update`], but for a known symbol object: the frame walk
    /// and the root cell both use the canonical binding for `sym` per
    /// [`Environment::binder_id`], so SETQ on a gensym writes the gensym's
    /// own cell/frame entry instead of minting an interned twin (#285).
    pub fn update_sym(env: &Shared<Environment>, sym: &Shared<SharedCell<Symbol>>, val: LispVal) {
        let (is_dynamic, name) = {
            let s = sym.borrow();
            (s.is_dynamic, s.name.clone())
        };
        if env.shared.has_dynamic.get() && is_dynamic {
            // Shallow binding: the (interned) symbol cell IS the binding.
            env.global_set(&name, val);
            return;
        }
        // Canonical symbol + id, then the same walk as `update_lexical`.
        let canon = {
            let canonical_here = {
                let table = env.shared.symbols.borrow();
                matches!(table.symbol_by_id(sym.borrow().id), Some(c) if Shared::ptr_eq(&c, sym))
            };
            if canonical_here {
                sym.clone()
            } else {
                env.intern_symbol(&name)
            }
        };
        let id = canon.borrow().id;
        let mut maybe_env = Some(env.clone());
        while let Some(current_env) = maybe_env {
            if current_env.is_root() {
                if canon.borrow().value.is_some() {
                    canon.borrow_mut().value = Some(val);
                    return;
                }
                break;
            }
            if current_env.frame_update_existing(id, &val) {
                return;
            }
            maybe_env = current_env.parent.clone();
        }
        // Not found — create it in the current environment (SETQ semantics).
        if env.is_root() {
            canon.borrow_mut().value = Some(val);
        } else {
            env.set_id(id, val);
        }
    }

    /// Update a variable's value, searching up the environment chain.
    /// For dynamic variables, this searches the dynamic parent chain.
    /// For lexical variables, this searches the lexical parent chain.
    /// If the variable is not found in any environment, it is CREATED in
    /// the current environment. This supports dynamic variable creation via
    /// SETQ and is intentional behavior for interactive development.
    pub fn update(env: &Shared<Environment>, name: &str, val: LispVal) {
        if env.shared.has_dynamic.get() && env.is_dynamic(name) {
            // Shallow binding: the symbol cell IS the current binding — write
            // there directly so the change is visible to all frames that read
            // the same cell (O(1), no dynamic-parent chain walk).
            env.global_set(name, val);
        } else {
            // For lexical variables, search the lexical parent chain
            Self::update_lexical(env, name, val);
        }
    }

    /// Update a lexical variable by walking the lexical parent chain.
    fn update_lexical(env: &Shared<Environment>, name: &str, val: LispVal) {
        // Intern (or find) the symbol to get its id — do this once.
        let sym = env.shared.symbols.borrow_mut().intern(name);
        let id = sym.borrow().id;

        let mut maybe_env = Some(env.clone());
        while let Some(current_env) = maybe_env {
            if current_env.is_root() {
                // Root storage is the symbol value cell.
                if sym.borrow().value.is_some() {
                    sym.borrow_mut().value = Some(val);
                    return;
                }
                break;
            }
            if current_env.frame_update_existing(id, &val) {
                return;
            }
            maybe_env = current_env.parent.clone();
        }
        // Variable not found — create it in the current environment.
        if env.is_root() {
            sym.borrow_mut().value = Some(val);
        } else {
            env.set_id(id, val);
        }
    }

    /// Collect all bindings visible from this frame (including parent frames).
    /// Parent bindings are shadowed by child bindings.
    pub fn all_bindings(&self) -> HashMap<String, LispVal> {
        let mut all = HashMap::new();
        if self.is_root() {
            // Root bindings live in the per-symbol value cells.
            for sym in self.shared.symbols.borrow().all_symbols() {
                let s = sym.borrow();
                if let Some(v) = &s.value {
                    all.insert(s.name.clone(), v.clone());
                }
            }
            return all;
        }
        if let Some(parent) = &self.parent {
            all.extend(parent.all_bindings());
        }
        // Convert id → name via the reverse map.
        let table = self.shared.symbols.borrow();
        if let Some(rt) = &self.routing {
            let slots = self.slots.borrow();
            for (pos, id) in rt.iter().enumerate() {
                if let Some(sym) = table.symbol_by_id(*id) {
                    all.insert(sym.borrow().name.clone(), slots[pos].clone());
                }
            }
        }
        for (id, val) in self.bindings.borrow().iter() {
            if let Some(sym) = table.symbol_by_id(*id) {
                all.insert(sym.borrow().name.clone(), val.clone());
            }
        }
        all
    }

    /// Set the boolean condition flag `flag` to `true`.
    ///
    /// Flags are global (shared across the whole chain) and are used to signal
    /// exceptional conditions such as arithmetic overflow (`"OVERFLOW"`).
    /// Check with [`Environment::flag_set`]; clear with [`Environment::clear_flag`].
    pub fn set_flag(&self, flag: &str) {
        self.shared
            .condition_flags
            .borrow_mut()
            .insert(flag.to_string(), true);
    }

    /// Set condition flag `flag` to `false`.
    pub fn clear_flag(&self, flag: &str) {
        self.shared
            .condition_flags
            .borrow_mut()
            .insert(flag.to_string(), false);
    }

    /// Return `true` if condition flag `flag` has been set.
    pub fn flag_set(&self, flag: &str) -> bool {
        self.shared
            .condition_flags
            .borrow()
            .get(flag)
            .copied()
            .unwrap_or(false)
    }

    /// Clear all condition flags.
    pub fn clear_all_flags(&self) {
        self.shared.condition_flags.borrow_mut().clear();
    }

    // Dynamic variable operations

    /// Check if a variable is marked as dynamic (special variable)
    pub fn is_dynamic(&self, name: &str) -> bool {
        self.shared.dynamic_vars.borrow().contains(name)
    }

    /// Mark a variable as dynamic (global registration)
    pub fn mark_dynamic(&self, name: String) {
        // Set is_dynamic on the interned symbol so resolve() can use a plain
        // bool field read instead of a HashSet probe on the hot path.
        let sym = self.shared.symbols.borrow_mut().intern(&name);
        sym.borrow_mut().is_dynamic = true;
        self.shared.dynamic_vars.borrow_mut().insert(name);
        // Flip the fast-path flag so future lookups take the dynamic-aware path.
        self.shared.has_dynamic.set(true);
    }

    /// Get variable value, handling both dynamic and lexical scoping.
    /// For dynamic variables, this searches the dynamic parent chain (caller's env).
    /// For lexical variables, this uses the standard get() method (parent chain).
    pub fn get_var(&self, name: &str) -> Option<LispVal> {
        // Fast path: if nothing has ever been marked dynamic, every variable is
        // lexical — skip the dynamic-set membership probe and go straight to the
        // lexical chain walk. This is the overwhelmingly common case and runs on
        // every variable reference, so the saved HashSet borrow + string hash
        // matters in tight loops.
        if !self.shared.has_dynamic.get() {
            return self.get(name);
        }
        if self.is_dynamic(name) {
            // Shallow binding: the symbol cell holds the current dynamic value — O(1).
            self.shared
                .symbols
                .borrow()
                .get_symbol(name)
                .and_then(|s| s.borrow().value.clone())
        } else {
            // Lexical lookup: walk the lexical parent chain
            self.get(name)
        }
    }

    // Capability / feature operations.
    // Features are shared across the whole environment chain and default off.

    /// Enable a capability (e.g. "SHELL"). Names are case-normalized to uppercase.
    pub fn enable_feature(&self, name: &str) {
        self.shared
            .features
            .borrow_mut()
            .insert(name.to_uppercase());
    }

    /// Disable a capability.
    pub fn disable_feature(&self, name: &str) {
        self.shared
            .features
            .borrow_mut()
            .remove(&name.to_uppercase());
    }

    /// Check whether a capability is enabled.
    pub fn feature_enabled(&self, name: &str) -> bool {
        self.shared.features.borrow().contains(&name.to_uppercase())
    }

    /// List enabled capabilities.
    pub fn features_list(&self) -> Vec<String> {
        self.shared.features.borrow().iter().cloned().collect()
    }

    // Reader depth limit (issue #270).
    // Shared across the whole environment chain, like features.

    /// The maximum s-expression nesting depth the evaluator-facing reader
    /// entry points (`load_file`, `eval_str`/`eval_all`, `read-from-string`)
    /// will parse with this environment. Defaults to
    /// [`crate::reader::DEFAULT_READER_DEPTH`] (512);
    /// [`Environment::with_stdlib`] raises it to 50,000 for use on the
    /// 512 MiB [`crate::with_large_stack`] thread.
    pub fn reader_depth_limit(&self) -> usize {
        self.shared.reader_depth_limit.get()
    }

    /// Set the maximum reader nesting depth for this environment chain.
    ///
    /// Size it to the stack the interpreter runs on: parsing costs roughly
    /// 6.4 KB of native stack per nesting level (measured, debug build), so
    /// keep `limit * 6.4 KB` at no more than about half the available stack.
    /// Embedders running on small threads (< 4 MiB) should *lower* this.
    pub fn set_reader_depth_limit(&self, limit: usize) {
        self.shared.reader_depth_limit.set(limit);
    }

    // Module registry (issue #256): the embedder half of `require`. Lisp's
    // half lives in `lib/06-require.lisp`; see that file's header for the
    // full resolution order and load-once semantics.

    /// Register `source` as `name`'s module source for `(require 'name)` in
    /// this environment chain, without evaluating it. Takes priority over any
    /// embedded module of the same name (host-registered is `require`'s first
    /// resolution tier). `name` is case-normalized to uppercase, matching
    /// Lisp symbol convention. Shared across the whole environment chain,
    /// like [`Environment::enable_feature`].
    pub fn register_module(&self, name: &str, source: &str) {
        self.shared
            .module_sources
            .borrow_mut()
            .insert(name.to_uppercase(), source.to_string());
    }

    /// The host-registered source for `name` (already uppercased), if any.
    /// Used by the `$MODULE-SOURCE-LOOKUP` builtin; embedders normally don't
    /// need this directly (registering is one-way — Lisp discovers sources
    /// through `require`).
    pub fn registered_module_source(&self, name: &str) -> Option<String> {
        self.shared.module_sources.borrow().get(name).cloned()
    }

    /// Add a directory to `require`'s disk search path list (its third,
    /// `READ-FS`-gated, resolution tier). Rust-only to mutate — Lisp can only
    /// read the list via `$MODULE-SEARCH-PATHS` — so a host constrains disk
    /// module resolution without exposing that authority to sandboxed Lisp
    /// code. Empty by default: no implicit filesystem module directory.
    pub fn add_module_search_path<P: Into<PathBuf>>(&self, path: P) {
        self.shared
            .module_search_paths
            .borrow_mut()
            .push(path.into());
    }

    /// Remove every configured disk module search path.
    pub fn clear_module_search_paths(&self) {
        self.shared.module_search_paths.borrow_mut().clear();
    }

    /// The currently configured disk module search paths, in the order they
    /// were added.
    pub fn module_search_paths(&self) -> Vec<PathBuf> {
        self.shared.module_search_paths.borrow().clone()
    }

    // Networking policy hook (issue #258, epic #253): the embedder half of
    // scoped network grants, complementing the coarse-grained NET-DNS/
    // NET-CONNECT/NET-LISTEN capabilities the same way `enable_feature` does
    // for filesystem access. Shared across the whole environment chain, like
    // `register_module`.

    /// Install a policy callback consulted before every DNS resolution,
    /// outbound connection, or bind/listen (`src/evaluator/builtins_net.rs`).
    /// Called with the operation, the caller-supplied host/address string
    /// (not yet DNS-resolved), and the port; return `true` to allow, `false`
    /// to deny with a structured `:policy-denied` error.
    ///
    /// This is *in addition to* the coarse capability check, not a
    /// replacement for it -- both must pass. Lets a host scope a broad grant
    /// (e.g. NET-CONNECT for an HTTP client) to specific destinations so it
    /// is not unrestricted SSRF authority. Rust-only to install: Lisp code
    /// cannot install, replace, or inspect the policy.
    ///
    /// # Example
    /// ```rust,ignore
    /// use lamedh::NetOperation;
    /// env.set_net_policy(|op, host, port| {
    ///     !(op == NetOperation::Connect && host == "169.254.169.254")
    /// });
    /// ```
    pub fn set_net_policy<F>(&self, policy: F)
    where
        F: Fn(NetOperation, &str, u16) -> bool + 'static,
    {
        *self.shared.net_policy.borrow_mut() = Some(NetPolicy(Shared::new(policy)));
    }

    /// Remove any installed policy callback, reverting to allow-all-when-
    /// capability-granted.
    pub fn clear_net_policy(&self) {
        *self.shared.net_policy.borrow_mut() = None;
    }

    /// Consult the installed policy (if any) for `op` against `host`/`port`.
    /// `true` (allow) when no policy is installed -- the documented default.
    pub(crate) fn check_net_policy(&self, op: NetOperation, host: &str, port: u16) -> bool {
        match &*self.shared.net_policy.borrow() {
            Some(p) => (p.0)(op, host, port),
            None => true,
        }
    }

    // OS policy hook (issue #260, epic #253): the embedder half of scoped
    // OS-integration grants, complementing the coarse-grained OS-PROCESS/
    // OS-SIGNAL capabilities exactly like `set_net_policy` does for
    // networking.

    /// Install a policy callback consulted before every process spawn or
    /// signal delivery (`src/evaluator/builtins_os.rs`). Called with a
    /// [`crate::OsOperation`] describing the attempted operation; return
    /// `true` to allow, `false` to deny with a structured `:policy-denied`
    /// error.
    ///
    /// This is *in addition to* the coarse capability check, not a
    /// replacement for it -- both must pass. Lets a host scope a broad grant
    /// (e.g. `OS-PROCESS` for a build tool) to specific executables/argv/cwd,
    /// or `OS-SIGNAL` to specific target PIDs. Rust-only to install: Lisp
    /// code cannot install, replace, or inspect the policy.
    ///
    /// # Example
    /// ```rust,ignore
    /// use lamedh::OsOperation;
    /// env.set_os_policy(|op| match op {
    ///     OsOperation::Spawn { program, .. } => program == "/usr/bin/true",
    ///     OsOperation::Signal { .. } => false,
    /// });
    /// ```
    pub fn set_os_policy<F>(&self, policy: F)
    where
        F: Fn(&crate::OsOperation) -> bool + 'static,
    {
        *self.shared.os_policy.borrow_mut() = Some(OsPolicy(Shared::new(policy)));
    }

    /// Remove any installed OS policy callback, reverting to allow-all-when-
    /// capability-granted.
    pub fn clear_os_policy(&self) {
        *self.shared.os_policy.borrow_mut() = None;
    }

    /// Consult the installed OS policy (if any) for `op`. `true` (allow) when
    /// no policy is installed -- the documented default.
    pub(crate) fn check_os_policy(&self, op: &crate::OsOperation) -> bool {
        match &*self.shared.os_policy.borrow() {
            Some(p) => (p.0)(op),
            None => true,
        }
    }

    // TLS insecure-connect host opt-in (issue #365, epic #253). Only
    // compiled with the `net-tls` feature -- see `SharedState::allow_insecure_tls`'s
    // doc comment for why this exists as a *second*, independent, Rust-only
    // gate rather than folding into `set_net_policy`.

    /// Allow (`true`) or continue denying (`false`, the default) Lisp's
    /// `tls:connect-insecure!` -- the one explicitly-named API that skips
    /// certificate verification. With this left at its default, calling
    /// `tls:connect-insecure!` always signals a structured
    /// `:category :policy-denied` error, no matter what Lisp code does; only
    /// the host embedder can widen this gate.
    ///
    /// # Example
    /// ```rust,ignore
    /// // A test harness that trusts a throwaway self-signed CA would
    /// // normally pass it as an extra root instead -- this flag is for
    /// // the rarer case of truly skipping verification (e.g. a
    /// // developer-only debug tool).
    /// env.set_allow_insecure_tls(true);
    /// ```
    #[cfg(feature = "net-tls")]
    pub fn set_allow_insecure_tls(&self, allow: bool) {
        self.shared.allow_insecure_tls.set(allow);
    }

    /// Whether the host has opted in to `tls:connect-insecure!`. `false`
    /// (deny) unless [`Environment::set_allow_insecure_tls`] was called.
    #[cfg(feature = "net-tls")]
    pub(crate) fn allow_insecure_tls(&self) -> bool {
        self.shared.allow_insecure_tls.get()
    }

    /// Walk `env`'s lexical parent chain to its root (the frame whose
    /// bindings live in the shared per-symbol value cells rather than a
    /// per-frame map). `require` (issue #256) always evaluates a required
    /// module's forms at the root, via this, so `defun`/`def` inside it
    /// become ordinary global bindings regardless of the lexical frame
    /// `require` itself happens to be called from — mirroring what already
    /// happens for `with_stdlib`'s and top-level `load_file`'s loads, both of
    /// which are always invoked with `env` already at the root.
    pub fn root_of(env: &Shared<Environment>) -> Shared<Environment> {
        let mut current = env.clone();
        while let Some(parent) = current.parent.clone() {
            current = parent;
        }
        current
    }

    /// Deep-copy an entire interpreter world, returning an isomorphic,
    /// fully isolated copy of `proto` (typically a root environment).
    ///
    /// Everything reachable from `proto` is duplicated with fresh
    /// allocations: the symbol table (fresh `Rc<RefCell<Symbol>>` cells with
    /// identical names, ids, flags, plists, and global value cells),
    /// every environment frame, cons cell, closure (lambda/fexpr/macro/vau,
    /// including pre-compiled [`crate::Code`] bodies), hash table, array,
    /// struct, and error value. Sharing and identity are preserved
    /// *isomorphically*: two references to one object in the prototype map
    /// to two references to one (new) object in the copy, so `EQ` results
    /// inside the copied world are exactly those of the prototype world.
    /// Because the two worlds share no mutable cell, no mutation in one can
    /// ever be observed in the other.
    ///
    /// A few values are deliberately shared rather than copied, because they
    /// are immutable or host-owned: builtin tags, `Native` host closures,
    /// `Extension` values, slot-routing tables, net/OS policy callbacks, and
    /// the immutable definitions behind the typed registry's declaration
    /// plane (see `Jit::clone_for_fork`).
    ///
    /// Returns `Err` when the world contains state that cannot be soundly
    /// duplicated: live host handles (ports, sockets, child processes,
    /// channels) or registered typed (`defun-typed`) functions. A freshly
    /// loaded stdlib prototype contains none of these, which is what
    /// [`Environment::with_stdlib`] relies on; callers must fall back to a
    /// full fresh load on `Err`.
    pub fn fork_world(proto: &Shared<Environment>) -> Result<Shared<Environment>, String> {
        worldfork::WorldCopier::new().copy_env(proto)
    }
}

/// Memoized whole-world deep copy (the mechanism behind
/// [`Environment::fork_world`]). Lives in a child module of `environment`
/// so it can reach `SharedState`'s and `SymbolTable`'s private fields.
mod worldfork {
    use super::*;
    use crate::{Code, ErrorObj, Fexpr, Lambda, Macro, StructObj, TypedArrayObj, Vau};

    /// Memo map keyed by prototype allocation address, using the crate's
    /// fast integer hasher — the copier does one lookup+insert per copied
    /// allocation, so hashing is its hottest edge.
    type PtrMap<T> = HashMap<usize, T, BuildHasherDefault<FxHasher>>;

    /// Identity-preserving copier. Every memo table is keyed by the address
    /// of the *prototype* allocation; the prototype is kept alive by the
    /// caller for the whole copy, so addresses are stable and never reused
    /// while a `WorldCopier` is running.
    pub(super) struct WorldCopier {
        states: PtrMap<Shared<SharedState>>,
        symbols: PtrMap<Shared<SharedCell<Symbol>>>,
        envs: PtrMap<Shared<Environment>>,
        /// `Shared<LispVal>` cells (cons children). Also preserves structural
        /// sharing so a DAG never blows up into a tree.
        vals: PtrMap<Shared<LispVal>>,
        codes: PtrMap<Shared<Code>>,
        tables: PtrMap<Shared<SharedCell<HashMap<LispVal, LispVal>>>>,
        arrays: PtrMap<Shared<SharedCell<Vec<LispVal>>>>,
        typed_arrays: PtrMap<Shared<TypedArrayObj>>,
    }

    impl WorldCopier {
        pub(super) fn new() -> Self {
            // Capacities sized for the stdlib prototype (measured: ~2.5k
            // symbols, ~88k shared value cells, ~15k code nodes) so a
            // typical fork never rehashes its two big memo tables.
            WorldCopier {
                states: PtrMap::default(),
                symbols: PtrMap::with_capacity_and_hasher(4096, Default::default()),
                envs: PtrMap::default(),
                vals: PtrMap::with_capacity_and_hasher(1 << 17, Default::default()),
                codes: PtrMap::with_capacity_and_hasher(1 << 15, Default::default()),
                arrays: PtrMap::default(),
                tables: PtrMap::default(),
                typed_arrays: PtrMap::default(),
            }
        }

        /// Copy the globally shared interpreter state (symbol table, flags,
        /// dynamic set, features, typed registry, module registry, policies).
        fn copy_state(&mut self, old: &Shared<SharedState>) -> Result<Shared<SharedState>, String> {
            let key = Shared::as_ptr(old) as usize;
            if let Some(hit) = self.states.get(&key) {
                return Ok(hit.clone());
            }
            let jit = old.jit.borrow().clone_for_fork().ok_or_else(|| {
                "fork_world: typed (defun-typed) functions are registered; \
                     a world with compiled typed editions cannot be forked"
                    .to_string()
            })?;
            let new_state = Shared::new(SharedState {
                symbols: SharedCell::new(SymbolTable::new()),
                condition_flags: SharedCell::new(old.condition_flags.borrow().clone()),
                dynamic_vars: SharedCell::new(old.dynamic_vars.borrow().clone()),
                has_dynamic: Cell::new(old.has_dynamic.get()),
                features: SharedCell::new(old.features.borrow().clone()),
                jit: SharedCell::new(jit),
                reader_depth_limit: Cell::new(old.reader_depth_limit.get()),
                module_sources: SharedCell::new(old.module_sources.borrow().clone()),
                module_search_paths: SharedCell::new(old.module_search_paths.borrow().clone()),
                net_policy: SharedCell::new(old.net_policy.borrow().clone()),
                os_policy: SharedCell::new(old.os_policy.borrow().clone()),
                #[cfg(feature = "net-tls")]
                allow_insecure_tls: Cell::new(old.allow_insecure_tls.get()),
            });
            // Memoize BEFORE copying symbols: symbol values reach closures,
            // closures reach environments, and environments reach back to
            // this state.
            self.states.insert(key, new_state.clone());

            // Rebuild the symbol table with identical ids and counters. All
            // symbols (interned + gensym) live in by_id, in id order.
            let (old_by_id, old_names, gensym_counter, next_id) = {
                let t = old.symbols.borrow();
                (
                    t.by_id.clone(),
                    t.symbols
                        .iter()
                        .map(|(k, v)| (k.clone(), Shared::as_ptr(v) as usize))
                        .collect::<Vec<_>>(),
                    t.gensym_counter,
                    t.next_id,
                )
            };
            let mut new_by_id = Vec::with_capacity(old_by_id.len());
            for sym in &old_by_id {
                new_by_id.push(self.copy_symbol(sym)?);
            }
            let mut new_names = HashMap::with_capacity(old_names.len());
            for (name, ptr) in old_names {
                let mapped = self
                    .symbols
                    .get(&ptr)
                    .expect("interned symbol must be in by_id")
                    .clone();
                new_names.insert(name, mapped);
            }
            *new_state.symbols.borrow_mut() = SymbolTable {
                symbols: new_names,
                gensym_counter,
                next_id,
                by_id: new_by_id,
            };
            Ok(new_state)
        }

        /// Copy one symbol cell: same name/id/flags, deep-copied plist and
        /// global value cell.
        fn copy_symbol(
            &mut self,
            old: &Shared<SharedCell<Symbol>>,
        ) -> Result<Shared<SharedCell<Symbol>>, String> {
            let key = Shared::as_ptr(old) as usize;
            if let Some(hit) = self.symbols.get(&key) {
                return Ok(hit.clone());
            }
            let (name, id, is_keyword, is_dynamic, special_form) = {
                let s = old.borrow();
                (
                    s.name.clone(),
                    s.id,
                    s.is_keyword,
                    s.is_dynamic,
                    s.special_form,
                )
            };
            let new_sym = Shared::new(SharedCell::new(Symbol {
                name,
                plist: HashMap::new(),
                value: None,
                id,
                is_keyword,
                is_dynamic,
                special_form,
            }));
            // Memoize BEFORE filling value/plist: the value can contain a
            // closure whose body references this very symbol.
            self.symbols.insert(key, new_sym.clone());
            let (old_value, old_plist) = {
                let s = old.borrow();
                (s.value.clone(), s.plist.clone())
            };
            let value = match old_value {
                Some(v) => Some(self.copy_val(&v)?),
                None => None,
            };
            let mut plist = HashMap::with_capacity(old_plist.len());
            for (k, v) in old_plist {
                plist.insert(k, self.copy_val(&v)?);
            }
            {
                let mut s = new_sym.borrow_mut();
                s.value = value;
                s.plist = plist;
            }
            Ok(new_sym)
        }

        /// Copy an environment frame (and, transitively, its parent chain and
        /// shared state).
        pub(super) fn copy_env(
            &mut self,
            old: &Shared<Environment>,
        ) -> Result<Shared<Environment>, String> {
            let key = Shared::as_ptr(old) as usize;
            if let Some(hit) = self.envs.get(&key) {
                return Ok(hit.clone());
            }
            let shared = self.copy_state(&old.shared)?;
            // copy_state can recursively copy THIS env (a symbol's value may
            // close over it); re-check the memo before allocating a second copy.
            if let Some(hit) = self.envs.get(&key) {
                return Ok(hit.clone());
            }
            let parent = match &old.parent {
                Some(p) => Some(self.copy_env(p)?),
                None => None,
            };
            if let Some(hit) = self.envs.get(&key) {
                return Ok(hit.clone());
            }
            let new_env = Shared::new(Environment {
                parent,
                bindings: SharedCell::new(BindingMap::default()),
                // Slot-routing tables are immutable id lists — safe to share.
                routing: old.routing.clone(),
                slots: SharedCell::new(Vec::new()),
                shared,
            });
            self.envs.insert(key, new_env.clone());
            let old_bindings: Vec<(u32, LispVal)> = old
                .bindings
                .borrow()
                .iter()
                .map(|(k, v)| (*k, v.clone()))
                .collect();
            let old_slots: Vec<LispVal> = old.slots.borrow().clone();
            {
                let mut b = new_env.bindings.borrow_mut();
                for (id, v) in old_bindings {
                    let copied = self.copy_val(&v)?;
                    b.insert(id, copied);
                }
            }
            {
                let mut s = new_env.slots.borrow_mut();
                for v in old_slots {
                    let copied = self.copy_val(&v)?;
                    s.push(copied);
                }
            }
            Ok(new_env)
        }

        /// Copy a shared value cell (a cons child). Cons spines are walked
        /// iteratively so list length never becomes native recursion depth.
        fn copy_shared_val(&mut self, old: &Shared<LispVal>) -> Result<Shared<LispVal>, String> {
            // World-free immutable scalars: share the cell itself. These
            // contain no symbols, environments, or mutable state, and every
            // equality on them (including the EQ builtin) is by value, so
            // sharing is unobservable — and they are the majority of cons
            // leaves, so this saves both allocations and memo traffic.
            match &**old {
                LispVal::Number(_)
                | LispVal::Char(_)
                | LispVal::Float(_)
                | LispVal::String(_)
                | LispVal::Builtin(_)
                | LispVal::Nil => return Ok(old.clone()),
                _ => {}
            }
            let key = Shared::as_ptr(old) as usize;
            if let Some(hit) = self.vals.get(&key) {
                return Ok(hit.clone());
            }
            if let LispVal::Cons { .. } = &**old {
                // Walk the cdr spine, copying cars (tree recursion is bounded
                // by expression nesting depth, not list length).
                let mut spine: Vec<(usize, Shared<LispVal>)> = Vec::new();
                let mut cur: Shared<LispVal> = old.clone();
                let tail: Shared<LispVal> = loop {
                    let LispVal::Cons { car, cdr } = &*cur else {
                        unreachable!("spine walk only enters Cons cells")
                    };
                    let copied_car = self.copy_shared_val(car)?;
                    spine.push((Shared::as_ptr(&cur) as usize, copied_car));
                    let next = cdr.clone();
                    if let Some(hit) = self.vals.get(&(Shared::as_ptr(&next) as usize)) {
                        break hit.clone();
                    }
                    if matches!(&*next, LispVal::Cons { .. }) {
                        cur = next;
                    } else {
                        // Non-cons tail: scalar sharing and memoization are
                        // both handled by the entry logic above.
                        break self.copy_shared_val(&next)?;
                    }
                };
                let mut t = tail;
                for (cell_key, car) in spine.into_iter().rev() {
                    let cell = Shared::new(LispVal::Cons { car, cdr: t });
                    self.vals.insert(cell_key, cell.clone());
                    t = cell;
                }
                return Ok(t);
            }
            let copied = Shared::new(self.copy_val(old)?);
            self.vals.insert(key, copied.clone());
            Ok(copied)
        }

        /// Copy one value. Exhaustive over `LispVal`: adding a variant is a
        /// compile error here until its fork semantics are decided.
        fn copy_val(&mut self, old: &LispVal) -> Result<LispVal, String> {
            Ok(match old {
                LispVal::Symbol(s) => LispVal::Symbol(self.copy_symbol(s)?),
                LispVal::Number(n) => LispVal::Number(*n),
                LispVal::Char(c) => LispVal::Char(*c),
                LispVal::Float(f) => LispVal::Float(*f),
                LispVal::String(s) => LispVal::String(s.clone()),
                LispVal::Builtin(b) => LispVal::Builtin(b.clone()),
                LispVal::Nil => LispVal::Nil,
                LispVal::Cons { car, cdr } => LispVal::Cons {
                    car: self.copy_shared_val(car)?,
                    cdr: self.copy_shared_val(cdr)?,
                },
                LispVal::Lambda(l) => LispVal::Lambda(Box::new(Lambda {
                    params: l.params.clone(),
                    rest_param: l.rest_param.clone(),
                    body: Box::new(self.copy_val(&l.body)?),
                    env: self.copy_env(&l.env)?,
                    param_ids: l.param_ids.clone(),
                    rest_param_id: l.rest_param_id,
                    param_routing: l.param_routing.clone(),
                    compiled: match &l.compiled {
                        Some(c) => Some(self.copy_code(c)?),
                        None => None,
                    },
                })),
                LispVal::Fexpr(f) => LispVal::Fexpr(Box::new(Fexpr {
                    params: f.params.clone(),
                    body: Box::new(self.copy_val(&f.body)?),
                    env: self.copy_env(&f.env)?,
                    param_ids: f.param_ids.clone(),
                })),
                LispVal::Macro(m) => LispVal::Macro(Box::new(Macro {
                    params: m.params.clone(),
                    rest_param: m.rest_param.clone(),
                    body: Box::new(self.copy_val(&m.body)?),
                    env: self.copy_env(&m.env)?,
                    param_ids: m.param_ids.clone(),
                    rest_param_id: m.rest_param_id,
                })),
                LispVal::Vau(v) => LispVal::Vau(Box::new(Vau {
                    operands_param: v.operands_param.clone(),
                    env_param: v.env_param.clone(),
                    body: Box::new(self.copy_val(&v.body)?),
                    env: self.copy_env(&v.env)?,
                    operands_param_id: v.operands_param_id,
                    env_param_id: v.env_param_id,
                })),
                LispVal::HashTable(h) => {
                    let key = Shared::as_ptr(h) as usize;
                    if let Some(hit) = self.tables.get(&key) {
                        LispVal::HashTable(hit.clone())
                    } else {
                        let new_table = Shared::new(SharedCell::new(HashMap::new()));
                        // Memoize first: a table can (via its values) reach itself.
                        self.tables.insert(key, new_table.clone());
                        let entries: Vec<(LispVal, LispVal)> = h
                            .borrow()
                            .iter()
                            .map(|(k, v)| (k.clone(), v.clone()))
                            .collect();
                        for (k, v) in entries {
                            let ck = self.copy_val(&k)?;
                            let cv = self.copy_val(&v)?;
                            new_table.borrow_mut().insert(ck, cv);
                        }
                        LispVal::HashTable(new_table)
                    }
                }
                // Host-registered closures are opaque and host-owned; they
                // receive the calling environment as an argument, so sharing
                // the handle across worlds is sound.
                LispVal::Native(f) => LispVal::Native(f.clone()),
                LispVal::Environment(e) => LispVal::Environment(self.copy_env(e)?),
                LispVal::Array(a) => {
                    let key = Shared::as_ptr(a) as usize;
                    if let Some(hit) = self.arrays.get(&key) {
                        LispVal::Array(hit.clone())
                    } else {
                        let new_arr = Shared::new(SharedCell::new(Vec::new()));
                        self.arrays.insert(key, new_arr.clone());
                        let elems: Vec<LispVal> = a.borrow().clone();
                        for e in elems {
                            let copied = self.copy_val(&e)?;
                            new_arr.borrow_mut().push(copied);
                        }
                        LispVal::Array(new_arr)
                    }
                }
                LispVal::TypedArray(a) => {
                    let key = Shared::as_ptr(a) as usize;
                    if let Some(hit) = self.typed_arrays.get(&key) {
                        LispVal::TypedArray(hit.clone())
                    } else {
                        let new_arr = Shared::new(TypedArrayObj {
                            elem: a.elem,
                            data: SharedCell::new(a.data.borrow().clone()),
                        });
                        self.typed_arrays.insert(key, new_arr.clone());
                        LispVal::TypedArray(new_arr)
                    }
                }
                LispVal::Struct(s) => {
                    let mut fields = Vec::with_capacity(s.fields.len());
                    for f in &s.fields {
                        fields.push(self.copy_val(f)?);
                    }
                    LispVal::Struct(Shared::new(StructObj {
                        type_name: s.type_name.clone(),
                        fields,
                    }))
                }
                // Host extension values compare by identity and are
                // host-owned; share the handle (none exist in a freshly
                // loaded stdlib world).
                LispVal::Extension(e) => LispVal::Extension(e.clone()),
                LispVal::Error(e) => LispVal::Error(Shared::new(ErrorObj {
                    message: e.message.clone(),
                    data: self.copy_val(&e.data)?,
                })),
                LispVal::Port(_) => {
                    return Err("fork_world: live PORT handles cannot be forked".to_string());
                }
                LispVal::NetHandle(_) => {
                    return Err("fork_world: live network handles cannot be forked".to_string());
                }
                LispVal::OsChild(_) => {
                    return Err(
                        "fork_world: live child-process handles cannot be forked".to_string()
                    );
                }
                #[cfg(feature = "concurrency")]
                LispVal::Channel(_) => {
                    return Err("fork_world: live channels cannot be forked".to_string());
                }
            })
        }

        /// Copy a pre-compiled body. Symbol handles inside the tree are
        /// remapped to the fork's cells; ids and routing tables are already
        /// world-independent and are shared/copied verbatim.
        fn copy_code(&mut self, old: &Shared<Code>) -> Result<Shared<Code>, String> {
            let key = Shared::as_ptr(old) as usize;
            if let Some(hit) = self.codes.get(&key) {
                return Ok(hit.clone());
            }
            let copied = match &**old {
                Code::Const(v) => Code::Const(self.copy_val(v)?),
                Code::Var(s) => Code::Var(self.copy_symbol(s)?),
                Code::LocalGet { depth, slot, sym } => Code::LocalGet {
                    depth: *depth,
                    slot: *slot,
                    sym: self.copy_symbol(sym)?,
                },
                Code::If(c, t, e) => {
                    Code::If(self.copy_code(c)?, self.copy_code(t)?, self.copy_code(e)?)
                }
                Code::Seq(items) => {
                    let mut out = Vec::with_capacity(items.len());
                    for i in items {
                        out.push(self.copy_code(i)?);
                    }
                    Code::Seq(out)
                }
                Code::Let {
                    bindings,
                    routing,
                    body,
                } => {
                    let mut bs = Vec::with_capacity(bindings.len());
                    for (id, code) in bindings {
                        bs.push((*id, self.copy_code(code)?));
                    }
                    Code::Let {
                        bindings: bs,
                        routing: routing.clone(),
                        body: self.copy_code(body)?,
                    }
                }
                Code::Call {
                    callee,
                    args,
                    original,
                } => {
                    let mut cargs = Vec::with_capacity(args.len());
                    for a in args {
                        cargs.push(self.copy_code(a)?);
                    }
                    Code::Call {
                        callee: self.copy_code(callee)?,
                        args: cargs,
                        original: self.copy_val(original)?,
                    }
                }
                Code::SetVar(pairs) => {
                    let mut out = Vec::with_capacity(pairs.len());
                    for (sym, code) in pairs {
                        out.push((self.copy_symbol(sym)?, self.copy_code(code)?));
                    }
                    Code::SetVar(out)
                }
                Code::UnwindProtect { body, cleanups } => {
                    let mut cl = Vec::with_capacity(cleanups.len());
                    for c in cleanups {
                        cl.push(self.copy_code(c)?);
                    }
                    Code::UnwindProtect {
                        body: self.copy_code(body)?,
                        cleanups: cl,
                    }
                }
                Code::While { cond, body } => {
                    let mut b = Vec::with_capacity(body.len());
                    for c in body {
                        b.push(self.copy_code(c)?);
                    }
                    Code::While {
                        cond: self.copy_code(cond)?,
                        body: b,
                    }
                }
                Code::For {
                    var_id,
                    start,
                    end,
                    step,
                    body,
                } => {
                    let mut b = Vec::with_capacity(body.len());
                    for c in body {
                        b.push(self.copy_code(c)?);
                    }
                    Code::For {
                        var_id: *var_id,
                        start: self.copy_code(start)?,
                        end: self.copy_code(end)?,
                        step: match step {
                            Some(s) => Some(self.copy_code(s)?),
                            None => None,
                        },
                        body: b,
                    }
                }
                Code::MakeLambda {
                    params,
                    body_forms,
                    compiled_body,
                } => {
                    let mut bf = Vec::with_capacity(body_forms.len());
                    for f in body_forms {
                        bf.push(self.copy_val(f)?);
                    }
                    Code::MakeLambda {
                        params: self.copy_val(params)?,
                        body_forms: bf,
                        compiled_body: self.copy_code(compiled_body)?,
                    }
                }
                Code::Interp(v) => Code::Interp(self.copy_val(v)?),
            };
            let copied = Shared::new(copied);
            self.codes.insert(key, copied.clone());
            Ok(copied)
        }
    }
}
