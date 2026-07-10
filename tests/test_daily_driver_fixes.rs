//! Regression tests for the daily-driver review fixes
//! (issues #237–#250; see docs/review-daily-driver-2026-07.md).

use lamedh::environment::Environment;
use lamedh::{Shared, eval_line, reader};

fn env_with_stdlib() -> Shared<Environment> {
    Environment::with_stdlib()
}

// ── #237: T and keywords are constants ─────────────────────────────────────

#[test]
fn setq_t_is_rejected() {
    let env = env_with_stdlib();
    let out = eval_line("(setq t nil)", &env);
    assert!(out.contains("cannot rebind the constant T"), "{out}");
    // Truth still works afterwards.
    assert_eq!(eval_line("(if t 'yes 'no)", &env), "YES");
}

#[test]
fn t_rejected_in_binders_and_params() {
    let env = env_with_stdlib();
    for form in [
        "(def t 5)",
        "(let ((t 1)) t)",
        "(let* ((t 1)) t)",
        "(prog (t) (return 1))",
        "(for (t 1 3) nil)",
        "(defun bad-f (t) t)",
    ] {
        let out = eval_line(form, &env);
        assert!(
            out.contains("cannot rebind the constant T"),
            "{form} => {out}"
        );
    }
}

#[test]
fn t_rejected_through_compiled_bodies() {
    let env = env_with_stdlib();
    eval_line("(defun evil () (setq t nil))", &env);
    let out = eval_line("(evil)", &env);
    assert!(out.contains("cannot rebind the constant T"), "{out}");
}

#[test]
fn keywords_rejected_as_binding_targets() {
    let env = env_with_stdlib();
    let out = eval_line("(setq :a 5)", &env);
    assert!(out.contains("cannot bind the keyword"), "{out}");
}

// ── #242 → 0.3: DEFUN supports &optional/&key (expanded to &rest + a
// LET* prologue); bare LAMBDA and DEFMACRO still reject them.

#[test]
fn optional_and_key_supported_in_defun() {
    let env = env_with_stdlib();
    eval_line(
        "(defun f (a &optional (b 2) &key (c 3) label) (list a b c label))",
        &env,
    );
    assert_eq!(eval_line("(f 1)", &env), "(1 2 3 ())");
    assert_eq!(eval_line("(f 1 9 :label 'x :c 4)", &env), "(1 9 4 X)");
    let out = eval_line("(defmacro m (&key x) x)", &env);
    assert!(out.contains("&KEY is not supported"), "{out}");
    // &rest still works.
    eval_line("(defun g (a &rest rs) (list a rs))", &env);
    assert_eq!(eval_line("(g 1 2 3)", &env), "(1 (2 3))");
}

// ── #241: test runner isolates erroring test bodies ────────────────────────

#[test]
fn run_tests_survives_erroring_test() {
    let env = env_with_stdlib();
    eval_line("(deftest good (assert-equal 1 1))", &env);
    eval_line("(deftest bad (car 5))", &env);
    eval_line("(deftest good2 (assert-equal 2 2))", &env);
    // One failure recorded, run completes, returns NIL.
    assert_eq!(eval_line("(run-tests)", &env), "()");
    assert_eq!(eval_line("*test-pass*", &env), "2");
    assert_eq!(eval_line("*test-fail*", &env), "1");
}

#[test]
fn deftest_replaces_same_name() {
    let env = env_with_stdlib();
    eval_line("(deftest dup (assert-equal 1 1))", &env);
    eval_line("(deftest dup (assert-equal 1 1))", &env);
    assert_eq!(eval_line("(run-tests)", &env), "T");
    assert_eq!(eval_line("*test-pass*", &env), "1");
}

// ── #243 (historical): the untyped DEFSTRUCT and its keyword constructor
// were removed in 0.3 — DEFRECORD is the one record definition form.

#[test]
fn untyped_defstruct_is_gone() {
    let env = env_with_stdlib();
    let out = eval_line("(defstruct point x y)", &env);
    assert!(
        out.contains("Error"),
        "expected defstruct removed, got: {out}"
    );
}

// ── #245: CL compatibility layer ────────────────────────────────────────────

#[test]
fn setf_places() {
    let env = env_with_stdlib();
    assert_eq!(eval_line("(progn (setf x 5) x)", &env), "5");
    assert_eq!(
        eval_line(
            "(let ((h (make-hash-table))) (setf (gethash h :k) 42) (gethash h :k))",
            &env
        ),
        "42"
    );
    assert_eq!(
        eval_line(
            "(let ((a (array 3))) (setf (aref a 1) 9) (fetch a 1))",
            &env
        ),
        "9"
    );
    // The accessor-place convention still routes to SET-<accessor>! — the
    // mutator is user-defined now that records are functional (0.3).
    eval_line("(def *cell* (array 1))", &env);
    eval_line("(defun cell-v (c) (fetch c 0))", &env);
    eval_line("(defun set-cell-v! (c v) (store c 0 v))", &env);
    assert_eq!(
        eval_line("(let ((c (array 1))) (setf (cell-v c) 7) (cell-v c))", &env),
        "7"
    );
}

#[test]
fn push_pop_incf_decf() {
    let env = env_with_stdlib();
    assert_eq!(
        eval_line("(progn (def xs '(2 3)) (push 1 xs) xs)", &env),
        "(1 2 3)"
    );
    assert_eq!(
        eval_line("(progn (def ys '(1 2)) (list (pop ys) ys))", &env),
        "(1 (2))"
    );
    assert_eq!(
        eval_line("(progn (def n 5) (incf n) (incf n 10) n)", &env),
        "16"
    );
    assert_eq!(eval_line("(progn (def m 5) (decf m 2) m)", &env), "3");
}

