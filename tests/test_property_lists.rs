mod test_helpers;
use lamedh::eval_line;
use test_helpers::env_with_stdlib;

// GETP and PUTP tests (already exist but adding more comprehensive tests)
#[test]
fn test_putp_and_getp_basic() {
    let env = env_with_stdlib();
    eval_line("(putp 'test \"prop\" \"value\")", &env);
    let output = eval_line("(getp 'test \"prop\")", &env);
    assert_eq!(output, "\"value\"");
}

#[test]
fn test_getp_nonexistent_property() {
    let env = env_with_stdlib();
    let output = eval_line("(getp 'test \"nonexistent\")", &env);
    assert_eq!(output, "()");
}

#[test]
fn test_putp_overwrite() {
    let env = env_with_stdlib();
    eval_line("(putp 'x \"key\" 1)", &env);
    eval_line("(putp 'x \"key\" 2)", &env);
    let output = eval_line("(getp 'x \"key\")", &env);
    assert_eq!(output, "2");
}

#[test]
fn test_multiple_properties() {
    let env = env_with_stdlib();
    eval_line("(putp 'sym \"prop1\" 1)", &env);
    eval_line("(putp 'sym \"prop2\" 2)", &env);
    eval_line("(putp 'sym \"prop3\" 3)", &env);
    assert_eq!(eval_line("(getp 'sym \"prop1\")", &env), "1");
    assert_eq!(eval_line("(getp 'sym \"prop2\")", &env), "2");
    assert_eq!(eval_line("(getp 'sym \"prop3\")", &env), "3");
}

#[test]
fn test_property_with_list_value() {
    let env = env_with_stdlib();
    eval_line("(putp 'x \"list\" '(1 2 3))", &env);
    let output = eval_line("(getp 'x \"list\")", &env);
    assert_eq!(output, "(1 2 3)");
}

#[test]
fn test_property_with_symbol_value() {
    let env = env_with_stdlib();
    eval_line("(putp 'x \"sym\" 'another-symbol)", &env);
    let output = eval_line("(getp 'x \"sym\")", &env);
    assert_eq!(output, "ANOTHER-SYMBOL");
}

// REMPROP tests
#[test]
fn test_remprop_existing_property() {
    let env = env_with_stdlib();
    eval_line("(putp 'x \"key\" \"value\")", &env);
    let output = eval_line("(remprop 'x \"key\")", &env);
    assert_eq!(output, "T");
}

#[test]
fn test_remprop_verify_removal() {
    let env = env_with_stdlib();
    eval_line("(putp 'x \"key\" \"value\")", &env);
    eval_line("(remprop 'x \"key\")", &env);
    let output = eval_line("(getp 'x \"key\")", &env);
    assert_eq!(output, "()");
}

#[test]
fn test_remprop_nonexistent_property() {
    let env = env_with_stdlib();
    let output = eval_line("(remprop 'x \"nonexistent\")", &env);
    assert_eq!(output, "()");
}

#[test]
fn test_remprop_one_of_many() {
    let env = env_with_stdlib();
    eval_line("(putp 'x \"a\" 1)", &env);
    eval_line("(putp 'x \"b\" 2)", &env);
    eval_line("(putp 'x \"c\" 3)", &env);
    eval_line("(remprop 'x \"b\")", &env);
    assert_eq!(eval_line("(getp 'x \"a\")", &env), "1");
    assert_eq!(eval_line("(getp 'x \"b\")", &env), "()");
    assert_eq!(eval_line("(getp 'x \"c\")", &env), "3");
}

#[test]
fn test_remprop_add_back() {
    let env = env_with_stdlib();
    eval_line("(putp 'x \"key\" 1)", &env);
    eval_line("(remprop 'x \"key\")", &env);
    eval_line("(putp 'x \"key\" 2)", &env);
    let output = eval_line("(getp 'x \"key\")", &env);
    assert_eq!(output, "2");
}

#[test]
fn test_remprop_multiple_times() {
    let env = env_with_stdlib();
    eval_line("(putp 'x \"key\" 1)", &env);
    assert_eq!(eval_line("(remprop 'x \"key\")", &env), "T");
    assert_eq!(eval_line("(remprop 'x \"key\")", &env), "()");
    assert_eq!(eval_line("(remprop 'x \"key\")", &env), "()");
}

// DEFLIST tests
#[test]
fn test_deflist_simple() {
    let env = env_with_stdlib();
    eval_line("(deflist '((a 1) (b 2) (c 3)) \"value\")", &env);
    assert_eq!(eval_line("(getp 'a \"value\")", &env), "1");
    assert_eq!(eval_line("(getp 'b \"value\")", &env), "2");
    assert_eq!(eval_line("(getp 'c \"value\")", &env), "3");
}

