/// Tests targeting specific uncovered paths in evaluator.rs (coverage run 9).
/// Each group corresponds to a specific set of uncovered lines.
mod test_helpers;
use lamedh::{eval_line, with_large_stack};
use test_helpers::env_with_stdlib;

// ============================================================================
// Group 1: LABEL self-reference (line ~1515)
// ============================================================================

#[test]
fn test_label_self_reference() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        let result = eval_line("(label x x)", &env);
        assert!(
            result.contains("Error"),
            "label self-reference should error; got: {result}"
        );
        assert!(
            result.contains("pathological self-reference"),
            "expected 'pathological self-reference'; got: {result}"
        );
    });
}

// ============================================================================
// Group 2: APPLY with unbound function (line ~1531)
// ============================================================================

#[test]
fn test_apply_unbound_function() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        let result = eval_line("(apply 'xyzzy-unbound-9999 '(1 2))", &env);
        assert!(
            result.contains("Error"),
            "apply with unbound function should error; got: {result}"
        );
        // Error message includes "Function not found" or generic error
        assert!(
            result.contains("Function") || result.contains("Error"),
            "expected function error; got: {result}"
        );
    });
}

// ============================================================================
// Group 3: MAPLIST wrong arity (3+ args) (line ~1650)
// ============================================================================

#[test]
fn test_maplist_three_args() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        let result = eval_line("(maplist 'car '(1 2) 'extra)", &env);
        assert!(
            result.contains("Error"),
            "maplist with 3 args should error; got: {result}"
        );
        assert!(
            result.contains("maplist requires exactly two arguments"),
            "expected 'maplist requires exactly two arguments'; got: {result}"
        );
    });
}

// ============================================================================
// Group 4: MAPCAR wrong arity (3+ args) (line ~1656)
// ============================================================================

#[test]
fn test_mapcar_three_args() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        let result = eval_line("(mapcar 'car '(1 2) 'extra)", &env);
        assert!(
            result.contains("Error"),
            "mapcar with 3 args should error; got: {result}"
        );
        assert!(
            result.contains("mapcar requires exactly two arguments"),
            "expected 'mapcar requires exactly two arguments'; got: {result}"
        );
    });
}

// ============================================================================
// Group 5: RPLACA wrong arity (3 args) (line ~1740)
// ============================================================================

#[test]
fn test_rplaca_three_args() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        let result = eval_line("(rplaca '(1 2) 3 4)", &env);
        assert!(
            result.contains("Error"),
            "rplaca with 3 args should error; got: {result}"
        );
        assert!(
            result.contains("rplaca requires exactly two arguments"),
            "expected 'rplaca requires exactly two arguments'; got: {result}"
        );
    });
}

// ============================================================================
// Group 6: RPLACA on non-cons (line ~1742)
// ============================================================================

#[test]
fn test_rplaca_non_cons() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        let result = eval_line("(rplaca 5 10)", &env);
        assert!(
            result.contains("Error"),
            "rplaca on non-cons should error; got: {result}"
        );
        assert!(
            result.contains("rplaca requires a cons cell"),
            "expected 'rplaca requires a cons cell'; got: {result}"
        );
    });
}

// ============================================================================
// Group 7: RPLACD wrong arity (3 args) (line ~1750)
// ============================================================================

#[test]
fn test_rplacd_three_args() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        let result = eval_line("(rplacd '(1 2) 3 4)", &env);
        assert!(
            result.contains("Error"),
            "rplacd with 3 args should error; got: {result}"
        );
        assert!(
            result.contains("rplacd requires exactly two arguments"),
            "expected 'rplacd requires exactly two arguments'; got: {result}"
        );
    });
}

// ============================================================================
// Group 8: RPLACD on non-cons (line ~1752)
// ============================================================================

#[test]
fn test_rplacd_non_cons() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        let result = eval_line("(rplacd 5 10)", &env);
        assert!(
            result.contains("Error"),
            "rplacd on non-cons should error; got: {result}"
        );
        assert!(
            result.contains("rplacd requires a cons cell"),
            "expected 'rplacd requires a cons cell'; got: {result}"
        );
    });
}

