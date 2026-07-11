//! examples/oo-patterns.lisp — classic OO patterns implemented on row types,
//! and the type-checker claims the file teaches. This test keeps both the
//! runtime behavior and the pedagogical `check-type`/`see-type` verdicts
//! honest as the checker evolves.

mod test_helpers;

use lamedh::eval_line;
use test_helpers::env_with_stdlib;

fn env_with_patterns() -> lamedh::Shared<lamedh::environment::Environment> {
    let env = env_with_stdlib();
    lamedh::load_file("examples/oo-patterns.lisp", &env)
        .expect("examples/oo-patterns.lisp should load");
    env
}

#[test]
fn the_file_loads_and_runs() {
    // Loading runs every section's prints and inline check-type verdicts;
    // any load-time type error (implements!, etc.) would fail here.
    let _env = env_with_patterns();
}

#[test]
fn duck_typing_row_is_inferred_not_annotated() {
    let env = env_with_patterns();
    assert_eq!(
        eval_line("(see-type 'wounded-p)", &env),
        "(CHECKED (FORALL (A) (-> ((RECORD ((HP INT64)) A)) BOOL)))"
    );
}

#[test]
fn compound_list_field_flows_into_the_row() {
    let env = env_with_patterns();
    // loot is a list field; the compound type rides into the inferred row —
    // and with record-ref (#308) even the element type stays polymorphic.
    assert_eq!(
        eval_line("(see-type 'carrying-p)", &env),
        "(CHECKED (FORALL (A B) (-> ((RECORD ((LOOT (LIST A))) B)) BOOL)))"
    );
}

#[test]
fn row_polymorphism_gives_independent_argument_rows() {
    let env = env_with_patterns();
    // allied-p's two arguments each get their own row var — they need not be
    // the same kind, only both affiliation-bearing.
    assert_eq!(
        eval_line("(see-type 'allied-p)", &env),
        "(CHECKED (FORALL (A B C D E) (-> ((RECORD ((AFFILIATION A)) B) \
         (RECORD ((AFFILIATION C)) D)) E)))"
    );
}

#[test]
fn the_npc_concept_reads_all_four_fields() {
    let env = env_with_patterns();
    assert_eq!(
        eval_line(
            "(let ((g (make-npc \"Grix\" 7 \"Redfang\" (list \"dagger\")))) \
               (list (the-name g) (the-hp g) (the-affiliation g) (the-loot g) (carrying-p g)))",
            &env
        ),
        "(\"Grix\" 7 \"Redfang\" (\"dagger\") T)"
    );
}

#[test]
fn strategy_selects_algorithm_at_runtime() {
    let env = env_with_patterns();
    assert_eq!(
        eval_line("(checkout (make-order 200 3) #'full-price)", &env),
        "200"
    );
    assert_eq!(
        eval_line("(checkout (make-order 200 3) #'half-price)", &env),
        "100"
    );
    assert_eq!(
        eval_line("(checkout (make-order 200 3) #'bulk-price)", &env),
        "180"
    );
}

#[test]
fn composite_dispatches_uniformly_over_a_recursive_tree() {
    let env = env_with_patterns();
    // A group of {disc 2 -> 12, disc 3 -> 27, group{disc 1 -> 3, disc 1 -> 3}
    // -> 6} totals 45 through one (area ...) call site.
    assert_eq!(eval_line("(area *scene*)", &env), "45");
    // The leaf and composite are reached uniformly.
    assert_eq!(eval_line("(area (make-disc 4))", &env), "48");
}

#[test]
fn decorator_preserves_the_row_and_stacks() {
    let env = env_with_patterns();
    // The "beverage" contract is the inferred cost row (field type
    // polymorphic under record-ref, #308).
    assert_eq!(
        eval_line("(see-type 'total-cost)", &env),
        "(CHECKED (FORALL (A B) (-> ((RECORD ((COST A)) B)) A)))"
    );
    // Stacked decorators: espresso 10 + milk 2 + sugar 1.
    assert_eq!(
        eval_line(
            "(total-cost (with-sugar (with-milk (make-espresso 10 \"e\"))))",
            &env
        ),
        "13"
    );
}

#[test]
fn observer_notifies_a_heterogeneous_set_from_one_call_site() {
    let env = env_with_patterns();
    assert_eq!(
        eval_line(
            "(notify-all (list (make-logger \"audit\") (make-counter 0)) 7)",
            &env
        ),
        "(\"[audit] saw event 7\" \"count+1 on event 7\")"
    );
}

#[test]
fn state_is_data_with_a_pure_transition() {
    let env = env_with_patterns();
    assert_eq!(
        eval_line("(run-lights *red* 4)", &env),
        "(\"red\" \"green\" \"yellow\" \"red\" \"green\")"
    );
}

#[test]
fn cross_kind_misuse_is_a_static_row_error() {
    let env = env_with_patterns();
    // A disc {r} has no cost field; the-cost demands one. Caught by the
    // checker, never at runtime — duck typing with a proof.
    let out = eval_line("(check-type (the-cost (make-disc 2)))", &env);
    assert!(
        out.contains("type error") && out.contains("cost"),
        "expected a closed-record row error naming cost, got: {out}"
    );
}
