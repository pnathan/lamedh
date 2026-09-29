//! #524: a result-less `dotimes` / `for` / `while` in value position must
//! evaluate to NIL compiled, exactly as interpreted. The loop's value is
//! typed `bool` (native `false` is NIL), so a tail-position loop compiles and
//! returns NIL, a use of the value as a number leaves the function
//! interpreted, and statement-position loops keep compiling unchanged. The
//! portable codegen gate (lib/46-hm-check.lisp) mirrors the typing.

use lamedh::environment::Environment;
use lamedh::{Shared, eval_line};

fn env() -> Shared<Environment> {
    Environment::with_stdlib()
}

const TAIL_LOOPS: [(&str, &str); 3] = [
    (
        "lv-dotimes",
        "(let ((acc 0)) (dotimes (i n) (setq acc (+ acc i))))",
    ),
    (
        "lv-for",
        "(let ((acc 0)) (for (i 0 n) (setq acc (+ acc i))))",
    ),
    ("lv-while", "(let ((i 0)) (while (< i n) (setq i (+ i 1))))"),
];

#[test]
fn tail_position_loops_return_nil_compiled() {
    let e = env();
    for (name, body) in TAIL_LOOPS {
        eval_line(&format!("(defun {name} (n) {body})"), &e);
        assert_eq!(
            eval_line(&format!("(explain-compile '{name})"), &e),
            "((TIER . COMPILED) (SIGNATURE -> (INT64) BOOL))",
            "{name}"
        );
        for n in [0, 3] {
            let want = eval_line(&format!("(let ((n {n})) {body})"), &e);
            assert_eq!(want, "()", "interpreted {name} {n}");
            assert_eq!(eval_line(&format!("({name} {n})"), &e), want, "{name} {n}");
        }
    }
}

#[test]
fn a_loop_value_used_as_a_number_stays_interpreted() {
    let e = env();
    eval_line("(defun lv-num (n) (+ 1 (dotimes (i n) i)))", &e);
    assert_eq!(eval_line("(compiled-p 'lv-num)", &e), "()");
    eval_line("(defun lv-join (n) (if (< n 0) 5 (for (i 0 n) i)))", &e);
    assert_eq!(eval_line("(compiled-p 'lv-join)", &e), "()");
}

#[test]
fn statement_position_loops_still_compile() {
    let e = env();
    eval_line(
        "(defun lv-stmt (n) (let ((acc 0)) (dotimes (i n) (setq acc (+ acc i))) acc))",
        &e,
    );
    assert_eq!(
        eval_line("(explain-compile 'lv-stmt)", &e),
        "((TIER . COMPILED) (SIGNATURE -> (INT64) INT64))"
    );
    assert_eq!(eval_line("(lv-stmt 5)", &e), "10");
}

#[test]
fn the_portable_gate_agrees_on_loop_values() {
    let e = env();
    for (_, body) in TAIL_LOOPS {
        assert_eq!(
            eval_line(&format!("(hm-compile-lambda 'lv '(n) '({body}))"), &e),
            "(COMPILEABLE (-> (INT64) BOOL))",
            "{body}"
        );
    }
    assert_eq!(
        eval_line(
            "(hm-compile-lambda 'lv '(n) '((+ 1 (dotimes (i n) i))))",
            &e
        ),
        "(BLOCKED \"`+` operands disagree\")"
    );
}
