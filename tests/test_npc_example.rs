mod test_helpers;

use lamedh::{eval_line, load_file};
use test_helpers::env_with_stdlib;

fn env_with_npcs() -> lamedh::Shared<lamedh::environment::Environment> {
    let env = env_with_stdlib();
    load_file("examples/game/npcs.lisp", &env).expect("npcs.lisp should load");
    env
}

#[test]
fn shared_op_dispatches_to_the_specialized_method() {
    let env = env_with_npcs();
    assert_eq!(
        eval_line("(method 'greet (make-goblin \"Snag\" 7 3))", &env),
        "\"Grr. Snag waves a rusty knife.\""
    );
    assert_eq!(
        eval_line("(method 'greet (make-merchant \"Oren\" 12 250))", &env),
        "\"Welcome! Oren opens a pack of 250 gold worth of wares.\""
    );
}

#[test]
fn late_bound_shared_method_works_on_every_kind() {
    let env = env_with_npcs();
    assert_eq!(
        eval_line("(introduce (make-goblin \"Snag\" 2 3))", &env),
        "\"Snag [2 hp]: Grr. Snag waves a rusty knife.\""
    );
    assert_eq!(
        eval_line("(introduce (make-merchant \"Oren\" 12 250))", &env),
        "\"Oren [12 hp]: Welcome! Oren opens a pack of 250 gold worth of wares.\""
    );
}

#[test]
fn row_typed_shared_method_is_proven_and_works_across_kinds() {
    let env = env_with_npcs();
    // The shared projection infers a row scheme: any record with an int64 hp.
    assert_eq!(
        eval_line("(see-type 'npc-hp)", &env),
        "(CHECKED (FORALL (A) (-> ((RECORD ((HP INT64)) A)) INT64)))"
    );
    assert_eq!(
        eval_line("(wounded-p (make-goblin \"Snag\" 2 3))", &env),
        "T"
    );
    assert_eq!(
        eval_line("(wounded-p (make-merchant \"Oren\" 12 250))", &env),
        "()"
    );
}

#[test]
fn both_kinds_implement_npc_and_the_claims_are_recorded() {
    let env = env_with_npcs();
    assert!(eval_line("(implements? 'goblin 'npc)", &env).starts_with("(T"));
    assert!(eval_line("(implements? 'merchant 'npc)", &env).starts_with("(T"));
    // implements! in the file recorded the claim on both sides.
    assert_eq!(
        eval_line("(getp 'npc \"interface.types\")", &env),
        "(GOBLIN MERCHANT)"
    );
}

#[test]
fn a_kind_missing_the_specialized_op_fails_conformance() {
    let env = env_with_npcs();
    let report = eval_line("(implements? 'training-dummy 'npc)", &env);
    assert!(report.starts_with("(()"), "got: {report}");
    // The row accessors it does have still CONFORM; only GREET is MISSING.
    assert!(report.contains("(NAME CONFORMS"), "got: {report}");
    assert!(report.contains("(GREET MISSING"), "got: {report}");
    assert_eq!(
        eval_line("(errorset '(implements! 'training-dummy 'npc))", &env),
        "()"
    );
}

#[test]
fn row_accessor_ops_conform_greet_stays_unproven() {
    let env = env_with_npcs();
    // NAME and HP carry DECLARED row schemes that subsume the op signatures at
    // self := the concept's record type: a real, checker-backed guarantee.
    let goblin = eval_line("(implements? 'goblin 'npc)", &env);
    assert!(
        goblin.contains("(NAME CONFORMS GOBLIN-NAME"),
        "got: {goblin}"
    );
    assert!(goblin.contains("(HP CONFORMS GOBLIN-HP"), "got: {goblin}");
    // GREET builds a string with CONCAT, defeating inference: its scheme is
    // vacuous, so it exists but proves nothing — honestly UNPROVEN, not CONFORMS.
    assert!(
        goblin.contains("(GREET UNPROVEN GOBLIN-GREET"),
        "got: {goblin}"
    );
}

#[test]
fn cross_concept_misuse_is_a_static_type_error() {
    let env = env_with_npcs();
    let verdict = eval_line("(see-type 'rob)", &env);
    assert!(verdict.starts_with("(TYPE-ERROR"), "got: {verdict}");
    assert!(verdict.contains("lacks field(s) gold"), "got: {verdict}");
}

#[test]
fn invariants_travel_with_the_seed() {
    let env = env_with_npcs();
    assert_eq!(
        eval_line("(validate-goblin (make-goblin \"Snag\" 7 3))", &env),
        "T"
    );
    assert_eq!(
        eval_line("(validate-goblin (make-goblin \"Snag\" -1 3))", &env),
        "()"
    );
}