// ============================================================================
// Group 9: LOGAND with non-integer second arg (line ~1830)
// ============================================================================

#[test]
fn test_logand_non_integer_second_arg() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        let result = eval_line(r#"(logand 1 "hello")"#, &env);
        assert!(
            result.contains("Error"),
            "logand with non-integer second arg should error; got: {result}"
        );
        assert!(
            result.contains("logand requires integer arguments"),
            "expected 'logand requires integer arguments'; got: {result}"
        );
    });
}

// ============================================================================
// Group 10: LEFTSHIFT wrong arity (line ~1843)
// ============================================================================

#[test]
fn test_leftshift_three_args() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        let result = eval_line("(leftshift 1 2 3)", &env);
        assert!(
            result.contains("Error"),
            "leftshift with 3 args should error; got: {result}"
        );
        assert!(
            result.contains("leftshift requires exactly two arguments"),
            "expected 'leftshift requires exactly two arguments'; got: {result}"
        );
    });
}

// ============================================================================
// Group 11: LAST wrong arity (line ~1906)
// ============================================================================

#[test]
fn test_last_two_args() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        let result = eval_line("(last '(1 2) '(3))", &env);
        assert!(
            result.contains("Error"),
            "last with 2 args should error; got: {result}"
        );
        assert!(
            result.contains("last requires exactly one argument"),
            "expected 'last requires exactly one argument'; got: {result}"
        );
    });
}

// ============================================================================
// Group 12: NTH wrong arity (line ~1914)
// ============================================================================

#[test]
fn test_nth_three_args() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        let result = eval_line("(nth 0 '(1 2) 'extra)", &env);
        assert!(
            result.contains("Error"),
            "nth with 3 args should error; got: {result}"
        );
        assert!(
            result.contains("nth requires exactly two arguments"),
            "expected 'nth requires exactly two arguments'; got: {result}"
        );
    });
}

// ============================================================================
// Group 13: NTH past end of list returns nil (line ~1922)
// ============================================================================

#[test]
fn test_nth_past_end_returns_nil() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        let result = eval_line("(nth 5 '(1 2 3))", &env);
        assert!(
            !result.contains("Error"),
            "nth past end should return nil; got: {result}"
        );
        assert_eq!(result, "()", "nth past end should return ()");
    });
}

// ============================================================================
// Group 14: NTHCDR wrong arity (line ~1926)
// ============================================================================

#[test]
fn test_nthcdr_three_args() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        let result = eval_line("(nthcdr 1 '(1 2) 'extra)", &env);
        assert!(
            result.contains("Error"),
            "nthcdr with 3 args should error; got: {result}"
        );
        assert!(
            result.contains("nthcdr requires exactly two arguments"),
            "expected 'nthcdr requires exactly two arguments'; got: {result}"
        );
    });
}

// ============================================================================
// Group 15: MOD wrong arity (line ~1958)
// ============================================================================

#[test]
fn test_mod_three_args() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        let result = eval_line("(mod 10 3 2)", &env);
        assert!(
            result.contains("Error"),
            "mod with 3 args should error; got: {result}"
        );
        assert!(
            result.contains("mod requires exactly two arguments"),
            "expected 'mod requires exactly two arguments'; got: {result}"
        );
    });
}

// ============================================================================
// Group 16: PLUSP wrong arity (line ~1992)
// ============================================================================

#[test]
fn test_plusp_two_args() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        let result = eval_line("(plusp 1 2)", &env);
        assert!(
            result.contains("Error"),
            "plusp with 2 args should error; got: {result}"
        );
        assert!(
            result.contains("plusp requires exactly one argument"),
            "expected 'plusp requires exactly one argument'; got: {result}"
        );
    });
}

// ============================================================================
// Group 17: PLUSP with negative float returns nil (line ~1994)
// ============================================================================

#[test]
fn test_plusp_negative_float() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        let result = eval_line("(plusp -1.5)", &env);
        assert!(
            !result.contains("Error"),
            "plusp of negative float should succeed; got: {result}"
        );
        assert_eq!(result, "()", "plusp of negative float should return ()");
    });
}

