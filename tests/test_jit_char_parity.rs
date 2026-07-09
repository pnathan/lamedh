//! Differential CODE-CHAR / CHAR-CODE parity tests between the tree-walking
//! evaluator and the typed tiers (issue #281).
//!
//! Two conformance fixes, plus one intentional, documented residual deviation:
//!
//! * **CODE-CHAR** — the typed island's `char` is deliberately a byte
//!   (`(array char)` == string, #137), so CODE-CHAR is only representable for
//!   code points `0..=255`. It used to silently mask an out-of-range argument
//!   to a byte (`(code-char 321)` → `#\A`, a #209-direction silent wrong
//!   value). It now records an error: for a negative argument, the evaluator's
//!   own "non-negative integer" message verbatim; for an argument above 255 —
//!   which the evaluator *accepts*, returning a multi-byte string the typed
//!   island cannot represent — a typed-island range message. The RESIDUAL,
//!   INTENTIONAL deviation: in range, the typed CODE-CHAR yields a `char` byte
//!   while the evaluator yields a one-character string. That representational
//!   difference is by design (the (array char) == string island); the bug that
//!   #281 fixes is only the silent wrong-value masking for out-of-range inputs.
//!
//! * **CHAR-CODE** — the evaluator accepts a char *or* a one-character string.
//!   The typed checker accepted only `char`, so `check-type` could reject a
//!   program the interpreter runs (#202 direction). In checker mode it now also
//!   accepts a string / `(array char)` operand. In codegen mode a scalar
//!   `char` is still required (a boxed string is not an unboxed scalar char),
//!   which is correct.

use lamedh::environment::Environment;
use lamedh::eval_line;
use lamedh::jit::{Jit, Value};
use lamedh::reader::read;

fn jit_with(defs: &[&str]) -> Jit {
    let env = Environment::new_with_builtins();
    let mut j = Jit::new();
    for src in defs {
        let form = read(src, &env).unwrap_or_else(|e| panic!("read failed for `{src}`: {e}"));
        j.define(&form)
            .unwrap_or_else(|e| panic!("define failed for `{src}`: {e}"));
    }
    j
}

/// (compiled, deopt-interpreter, traced) results for `name(args)`.
fn call_all_tiers(j: &Jit, name: &str, args: &[Value]) -> [Result<Value, String>; 3] {
    j.compile_all();
    let compiled = j.call(name, args);
    j.deoptimize_all();
    let interpreted = j.call(name, args);
    let traced = j.trace_call(name, args).map(|(v, _log)| v);
    [compiled, interpreted, traced]
}

fn assert_all_tiers_error(j: &Jit, name: &str, args: &[Value], expected: &str) {
    let labels = ["compiled", "deopt-interpreter", "traced"];
    for (tier, r) in labels.iter().zip(call_all_tiers(j, name, args)) {
        match r {
            Err(msg) => assert_eq!(msg, expected, "{tier} tier: message must match"),
            Ok(v) => panic!("{tier} tier: expected error `{expected}`, got {v:?}"),
        }
    }
}

// ---- CODE-CHAR: out-of-range errors ---------------------------------------

#[test]
fn code_char_negative_matches_evaluator_all_tiers() {
    // The evaluator errors on a negative code point; the typed tiers reuse the
    // exact same message.
    let ev = eval_line("(code-char -1)", &Environment::with_stdlib());
    assert_eq!(
        ev,
        "Error: CODE-CHAR: expected a non-negative integer, got -1"
    );
    let j = jit_with(&["(defun-typed (mk char) ((n int64)) (code-char n))"]);
    assert_all_tiers_error(
        &j,
        "mk",
        &[Value::Int(-1)],
        "CODE-CHAR: expected a non-negative integer, got -1",
    );
}

#[test]
fn code_char_above_byte_range_errors_all_tiers() {
    // 321 is a valid Unicode code point the evaluator turns into a string, but
    // the typed island's byte-`char` cannot hold it. Instead of the old silent
    // mask to `#\A` (321 & 0xff == 65), every typed tier now errors.
    let j = jit_with(&["(defun-typed (mk char) ((n int64)) (code-char n))"]);
    assert_all_tiers_error(
        &j,
        "mk",
        &[Value::Int(321)],
        "CODE-CHAR: code point 321 is outside the typed char range 0-255",
    );
    // The evaluator, by contrast, represents it (residual, documented
    // deviation): it returns a one-character string, not an error.
    let ev = eval_line("(code-char 321)", &Environment::with_stdlib());
    assert!(
        !ev.to_lowercase().contains("error"),
        "evaluator represents code point 321 as a string: {ev}"
    );
}

#[test]
fn code_char_in_range_still_works_all_tiers() {
    let j = jit_with(&["(defun-typed (mk char) ((n int64)) (code-char n))"]);
    for (n, byte) in [(0u8, 0u8), (65, 65), (255, 255)] {
        for r in call_all_tiers(&j, "mk", &[Value::Int(n as i64)]) {
            assert_eq!(r.unwrap(), Value::Char(byte), "in-range code-char({n})");
        }
    }
}

// ---- CHAR-CODE: checker-mode acceptance -----------------------------------

#[test]
fn char_code_checker_accepts_string_operand() {
    // `check-type` must not reject `(char-code <string>)`: the evaluator runs
    // it (first char's code point). A char operand still checks, and a genuine
    // non-char/non-string operand (int64) is still rejected.
    let mut j = Jit::new();
    let env = Environment::new_with_builtins();

    let s_form = read("(char-code \"abc\")", &env).unwrap();
    assert_eq!(
        j.check_expr(&s_form, None).unwrap(),
        "int64",
        "char-code of a string must check in checker mode (#281)"
    );

    // The evaluator indeed runs the same expression rather than erroring.
    let ev = eval_line("(char-code \"abc\")", &Environment::with_stdlib());
    assert_eq!(
        ev, "97",
        "evaluator char-code of a string is the first code point"
    );

    let n_form = read("(char-code 5)", &env).unwrap();
    assert!(
        j.check_expr(&n_form, None).is_err(),
        "char-code of a plain int64 is still a type error"
    );
}

#[test]
fn char_code_codegen_still_requires_char() {
    // In codegen mode a scalar `char` operand is required (and works); a boxed
    // string is not a scalar char, so a `defun-typed` body applying char-code
    // to a whole `(array char)` argument is correctly rejected.
    let j = jit_with(&["(defun-typed (code int64) ((c char)) (char-code c))"]);
    j.compile_all();
    assert_eq!(j.call("code", &[Value::Char(65)]).unwrap(), Value::Int(65));

    let env = Environment::new_with_builtins();
    let mut j2 = Jit::new();
    let form = read(
        "(defun-typed (bad int64) ((s (array char))) (char-code s))",
        &env,
    )
    .unwrap();
    assert!(
        j2.define(&form).is_err(),
        "codegen must reject char-code on a whole string (not a scalar char)"
    );
}
