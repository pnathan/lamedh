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
//! Dangerous operations (`SHELL`, `READ-FS`, `CREATE-FS`, `TEMP-FS`, `IO`) are gated behind feature
//! flags that are all **off by default**.  Call [`Environment::enable_feature`]
//! to opt in.  Because `SharedState` is shared across the whole chain, a
//! feature enabled anywhere is visible everywhere.

use crate::{BuiltinFunc, LispError, LispVal, Shared, SharedCell, SpecialForm, Symbol};
use std::cell::Cell;
use std::collections::{HashMap, HashSet};
use std::hash::{BuildHasherDefault, Hasher};

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

    /// Look up a symbol by its numeric id (the reverse of [`intern`]).
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
            "DEFSTRUCT" => Some(SpecialForm::Defstruct),
            "DEFUN-TYPED" => Some(SpecialForm::DefunTyped),
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
            shared: Shared::new(SharedState::new()),
        }
    }

    /// Create a new child environment for lexical scoping.
    pub fn new_child(parent: &Shared<Environment>) -> Shared<Environment> {
        Shared::new(Environment {
            parent: Some(parent.clone()),
            bindings: SharedCell::new(BindingMap::default()),
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
    /// All capability flags (`SHELL`, `READ-FS`, `CREATE-FS`, `TEMP-FS`, `IO`) are disabled by default.
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
        env.set(
            "DELETE-KEY".to_string(),
            LispVal::Builtin(BuiltinFunc::DeleteKey),
        );
        env.set(
            "DELETE-KEY-BANG".to_string(),
            LispVal::Builtin(BuiltinFunc::DeleteKey),
        );
        env.set(
            "CURRENT-ENVIRONMENT".to_string(),
            LispVal::Builtin(BuiltinFunc::CurrentEnvironment),
        );
        env.set("KEYS".to_string(), LispVal::Builtin(BuiltinFunc::Keys));
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
        env.set("LOGOR".to_string(), LispVal::Builtin(BuiltinFunc::Logor));
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
            "ARRAY-LENGTH".to_string(),
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
        // ARRAY/FETCH/STORE/ARRAY-LENGTH, with MAKE-ARRAY/ARRAY-FETCH/ARRAY-STORE
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
            "ARRAY-FETCH".to_string(),
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
            "ARRAY-STORE".to_string(),
            LispVal::Builtin(BuiltinFunc::ArrayStore),
        );
        env.set(
            "ASET".to_string(),
            LispVal::Builtin(BuiltinFunc::ArrayStore),
        );
        env.set(
            "ARRAY-LENGTH".to_string(),
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

        // Sorting (stable, non-destructive; takes a comparator predicate)
        env.set("SORT".to_string(), LispVal::Builtin(BuiltinFunc::Sort));

        // Math library (implemented in Rust: f64 transcendentals, float->int
        // rounding, i64 integer math)
        env.set("SQRT".to_string(), LispVal::Builtin(BuiltinFunc::Sqrt));
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
            "STRING-LENGTH".to_string(),
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
    /// Panics if the embedded stdlib fails to parse or evaluate (which would be
    /// a compile-time bug, not a runtime condition).
    pub fn with_stdlib() -> Shared<Environment> {
        let env = Self::new_with_builtins();
        crate::load_stdlib(&env).expect("embedded stdlib should always load cleanly");
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

    /// Generate a fresh uninterned symbol.  Equivalent to `(gensym)` in Lisp.
    pub fn gensym(&self) -> Shared<SharedCell<Symbol>> {
        self.shared.symbols.borrow_mut().gensym()
    }

    pub fn all_symbols(&self) -> Vec<Shared<SharedCell<Symbol>>> {
        self.shared.symbols.borrow().all_symbols()
    }

    /// Return `true` if `name` is bound anywhere in the lexical chain.
    pub fn is_bound(&self, name: &str) -> bool {
        self.get(name).is_some()
    }

    /// `true` if this is the root (global) frame, whose variable storage is the
    /// per-symbol value cells rather than a `HashMap`.
    #[inline]
    fn is_root(&self) -> bool {
        self.parent.is_none()
    }

    /// Write a global binding into the symbol value cell, by name.
    fn global_set(&self, name: &str, val: LispVal) {
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
        self.bindings.borrow_mut().insert(id, val);
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
            if let Some(val) = frame.bindings.borrow().get(&id) {
                return Some(val.clone());
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
    pub fn jit_check_untyped(
        &self,
        name: &str,
        params: &[String],
        body: &[LispVal],
    ) -> Result<String, String> {
        self.shared
            .jit
            .borrow_mut()
            .check_untyped(name, params, body)
    }

    /// Type-check a single expression and return its inferred type as a string.
    pub fn jit_check_expr(&self, expr: &LispVal) -> Result<String, String> {
        self.shared.jit.borrow_mut().check_expr(expr)
    }

    /// One-pass analyze: check, then compile if compileable (#162 stage 4).
    pub fn jit_analyze_untyped(
        &self,
        name: &str,
        params: &[String],
        body: &[LispVal],
    ) -> crate::jit::Analysis {
        self.shared
            .jit
            .borrow_mut()
            .analyze_untyped(name, params, body)
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
            if current_env.bindings.borrow().contains_key(&id) {
                current_env.bindings.borrow_mut().insert(id, val);
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
}