// ============================================================================
// Group 18: EVENP wrong arity (line ~2001)
// ============================================================================

#[test]
fn test_evenp_two_args() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        let result = eval_line("(evenp 2 3)", &env);
        assert!(
            result.contains("Error"),
            "evenp with 2 args should error; got: {result}"
        );
        assert!(
            result.contains("evenp requires exactly one argument"),
            "expected 'evenp requires exactly one argument'; got: {result}"
        );
    });
}

// ============================================================================
// Group 19: ODDP wrong arity (line ~2007)
// ============================================================================

#[test]
fn test_oddp_two_args() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        let result = eval_line("(oddp 3 2)", &env);
        assert!(
            result.contains("Error"),
            "oddp with 2 args should error; got: {result}"
        );
        assert!(
            result.contains("oddp requires exactly one argument"),
            "expected 'oddp requires exactly one argument'; got: {result}"
        );
    });
}

// ============================================================================
// Group 20: ADD1 wrong arity (line ~2013)
// ============================================================================

#[test]
fn test_add1_two_args() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        let result = eval_line("(add1 1 2)", &env);
        assert!(
            result.contains("Error"),
            "add1 with 2 args should error; got: {result}"
        );
        assert!(
            result.contains("add1 requires exactly one argument"),
            "expected 'add1 requires exactly one argument'; got: {result}"
        );
    });
}

// ============================================================================
// Group 21: SUB1 wrong arity (line ~2019)
// ============================================================================

#[test]
fn test_sub1_two_args() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        let result = eval_line("(sub1 1 2)", &env);
        assert!(
            result.contains("Error"),
            "sub1 with 2 args should error; got: {result}"
        );
        assert!(
            result.contains("sub1 requires exactly one argument"),
            "expected 'sub1 requires exactly one argument'; got: {result}"
        );
    });
}

// ============================================================================
// Group 22: RANDOM wrong arity (line ~2025)
// ============================================================================

#[test]
fn test_random_two_args() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        let result = eval_line("(random 5 10)", &env);
        assert!(
            result.contains("Error"),
            "random with 2 args should error; got: {result}"
        );
        assert!(
            result.contains("random requires exactly one argument"),
            "expected 'random requires exactly one argument'; got: {result}"
        );
    });
}

// ============================================================================
// Group 23: FUNCALL with unbound symbol (line ~1810)
// ============================================================================

#[test]
fn test_funcall_unbound_symbol() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        let result = eval_line("(funcall 'xyzzy-unbound-fn-9999 1 2)", &env);
        assert!(
            result.contains("Error"),
            "funcall with unbound symbol should error; got: {result}"
        );
        // Error message includes "Function" or "not found"
        assert!(
            result.contains("Function") || result.contains("Error"),
            "expected function error; got: {result}"
        );
    });
}

// ============================================================================
// Group 24: MACROEXPAND wrong arity (line ~1818)
// ============================================================================

#[test]
fn test_macroexpand_two_args() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        let result = eval_line("(macroexpand 1 2)", &env);
        assert!(
            result.contains("Error"),
            "macroexpand with 2 args should error; got: {result}"
        );
        assert!(
            result.contains("macroexpand requires exactly one argument"),
            "expected 'macroexpand requires exactly one argument'; got: {result}"
        );
    });
}

// ============================================================================
// Group 25: EXPLODE wrong arity (line ~2032)
// ============================================================================

#[test]
fn test_explode_two_args() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        let result = eval_line("(explode 'hello 'extra)", &env);
        assert!(
            result.contains("Error"),
            "explode with 2 args should error; got: {result}"
        );
        assert!(
            result.contains("explode requires exactly one argument"),
            "expected 'explode requires exactly one argument'; got: {result}"
        );
    });
}

// ============================================================================
// Group 26: IMPLODE wrong arity (line ~2042)
// ============================================================================

