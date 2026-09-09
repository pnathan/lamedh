mod test_helpers;

use lamedh::eval_line;
use test_helpers::env_with_stdlib;

/// Helper: evaluate an expression and return the printed result as a sorted
/// word list so order-insensitive comparisons are straightforward.
fn sorted_members(expr: &str, env: &std::rc::Rc<lamedh::environment::Environment>) -> Vec<String> {
    let raw = eval_line(expr, env);
    // Result looks like "(FOO BAR BAZ)" or "()"
    let trimmed = raw.trim_matches(|c| c == '(' || c == ')');
    if trimmed.is_empty() {
        return vec![];
    }
    let mut items: Vec<String> = trimmed.split_whitespace().map(str::to_owned).collect();
    items.sort();
    items
}

#[test]
fn call_graph_basic_callees() {
    let env = env_with_stdlib();
    // Define two functions where leaf calls bar and baz.
    eval_line("(defun cg-baz (x) (+ x 1))", &env);
    eval_line("(defun cg-bar (x) (* x 2))", &env);
    eval_line("(defun cg-leaf (x) (cg-bar (cg-baz x)))", &env);

    let callees = sorted_members("(call-graph-callees 'cg-leaf)", &env);
    assert!(
        callees.contains(&"CG-BAR".to_owned()),
        "expected CG-BAR in callees, got {:?}",
        callees
    );
    assert!(
        callees.contains(&"CG-BAZ".to_owned()),
        "expected CG-BAZ in callees, got {:?}",
        callees
    );
}

#[test]
fn call_graph_callers_reverse_lookup() {
    let env = env_with_stdlib();
    eval_line("(defun cg-util (x) x)", &env);
    eval_line("(defun cg-a (x) (cg-util x))", &env);
    eval_line("(defun cg-b (x) (cg-util (+ x 1)))", &env);

    let callers = sorted_members("(call-graph-callers 'cg-util)", &env);
    assert!(
        callers.contains(&"CG-A".to_owned()),
        "expected CG-A as caller of CG-UTIL, got {:?}",
        callers
    );
    assert!(
        callers.contains(&"CG-B".to_owned()),
        "expected CG-B as caller of CG-UTIL, got {:?}",
        callers
    );
}

#[test]
fn call_graph_locals_are_excluded() {
    let env = env_with_stdlib();
    // The lambda parameter 'x' must not appear in the callee list,
    // even though 'x' appears in operator position if code were naive.
    // Here fn-param calls helper; the parameter 'helper' must NOT
    // appear as a callee (it is a local).
    eval_line("(defun cg-helper () 42)", &env);
    eval_line("(defun cg-uses-param (helper) (+ helper 1))", &env);

    // cg-uses-param has param 'helper', which shadows any global.
    // So 'helper' must NOT appear in callees (it is the local param).
    let callees = sorted_members("(call-graph-callees 'cg-uses-param)", &env);
    assert!(
        !callees.contains(&"CG-HELPER".to_owned()),
        "local param 'helper' must not appear as callee, got {:?}",
        callees
    );
}

#[test]
fn call_graph_quoted_forms_not_walked() {
    let env = env_with_stdlib();
    // The body mentions 'cg-phantom' only inside a quoted list — not a real call.
    eval_line("(defun cg-quotes () '(cg-phantom a b c))", &env);

    let callees = sorted_members("(call-graph-callees 'cg-quotes)", &env);
    assert!(
        !callees.contains(&"CG-PHANTOM".to_owned()),
        "quoted symbol must not appear as callee, got {:?}",
        callees
    );
}

#[test]
fn call_graph_let_locals_excluded() {
    let env = env_with_stdlib();
    // 'fn' is bound as a let-variable; calling (fn x) refers to the local,
    // not a global function named FN.
    eval_line(
        "(defun cg-let-shadow (x) (let ((fn (lambda (v) (* v 2)))) (fn x)))",
        &env,
    );

    let callees = sorted_members("(call-graph-callees 'cg-let-shadow)", &env);
    // 'fn' is a let-local so must NOT appear; lambda is structural, not a call.
    assert!(
        !callees.contains(&"FN".to_owned()),
        "let-bound name 'fn' must not appear as callee, got {:?}",
        callees
    );
}

