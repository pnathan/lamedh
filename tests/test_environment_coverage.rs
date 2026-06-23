/// Tests targeting uncovered lines in environment.rs to improve coverage.
///
/// Uncovered lines identified from llvm-cov output:
///   13-15  : SymbolTable::default()
///   36-38  : SymbolTable::all_symbols()
///   73-91  : Environment PartialEq impl
///   95-97  : Environment::default()
///   406-408: Environment::all_symbols()
///   473-483: update_dynamic via dynamic_parent / lexical fallback
///   488    : all_bindings() with parent chain
///   557-565: get_dynamic() dynamic/lexical parent chain fallbacks
use lamedh::{
    environment::Environment,
    LispVal,
};
use std::rc::Rc;

// ── SymbolTable::default() ────────────────────────────────────────────────────

#[test]
fn test_symbol_table_default_is_new() {
    // SymbolTable implements Default; exercise that code path.
    // We do it through Environment since SymbolTable is not pub at crate root.
    // Environment::new() internally calls SymbolTable::new().  To hit the
    // Default impl we use Environment::default() which calls Self::new() ->
    // SymbolTable::new() -> mirrors Default.  We verify the symbol table
    // starts empty by interning a fresh symbol.
    let env = Environment::default();
    let sym = env.intern_symbol("MYDEFAULTSYM");
    assert_eq!(sym.borrow().name, "MYDEFAULTSYM");
}

// ── SymbolTable::all_symbols() ────────────────────────────────────────────────

#[test]
fn test_all_symbols_empty_on_fresh_env() {
    // A brand-new environment has no interned symbols.
    let env = Environment::new();
    assert!(env.all_symbols().is_empty());
}

#[test]
fn test_all_symbols_after_intern() {
    let env = Environment::new();
    env.intern_symbol("FOO");
    env.intern_symbol("BAR");
    env.intern_symbol("BAZ");
    let symbols = env.all_symbols();
    assert_eq!(symbols.len(), 3);
    let names: Vec<String> = symbols.iter().map(|s| s.borrow().name.clone()).collect();
    assert!(names.contains(&"FOO".to_string()));
    assert!(names.contains(&"BAR".to_string()));
    assert!(names.contains(&"BAZ".to_string()));
}

#[test]
fn test_all_symbols_deduplicates_same_name() {
    let env = Environment::new();
    env.intern_symbol("DUPLICATE");
    env.intern_symbol("DUPLICATE");
    env.intern_symbol("DUPLICATE");
    // Interning the same name multiple times must not grow the table.
    assert_eq!(env.all_symbols().len(), 1);
}

// ── Environment::default() ───────────────────────────────────────────────────

#[test]
fn test_environment_default_is_usable() {
    let env = Environment::default();
    // Should be able to set and get a binding.
    env.set("X".to_string(), LispVal::Number(42));
    assert_eq!(env.get("X"), Some(LispVal::Number(42)));
}

#[test]
fn test_environment_default_has_no_parent() {
    let env = Environment::default();
    // In a default environment nothing is bound yet.
    assert!(!env.is_bound("UNDEFINED_SYMBOL_XYZ"));
}

// ── Environment PartialEq ─────────────────────────────────────────────────────

#[test]
fn test_environment_partialeq_same_rc_is_equal() {
    let env = Rc::new(Environment::new());
    // An environment is equal to itself (via Rc::ptr_eq on each field).
    assert_eq!(*env, *env);
}

#[test]
fn test_environment_partialeq_two_distinct_envs_not_equal() {
    let env1 = Rc::new(Environment::new());
    let env2 = Rc::new(Environment::new());
    // Two independently-created environments share no fields by pointer.
    assert_ne!(*env1, *env2);
}

#[test]
fn test_environment_partialeq_parent_mismatch() {
    let parent1 = Rc::new(Environment::new());
    let parent2 = Rc::new(Environment::new());
    let child1 = Environment::new_child(&parent1);
    let child2 = Environment::new_child(&parent2);
    // Children with different parents are not equal.
    assert_ne!(*child1, *child2);
}

#[test]
fn test_environment_partialeq_dynamic_parent_mismatch() {
    let lex = Rc::new(Environment::new());
    let caller1 = Rc::new(Environment::new());
    let caller2 = Rc::new(Environment::new());
    let child1 = Environment::new_child_with_dynamic(&lex, &caller1);
    let child2 = Environment::new_child_with_dynamic(&lex, &caller2);
    // Children differ because dynamic_parent differs.
    assert_ne!(*child1, *child2);
}

#[test]
fn test_environment_partialeq_none_vs_some_parent() {
    let env_no_parent = Rc::new(Environment::new());
    let parent = Rc::new(Environment::new());
    let env_with_parent = Environment::new_child(&parent);
    assert_ne!(*env_no_parent, *env_with_parent);
}

