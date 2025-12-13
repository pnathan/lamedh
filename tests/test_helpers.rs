use lamedh::{self, environment::Environment};
use std::rc::Rc;

pub fn env_with_stdlib() -> Rc<Environment> {
    let env = Environment::new_with_builtins();
    match lamedh::load_file("prologue.lisp", &env) {
        Ok(_) => {}
        Err(e) => {
            panic!("error loading prologue.lisp: {:?}", e);
        }
    };
    match lamedh::load_directory("lib", &env) {
        Ok(_) => {}
        Err(e) => {
            panic!("error loading lib directory: {:?}", e);
        }
    };
    env
}