#[test]
fn sequence_staples() {
    let env = env_with_stdlib();
    assert_eq!(eval_line("(remove 2 '(1 2 3 2))", &env), "(1 3)");
    assert_eq!(eval_line("(count 2 '(1 2 2 3))", &env), "2");
    assert_eq!(eval_line("(copy-list* '(1 2 3))", &env), "(1 2 3)");
    assert_eq!(eval_line("(subseq '(1 2 3 4) 1 3)", &env), "(2 3)");
    assert_eq!(eval_line("(subseq \"hello\" 1 3)", &env), "\"el\"");
    assert_eq!(eval_line("(elt \"abc\" 1)", &env), "\"b\"");
    assert_eq!(eval_line("(elt '(a b c) 1)", &env), "B");
    assert_eq!(eval_line("(reverse \"abc\")", &env), "\"cba\"");
    assert_eq!(eval_line("(reverse '(1 2 3))", &env), "(3 2 1)");
    assert_eq!(eval_line("(nreverse '(1 2 3))", &env), "(3 2 1)");
    assert_eq!(eval_line("(rem -7 3)", &env), "-1");
}

#[test]
fn kernel_cl_bits() {
    let env = env_with_stdlib();
    // Spread apply.
    assert_eq!(eval_line("(apply #'+ 1 2 '(3 4))", &env), "10");
    assert_eq!(eval_line("(apply #'+ '(1 2))", &env), "3");
    // Polymorphic length.
    assert_eq!(eval_line("(length \"héllo\")", &env), "5");
    assert_eq!(eval_line("(length (array 4))", &env), "4");
    // read-from-string.
    assert_eq!(
        eval_line("(eval (read-from-string \"(+ 1 2)\"))", &env),
        "3"
    );
    // Float division by zero is IEEE inf, integer stays an error.
    assert_eq!(eval_line("(/ 1.0 0.0)", &env), "inf");
    assert!(eval_line("(/ 1 0)", &env).contains("Division by zero"));
}

// ── #246: dual argument orders ──────────────────────────────────────────────

#[test]
fn hash_and_alist_helpers_accept_both_orders() {
    let env = env_with_stdlib();
    eval_line("(def h (make-hash-table))", &env);
    eval_line("(set-bang h :k 42)", &env);
    assert_eq!(eval_line("(gethash h :k)", &env), "42");
    assert_eq!(eval_line("(gethash h :k)", &env), "42");
    assert_eq!(eval_line("(alist-get '((a . 1) (b . 2)) 'b)", &env), "2");
    assert_eq!(eval_line("(alist-get 'b '((a . 1) (b . 2)))", &env), "2");
    // maphash with the function in either position.
    assert_eq!(
        eval_line(
            "(let ((n 0)) (maphash (lambda (k v) (setq n (+ n v))) h) n)",
            &env
        ),
        "42"
    );
    assert_eq!(
        eval_line(
            "(let ((n 0)) (maphash h (lambda (k v) (setq n (+ n v)))) n)",
            &env
        ),
        "42"
    );
}

// ── #247: error messages carry the offending value ──────────────────────────

#[test]
fn errors_name_the_value() {
    let env = env_with_stdlib();
    assert!(eval_line("(car 5)", &env).contains("got 5"));
    assert!(eval_line("(+ 1 \"a\")", &env).contains("got \"a\""));
    assert!(eval_line("(concat \"a\" 1)", &env).contains("got 1"));
}

// ── #238/#248: reader — positions, block comments, radix, shebang ──────────

#[test]
fn parse_errors_have_line_and_column() {
    let env = env_with_stdlib();
    let out = eval_line("(foo", &env);
    assert!(out.contains("line 1"), "{out}");
    let err = reader::read_all("(ok)\n(also ok)\n(broken", &env).unwrap_err();
    assert!(err.contains("line 3"), "{err}");
}

#[test]
fn block_comments_radix_shebang() {
    let env = env_with_stdlib();
    assert_eq!(eval_line("#| comment #| nested |# still |# 5", &env), "5");
    assert_eq!(
        eval_line("(list #x1F #b101 #o17 #X-a)", &env),
        "(31 5 15 -10)"
    );
    let forms = reader::read_all("#!/usr/bin/env lamedh\n(+ 1 2)", &env).unwrap();
    assert_eq!(forms.len(), 1);
}

#[test]
fn incomplete_detection() {
    assert!(reader::is_incomplete("(defun f (x)"));
    assert!(reader::is_incomplete("\"unterminated"));
    assert!(reader::is_incomplete("#| open comment"));
    assert!(!reader::is_incomplete("(defun f (x) (+ x 2))"));
    assert!(!reader::is_incomplete("(char-code '(')")); // quoted paren
    assert!(!reader::is_incomplete("too) many) parens)")); // malformed, not incomplete
}

// ── #239: partial file loads with positions ─────────────────────────────────

#[test]
fn load_file_applies_forms_before_an_error() {
    let env = env_with_stdlib();
    let dir = std::env::temp_dir().join("lamedh-test-239");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("partial.lisp");
    std::fs::write(&path, "(def early 1)\n(def mid 2)\n(broken\n").unwrap();
    let err = lamedh::load_file(path.to_str().unwrap(), &env).unwrap_err();
    let msg = format!("{err}");
    assert!(msg.contains("partial.lisp:3"), "{msg}");
    assert_eq!(eval_line("(list early mid)", &env), "(1 2)");
    std::fs::remove_dir_all(&dir).ok();
}
