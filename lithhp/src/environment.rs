use crate::{BuiltinFunc, LispVal};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub struct Environment {
    scopes: Vec<HashMap<String, LispVal>>,
}

impl Default for Environment {
    fn default() -> Self {
        Self::new()
    }
}

impl Environment {
    pub fn new() -> Self {
        Environment {
            scopes: vec![HashMap::new()],
        }
    }

    pub fn new_with_builtins() -> Self {
        let mut env = Environment::new();
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
        env
    }

    pub fn get(&self, name: &str) -> Option<LispVal> {
        for scope in self.scopes.iter().rev() {
            if let Some(val) = scope.get(name) {
                return Some(val.clone());
            }
        }
        None
    }

    // `set` defines a variable in the current (innermost) scope.
    pub fn set(&mut self, name: String, val: LispVal) {
        self.scopes.last_mut().unwrap().insert(name, val);
    }

    pub fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    pub fn pop_scope(&mut self) {
        self.scopes.pop();
    }
}
