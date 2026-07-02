mod test_helpers;

use lamedh::eval_line;
use test_helpers::env_with_stdlib;

#[test]
fn sexpr_ref_and_set_address_subforms_by_path() {
    let env = env_with_stdlib();
    assert_eq!(
        eval_line("(sexpr-ref '(defun f (x) (+ x 1)) '(3 2))", &env),
        "1"
    );
    assert_eq!(
        eval_line("(sexpr-set '(defun f (x) (+ x 1)) '(3 2) 9)", &env),
        "(DEFUN F (X) (+ X 9))"
    );
}

#[test]
fn diff_and_patch_are_inverses() {
    let env = env_with_stdlib();
    assert_eq!(
        eval_line(
            "(let* ((old '(defun f (x) (if (> x 0) (+ x 1) (- x 1))))
                    (new '(defun f (x) (if (>= x 0) (+ x 2) (- x 1)))))
               (equal (sexpr-patch old (condense-diff old new)) new))",
            &env
        ),
        "T"
    );
}

#[test]
fn patch_guards_reject_stale_edits() {
    let env = env_with_stdlib();
    assert_eq!(
        eval_line("(errorset '(sexpr-patch '(+ x 1) '(((2) 99 2))))", &env),
        "()"
    );
}

#[test]
fn edit_applies_a_minimal_change_to_a_live_function() {
    let env = env_with_stdlib();
    eval_line("(defun price (base qty) (* base qty))", &env);
    let report = eval_line(
        "(edit! 'price '(((2) (* BASE QTY) (* base (+ qty 1)))))",
        &env,
    );
    assert!(report.contains("(WAS . CHECKED)"), "got: {report}");
    assert!(report.contains("(NOW . CHECKED)"), "got: {report}");
    assert_eq!(eval_line("(price 10 2)", &env), "30");
    // The edit is provenance: recorded on the symbol, visible in the trace.
    let edits = eval_line("(condense-get 'price \"condense.edits\")", &env);
    assert!(edits.contains("BASE"), "got: {edits}");
}

#[test]
fn edit_that_introduces_a_type_error_is_rolled_back_and_rejected() {
    let env = env_with_stdlib();
    eval_line("(defun inc (x) (+ x 1))", &env);
    assert_eq!(
        eval_line(
            "(errorset '(edit! 'inc '(((2) (+ X 1) (+ 1 \"s\")))))",
            &env
        ),
        "()"
    );
    // Rollback: the original definition and its informative type survive.
    assert_eq!(eval_line("(inc 5)", &env), "6");
    assert_eq!(
        eval_line("(see-type 'inc)", &env),
        "(CHECKED (-> (INT64) INT64))"
    );
}

#[test]
fn edit_may_repair_a_function_that_already_had_a_type_error() {
    let env = env_with_stdlib();
    eval_line("(defun bad (x) (+ 1 \"s\"))", &env);
    let report = eval_line("(edit! 'bad '(((2) (+ 1 \"s\") (+ 1 x))))", &env);
    assert!(report.contains("(WAS . TYPE-ERROR)"), "got: {report}");
    assert!(report.contains("(NOW . CHECKED)"), "got: {report}");
    assert_eq!(eval_line("(bad 4)", &env), "5");
}

#[test]
fn editing_a_concept_seed_regenerates_and_reverifies_the_artifact() {
    let env = env_with_stdlib();
    eval_line(
        "(defconcept invoice (:fields ((id int64) (amount int64))) (:invariant (>= amount 0)) (:derive equality lens))",
        &env,
    );
    eval_line(
        "(example ok-invoice (:for invoice) (:given (make-invoice 1 5)) (:expect (validate-invoice *it*)))",
        &env,
    );
    // One minimal edit to the seed: tighten the invariant from >= 0 to >= 1.
    let report = eval_line(
        "(edit! 'invoice '(((3 1) (>= AMOUNT 0) (>= amount 1))))",
        &env,
    );
    // The report re-ran the attached examples.
    assert!(
        report.contains("(CHECKS T (OK-INVOICE . T))"),
        "got: {report}"
    );
    // The new invariant is live in the regenerated validator...
    assert_eq!(
        eval_line("(validate-invoice (make-invoice 1 0))", &env),
        "()"
    );
    assert_eq!(
        eval_line("(validate-invoice (make-invoice 1 5))", &env),
        "T"
    );
    // ...and the derivations were re-derived, laws intact.
    assert_eq!(
        eval_line("(invoice-lens-roundtrip (make-invoice 1 5))", &env),
        "T"
    );
    assert_eq!(
        eval_line(
            "(invoice-equal (make-invoice 1 5) (make-invoice 1 5))",
            &env
        ),
        "T"
    );
    // The seed edit is recorded, and the seed's source reflects the change.
    let source = eval_line("(condense-source 'invoice)", &env);
    assert!(source.contains("(>= AMOUNT 1)"), "got: {source}");
}

#[test]
fn concept_last_diff_localizes_the_expansion_change() {
    let env = env_with_stdlib();
    eval_line(
        "(defconcept invoice (:fields ((id int64) (amount int64))) (:invariant (>= amount 0)))",
        &env,
    );
    eval_line(
        "(edit! 'invoice '(((3 1) (>= AMOUNT 0) (>= amount 1))))",
        &env,
    );
    let diff = eval_line("(condense-get 'invoice \"condense.last-diff\")", &env);
    assert!(diff.contains("0 1"), "got: {diff}");
    assert!(!diff.contains("PROGN"), "diff should localize, got: {diff}");
}

#[test]
fn subform_edits_locate_old_uniquely_without_paths() {
    let env = env_with_stdlib();
    eval_line("(defun price (base qty) (* base qty))", &env);
    let report = eval_line("(edit! 'price '(((* base qty) (* base (+ qty 1)))))", &env);
    assert!(report.contains("(NOW . CHECKED)"), "got: {report}");
    assert_eq!(eval_line("(price 10 2)", &env), "30");
    // Ambiguous targets are refused: an edit must name its site uniquely.
    assert_eq!(
        eval_line(
            "(errorset '(sexpr-patch '(+ (f 1) (f 1)) '(((f 1) (g 1)))))",
            &env
        ),
        "()"
    );
    assert_eq!(
        eval_line("(sexpr-locate '(defun f (x) (+ x 1)) '(+ x 1))", &env),
        "(3)"
    );
}

#[test]
fn check_file_reports_honest_verdicts_for_a_whole_file() {
    let env = env_with_stdlib();
    env.enable_feature("READ-FS");
    let dir = std::env::temp_dir().join("lamedh_check_file_test");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("agent.lisp");
    std::fs::write(
        &path,
        "(defun inc (x) (+ x 1))\n(defun broken (x) (+ 1 \"s\"))\n(defconcept invoice (:fields ((id int64) (amount int64))) (:derive equality))\n",
    )
    .unwrap();
    let report = eval_line(&format!("(check-file! \"{}\")", path.display()), &env);
    assert!(report.contains("(INC CHECKED"), "got: {report}");
    assert!(report.contains("(BROKEN TYPE-ERROR"), "got: {report}");
    assert!(report.contains("(INVOICE-EQUAL DECLARED"), "got: {report}");
    // The frontier is the unproven remainder: BROKEN is on it, INC is not.
    let frontier = report.split("FRONTIER").nth(1).unwrap_or("");
    assert!(frontier.contains("BROKEN"), "got: {frontier}");
    assert!(!frontier.contains("(INC "), "got: {frontier}");
}
