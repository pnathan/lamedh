// LispVal is intentionally used as a HashMap key (interior mutability is by design).
#![allow(clippy::mutable_key_type)]

/// Tests targeting uncovered lines in printer.rs to improve coverage.
///
/// Uncovered lines identified from llvm-cov output:
///   24-29 : String escape characters (\n, \t, \r, \0)
///   38    : LispVal::Fexpr  -> "<fexpr>"
///   39    : LispVal::Macro  -> "<macro>"
///
/// LispVal::HashTable -> "<hash-table>" (line 40) is also tested here for
/// completeness even if already covered.
use lamedh::{Fexpr, Lambda, LispVal, Macro, environment::Environment, printer::print};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

// ── String escape characters ──────────────────────────────────────────────────

#[test]
fn test_print_string_with_newline() {
    let s = LispVal::String("hello\nworld".to_string());
    assert_eq!(print(&s), "\"hello\\nworld\"");
}

#[test]
fn test_print_string_with_tab() {
    let s = LispVal::String("tab\there".to_string());
    assert_eq!(print(&s), "\"tab\\there\"");
}

#[test]
fn test_print_string_with_carriage_return() {
    let s = LispVal::String("cr\rend".to_string());
    assert_eq!(print(&s), "\"cr\\rend\"");
}

#[test]
fn test_print_string_with_null_byte() {
    let s = LispVal::String("null\0byte".to_string());
    assert_eq!(print(&s), "\"null\\0byte\"");
}

#[test]
fn test_print_string_with_backslash() {
    let s = LispVal::String("back\\slash".to_string());
    assert_eq!(print(&s), "\"back\\\\slash\"");
}

#[test]
fn test_print_string_with_double_quote() {
    let s = LispVal::String("say \"hi\"".to_string());
    assert_eq!(print(&s), "\"say \\\"hi\\\"\"");
}

#[test]
fn test_print_string_all_escapes_combined() {
    // A string containing every special character in one go.
    let s = LispVal::String("\"\\\n\t\r\0".to_string());
    assert_eq!(print(&s), "\"\\\"\\\\\\n\\t\\r\\0\"");
}

#[test]
fn test_print_string_no_escapes_needed() {
    let s = LispVal::String("plain text 123".to_string());
    assert_eq!(print(&s), "\"plain text 123\"");
}

// ── LispVal::Fexpr ────────────────────────────────────────────────────────────

#[test]
fn test_print_fexpr() {
    let env = Environment::new_with_builtins();
    let fexpr = LispVal::Fexpr(Fexpr {
        params: vec!["X".to_string()],
        body: Box::new(LispVal::Nil),
        env: env.clone(),
    });
    assert_eq!(print(&fexpr), "<fexpr>");
}

// ── LispVal::Macro ────────────────────────────────────────────────────────────

#[test]
fn test_print_macro() {
    let env = Environment::new_with_builtins();
    let mac = LispVal::Macro(Macro {
        params: vec!["X".to_string()],
        rest_param: None,
        body: Box::new(LispVal::Nil),
        env: env.clone(),
    });
    assert_eq!(print(&mac), "<macro>");
}

#[test]
fn test_print_macro_with_rest_param() {
    let env = Environment::new_with_builtins();
    let mac = LispVal::Macro(Macro {
        params: vec![],
        rest_param: Some("ARGS".to_string()),
        body: Box::new(LispVal::Number(0)),
        env: env.clone(),
    });
    assert_eq!(print(&mac), "<macro>");
}

// ── LispVal::HashTable ────────────────────────────────────────────────────────

#[test]
fn test_print_empty_hash_table() {
    let ht = LispVal::HashTable(Rc::new(RefCell::new(HashMap::<LispVal, LispVal>::new())));
    assert_eq!(print(&ht), "<hash-table>");
}

#[test]
fn test_print_nonempty_hash_table() {
    let mut map: HashMap<LispVal, LispVal> = HashMap::new();
    map.insert(LispVal::Number(1), LispVal::Number(42));
    let ht = LispVal::HashTable(Rc::new(RefCell::new(map)));
    assert_eq!(print(&ht), "<hash-table>");
}

// ── LispVal::Lambda (already covered, but included for completeness) ──────────

#[test]
fn test_print_lambda() {
    let env = Environment::new_with_builtins();
    let lam = LispVal::Lambda(Lambda {
        params: vec!["X".to_string()],
        rest_param: None,
        body: Box::new(LispVal::Number(1)),
        env: env.clone(),
    });
    assert_eq!(print(&lam), "<lambda>");
}

// ── Dotted pairs and nested cons cells ───────────────────────────────────────

#[test]
fn test_print_deeply_nested_dotted_pair() {
    // ((a . b) . c)
    let env = Environment::new();
    let inner = LispVal::Cons {
        car: Rc::new(LispVal::Symbol(env.intern_symbol("A"))),
        cdr: Rc::new(LispVal::Symbol(env.intern_symbol("B"))),
    };
    let outer = LispVal::Cons {
        car: Rc::new(inner),
        cdr: Rc::new(LispVal::Symbol(env.intern_symbol("C"))),
    };
    assert_eq!(print(&outer), "((A . B) . C)");
}

#[test]
fn test_print_list_with_dotted_tail() {
    // (1 2 . 3)
    let list = LispVal::Cons {
        car: Rc::new(LispVal::Number(1)),
        cdr: Rc::new(LispVal::Cons {
            car: Rc::new(LispVal::Number(2)),
            cdr: Rc::new(LispVal::Number(3)),
        }),
    };
    assert_eq!(print(&list), "(1 2 . 3)");
}

// ── Additional scalar types ───────────────────────────────────────────────────

#[test]
fn test_print_float() {
    assert_eq!(print(&LispVal::Float(3.25)), "3.25");
}

#[test]
fn test_print_negative_number() {
    assert_eq!(print(&LispVal::Number(-42)), "-42");
}

#[test]
fn test_print_builtin() {
    assert_eq!(
        print(&LispVal::Builtin(lamedh::BuiltinFunc::Plus)),
        "<builtin>"
    );
}
