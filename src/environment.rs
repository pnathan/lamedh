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
    parent: Option<Rc<Environment>>,
    bindings: Rc<RefCell<HashMap<String, LispVal>>>,
    pub symbols: Rc<RefCell<SymbolTable>>,
}

impl PartialEq for Environment {
    fn eq(&self, other: &Self) -> bool {
        let parents_equal = match (&self.parent, &other.parent) {
            (Some(p1), Some(p2)) => Rc::ptr_eq(p1, p2),
            (None, None) => true,
            _ => false,
        };
        parents_equal
            && Rc::ptr_eq(&self.bindings, &other.bindings)
            && Rc::ptr_eq(&self.symbols, &other.symbols)
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
        }
    }

    pub fn new_child(parent: &Rc<Environment>) -> Rc<Environment> {
        Rc::new(Environment {
            parent: Some(parent.clone()),
            bindings: Rc::new(RefCell::new(HashMap::new())),
            symbols: parent.symbols.clone(),
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
        env.set("=".to_string(), LispVal::Builtin(BuiltinFunc::NumericEquals));
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
        env
    }

    pub fn intern_symbol(&self, name: &str) -> Rc<RefCell<Symbol>> {
        self.symbols.borrow_mut().intern(name)
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

    pub fn update(env: &Rc<Environment>, name: &str, val: LispVal) {
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
}