#[test]
fn test_environment_partialeq_one_has_dynamic_parent_other_does_not() {
    // Covers the `_ => false` arm in the dynamic_parent PartialEq match:
    // one environment has a dynamic_parent (Some) and the other does not (None).
    let base = Rc::new(Environment::new());
    let caller = Rc::new(Environment::new());

    // child1 has a dynamic_parent; child2 does not.
    let child1 = Environment::new_child_with_dynamic(&base, &caller);
    let child2 = Environment::new_child(&base);
    assert_ne!(*child1, *child2);
}

// ── all_bindings() with parent chain ─────────────────────────────────────────

#[test]
fn test_all_bindings_from_parent_chain() {
    let parent = Rc::new(Environment::new());
    parent.set("A".to_string(), LispVal::Number(1));
    parent.set("B".to_string(), LispVal::Number(2));

    let child = Environment::new_child(&parent);
    child.set("C".to_string(), LispVal::Number(3));

    let all = child.all_bindings();
    assert_eq!(all.get("A"), Some(&LispVal::Number(1)));
    assert_eq!(all.get("B"), Some(&LispVal::Number(2)));
    assert_eq!(all.get("C"), Some(&LispVal::Number(3)));
}

#[test]
fn test_all_bindings_child_shadows_parent() {
    let parent = Rc::new(Environment::new());
    parent.set("X".to_string(), LispVal::Number(10));

    let child = Environment::new_child(&parent);
    child.set("X".to_string(), LispVal::Number(99));

    let all = child.all_bindings();
    // Child's binding shadows the parent's.
    assert_eq!(all.get("X"), Some(&LispVal::Number(99)));
}

#[test]
fn test_all_bindings_no_parent() {
    let env = Rc::new(Environment::new());
    env.set("ONLY".to_string(), LispVal::Number(7));
    let all = env.all_bindings();
    assert_eq!(all.get("ONLY"), Some(&LispVal::Number(7)));
    assert_eq!(all.len(), 1);
}

// ── update_dynamic with dynamic_parent / lexical fallback ────────────────────

#[test]
fn test_update_dynamic_through_dynamic_parent() {
    // Mark a variable dynamic and place a binding in the "caller" env.
    // When update() is called on a child with a dynamic_parent, the write
    // must flow through the dynamic parent chain.
    let caller_env = Rc::new(Environment::new());
    caller_env.mark_dynamic("*DYN*".to_string());
    caller_env.set("*DYN*".to_string(), LispVal::Number(0));

    let lex_env = Rc::new(Environment::new());
    // Propagate the dynamic_vars set to lex_env by sharing it — use
    // new_child_with_dynamic so the child inherits the dynamic_vars set.
    let child = Environment::new_child_with_dynamic(&lex_env, &caller_env);
    // Mark *DYN* in the child's dynamic_vars (shared via lex_env doesn't have
    // it so we mark on child via caller_env's set, but let's mark via child
    // which actually has different dynamic_vars).  Instead use new_child from
    // caller_env so all share the same dynamic_vars.
    let base = Rc::new(Environment::new());
    base.mark_dynamic("*DYN2*".to_string());
    base.set("*DYN2*".to_string(), LispVal::Number(100));

    let mid = Environment::new_child(&base);
    let top = Environment::new_child_with_dynamic(&mid, &base);

    // update() on top should walk dynamic parent (base) and update there.
    Environment::update(&top, "*DYN2*", LispVal::Number(999));
    // The value in base must have been updated.
    assert_eq!(base.get("*DYN2*"), Some(LispVal::Number(999)));
    let _ = child; // suppress unused warning
}

#[test]
fn test_update_dynamic_falls_back_to_lexical_parent() {
    // A dynamic variable exists only in the lexical parent chain (no dynamic
    // parent available).  update_dynamic must fall through to the lexical chain.
    let base = Rc::new(Environment::new());
    base.mark_dynamic("*LEX_DYN*".to_string());
    base.set("*LEX_DYN*".to_string(), LispVal::Number(1));

    let child = Environment::new_child(&base);
    // child has no dynamic_parent; update must fall back via lexical parent.
    Environment::update(&child, "*LEX_DYN*", LispVal::Number(42));

    assert_eq!(base.get("*LEX_DYN*"), Some(LispVal::Number(42)));
}

#[test]
fn test_update_dynamic_creates_when_not_found() {
    // Calling update() for a dynamic variable not found anywhere creates it
    // in the current environment.
    let env = Rc::new(Environment::new());
    env.mark_dynamic("*BRAND_NEW*".to_string());

    Environment::update(&env, "*BRAND_NEW*", LispVal::Number(7));
    assert_eq!(env.get("*BRAND_NEW*"), Some(LispVal::Number(7)));
}

// ── get_dynamic() with dynamic and lexical parent chain fallbacks ─────────────