#[test]
fn test_deflist_empty() {
    let env = env_with_stdlib();
    let output = eval_line("(deflist '() \"prop\")", &env);
    assert_eq!(output, "T");
}

#[test]
fn test_deflist_single_entry() {
    let env = env_with_stdlib();
    eval_line("(deflist '((x 99)) \"number\")", &env);
    let output = eval_line("(getp 'x \"number\")", &env);
    assert_eq!(output, "99");
}

#[test]
fn test_deflist_overwrites() {
    let env = env_with_stdlib();
    eval_line("(putp 'a \"type\" \"old\")", &env);
    eval_line("(deflist '((a \"new\")) \"type\")", &env);
    let output = eval_line("(getp 'a \"type\")", &env);
    assert_eq!(output, "\"new\"");
}

#[test]
fn test_deflist_with_list_values() {
    let env = env_with_stdlib();
    eval_line("(deflist '((x (1 2)) (y (3 4))) \"coords\")", &env);
    assert_eq!(eval_line("(getp 'x \"coords\")", &env), "(1 2)");
    assert_eq!(eval_line("(getp 'y \"coords\")", &env), "(3 4)");
}

#[test]
fn test_deflist_multiple_indicators() {
    let env = env_with_stdlib();
    eval_line("(deflist '((a 1) (b 2)) \"first\")", &env);
    eval_line("(deflist '((a 10) (b 20)) \"second\")", &env);
    assert_eq!(eval_line("(getp 'a \"first\")", &env), "1");
    assert_eq!(eval_line("(getp 'a \"second\")", &env), "10");
    assert_eq!(eval_line("(getp 'b \"first\")", &env), "2");
    assert_eq!(eval_line("(getp 'b \"second\")", &env), "20");
}

#[test]
fn test_deflist_symbol_values() {
    let env = env_with_stdlib();
    eval_line("(deflist '((north 'up) (south 'down)) \"direction\")", &env);
    assert_eq!(eval_line("(getp 'north \"direction\")", &env), "(QUOTE UP)");
    assert_eq!(eval_line("(getp 'south \"direction\")", &env), "(QUOTE DOWN)");
}

// Combined property list operations
#[test]
fn test_property_workflow() {
    let env = env_with_stdlib();
    // Define some properties with deflist
    eval_line("(deflist '((red \"#FF0000\") (green \"#00FF00\") (blue \"#0000FF\")) \"color\")", &env);

    // Add another property manually
    eval_line("(putp 'red \"name\" \"Red Color\")", &env);

    // Verify all properties
    assert_eq!(eval_line("(getp 'red \"color\")", &env), "\"#FF0000\"");
    assert_eq!(eval_line("(getp 'red \"name\")", &env), "\"Red Color\"");

    // Remove one property
    eval_line("(remprop 'red \"color\")", &env);
    assert_eq!(eval_line("(getp 'red \"color\")", &env), "()");
    assert_eq!(eval_line("(getp 'red \"name\")", &env), "\"Red Color\"");
}

#[test]
fn test_different_symbols_independent() {
    let env = env_with_stdlib();
    eval_line("(putp 'sym1 \"key\" 1)", &env);
    eval_line("(putp 'sym2 \"key\" 2)", &env);
    eval_line("(remprop 'sym1 \"key\")", &env);

    assert_eq!(eval_line("(getp 'sym1 \"key\")", &env), "()");
    assert_eq!(eval_line("(getp 'sym2 \"key\")", &env), "2");
}

#[test]
fn test_property_with_docstring() {
    let env = env_with_stdlib();
    eval_line("(defun my-func (x) \"This is a docstring.\" (* x x))", &env);
    let output = eval_line("(getp 'my-func \"docstring\")", &env);
    assert_eq!(output, "\"This is a docstring.\"");
}

#[test]
fn test_deflist_preserves_other_properties() {
    let env = env_with_stdlib();
    eval_line("(putp 'a \"old-prop\" \"keep-me\")", &env);
    eval_line("(deflist '((a 1)) \"new-prop\")", &env);

    assert_eq!(eval_line("(getp 'a \"old-prop\")", &env), "\"keep-me\"");
    assert_eq!(eval_line("(getp 'a \"new-prop\")", &env), "1");
}

#[test]
fn test_numeric_property_values() {
    let env = env_with_stdlib();
    eval_line("(deflist '((zero 0) (one 1) (two 2)) \"number\")", &env);

    // Use the properties in calculations
    let output = eval_line("(+ (getp 'zero \"number\") (getp 'one \"number\") (getp 'two \"number\"))", &env);
    assert_eq!(output, "3");
}
