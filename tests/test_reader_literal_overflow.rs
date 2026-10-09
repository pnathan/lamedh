//! Issue #515: a decimal integer literal outside `i64` reads as a float
//! (KERNEL.md Part II, unchanged), but the precision loss must now be
//! observable: reading it sets the global `OVERFLOW` flag, exactly like
//! overflowing integer arithmetic. Q/H/radix literals stay parse errors.
use lamedh::environment::Environment;
use lamedh::printer::print;
use lamedh::reader::read_all;
use lamedh::{LispVal, Shared};

fn read_in(src: &str) -> (Result<Vec<LispVal>, String>, bool) {
    let env = Shared::new(Environment::new());
    let r = read_all(src, &env);
    (r, env.flag_set("OVERFLOW"))
}

#[test]
fn oversized_decimal_reads_as_float_and_sets_overflow() {
    for src in [
        "9223372036854775808",
        "-9223372036854775809",
        "99999999999999999999",
    ] {
        let (r, flag) = read_in(src);
        assert!(
            matches!(r.unwrap().as_slice(), [LispVal::Float(_)]),
            "{src}"
        );
        assert!(flag, "{src} must set OVERFLOW");
    }
}

#[test]
fn oversized_decimal_inside_a_list_sets_overflow() {
    let (r, flag) = read_in("(list 1 9223372036854775808)");
    assert!(r.is_ok());
    assert!(flag);
}

#[test]
fn oversized_decimal_in_dotted_pair_sets_overflow() {
    let (r, flag) = read_in("(1 . 99999999999999999999)");
    assert!(r.is_ok());
    assert!(flag);
}

#[test]
fn in_range_and_explicit_float_literals_leave_overflow_clear() {
    for src in [
        "9223372036854775807",
        "-9223372036854775808",
        "0",
        "99999999999999999999.0",
        "1e30",
        "1.0e999",
        "-8000000000000000H",
        "-0",
        "000000000000000000000000000001",
        "1e5h",
        "#x7FFFFFFFFFFFFFFF",
    ] {
        let (r, flag) = read_in(src);
        assert!(r.is_ok(), "{src}");
        assert!(!flag, "{src} must not set OVERFLOW");
    }
}

#[test]
fn out_of_range_q_h_radix_stay_errors_without_flag() {
    for src in [
        "1000000000000000000000Q",
        "8000000000000000H",
        "#x8000000000000000",
    ] {
        let (r, flag) = read_in(src);
        assert!(r.is_err(), "{src}");
        assert!(!flag, "{src} must not set OVERFLOW");
    }
}

#[test]
fn oversized_literal_in_a_discarded_conditional_form_sets_nothing() {
    let (r, flag) = read_in("#+NOSUCHFEATURE 99999999999999999999 1");
    assert_eq!(r.unwrap().len(), 1);
    assert!(!flag, "a skipped form was never read");
    // A kept form still signals; a pre-existing flag survives a skip.
    let (_, flag) = read_in("#-NOSUCHFEATURE 99999999999999999999");
    assert!(flag);
    let env = Shared::new(Environment::new());
    env.set_flag("OVERFLOW");
    read_all("#+NOSUCHFEATURE 99999999999999999999 1", &env).unwrap();
    assert!(env.flag_set("OVERFLOW"));
}

#[test]
fn flag_is_visible_from_lisp_and_clearable() {
    let env = Environment::with_stdlib();
    let forms = read_all("(clear-flag 'OVERFLOW)", &env).unwrap();
    assert_eq!(print(&forms[0]), "(CLEAR-FLAG (QUOTE OVERFLOW))");
    assert!(!env.flag_set("OVERFLOW"));
    read_all("9223372036854775808", &env).unwrap();
    assert!(env.flag_set("OVERFLOW"));
    env.clear_flag("OVERFLOW");
    assert!(!env.flag_set("OVERFLOW"));
}