#[test]
fn call_graph_has_p_and_all_known() {
    let env = env_with_stdlib();
    eval_line("(defun cg-tracked () 99)", &env);

    assert_eq!(
        eval_line("(call-graph-has-p 'cg-tracked)", &env),
        "T",
        "call-graph-has-p should return T for a defined function"
    );
    assert_eq!(
        eval_line("(call-graph-has-p 'cg-no-such-function-xyz)", &env),
        "()",
        "call-graph-has-p should return NIL for unknown names"
    );

    // all-known must include cg-tracked.
    let all = sorted_members("(call-graph-all-known)", &env);
    assert!(
        all.contains(&"CG-TRACKED".to_owned()),
        "call-graph-all-known should include CG-TRACKED, got {:?}",
        all
    );
}

#[test]
fn call_graph_add_retroactive() {
    let env = env_with_stdlib();
    // Define a function without going through the hook (simulate pre-stdlib).
    // We use the raw def + lambda to bypass the defun macro.
    eval_line("(def cg-retro (lambda (x) (+ x (cg-helper2))))", &env);
    eval_line("(defun cg-helper2 () 0)", &env);

    // cg-retro was defined without the hook, so it may not be in the graph yet.
    // call-graph-add! should retroactively add it.
    eval_line("(call-graph-add! 'cg-retro)", &env);

    let callees = sorted_members("(call-graph-callees 'cg-retro)", &env);
    assert!(
        callees.contains(&"CG-HELPER2".to_owned()),
        "retroactively added cg-retro should list CG-HELPER2 as callee, got {:?}",
        callees
    );
}

#[test]
fn call_graph_recursive_self_call() {
    let env = env_with_stdlib();
    // A recursive function should list itself as a callee.
    eval_line(
        "(defun cg-fact (n) (if (= n 0) 1 (* n (cg-fact (- n 1)))))",
        &env,
    );

    let callees = sorted_members("(call-graph-callees 'cg-fact)", &env);
    assert!(
        callees.contains(&"CG-FACT".to_owned()),
        "recursive function should appear as its own callee, got {:?}",
        callees
    );
}

#[test]
fn call_graph_survives_a_defun_star_typed_function_in_the_graph() {
    // A defun*-typed function's SEE-SOURCE returns its own original
    // `(defun-typed name params body...)` form -- not `(lambda params
    // body...)` like an ordinary DEFUN-backed function -- one extra leading
    // NAME element, shifting params/body by one position. CALL-GRAPH-ADD!
    // must recognize that shape rather than misreading NAME as the
    // parameter list, which previously crashed CG-COLLECT-FORMS (a CAR on
    // a bare symbol) and took down CALL-GRAPH-CALLERS for every other
    // pending function flushed in the same pass, not just this one.
    let env = env_with_stdlib();
    eval_line("(defun* cg-typed-leaf (n) (+ n 1))", &env);
    eval_line("(defun cg-typed-caller (n) (cg-typed-leaf n))", &env);
    eval_line("(defun cg-normal-a (x) (cg-normal-util x))", &env);
    eval_line("(defun cg-normal-util (x) x)", &env);

    // The typed function itself must be inspectable, not silently poisoned.
    let leaf_callees = sorted_members("(call-graph-callees 'cg-typed-leaf)", &env);
    assert!(
        !leaf_callees.iter().any(|c| c.starts_with("ERROR")),
        "call-graph-callees on a defun*-typed function should not error, got {:?}",
        leaf_callees
    );

    // The typed function must show up as a callee of its caller.
    let caller_callees = sorted_members("(call-graph-callees 'cg-typed-caller)", &env);
    assert!(
        caller_callees.contains(&"CG-TYPED-LEAF".to_owned()),
        "expected CG-TYPED-LEAF in callees, got {:?}",
        caller_callees
    );

    // Flushing $CG-PENDING (which a typed function's name also enters) must
    // not corrupt the reverse lookup for unrelated ordinary functions
    // defined in the same pass.
    let callers = sorted_members("(call-graph-callers 'cg-normal-util)", &env);
    assert!(
        callers.contains(&"CG-NORMAL-A".to_owned()),
        "expected CG-NORMAL-A as caller of CG-NORMAL-UTIL even with a typed \
         function pending in the same flush, got {:?}",
        callers
    );
}