#[test]
fn test_implode_two_args() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        let result = eval_line(r#"(implode '(h e) 'extra)"#, &env);
        assert!(
            result.contains("Error"),
            "implode with 2 args should error; got: {result}"
        );
        assert!(
            result.contains("implode requires exactly one argument"),
            "expected 'implode requires exactly one argument'; got: {result}"
        );
    });
}

// ============================================================================
// Group 27: IMPLODE with string in list (line ~2052)
// ============================================================================

#[test]
fn test_implode_with_string_in_list() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        let result = eval_line(r#"(implode '("hello"))"#, &env);
        assert!(
            !result.contains("Error"),
            "implode with string in list should succeed; got: {result}"
        );
        // Should return a symbol (not error)
        // The string "hello" gets converted to its string characters
    });
}

// ============================================================================
// Group 28: MAKNAM wrong arity (line ~2066)
// ============================================================================

#[test]
fn test_maknam_two_args() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        let result = eval_line(r#"(maknam '(h e) 'extra)"#, &env);
        assert!(
            result.contains("Error"),
            "maknam with 2 args should error; got: {result}"
        );
        assert!(
            result.contains("maknam requires exactly one argument"),
            "expected 'maknam requires exactly one argument'; got: {result}"
        );
    });
}

// ============================================================================
// Group 29: MAKNAM with string in list (line ~2076)
// ============================================================================

#[test]
fn test_maknam_with_string_in_list() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        let result = eval_line(r#"(maknam '("hello"))"#, &env);
        assert!(
            !result.contains("Error"),
            "maknam with string in list should succeed; got: {result}"
        );
        // Should return a symbol (not error)
    });
}

// ============================================================================
// Group 30: GENSYM wrong arity (line ~2093)
// ============================================================================

#[test]
fn test_gensym_with_arg() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        let result = eval_line("(gensym 5)", &env);
        assert!(
            result.contains("Error"),
            "gensym with arg should error; got: {result}"
        );
        assert!(
            result.contains("gensym takes no arguments"),
            "expected 'gensym takes no arguments'; got: {result}"
        );
    });
}

// ============================================================================
// Group 31: INTERN wrong arity (line ~2102)
// ============================================================================

#[test]
fn test_intern_two_args() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        let result = eval_line("(intern 'a 'b)", &env);
        assert!(
            result.contains("Error"),
            "intern with 2 args should error; got: {result}"
        );
        assert!(
            result.contains("intern requires exactly one argument"),
            "expected 'intern requires exactly one argument'; got: {result}"
        );
    });
}

// ============================================================================
// Group 32: PLIST wrong arity (line ~2110)
// ============================================================================

#[test]
fn test_plist_two_args() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        let result = eval_line("(plist 'a 'b)", &env);
        assert!(
            result.contains("Error"),
            "plist with 2 args should error; got: {result}"
        );
        assert!(
            result.contains("plist requires exactly one argument"),
            "expected 'plist requires exactly one argument'; got: {result}"
        );
    });
}

// ============================================================================
// Group 33: ASH with right-shift >= 64 (line ~2160)
// ============================================================================

#[test]
fn test_ash_right_shift_64() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        let result = eval_line("(ash 5 -65)", &env);
        assert!(
            !result.contains("Error"),
            "ash with right-shift >= 64 should succeed; got: {result}"
        );
        assert_eq!(result, "0", "ash with right-shift >= 64 should return 0");
    });
}

// ============================================================================
// Group 34: ASH with left-shift >= 64 (line ~2160)
// ============================================================================

#[test]
fn test_ash_left_shift_64() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        let result = eval_line("(ash 1 65)", &env);
        assert!(
            !result.contains("Error"),
            "ash with left-shift >= 64 should succeed; got: {result}"
        );
        assert_eq!(result, "0", "ash with left-shift >= 64 should return 0");
    });
}

// ============================================================================
// Group 35: LOGNOT wrong arity (line ~2167)
// ============================================================================

#[test]
fn test_lognot_two_args() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        let result = eval_line("(lognot 1 2)", &env);
        assert!(
            result.contains("Error"),
            "lognot with 2 args should error; got: {result}"
        );
        assert!(
            result.contains("lognot requires exactly one argument"),
            "expected 'lognot requires exactly one argument'; got: {result}"
        );
    });
}