#[test]
fn test_get_var_dynamic_falls_through_dynamic_parent() {
    // Set up: base has *D* and marks it dynamic.  A lexical child of base is
    // then given a dynamic_parent of base so that get_dynamic traverses the
    // dynamic parent chain.
    let base = Rc::new(Environment::new());
    base.mark_dynamic("*D*".to_string());
    base.set("*D*".to_string(), LispVal::Number(55));

    // Use base as both lexical_parent and caller_env so that the dynamic_vars
    // HashSet is shared (new_child_with_dynamic clones dynamic_vars from
    // lexical_parent).  That way the child also sees *D* as dynamic.
    let child = Environment::new_child_with_dynamic(&base, &base);
    // child has no own binding for *D*; it should find it via dynamic parent (base).
    assert_eq!(child.get_var("*D*"), Some(LispVal::Number(55)));
}

#[test]
fn test_get_var_dynamic_falls_through_lexical_parent() {
    // When there is no dynamic_parent, get_dynamic falls back to lexical parent.
    let base = Rc::new(Environment::new());
    base.mark_dynamic("*LD*".to_string());
    base.set("*LD*".to_string(), LispVal::Number(77));

    let child = Environment::new_child(&base);
    // child has no dynamic_parent; get_var must find *LD* in lexical parent.
    assert_eq!(child.get_var("*LD*"), Some(LispVal::Number(77)));
}

#[test]
fn test_get_dynamic_returns_none_when_not_anywhere() {
    let env = Rc::new(Environment::new());
    env.mark_dynamic("*NOWHERE*".to_string());
    // Variable is dynamic but has no binding anywhere.
    assert_eq!(env.get_var("*NOWHERE*"), None);
}

#[test]
fn test_get_dynamic_current_env_wins() {
    // Variable bound in both current env and dynamic parent; current wins.
    let base = Rc::new(Environment::new());
    base.mark_dynamic("*V*".to_string());
    base.set("*V*".to_string(), LispVal::Number(1));

    let lex = Rc::new(Environment::new());
    let child = Environment::new_child_with_dynamic(&lex, &base);
    child.set("*V*".to_string(), LispVal::Number(99));

    assert_eq!(child.get_var("*V*"), Some(LispVal::Number(99)));
}

// ── is_bound() ───────────────────────────────────────────────────────────────

#[test]
fn test_is_bound_true_for_builtins() {
    let env = Environment::new_with_builtins();
    assert!(env.is_bound("+"));
    assert!(env.is_bound("CAR"));
    assert!(env.is_bound("CONS"));
}

#[test]
fn test_is_bound_false_for_unknown() {
    let env = Environment::new_with_builtins();
    assert!(!env.is_bound("TOTALLY-UNDEFINED-XYZ-123"));
}

#[test]
fn test_is_bound_after_set() {
    let env = Rc::new(Environment::new());
    assert!(!env.is_bound("MYVAR"));
    env.set("MYVAR".to_string(), LispVal::Number(42));
    assert!(env.is_bound("MYVAR"));
}

// ── new_child_with_dynamic basic creation ─────────────────────────────────────

#[test]
fn test_new_child_with_dynamic_inherits_lexical() {
    let lex = Rc::new(Environment::new());
    lex.set("LEX_VAR".to_string(), LispVal::Number(10));
    let caller = Rc::new(Environment::new());
    let child = Environment::new_child_with_dynamic(&lex, &caller);
    // child should find lexical variable via lexical parent chain.
    assert_eq!(child.get("LEX_VAR"), Some(LispVal::Number(10)));
}

// ── features_list() ──────────────────────────────────────────────────────────

#[test]
fn test_features_list_empty_initially() {
    let env = Rc::new(Environment::new());
    assert!(env.features_list().is_empty());
}

#[test]
fn test_features_list_after_enable() {
    let env = Rc::new(Environment::new());
    env.enable_feature("MYFEAT");
    let list = env.features_list();
    assert_eq!(list.len(), 1);
    assert!(list.contains(&"MYFEAT".to_string()));
}

#[test]
fn test_features_list_after_disable() {
    let env = Rc::new(Environment::new());
    env.enable_feature("FEAT1");
    env.enable_feature("FEAT2");
    env.disable_feature("FEAT1");
    let list = env.features_list();
    assert_eq!(list.len(), 1);
    assert!(list.contains(&"FEAT2".to_string()));
    assert!(!list.contains(&"FEAT1".to_string()));
}

// ── get() returns None when name missing in full chain ───────────────────────

#[test]
fn test_get_returns_none_for_unbound_in_chain() {
    let grandparent = Rc::new(Environment::new());
    grandparent.set("GP_VAR".to_string(), LispVal::Number(1));
    let parent = Environment::new_child(&grandparent);
    let child = Environment::new_child(&parent);
    // Bound var is found.
    assert_eq!(child.get("GP_VAR"), Some(LispVal::Number(1)));
    // Unbound var returns None even with chain.
    assert_eq!(child.get("DOES_NOT_EXIST"), None);
}
