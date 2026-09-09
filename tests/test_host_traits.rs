// Issue #463: `+HOST-TRAITS+` / `+NUMERIC-PRECISION-MODEL+` — an
// introspectable global exposing KERNEL.md Part XII's declared-axis
// choices, so portable Lamedh code can ask which model a host implements
// instead of probing for it behaviorally (e.g. deliberately overflowing a
// computation and checking `(flag-set-p 'OVERFLOW)`).

use lamedh::LispVal;
use lamedh::environment::Environment;
use lamedh::evaluator::eval;
use lamedh::reader::read;

fn eval_str(input: &str) -> Result<LispVal, String> {
    let env = Environment::new_with_builtins();
    let expr = read(input, &env).map_err(|e| format!("Parse error: {}", e))?;
    eval(&expr, &env).map_err(|e| format!("Eval error: {:?}", e))
}

fn eval_str_stdlib(input: &str) -> Result<LispVal, String> {
    let env = Environment::with_stdlib();
    let expr = read(input, &env).map_err(|e| format!("Parse error: {}", e))?;
    eval(&expr, &env).map_err(|e| format!("Eval error: {:?}", e))
}

fn assert_symbol_named(val: &LispVal, expected: &str) {
    match val {
        LispVal::Symbol(s) => assert_eq!(s.borrow().name, expected),
        other => panic!("expected symbol {expected}, got {other:?}"),
    }
}

fn lisp_list_to_vec(val: &LispVal) -> Vec<LispVal> {
    let mut out = Vec::new();
    let mut cur = val.clone();
    loop {
        match cur {
            LispVal::Cons { car, cdr } => {
                out.push((*car).clone());
                cur = (*cdr).clone();
            }
            LispVal::Nil => break,
            other => {
                out.push(other);
                break;
            }
        }
    }
    out
}

/// The reader must be able to parse the `+EARMUFF+` token at all —
/// `+NUMERIC-PRECISION-MODEL+` is unreadable without a reader-level fix
/// (verified against the reader as it stood before this change: a bare
/// leading `+` is consumed as the `+` operator symbol on its own, stranding
/// the rest of the token as a wrongly-named symbol with a trailing `+`).
#[test]
fn plus_earmuff_symbols_are_readable() {
    let result = eval_str("(quote +NUMERIC-PRECISION-MODEL+)").unwrap();
    assert_symbol_named(&result, "+NUMERIC-PRECISION-MODEL+");

    let result = eval_str("(quote +HOST-TRAITS+)").unwrap();
    assert_symbol_named(&result, "+HOST-TRAITS+");
}

/// Ordinary arithmetic symbols that merely start or end with `+` (but are
/// not a `+earmuff+` pair) must still read exactly as before.
#[test]
fn plain_plus_operator_still_reads_as_before() {
    assert_eq!(eval_str("(+ 1 2)").unwrap(), LispVal::Number(3));
    assert_symbol_named(&eval_str("(quote +)").unwrap(), "+");
    assert_eq!(eval_str("(1+ 5)").unwrap(), LispVal::Number(6));
}

/// `+NUMERIC-PRECISION-MODEL+` is bound at environment-construction time
/// (`Environment::new_with_builtins`, which every other constructor —
/// `with_stdlib`, `with_prelude`, sandboxed environments — builds on), to
/// `WRAPAROUND-64`: `src/evaluator/builtins_core.rs`'s `+`/`-`/`*`//`
/// (`BuiltinFunc::Plus`/`Minus`/`Multiply`/`Divide`) use `checked_*`
/// arithmetic over `i64` that falls back to `wrapping_*` and sets the
/// `OVERFLOW` flag on over/underflow, rather than promoting to an
/// arbitrary-precision representation.
#[test]
fn numeric_precision_model_is_wraparound_64() {
    let result = eval_str("+NUMERIC-PRECISION-MODEL+").unwrap();
    assert_symbol_named(&result, "WRAPAROUND-64");
}

/// The verified claim behind the value above: overflowing arithmetic
/// actually wraps (rather than, say, panicking or promoting), and sets
/// `OVERFLOW` when it does.
#[test]
fn overflowing_arithmetic_actually_wraps_and_flags_overflow() {
    let input = r#"
        (PROGN
            (CLEAR-ALL-FLAGS)
            (SETQ RESULT (PLUS 9223372036854775807 1))
            (LIST RESULT (FLAG-SET-P 'OVERFLOW)))
    "#;
    let result = eval_str(input).unwrap();
    let items = lisp_list_to_vec(&result);
    assert_eq!(items.len(), 2);
    // i64::MAX + 1 wraps to i64::MIN under two's-complement wrapping.
    assert_eq!(items[0], LispVal::Number(i64::MIN));
    assert_symbol_named(&items[1], "T");
}

/// `+HOST-TRAITS+` is the generalized registry (an alist of
/// `(AXIS-NAME . CHOSEN-VALUE)` pairs) that `+NUMERIC-PRECISION-MODEL+` is
/// drawn from — its `NUMERIC-PRECISION-MODEL` entry must agree with the
/// standalone constant.
#[test]
fn host_traits_registry_contains_matching_entry() {
    let input = "(cdr (assoc 'NUMERIC-PRECISION-MODEL +HOST-TRAITS+))";
    let result = eval_str(input).unwrap();
    assert_symbol_named(&result, "WRAPAROUND-64");
}

/// Both globals are ordinary Lamedh code, reachable exactly like any other
/// global on a fully-loaded `with_stdlib()` environment (not just the bare
/// kernel) — including through `print`, per the issue's example.
#[test]
fn accessible_from_ordinary_code_on_stdlib_environment() {
    // PRINT (Lisp 1.5) writes its argument and returns NIL, so this only
    // confirms `print` evaluates its argument without error; the value
    // itself is checked directly below.
    let result = eval_str_stdlib("(print +NUMERIC-PRECISION-MODEL+)").unwrap();
    assert_eq!(result, LispVal::Nil);

    let result = eval_str_stdlib("+NUMERIC-PRECISION-MODEL+").unwrap();
    assert_symbol_named(&result, "WRAPAROUND-64");

    let result = eval_str_stdlib(
        "(let ((entry (assoc 'NUMERIC-PRECISION-MODEL +HOST-TRAITS+))) (eq (cdr entry) +NUMERIC-PRECISION-MODEL+))",
    )
    .unwrap();
    assert_symbol_named(&result, "T");
}
