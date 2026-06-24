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
//!   are looked up by walking `dynamic_parent` pointers — the actual call
//!   stack.  Use the `DEFDYNAMIC`/`DEFVAR` special forms or `*EARMUFF*`
//!   naming to signal dynamic intent.
//!
//! ## Capabilities
//!
//! Dangerous operations (`SHELL`, `FILE-IO`, `IO`) are gated behind feature
//! flags that are all **off by default**.  Call [`Environment::enable_feature`]
//! to opt in.  Because `SharedState` is shared across the whole chain, a
//! feature enabled anywhere is visible everywhere.

use crate::{BuiltinFunc, LispError, LispVal, Symbol};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

/// Global symbol table shared by all environments in an interpreter session.
///
/// Maintains a `HashMap<String, Rc<RefCell<Symbol>>>` so that every distinct
/// symbol name maps to exactly one heap allocation.  Pointer equality of the
/// `Rc` handles is the Lisp `EQ` predicate for symbols.
///
/// Uninterned symbols (created by [`SymbolTable::gensym`]) are *not* stored in
/// the table — they are guaranteed unique but not sharable by name.
#[derive(Debug, Clone, PartialEq)]
pub struct SymbolTable {
    symbols: HashMap<String, Rc<RefCell<Symbol>>>,
    gensym_counter: u64,
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
        }
    }

    /// Create a fresh uninterned symbol with a unique name (`G0000`, `G0001`, …).
    ///
    /// Uninterned symbols are *not* stored in the table, so two `gensym` calls
    /// always return distinct `Rc` pointers even if the names collide.
    pub fn gensym(&mut self) -> Rc<RefCell<Symbol>> {
        let name = format!("G{:04}", self.gensym_counter);
        self.gensym_counter += 1;
        // Create an uninterned symbol (not stored in the hash table)
        Rc::new(RefCell::new(Symbol {
            name,
            plist: HashMap::new(),
        }))
    }

    pub fn all_symbols(&self) -> Vec<Rc<RefCell<Symbol>>> {
        self.symbols.values().cloned().collect()
    }

    /// Return the interned `Rc` for `name`, creating a new `Symbol` if needed.
    ///
    /// The name must already be uppercased; callers are responsible for
    /// normalisation (the environment does this via [`Environment::intern_symbol`]).
    pub fn intern(&mut self, name: &str) -> Rc<RefCell<Symbol>> {
        if let Some(symbol) = self.symbols.get(name) {
            return symbol.clone();
        }

        let symbol = Rc::new(RefCell::new(Symbol {
            name: name.to_string(),
            plist: HashMap::new(),
        }));
        self.symbols.insert(name.to_string(), symbol.clone());
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
    symbols: RefCell<SymbolTable>,
    condition_flags: RefCell<HashMap<String, bool>>,
    /// Set of variable names that are marked as dynamic (special variables).
    dynamic_vars: RefCell<HashSet<String>>,
    /// Set of enabled capabilities/features (e.g. "SHELL"). Off by default; the
    /// host or a Lisp program must opt in. This is the foundation for
    /// sandboxing (see issue #64).
    features: RefCell<HashSet<String>>,
}

