//! #403: `dotimes` (a macro in lib/12-control.lisp) reaches the compiled
//! tier. The typed elaborator desugars it to the macro's own expansion
//! (`let` + `for` + result `let`), and the portable codegen gate
//! (lib/46-hm-check.lisp) mirrors that desugaring.

use lamedh::environment::Environment;
use lamedh::{Shared, eval_line};

fn env() -> Shared<Environment> {
    Environment::with_stdlib()
}

const SUM_NO_RESULT: &str = "(let ((acc 0)) (dotimes (i n) (setq acc (+ acc i))) acc)";
const SUM_WITH_RESULT: &str = "(let ((acc 0)) (dotimes (i n (+ acc i)) (setq acc (+ acc i))))";

#[test]
fn dotimes_defuns_compile_natively() {
    let e = env();
    for (name, body) in [("dt-a", SUM_NO_RESULT), ("dt-b", SUM_WITH_RESULT)] {
        eval_line(&format!("(defun {name} (n) {body})"), &e);
        assert_eq!(
            eval_line(&format!("(explain-compile '{name})"), &e),
            "((TIER . COMPILED) (SIGNATURE -> (INT64) INT64))",
            "{name}"
        );
        assert_ne!(eval_line(&format!("(compiled-p '{name})"), &e), "NIL");
    }
}

#[test]
fn compiled_dotimes_matches_the_interpreter() {
    let e = env();
    for (name, body) in [("dt-c", SUM_NO_RESULT), ("dt-d", SUM_WITH_RESULT)] {
        eval_line(&format!("(defun {name} (n) {body})"), &e);
        for n in [0, 1, 7, 100] {
            // The interpreted reference: the same body under a plain LET.
            let want = eval_line(&format!("(let ((n {n})) {body})"), &e);
            assert_eq!(eval_line(&format!("({name} {n})"), &e), want, "{name} {n}");
        }
    }
    // The result form sees VAR bound to COUNT.
    eval_line("(defun dt-e (n) (dotimes (i n i) (+ i 1)))", &e);
    assert_ne!(eval_line("(compiled-p 'dt-e)", &e), "NIL");
    assert_eq!(eval_line("(dt-e 5)", &e), "5");
}

#[test]
fn the_portable_gate_agrees_on_dotimes() {
    let e = env();
    for body in [SUM_NO_RESULT, SUM_WITH_RESULT] {
        assert_eq!(
            eval_line(&format!("(hm-compile-lambda 'dt '(n) '({body}))"), &e),
            "(COMPILEABLE (-> (INT64) INT64))"
        );
    }
}
