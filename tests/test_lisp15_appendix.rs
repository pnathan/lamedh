/// Tests for Lisp 1.5 Appendix A functions added in lib/09-lisp15.lisp
/// and related Rust-level fixes.
mod test_helpers;
use lamedh::eval_line;
use lamedh::with_large_stack;
use test_helpers::env_with_stdlib;

// ─── PAIR ────────────────────────────────────────────────────────────────────

#[test]
fn test_pair_basic() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        let r = eval_line("(pair '(a b c) '(1 2 3))", &env);
        assert_eq!(r, "((A . 1) (B . 2) (C . 3))");
    });
}

#[test]
fn test_pair_empty() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        assert_eq!(eval_line("(pair '() '())", &env), "()");
    });
}

#[test]
fn test_pair_truncates_to_shorter() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        // stops when either list is exhausted
        let r = eval_line("(pair '(a b) '(1 2 3))", &env);
        assert_eq!(r, "((A . 1) (B . 2))");
    });
}

// ─── ATTRIB ──────────────────────────────────────────────────────────────────

#[test]
fn test_attrib_basic() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        // attrib returns e (the second arg)
        let r = eval_line("(attrib '(a b) '(c d))", &env);
        assert_eq!(r, "(C D)");
    });
}

// ─── PROP ────────────────────────────────────────────────────────────────────

#[test]
fn test_prop_found() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        // prop returns the cdr of the plist starting after the indicator
        let r = eval_line("(prop '(x 1 y 2 z 3) 'y (lambda () 'miss))", &env);
        assert_eq!(r, "(2 Z 3)");
    });
}

#[test]
fn test_prop_not_found() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        let r = eval_line("(prop '(x 1 y 2) 'q (lambda () 'miss))", &env);
        assert_eq!(r, "MISS");
    });
}

// ─── FLAG / REMFLAG ──────────────────────────────────────────────────────────

#[test]
fn test_flag_sets_indicator() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        eval_line("(flag '(foo bar) 'traced)", &env);
        assert_eq!(eval_line("(getp 'foo 'traced)", &env), "T");
        assert_eq!(eval_line("(getp 'bar 'traced)", &env), "T");
    });
}

#[test]
fn test_remflag_removes_indicator() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        eval_line("(flag '(foo) 'traced)", &env);
        eval_line("(remflag '(foo) 'traced)", &env);
        assert_eq!(eval_line("(getp 'foo 'traced)", &env), "()");
    });
}

// ─── MAP ─────────────────────────────────────────────────────────────────────

#[test]
fn test_map_returns_nil() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        // map returns NIL
        let r = eval_line("(map '(1 2 3) (lambda (x) x))", &env);
        assert_eq!(r, "()");
    });
}

#[test]
fn test_map_side_effects() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        // use map to collect cars of sublists via setq
        eval_line("(setq acc '())", &env);
        eval_line(
            "(map '(1 2 3) (lambda (x) (setq acc (cons (car x) acc))))",
            &env,
        );
        // acc should be (3 2 1) - reversed because cons prepends
        assert_eq!(eval_line("acc", &env), "(3 2 1)");
    });
}

// ─── SEARCH ──────────────────────────────────────────────────────────────────

#[test]
fn test_search_found() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        let r = eval_line(
            "(search '(a b c d) (lambda (x) (eq x 'c)) (lambda (x) (list 'found x)) (lambda (x) 'miss))",
            &env,
        );
        assert_eq!(r, "(FOUND C)");
    });
}

#[test]
fn test_search_not_found() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        let r = eval_line(
            "(search '(a b c) (lambda (x) (eq x 'z)) (lambda (x) 'found) (lambda (x) 'miss))",
            &env,
        );
        assert_eq!(r, "MISS");
    });
}

// ─── RECIP ───────────────────────────────────────────────────────────────────

#[test]
fn test_recip_float() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        assert_eq!(eval_line("(recip 4.0)", &env), "0.25");
    });
}

#[test]
fn test_recip_integer_promotes() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        assert_eq!(eval_line("(recip 2.0)", &env), "0.5");
    });
}

// ─── SELECT ──────────────────────────────────────────────────────────────────

#[test]
fn test_select_matches_first() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        let r = eval_line("(select 'a ('a 1) ('b 2) ('c 3) 99)", &env);
        assert_eq!(r, "1");
    });
}

#[test]
fn test_select_matches_middle() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        let r = eval_line("(select 'b ('a 1) ('b 2) ('c 3) 99)", &env);
        assert_eq!(r, "2");
    });
}

#[test]
fn test_select_default() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        let r = eval_line("(select 'z ('a 1) ('b 2) 99)", &env);
        assert_eq!(r, "99");
    });
}

#[test]
fn test_select_numeric_keys() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        let r = eval_line("(select 2 (1 'one) (2 'two) (3 'three) 'other)", &env);
        assert_eq!(r, "TWO");
    });
}

// ─── Float arithmetic ────────────────────────────────────────────────────────

#[test]
fn test_float_addition() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        assert_eq!(eval_line("(+ 1.5 2.5)", &env), "4.0");
    });
}

#[test]
fn test_float_division() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        assert_eq!(eval_line("(/ 1.0 4.0)", &env), "0.25");
    });
}

#[test]
fn test_float_multiplication() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        assert_eq!(eval_line("(* 2.0 3.0)", &env), "6.0");
    });
}

#[test]
fn test_mixed_arithmetic_promotes_to_float() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        assert_eq!(eval_line("(floatp (+ 1 2.0))", &env), "T");
    });
}

// ─── EXPT enhancements ───────────────────────────────────────────────────────

#[test]
fn test_expt_float_base() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        assert_eq!(eval_line("(expt 2.0 10)", &env), "1024.0");
    });
}

#[test]
fn test_expt_negative_exponent_float() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        assert_eq!(eval_line("(expt 2 -1)", &env), "0.5");
    });
}

// ─── REMPROP / DEFLIST accept symbols as indicator ───────────────────────────

#[test]
fn test_remprop_symbol_indicator() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        eval_line("(putp 'mysym 'myind 42)", &env);
        eval_line("(remprop 'mysym 'myind)", &env);
        assert_eq!(eval_line("(getp 'mysym 'myind)", &env), "()");
    });
}

#[test]
fn test_deflist_symbol_indicator() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        let r = eval_line("(deflist '((foo (lambda (x) x))) 'expr)", &env);
        assert_eq!(r, "T");
    });
}

#[test]
fn test_getp_symbol_indicator() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        eval_line("(putp 'sym 'key 99)", &env);
        assert_eq!(eval_line("(getp 'sym 'key)", &env), "99");
    });
}
