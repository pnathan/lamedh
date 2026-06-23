// Runs the Lisp-level test suite (tests/lisp/*.lisp) through the testing
// framework (lib/10-testing.lisp) so that `cargo test` enforces it.
//
// Each .lisp file registers tests via (deftest ...). We load them all, then
// (run-tests) returns T iff every assertion passed. Failure detail is printed
// to stdout by the framework (visible with `cargo test -- --nocapture`).
//
// NOTE: the suite runs via `with_large_stack`. The tree-walking interpreter
// uses large stack frames (see issue #76), and the default `cargo test` thread
// stack (~2 MB) is far smaller than the CLI's, so it would overflow there. The
// big stack keeps the harness honest without masking assertion failures; the
// depth guard (#61) bounds runaway recursion within it.

mod test_helpers;
use lamedh::{eval_line, load_directory, with_large_stack};
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
    with_large_stack(run_suite);
}
