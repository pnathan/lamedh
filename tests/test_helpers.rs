use lamedh::{self, Shared, environment::Environment};

/// A fully loaded stdlib environment for integration tests.
///
/// Uses [`Environment::with_stdlib`], which serves a deep-copy fork of a
/// per-thread prototype world: the first call on a thread pays one real
/// stdlib evaluation, every further call is a few-milliseconds fork with
/// full isolation (see `fork_world`). Tests that build many environments
/// (e.g. `test_examples`, which loops the whole classics suite on one
/// large-stack thread) get the fork price instead of a full reload each
/// time. The embedded sources are `include_str!`s of `lib/*.lisp`, so this
/// tests exactly the same library text the old disk-loading helper did.
pub fn env_with_stdlib() -> Shared<Environment> {
    let env = Environment::with_stdlib();
    match lamedh::load_file("prologue.lisp", &env) {
        Ok(_) => {}
        Err(e) => {
            panic!("error loading prologue.lisp: {:?}", e);
        }
    };
    env
}
