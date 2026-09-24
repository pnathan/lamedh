//! #399: passing a `(make-array n)` array with unstored (NIL) slots to a
//! typed `(array T)` parameter names the problem and the fix.

use lamedh::environment::Environment;
use lamedh::{Shared, eval_str};

fn env() -> Shared<Environment> {
    Environment::with_stdlib()
}

fn err_of(src: &str, e: &Shared<Environment>) -> String {
    match eval_str(src, e) {
        Ok(v) => panic!(
            "{src}: expected an error, got {}",
            lamedh::printer::print(&v)
        ),
        Err(err) => format!("{err}"),
    }
}

#[test]
fn nil_initialized_array_error_names_index_and_typed_array() {
    let e = env();
    eval_str(
        "(defun-typed (sa int64) ((a (array int64))) (array-sum a))",
        &e,
    )
    .unwrap();
    eval_str(
        "(defun-typed (sf float64) ((a (array float64))) (fetch a 0))",
        &e,
    )
    .unwrap();

    let msg = err_of("(sa (make-array 3))", &e);
    assert!(msg.contains("expected (array int64) argument"), "{msg}");
    assert!(msg.contains("element 0 is ()"), "{msg}");
    assert!(msg.contains("(make-array n)"), "{msg}");
    assert!(msg.contains("(typed-array n 'int64)"), "{msg}");

    // A partly stored array names the first unstored index.
    let msg = err_of("(let ((x (make-array 3))) (aset x 0 1) (sa x))", &e);
    assert!(msg.contains("element 1 is ()"), "{msg}");

    let msg = err_of("(sf (make-array 2))", &e);
    assert!(msg.contains("(typed-array n 'float64)"), "{msg}");

    // The suggested fix works.
    assert_eq!(
        lamedh::printer::print(&eval_str("(sa (typed-array 3 'int64))", &e).unwrap()),
        "0"
    );
}

#[test]
fn a_wrong_typed_element_still_names_its_index() {
    let e = env();
    eval_str(
        "(defun-typed (sa int64) ((a (array int64))) (array-sum a))",
        &e,
    )
    .unwrap();
    let msg = err_of(
        "(let ((x (make-array 2))) (aset x 0 1) (aset x 1 \"s\") (sa x))",
        &e,
    );
    assert!(msg.contains("element 1: expected int64 argument"), "{msg}");
    assert!(!msg.contains("typed-array"), "{msg}");
}
