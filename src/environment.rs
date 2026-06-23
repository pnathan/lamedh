use crate::{BuiltinFunc, LispError, LispVal, Symbol};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

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
    pub fn new() -> Self {
        SymbolTable {
            symbols: HashMap::new(),
            gensym_counter: 0,
        }
    }

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

#[derive(Debug, Clone)]
pub struct Environment {
    parent: Option<Rc<Environment>>,
    bindings: Rc<RefCell<HashMap<String, LispVal>>>,
    pub symbols: Rc<RefCell<SymbolTable>>,
    condition_flags: Rc<RefCell<HashMap<String, bool>>>,
    /// Set of variable names that are marked as dynamic (special variables).
    /// This is shared globally across all environments.
    dynamic_vars: Rc<RefCell<HashSet<String>>>,
    /// Dynamic parent environment (caller's environment for dynamic scoping).
    /// This is used to look up dynamic variables from the call chain.
    dynamic_parent: Option<Rc<Environment>>,
    /// Set of enabled capabilities/features (e.g. "SHELL"). Shared across the
    /// whole environment chain. Off by default; the host or a Lisp program must
    /// opt in. This is the foundation for sandboxing (see issue #64).
    features: Rc<RefCell<HashSet<String>>>,
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
            && Rc::ptr_eq(&self.symbols, &other.symbols)
            && Rc::ptr_eq(&self.condition_flags, &other.condition_flags)
            && Rc::ptr_eq(&self.dynamic_vars, &other.dynamic_vars)
            && Rc::ptr_eq(&self.features, &other.features)
    }
}

impl Default for Environment {
    fn default() -> Self {
        Self::new()
    }
}

impl Environment {
    pub fn new() -> Self {
        Environment {
            parent: None,
            bindings: Rc::new(RefCell::new(HashMap::new())),
            symbols: Rc::new(RefCell::new(SymbolTable::new())),
            condition_flags: Rc::new(RefCell::new(HashMap::new())),
            dynamic_vars: Rc::new(RefCell::new(HashSet::new())),
            dynamic_parent: None,
            features: Rc::new(RefCell::new(HashSet::new())),
        }
    }

    /// Create a new child environment for lexical scoping.
    /// The child inherits the parent's dynamic_parent by default.
    pub fn new_child(parent: &Rc<Environment>) -> Rc<Environment> {
        Rc::new(Environment {
            parent: Some(parent.clone()),
            bindings: Rc::new(RefCell::new(HashMap::new())),
            symbols: parent.symbols.clone(),
            condition_flags: parent.condition_flags.clone(),
            dynamic_vars: parent.dynamic_vars.clone(),
            dynamic_parent: parent.dynamic_parent.clone(),
            features: parent.features.clone(),
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
            symbols: lexical_parent.symbols.clone(),
            condition_flags: lexical_parent.condition_flags.clone(),
            dynamic_vars: lexical_parent.dynamic_vars.clone(),
            dynamic_parent: Some(caller_env.clone()),
            features: lexical_parent.features.clone(),
        })
    }

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
        env.set("GET".to_string(), LispVal::Builtin(BuiltinFunc::Get));
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

    pub fn intern_symbol(&self, name: &str) -> Rc<RefCell<Symbol>> {
        self.symbols.borrow_mut().intern(name)
    }

    pub fn gensym(&self) -> Rc<RefCell<Symbol>> {
        self.symbols.borrow_mut().gensym()
    }

    pub fn all_symbols(&self) -> Vec<Rc<RefCell<Symbol>>> {
        self.symbols.borrow().all_symbols()
    }

    pub fn is_bound(&self, name: &str) -> bool {
        self.get(name).is_some()
    }

    pub fn get(&self, name: &str) -> Option<LispVal> {
        if let Some(val) = self.bindings.borrow().get(name) {
            return Some(val.clone());
        }
        if let Some(parent) = &self.parent {
            return parent.get(name);
        }
        None
    }

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

    pub fn all_bindings(&self) -> HashMap<String, LispVal> {
        let mut all = HashMap::new();
        if let Some(parent) = &self.parent {
            all.extend(parent.all_bindings());
        }
        all.extend(self.bindings.borrow().clone());
        all
    }

    // Condition flag operations (dynamically scoped)
    pub fn set_flag(&self, flag: &str) {
        self.condition_flags
            .borrow_mut()
            .insert(flag.to_string(), true);
    }

    pub fn clear_flag(&self, flag: &str) {
        self.condition_flags
            .borrow_mut()
            .insert(flag.to_string(), false);
    }

    pub fn flag_set(&self, flag: &str) -> bool {
        self.condition_flags
            .borrow()
            .get(flag)
            .copied()
            .unwrap_or(false)
    }

    pub fn clear_all_flags(&self) {
        self.condition_flags.borrow_mut().clear();
    }

    // Dynamic variable operations

    /// Check if a variable is marked as dynamic (special variable)
    pub fn is_dynamic(&self, name: &str) -> bool {
        self.dynamic_vars.borrow().contains(name)
    }

    /// Mark a variable as dynamic (global registration)
    pub fn mark_dynamic(&self, name: String) {
        self.dynamic_vars.borrow_mut().insert(name);
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
        self.features.borrow_mut().insert(name.to_uppercase());
    }

    /// Disable a capability.
    pub fn disable_feature(&self, name: &str) {
        self.features.borrow_mut().remove(&name.to_uppercase());
    }

    /// Check whether a capability is enabled.
    pub fn feature_enabled(&self, name: &str) -> bool {
        self.features.borrow().contains(&name.to_uppercase())
    }

    /// List enabled capabilities.
    pub fn features_list(&self) -> Vec<String> {
        self.features.borrow().iter().cloned().collect()
    }
}
