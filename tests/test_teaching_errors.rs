//! "Teaching errors" (see `src/teaching_errors.rs`): did-you-mean suggestions
//! and Common-Lisp-ism guidance appended to unbound-symbol / undefined-
//! function error messages.

mod test_helpers;

use lamedh::eval_line;
use test_helpers::env_with_stdlib;

#[test]
fn misspelling_gets_a_suggestion() {
    let e = env_with_stdlib();
    assert_eq!(
        eval_line("lenght", &e),
        "Error: Unbound variable: LENGHT — did you mean LENGTH?"
    );
}

#[test]
fn short_symbol_gets_no_suggestion() {
    let e = env_with_stdlib();
    // "ZZ" is only 2 characters -- below the length-3 floor, so no
    // suggestion is appended even though it is unbound.
    assert_eq!(eval_line("zz", &e), "Error: Unbound variable: ZZ");
}

#[test]
fn unbound_symbol_only_suggests_bound_names() {
    let e = env_with_stdlib();
    // Bind a fresh global whose name is one edit away from an otherwise
    // unbound, merely-plausible-looking symbol, and confirm the *bound*
    // name is the one suggested, not some interned-but-unbound look-alike.
    eval_line("(def my-custom-fn (lambda (x) x))", &e);
    assert_eq!(
        eval_line("my-custom-fnx", &e),
        "Error: Unbound variable: MY-CUSTOM-FNX — did you mean MY-CUSTOM-FN?"
    );
}

#[test]
fn unbound_symbol_suggestion_excludes_unbound_lookalikes() {
    let e = env_with_stdlib();
    // Referencing a symbol only as quoted data interns it without binding
    // it. It must never show up as a "did you mean" suggestion.
    eval_line("(quote totally-unbound-lookalike)", &e);
    assert_eq!(
        eval_line("totally-unbound-lookalikee", &e),
        "Error: Unbound variable: TOTALLY-UNBOUND-LOOKALIKEE"
    );
}

#[test]
fn cl_ism_loop() {
    let e = env_with_stdlib();
    assert_eq!(
        eval_line("(loop for i from 1 to 3 collect i)", &e),
        "Error: Unbound variable: LOOP is Common Lisp, not Lamedh — use DOTIMES, WHILE, or MAP"
    );
}

#[test]
fn cl_ism_defstruct() {
    let e = env_with_stdlib();
    assert_eq!(
        eval_line("(defstruct point x y)", &e),
        "Error: Unbound variable: DEFSTRUCT is Common Lisp, not Lamedh — removed in 0.3 — use DEFRECORD"
    );
}

#[test]
fn cl_ism_defclass() {
    let e = env_with_stdlib();
    assert_eq!(
        eval_line("(defclass point () ())", &e),
        "Error: Unbound variable: DEFCLASS is Common Lisp, not Lamedh — use DEFPROTOCOL and DEFINSTANCE"
    );
}

#[test]
fn cl_ism_with_open_file() {
    let e = env_with_stdlib();
    assert_eq!(
        eval_line("(with-open-file (f \"x\") 1)", &e),
        "Error: Unbound variable: WITH-OPEN-FILE is Common Lisp, not Lamedh — use WITH-OPEN-PORT"
    );
}

#[test]
fn cl_ism_multiple_value_bind() {
    let e = env_with_stdlib();
    let out = eval_line("(multiple-value-bind (a b) (values 1 2) a)", &e);
    assert_eq!(
        out,
        "Error: Unbound variable: MULTIPLE-VALUE-BIND is Common Lisp, not Lamedh — Lamedh has no multiple return values — return a LIST and use DESTRUCTURING-BIND"
    );
}

#[test]
fn cl_ism_takes_precedence_over_did_you_mean() {
    let e = env_with_stdlib();
    // LOOP is one edit away from bound names like LOG (math) or CAR-adjacent
    // symbols; regardless of any near bound name, the CL-ism redirect must
    // win over a fuzzy did-you-mean guess.
    let out = eval_line("loop", &e);
    assert_eq!(
        out,
        "Error: Unbound variable: LOOP is Common Lisp, not Lamedh — use DOTIMES, WHILE, or MAP"
    );
    assert!(!out.contains("did you mean"));
}

#[test]
fn undefined_function_via_function_special_form_gets_suggestion() {
    let e = env_with_stdlib();
    assert_eq!(
        eval_line("(function lenght)", &e),
        "Error: Undefined function: LENGHT — did you mean LENGTH?"
    );
}

#[test]
fn nonsense_word_gets_no_suggestion_and_no_cl_ism() {
    let e = env_with_stdlib();
    let out = eval_line("definitely-not-a-real-lamedh-or-cl-symbol", &e);
    assert_eq!(
        out,
        "Error: Unbound variable: DEFINITELY-NOT-A-REAL-LAMEDH-OR-CL-SYMBOL"
    );
}