// ============================================================================
// Group 36: ROT wrong arity (line ~2186)
// ============================================================================

#[test]
fn test_rot_three_args() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        let result = eval_line("(rot 1 2 3)", &env);
        assert!(
            result.contains("Error"),
            "rot with 3 args should error; got: {result}"
        );
        assert!(
            result.contains("rot requires exactly two arguments"),
            "expected 'rot requires exactly two arguments'; got: {result}"
        );
    });
}

// ============================================================================
// Group 37: REMPROP wrong arity (line ~2212)
// ============================================================================

#[test]
fn test_remprop_one_arg() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        let result = eval_line("(remprop 'a)", &env);
        assert!(
            result.contains("Error"),
            "remprop with 1 arg should error; got: {result}"
        );
        assert!(
            result.contains("remprop requires exactly two arguments"),
            "expected 'remprop requires exactly two arguments'; got: {result}"
        );
    });
}

// ============================================================================
// Group 38: REMPROP non-symbol first arg (line ~2214)
// ============================================================================

#[test]
fn test_remprop_non_symbol_first_arg() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        let result = eval_line(r#"(remprop 5 "key")"#, &env);
        assert!(
            result.contains("Error"),
            "remprop with non-symbol first arg should error; got: {result}"
        );
        assert!(
            result.contains("remprop requires a symbol"),
            "expected 'remprop requires a symbol'; got: {result}"
        );
    });
}

// ============================================================================
// Group 39: REMPROP non-string second arg (line ~2216)
// ============================================================================

#[test]
fn test_remprop_non_string_second_arg() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        let result = eval_line("(remprop 'a 5)", &env);
        assert!(
            result.contains("Error"),
            "remprop with non-string second arg should error; got: {result}"
        );
        assert!(
            result.contains("remprop requires a symbol or string"),
            "expected 'remprop requires a symbol or string'; got: {result}"
        );
    });
}

// ============================================================================
// Group 40: DEFLIST wrong arity (line ~2877)
// ============================================================================

#[test]
fn test_deflist_one_arg() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        let result = eval_line("(deflist '((a . 1)))", &env);
        assert!(
            result.contains("Error"),
            "deflist with 1 arg should error; got: {result}"
        );
        assert!(
            result.contains("deflist requires exactly two arguments"),
            "expected 'deflist requires exactly two arguments'; got: {result}"
        );
    });
}

// ============================================================================
// Group 41: DEFLIST non-string second arg (line ~2879)
// ============================================================================

#[test]
fn test_deflist_non_string_second_arg() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        let result = eval_line("(deflist '((a . 1)) 5)", &env);
        assert!(
            result.contains("Error"),
            "deflist with non-string second arg should error; got: {result}"
        );
        assert!(
            result.contains("deflist requires a symbol or string"),
            "expected 'deflist requires a symbol or string'; got: {result}"
        );
    });
}

// ============================================================================
// Group 42: DEFLIST with symbol-less alist entry (line ~2870)
// ============================================================================

#[test]
fn test_deflist_non_symbol_alist_entry() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        // alist entry has Cons (1) as car, not a Symbol
        // This causes the inner if-let to fail and we skip to line 2870
        let result = eval_line(r#"(deflist '(((1) . value)) "key")"#, &env);
        assert!(
            !result.contains("Error"),
            "deflist with non-symbol alist entry should succeed; got: {result}"
        );
        assert_eq!(result, "T", "deflist should return T");
    });
}

// ============================================================================
// Group 43: SUBLIS multi-entry alist where first doesn't match (line ~2047)
// ============================================================================

#[test]
fn test_sublis_multi_entry_alist_no_match() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        // alist has (a . b) and (c . d), but we're substituting 'e
        // First entry doesn't match, so we continue to next
        let result = eval_line("(sublis '((a . b) (c . d)) 'e)", &env);
        assert!(
            !result.contains("Error"),
            "sublis with no-match alist should succeed; got: {result}"
        );
        assert_eq!(
            result, "E",
            "sublis with no-match should return original symbol"
        );
    });
}
