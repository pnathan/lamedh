use crate::{BuiltinFunc, LispVal, Symbol};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

#[derive(Debug, Clone, PartialEq)]
pub struct SymbolTable {
    symbols: HashMap<String, Rc<RefCell<Symbol>>>,
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
        }
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
    scopes: Rc<RefCell<Vec<HashMap<String, LispVal>>>>,
    symbols: Rc<RefCell<SymbolTable>>,
}

impl PartialEq for Environment {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.scopes, &other.scopes) && Rc::ptr_eq(&self.symbols, &other.symbols)
    }
}

impl Default for Environment {
    fn default() -> Self {
        Self::new()
    }
}

impl Environment {
    pub fn new() -> Self {
        let symbols = Rc::new(RefCell::new(SymbolTable::new()));
        Environment {
            scopes: Rc::new(RefCell::new(vec![HashMap::new()])),
            symbols,
        }
    }

    pub fn new_with_builtins() -> Self {
        let mut env = Environment::new();
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
        env
    }

    pub fn intern_symbol(&mut self, name: &str) -> Rc<RefCell<Symbol>> {
        self.symbols.borrow_mut().intern(name)
    }

    pub fn get(&self, name: &str) -> Option<LispVal> {
        for scope in self.scopes.borrow().iter().rev() {
            if let Some(val) = scope.get(name) {
                return Some(val.clone());
            }
        }
        None
    }

    // `set` defines a variable in the current (innermost) scope.
    pub fn set(&mut self, name: String, val: LispVal) {
        self.scopes
            .borrow_mut()
            .last_mut()
            .unwrap()
            .insert(name, val);
    }

    pub fn push_scope(&mut self) {
        self.scopes.borrow_mut().push(HashMap::new());
    }

    pub fn pop_scope(&mut self) {
        self.scopes.borrow_mut().pop();
    }

    pub fn all_bindings(&self) -> HashMap<String, LispVal> {
        let mut all = HashMap::new();
        for scope in self.scopes.borrow().iter() {
            all.extend(scope.clone());
        }
        all
    }
}
