mod test_helpers;

use lamedh::eval_line;
use test_helpers::env_with_stdlib;

fn env_with_npcs() -> lamedh::Shared<lamedh::environment::Environment> {
    let env = env_with_stdlib();
    lamedh::load_file("examples/npcs.lisp", &env).expect("examples/npcs.lisp should load");
    env
}

#[test]
fn specialized_greet_dispatches_per_kind_from_one_call_site() {
    let env = env_with_npcs();
    assert_eq!(
        eval_line("(method 'greet (make-goblin \"Grix\" 7 9))", &env),
        "\"Grix sharpens a rusty dagger and cackles.\""
    );
    assert_eq!(
        eval_line("(method 'greet (make-wisp \"Sel\" 3 0.8))", &env),
        "\"Sel flickers softly in the gloom.\""
    );
}

#[test]
fn shared_damage_logic_is_written_once_and_works_for_every_kind() {
    let env = env_with_npcs();
    assert_eq!(
        eval_line(
            "(npc-hp (method 'damage (make-goblin \"Grix\" 7 9) 5))",
            &env
        ),
        "2"
    );
    assert_eq!(
        eval_line(
            "(npc-hp (method 'damage (make-merchant \"Oleander\" 12 240) 5))",
            &env
        ),
        "7"
    );
    // Floors at zero (the shared HIT-POINTS-AFTER), and the invariant holds.
    assert_eq!(
        eval_line(
            "(npc-hp (method 'damage (make-wisp \"Sel\" 3 0.8) 5))",
            &env
        ),
        "0"
    );
    assert_eq!(
        eval_line(
            "(validate-wisp (method 'damage (make-wisp \"Sel\" 3 0.8) 5))",
            &env
        ),
        "T"
    );
    // Damage preserves the kind: the result still speaks in its own voice.
    assert_eq!(
        eval_line(
            "(method 'greet (method 'damage (make-goblin \"Grix\" 7 9) 3))",
            &env
        ),
        "\"Grix sharpens a rusty dagger and cackles.\""
    );
}

#[test]
fn shared_methods_carry_inferred_row_schemes() {
    let env = env_with_npcs();
    assert_eq!(
        eval_line("(see-type 'alive-p)", &env),
        "(CHECKED (FORALL (A) (-> ((RECORD ((HP INT64)) A)) BOOL)))"
    );
    assert_eq!(eval_line("(alive-p (make-goblin \"Grix\" 7 9))", &env), "T");
    assert_eq!(eval_line("(alive-p (make-wisp \"Sel\" 0 0.8))", &env), "()");
}

#[test]
fn conformance_is_verified_and_honestly_graded() {
    let env = env_with_npcs();
    let report = eval_line("(implements? 'goblin 'npc)", &env);
    assert!(report.starts_with("(T"), "got: {report}");
    // DAMAGE is proved by row unification against the declared signature...
    assert!(report.contains("(DAMAGE CONFORMS"), "got: {report}");
    // ...while GREET's result flows through CONCAT, which the checker cannot
    // type today — so the grade says exactly that: unproven, not verified.
    assert!(report.contains("(GREET UNPROVEN"), "got: {report}");
    // The load-time IMPLEMENTS! assertions recorded the claims.
    assert_eq!(
        eval_line("(getp 'wisp \"interface.implements\")", &env),
        "(NPC)"
    );
}

#[test]
fn cross_kind_misuse_is_a_static_type_error() {
    let env = env_with_npcs();
    eval_line(
        "(defun oops () (goblin-mischief (make-merchant \"O\" 12 240)))",
        &env,
    );
    let verdict = eval_line("(see-type 'oops)", &env);
    assert!(verdict.starts_with("(TYPE-ERROR"), "got: {verdict}");
    assert!(
        verdict.contains("lacks field(s) mischief"),
        "got: {verdict}"
    );
    // And the edit barrier refuses to introduce the same misuse.
    eval_line("(defun poke (g) (goblin-mischief g))", &env);
    assert_eq!(
        eval_line(
            "(errorset '(edit! 'poke '(((goblin-mischief g) (goblin-mischief (make-merchant \"O\" 12 240))))))",
            &env
        ),
        "()"
    );
}
