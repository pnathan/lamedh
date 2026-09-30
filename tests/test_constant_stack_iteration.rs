//! #504: `dolist`, `mapc`, `mapcan`, `mapcon`, `alist->hash` and `clrhash`
//! run in constant stack. Each used to recurse once per element and hit the
//! 10,000-frame eval limit; each is exercised here on 10^5 elements, and the
//! small cases pin the return values the recursive versions produced.

use lamedh::environment::Environment;
use lamedh::{Shared, eval_line};

const N: usize = 100_000;

fn env() -> Shared<Environment> {
    Environment::with_stdlib()
}

#[test]
fn dolist_on_a_long_list() {
    let e = env();
    assert_eq!(eval_line(&format!("(dolist (x (iota {N})) x)"), &e), "()");
    assert_eq!(
        eval_line(
            &format!("(let ((s 0)) (dolist (x (iota {N}) s) (setq s (+ s x))))"),
            &e
        ),
        "4999950000"
    );
}

#[test]
fn dolist_inside_defun_star_on_a_long_list() {
    let e = env();
    eval_line(
        "(defun* dolist-sum-504 (l) (let ((s 0)) (dolist (x l s) (setq s (+ s x)))))",
        &e,
    );
    assert_eq!(
        eval_line(&format!("(dolist-sum-504 (iota {N}))"), &e),
        "4999950000"
    );
}

#[test]
fn dolist_semantics_are_unchanged() {
    let e = env();
    // Empty body, and RESULT evaluated with VAR bound to NIL.
    assert_eq!(eval_line("(dolist (x '(1 2)))", &e), "()");
    assert_eq!(eval_line("(dolist (x '(1 2) x))", &e), "()");
    assert_eq!(eval_line("(dolist (x nil 'done) x)", &e), "DONE");
    // A fresh binding of VAR per element: closures do not share it.
    assert_eq!(
        eval_line(
            "(mapcar #'funcall (let ((fs nil)) \
               (dolist (x '(1 2 3) fs) (setq fs (cons (lambda () x) fs)))))",
            &e
        ),
        "(3 2 1)"
    );
    // The list form is evaluated once, and a user TAIL variable is not captured.
    assert_eq!(
        eval_line(
            "(let ((n 0) (tail 'mine)) \
               (dolist (x (progn (setq n (+ n 1)) '(a b))) x) (list n tail))",
            &e
        ),
        "(1 MINE)"
    );
}

#[test]
fn mapc_on_a_long_list_returns_the_list() {
    let e = env();
    assert_eq!(
        eval_line(
            &format!(
                "(let ((l (iota {N})) (s 0)) \
                   (list (eq l (mapc (lambda (x) (setq s (+ s x))) l)) s))"
            ),
            &e
        ),
        "(T 4999950000)"
    );
    assert_eq!(eval_line("(mapc (lambda (x) x) nil)", &e), "()");
}

#[test]
fn mapcan_on_a_long_list() {
    let e = env();
    assert_eq!(
        eval_line(
            &format!("(length (mapcan (lambda (x) (list x x)) (iota {N})))"),
            &e
        ),
        "200000"
    );
    assert_eq!(
        eval_line("(mapcan (lambda (x) (list x (* x 10))) '(1 2 3))", &e),
        "(1 10 2 20 3 30)"
    );
    assert_eq!(
        eval_line(
            "(mapcan (lambda (x) (if (> x 1) (list x) nil)) '(1 2 3))",
            &e
        ),
        "(2 3)"
    );
    // FN is still called left to right.
    assert_eq!(
        eval_line(
            "(let ((seen nil)) (mapcan (lambda (x) (setq seen (cons x seen)) nil) '(1 2 3)) seen)",
            &e
        ),
        "(3 2 1)"
    );
    assert_eq!(eval_line("(mapcan #'list nil)", &e), "()");
}

#[test]
fn mapcon_on_a_long_list() {
    let e = env();
    assert_eq!(
        eval_line(
            &format!("(length (mapcon (lambda (x) (list (car x))) (iota {N})))"),
            &e
        ),
        "100000"
    );
    assert_eq!(
        eval_line("(mapcon (lambda (x) (list x)) '(1 2 3))", &e),
        "((1 2 3) (2 3) (3))"
    );
    assert_eq!(eval_line("(mapcon #'list nil)", &e), "()");
}

#[test]
fn alist_to_hash_and_clrhash_on_a_long_list() {
    let e = env();
    eval_line(
        &format!("(setq h504 (alist->hash (mapcar (lambda (i) (cons i (* i 2))) (iota {N}))))"),
        &e,
    );
    assert_eq!(eval_line("(hash-table-count* h504)", &e), "100000");
    assert_eq!(eval_line("(gethash h504 99999)", &e), "199998");
    assert_eq!(eval_line("(eq h504 (clrhash h504))", &e), "T");
    assert_eq!(eval_line("(hash-table-count* h504)", &e), "0");
}