impl SharedState {
    fn new() -> Self {
        SharedState {
            symbols: RefCell::new(SymbolTable::new()),
            condition_flags: RefCell::new(HashMap::new()),
            dynamic_vars: RefCell::new(HashSet::new()),
            features: RefCell::new(HashSet::new()),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Environment {
    parent: Option<Rc<Environment>>,
    bindings: Rc<RefCell<HashMap<String, LispVal>>>,
    /// Globally-shared interpreter state (symbols, flags, dynamic vars,
    /// features). Shared across the whole environment chain via a single `Rc`.
    shared: Rc<SharedState>,
    /// Dynamic parent environment (caller's environment for dynamic scoping).
    /// This is used to look up dynamic variables from the call chain.
    dynamic_parent: Option<Rc<Environment>>,
}

impl PartialEq for Environment {
    fn eq(&self, other: &Self) -> bool {
        let parents_equal = match (&self.parent, &other.parent) {
            (Some(p1), Some(p2)) => Rc::ptr_eq(p1, p2),
            (None, None) => true,
            _ => false,
        };
        let dynamic_parents_equal = match (&self.dynamic_parent, &other.dynamic_parent) {
            (Some(p1), Some(p2)) => Rc::ptr_eq(p1, p2),
            (None, None) => true,
            _ => false,
        };
        parents_equal
            && dynamic_parents_equal
            && Rc::ptr_eq(&self.bindings, &other.bindings)
            && Rc::ptr_eq(&self.shared, &other.shared)
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
            bindings: Rc::new(RefCell::new(HashMap::new())),
            shared: Rc::new(SharedState::new()),
            dynamic_parent: None,
        }
    }

    /// Create a new child environment for lexical scoping.
    /// The child inherits the parent's dynamic_parent by default.
    pub fn new_child(parent: &Rc<Environment>) -> Rc<Environment> {
        Rc::new(Environment {
            parent: Some(parent.clone()),
            bindings: Rc::new(RefCell::new(HashMap::new())),
            shared: parent.shared.clone(),
            dynamic_parent: parent.dynamic_parent.clone(),
        })
    }

    /// Create a new child environment for function application with dynamic scoping.
    /// The lexical parent is `lexical_parent` (the captured closure environment),
    /// and the dynamic parent is `caller_env` (for dynamic variable lookup).
    pub fn new_child_with_dynamic(
        lexical_parent: &Rc<Environment>,
        caller_env: &Rc<Environment>,
    ) -> Rc<Environment> {
        Rc::new(Environment {
            parent: Some(lexical_parent.clone()),
            bindings: Rc::new(RefCell::new(HashMap::new())),
            shared: lexical_parent.shared.clone(),
            dynamic_parent: Some(caller_env.clone()),
        })
    }

    /// Create a root environment with all 100+ built-in primitives registered.
    ///
    /// This does **not** load the Lisp standard library (`defun`, `append`,
    /// etc.).  Use [`Environment::with_stdlib`] for a fully-featured environment.
    ///
    /// All capability flags (`SHELL`, `FILE-IO`, `IO`) are disabled by default.
    pub fn new_with_builtins() -> Rc<Environment> {
        let env = Rc::new(Environment::new());
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
        env.set("SET-BANG".to_string(), LispVal::Builtin(BuiltinFunc::Set));
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

        // Capabilities / features
        env.set(
            "ENABLE-FEATURE".to_string(),
            LispVal::Builtin(BuiltinFunc::EnableFeature),
        );
        env.set(
            "DISABLE-FEATURE".to_string(),
            LispVal::Builtin(BuiltinFunc::DisableFeature),
        );
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
        env.set("ARRAYP".to_string(), LispVal::Builtin(BuiltinFunc::Arrayp));
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
        // Note: PLUS/DIFFERENCE/TIMES/QUOTIENT/LESSP/GREATERP/REMAINDER
        // are registered above with the other Lisp 1.5 spec functions.

        env
    }

    /// Create a new environment with all builtins **and** the embedded standard
    /// library pre-loaded. This is the recommended entry point for host code
    /// that wants a fully-featured Lisp interpreter without shipping .lisp files.
    ///
    /// Panics if the embedded stdlib fails to parse or evaluate (which would be
    /// a compile-time bug, not a runtime condition).
    pub fn with_stdlib() -> Rc<Environment> {
        let env = Self::new_with_builtins();
        crate::load_stdlib(&env).expect("embedded stdlib should always load cleanly");
        env
    }

    /// Create a sandboxed environment with all builtins registered but all
    /// dangerous capabilities disabled.
    ///
    /// All potentially dangerous feature flags (`SHELL`, `FILE-IO`, `IO`) are
    /// off by default in every environment, so this is semantically equivalent
    /// to `new_with_builtins()`.  The explicit name communicates intent clearly
    /// to embedders: scripts loaded into this environment cannot access the
    /// filesystem, spawn subprocesses, or read from stdin unless the host
    /// explicitly calls `enable_feature`.
    ///
    /// # Example
    /// ```rust,ignore
    /// let env = Environment::new_sandboxed();
    /// // All of SHELL, FILE-IO, IO are disabled.
    /// assert!(!env.feature_enabled("SHELL"));
    /// assert!(!env.feature_enabled("FILE-IO"));
    /// assert!(!env.feature_enabled("IO"));
    /// ```
    pub fn new_sandboxed() -> Rc<Environment> {
        Self::new_with_builtins()
    }

    /// Intern `name` (uppercased) into the global symbol table.
    ///
    /// Returns the shared `Rc<RefCell<Symbol>>` for this name, creating a new
    /// entry if the name has not been seen before.
    pub fn intern_symbol(&self, name: &str) -> Rc<RefCell<Symbol>> {
        self.shared.symbols.borrow_mut().intern(name)
    }

    /// Generate a fresh uninterned symbol.  Equivalent to `(gensym)` in Lisp.
    pub fn gensym(&self) -> Rc<RefCell<Symbol>> {
        self.shared.symbols.borrow_mut().gensym()
    }

    pub fn all_symbols(&self) -> Vec<Rc<RefCell<Symbol>>> {
        self.shared.symbols.borrow().all_symbols()
    }

    /// Return `true` if `name` is bound anywhere in the lexical chain.
    pub fn is_bound(&self, name: &str) -> bool {
        self.get(name).is_some()
    }

    /// Lexical variable lookup.  Walks the `parent` chain; does **not** check
    /// the dynamic-parent chain.  Use [`Environment::get_var`] for the
    /// scoping-aware lookup that respects dynamic variables.
    pub fn get(&self, name: &str) -> Option<LispVal> {
        if let Some(val) = self.bindings.borrow().get(name) {
            return Some(val.clone());
        }
        if let Some(parent) = &self.parent {
            return parent.get(name);
        }
        None
    }

    /// Bind `name` to `val` in this environment frame (not in any parent).
    ///
    /// Use [`Environment::update`] to modify an existing binding that may live
    /// in a parent frame.
    pub fn set(&self, name: String, val: LispVal) {
        self.bindings.borrow_mut().insert(name, val);
    }

    /// Register a host Rust closure as a callable Lisp function named `name`.
    ///
    /// The name is uppercased to match Lisp convention. After registration,
    /// `(NAME arg1 arg2 ...)` calls the closure with evaluated arguments.
    pub fn register_fn<F>(&self, name: &str, f: F)
    where
        F: Fn(&[LispVal], &Rc<Environment>) -> Result<LispVal, LispError> + 'static,
    {
        self.set(name.to_uppercase(), LispVal::Native(Rc::new(f)));
    }

    /// Update a variable's value, searching up the environment chain.
    /// For dynamic variables, this searches the dynamic parent chain.
    /// For lexical variables, this searches the lexical parent chain.
    /// If the variable is not found in any environment, it is CREATED in
    /// the current environment. This supports dynamic variable creation via
    /// SETQ and is intentional behavior for interactive development.
    pub fn update(env: &Rc<Environment>, name: &str, val: LispVal) {
        if env.is_dynamic(name) {
            // For dynamic variables, search the dynamic parent chain
            Self::update_dynamic(env, name, val);
        } else {
            // For lexical variables, search the lexical parent chain
            Self::update_lexical(env, name, val);
        }
    }

    /// Update a lexical variable by walking the lexical parent chain.
    fn update_lexical(env: &Rc<Environment>, name: &str, val: LispVal) {
        let mut maybe_env = Some(env.clone());
        while let Some(current_env) = maybe_env {
            if current_env.bindings.borrow().contains_key(name) {
                current_env
                    .bindings
                    .borrow_mut()
                    .insert(name.to_string(), val);
                return;
            }
            maybe_env = current_env.parent.clone();
        }
        // Variable not found - create it in the current environment
        env.set(name.to_string(), val);
    }

    /// Update a dynamic variable by walking the dynamic parent chain.
    fn update_dynamic(env: &Rc<Environment>, name: &str, val: LispVal) {
        // First check current bindings
        if env.bindings.borrow().contains_key(name) {
            env.bindings.borrow_mut().insert(name.to_string(), val);
            return;
        }

        // Then walk the dynamic parent chain
        if let Some(dyn_parent) = &env.dynamic_parent {
            Self::update_dynamic(dyn_parent, name, val);
            return;
        }

        // Fall back to lexical parent chain
        if let Some(parent) = &env.parent {
            Self::update_dynamic(parent, name, val);
            return;
        }

        // Variable not found - create it in the current environment
        env.set(name.to_string(), val);
    }

    /// Collect all bindings visible from this frame (including parent frames).
    /// Parent bindings are shadowed by child bindings.
    pub fn all_bindings(&self) -> HashMap<String, LispVal> {
        let mut all = HashMap::new();
        if let Some(parent) = &self.parent {
            all.extend(parent.all_bindings());
        }
        all.extend(self.bindings.borrow().clone());
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
        self.shared.dynamic_vars.borrow_mut().insert(name);
    }

    /// Get variable value, handling both dynamic and lexical scoping.
    /// For dynamic variables, this searches the dynamic parent chain (caller's env).
    /// For lexical variables, this uses the standard get() method (parent chain).
    pub fn get_var(&self, name: &str) -> Option<LispVal> {
        if self.is_dynamic(name) {
            // Dynamic lookup: first check current bindings, then dynamic parent chain
            self.get_dynamic(name)
        } else {
            // Lexical lookup: walk the lexical parent chain
            self.get(name)
        }
    }

    /// Dynamic lookup: search current bindings, then walk dynamic parent chain.
    /// This implements dynamic scoping where variables are resolved based on
    /// the call stack rather than the lexical definition site.
    fn get_dynamic(&self, name: &str) -> Option<LispVal> {
        // First check current bindings
        if let Some(val) = self.bindings.borrow().get(name) {
            return Some(val.clone());
        }

        // For dynamic variables, walk the dynamic parent chain first (caller's environment)
        // This is the key difference from lexical scoping
        if let Some(dyn_parent) = &self.dynamic_parent {
            return dyn_parent.get_dynamic(name);
        }

        // Fall back to lexical parent chain for global bindings
        // (when there's no dynamic parent, or at the bottom of the dynamic chain)
        if let Some(parent) = &self.parent {
            return parent.get_dynamic(name);
        }

        None
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
