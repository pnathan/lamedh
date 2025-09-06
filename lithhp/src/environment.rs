
use std::cell::RefCell;
use crate::{BuiltinFunc, LispVal, reader};
use std::collections::HashMap;
use std::rc::Rc;

#[derive(Debug, Clone, PartialEq)]
pub struct Environment {
    scopes: Rc<RefCell<Vec<HashMap<String, LispVal>>>>,
}

impl Default for Environment {
    fn default() -> Self {
        Self::new()
    }
}

impl Environment {
    pub fn new() -> Self {
        Environment {
            scopes: Rc::new(RefCell::new(vec![HashMap::new()])),
        }
    }

    pub fn new_with_builtins() -> Self {
        let mut env = Environment::new();
        env.set("t".to_string(), reader::new_symbol("t"));
        env.set("nil".to_string(), LispVal::Nil);
        env.set("+".to_string(), LispVal::Builtin(BuiltinFunc::Plus));
        env.set("-".to_string(), LispVal::Builtin(BuiltinFunc::Minus));
        env.set("*".to_string(), LispVal::Builtin(BuiltinFunc::Multiply));
        env.set("/".to_string(), LispVal::Builtin(BuiltinFunc::Divide));
        env.set("car".to_string(), LispVal::Builtin(BuiltinFunc::Car));
        env.set("cdr".to_string(), LispVal::Builtin(BuiltinFunc::Cdr));
        env.set("cons".to_string(), LispVal::Builtin(BuiltinFunc::Cons));
        env.set("concat".to_string(), LispVal::Builtin(BuiltinFunc::Concat));
        env.set("++".to_string(), LispVal::Builtin(BuiltinFunc::Concat)); // alias
        env.set("index".to_string(), LispVal::Builtin(BuiltinFunc::Index));
        env.set("eval".to_string(), LispVal::Builtin(BuiltinFunc::Eval));
        env.set("eq".to_string(), LispVal::Builtin(BuiltinFunc::Eq));
        env.set(
            "=".to_string(),
            LispVal::Builtin(BuiltinFunc::NumericEquals),
        );
        env.set("not".to_string(), LispVal::Builtin(BuiltinFunc::Not));
        env.set(
            "make-hash-table".to_string(),
            LispVal::Builtin(BuiltinFunc::MakeHashTable),
        );
        env.set("get".to_string(), LispVal::Builtin(BuiltinFunc::Get));
        env.set("set!".to_string(), LispVal::Builtin(BuiltinFunc::Set));
        env.set(
            "delete-key!".to_string(),
            LispVal::Builtin(BuiltinFunc::DeleteKey),
        );
        env.set(
            "current-environment".to_string(),
            LispVal::Builtin(BuiltinFunc::CurrentEnvironment),
        );
        env.set("keys".to_string(), LispVal::Builtin(BuiltinFunc::Keys));
        env.set("atom".to_string(), LispVal::Builtin(BuiltinFunc::Atom));
        env.set("print".to_string(), LispVal::Builtin(BuiltinFunc::Print));
        env.set("get".to_string(), LispVal::Builtin(BuiltinFunc::Get));
        env.set("put".to_string(), LispVal::Builtin(BuiltinFunc::Put));
        env.set(
            "symbol-plist".to_string(),
            LispVal::Builtin(BuiltinFunc::SymbolPlist),
        );
        env.set(
            "remprop".to_string(),
            LispVal::Builtin(BuiltinFunc::Remprop),
        );
        env
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
