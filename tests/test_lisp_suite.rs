// Runs the Lisp-level test suite (tests/lisp/*.lisp) through the testing
// framework (lib/10-testing.lisp) so that `cargo test` enforces it.
//
// Each .lisp file registers tests via (deftest ...). We load them all, then
// (run-tests) returns T iff every assertion passed. Failure detail is printed
// to stdout by the framework (visible with `cargo test -- --nocapture`).
//
// NOTE: the suite runs on a dedicated thread with a large stack. The
// tree-walking interpreter recurses deeply with heavy frames, and the default
// `cargo test` thread stack (~2 MB) is far smaller than the 8 MB main-thread
// stack the CLI uses, so it overflows there. The real fix is a recursion
// depth guard / tail-call optimization (issues #61, #62); until then the big
// stack keeps the harness honest without masking assertion failures.

mod test_helpers;
use lamedh::{eval_line, load_directory};
use test_helpers::env_with_stdlib;

fn run_suite() {
    let env = env_with_stdlib();
    load_directory("tests/lisp", &env).expect("failed to load tests/lisp");

    // Sanity: at least one test must have been registered.
    let count = eval_line("(length *tests*)", &env);
    assert_ne!(count, "0", "no Lisp tests were registered");

    let result = eval_line("(run-tests)", &env);
    assert_eq!(
        result, "T",
        "Lisp test suite reported failures; run `cargo test lisp_test_suite_passes -- --nocapture` for detail"
    );
}

#[test]
fn lisp_test_suite_passes() {
    let child = std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(run_suite)
        .expect("failed to spawn suite thread");
    child.join().expect("Lisp test suite thread failed");
}
