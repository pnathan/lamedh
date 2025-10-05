use lithhp::{environment::Environment, evaluator, reader};
use std::rc::Rc;

pub fn env_with_prologue() -> Rc<Environment> {
    let env = Environment::new_with_builtins();
    let prologue = std::fs::read_to_string("prologue.lisp").unwrap();
    let expressions = reader::read_all(&prologue, &env).unwrap();
    for expr in expressions {
        evaluator::eval(&expr, &env).unwrap();
    }
    env
}
