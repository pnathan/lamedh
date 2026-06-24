/// Tests for DEFSTRUCT, EVCON, and Lisp 1.5 arithmetic aliases.
mod test_helpers;
use lamedh::eval_line;
use lamedh::with_large_stack;
use test_helpers::env_with_stdlib;

// ─── DEFSTRUCT ───────────────────────────────────────────────────────────────

#[test]
fn test_defstruct_constructor_and_accessors() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        eval_line("(defstruct point x y)", &env);
        eval_line("(def p (make-point 3 4))", &env);
        assert_eq!(eval_line("(point-x p)", &env), "3");
        assert_eq!(eval_line("(point-y p)", &env), "4");
    });
}

#[test]
fn test_defstruct_predicate() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        eval_line("(defstruct thing a b)", &env);
        eval_line("(def t1 (make-thing 1 2))", &env);
        assert_eq!(eval_line("(thing-p t1)", &env), "T");
        assert_eq!(eval_line("(thing-p 42)", &env), "()");
        assert_eq!(eval_line("(thing-p '(1 2 3))", &env), "()");
    });
}

#[test]
fn test_defstruct_mutation() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        eval_line("(defstruct cell value)", &env);
        eval_line("(def c (make-cell 10))", &env);
        assert_eq!(eval_line("(cell-value c)", &env), "10");
        eval_line("(set-cell-value! c 99)", &env);
        assert_eq!(eval_line("(cell-value c)", &env), "99");
    });
}

#[test]
fn test_defstruct_independence() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        eval_line("(defstruct pair a b)", &env);
        eval_line("(def p1 (make-pair 1 2))", &env);
        eval_line("(def p2 (make-pair 3 4))", &env);
        eval_line("(set-pair-a! p1 100)", &env);
        // Mutating p1 must not affect p2
        assert_eq!(eval_line("(pair-a p1)", &env), "100");
        assert_eq!(eval_line("(pair-a p2)", &env), "3");
    });
}

#[test]
fn test_defstruct_three_fields() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        eval_line("(defstruct rgb r g b)", &env);
        eval_line("(def color (make-rgb 255 128 0))", &env);
        assert_eq!(eval_line("(rgb-r color)", &env), "255");
        assert_eq!(eval_line("(rgb-g color)", &env), "128");
        assert_eq!(eval_line("(rgb-b color)", &env), "0");
    });
}

// ─── EVCON ───────────────────────────────────────────────────────────────────

#[test]
fn test_evcon_first_true() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        assert_eq!(eval_line("(evcon '((t 1) (t 2)))", &env), "1");
    });
}

#[test]
fn test_evcon_skips_false() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        assert_eq!(
            eval_line("(evcon '((nil 1) ((> 3 2) 99) (t 0)))", &env),
            "99"
        );
    });
}

#[test]
fn test_evcon_all_false() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        assert_eq!(eval_line("(evcon '((nil 1) (nil 2)))", &env), "()");
    });
}

// ─── Lisp 1.5 arithmetic aliases ─────────────────────────────────────────────

#[test]
fn test_arithmetic_aliases() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        assert_eq!(eval_line("(plus 3 4)", &env), "7");
        assert_eq!(eval_line("(difference 10 3)", &env), "7");
        assert_eq!(eval_line("(times 6 7)", &env), "42");
        assert_eq!(eval_line("(quotient 20 4)", &env), "5");
    });
}

#[test]
fn test_lessp_greaterp() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        assert_eq!(eval_line("(lessp 2 3)", &env), "T");
        assert_eq!(eval_line("(greaterp 5 1)", &env), "T");
        assert_eq!(eval_line("(lessp 5 1)", &env), "()");
    });
}

#[test]
fn test_remainder_truncated() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        // REMAINDER sign follows dividend; MOD is euclidean (always non-negative)
        assert_eq!(eval_line("(remainder -7 3)", &env), "-1");
        assert_eq!(eval_line("(mod -7 3)", &env), "2");
    });
}

// ─── GET as plist lookup ─────────────────────────────────────────────────────

#[test]
fn test_get_is_plist_lookup() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        eval_line("(putp 'mysym 'color 'red)", &env);
        assert_eq!(eval_line("(get 'mysym 'color)", &env), "RED");
    });
}

#[test]
fn test_define_sets_expr_plist() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        eval_line("(define '((dbl (lambda (x) (* x 2)))))", &env);
        assert_eq!(eval_line("(dbl 21)", &env), "42");
        // DEFINE also records the lambda under the EXPR indicator (Lisp 1.5)
        assert_eq!(eval_line("(get 'dbl 'EXPR)", &env), "<lambda>");
    });
}
