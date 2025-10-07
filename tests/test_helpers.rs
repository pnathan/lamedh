use lamedh::{self, environment::Environment};
use std::rc::Rc;

pub fn env_with_stdlib() -> Rc<Environment> {
    let env = Environment::new_with_builtins();
    lamedh::load_directory("lib", &env).unwrap();
    env
}
