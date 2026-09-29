//! Printer regression tests for issue #509: list printing must be linear in
//! the output size and must not use Rust stack per spine cons, while staying
//! byte-identical to the previous (recursive, string-concatenating) printer.

mod test_helpers;

use lamedh::environment::Environment;
use lamedh::printer::print;
use lamedh::reader::read;
use lamedh::{ErrorObj, LispVal, Shared, StructObj, eval_line};
use std::time::{Duration, Instant};
use test_helpers::env_with_stdlib;

fn cons(car: LispVal, cdr: LispVal) -> LispVal {
    LispVal::Cons {
        car: Shared::new(car),
        cdr: Shared::new(cdr),
    }
}

fn list(items: Vec<LispVal>) -> LispVal {
    items
        .into_iter()
        .rev()
        .fold(LispVal::Nil, |acc, x| cons(x, acc))
}

fn n(i: i64) -> LispVal {
    LispVal::Number(i)
}

fn s(x: &str) -> LispVal {
    LispVal::String(x.to_string())
}

/// The pre-#509 list algorithm, kept as a reference: every list is printed by
/// it and by `print`, and the two must agree byte for byte. Leaves delegate to
/// `print`, so this pins the list/dotted-tail structure specifically.
fn reference_print(val: &LispVal) -> String {
    fn contents(cdr: &LispVal) -> String {
        match cdr {
            LispVal::Cons { car, cdr } => format!(" {}", reference_print(car)) + &contents(cdr),
            LispVal::Nil => "".to_string(),
            _ => format!(" . {}", reference_print(cdr)),
        }
    }
    match val {
        LispVal::Cons { car, cdr } => format!("({}{})", reference_print(car), contents(cdr)),
        other => print(other),
    }
}

fn assert_prints(val: &LispVal, expected: &str) {
    assert_eq!(print(val), expected);
    assert_eq!(print(val), reference_print(val));
}

#[test]
fn dotted_lists_print_unchanged() {
    assert_prints(&cons(n(1), n(2)), "(1 . 2)");
    assert_prints(&cons(n(1), cons(n(2), n(3))), "(1 2 . 3)");
    assert_prints(&cons(cons(n(1), n(2)), cons(n(3), n(4))), "((1 . 2) 3 . 4)");
    assert_prints(&cons(n(1), s("tail")), "(1 . \"tail\")");
    assert_prints(&cons(n(1), LispVal::Float(2.0)), "(1 . 2.0)");
    assert_prints(&cons(LispVal::Nil, LispVal::Nil), "(())");
}

#[test]
fn nested_lists_print_unchanged() {
    assert_prints(&list(vec![]), "()");
    assert_prints(&list(vec![list(vec![list(vec![])])]), "((()))");
    let deep = list(vec![
        n(1),
        list(vec![n(2), list(vec![n(3), list(vec![n(4)])])]),
        n(5),
    ]);
    assert_prints(&deep, "(1 (2 (3 (4))) 5)");
    assert_prints(
        &list(vec![LispVal::Nil, cons(n(1), LispVal::Nil), LispVal::Nil]),
        "(() (1) ())",
    );
}

#[test]
fn strings_and_chars_inside_lists_print_unchanged() {
    assert_prints(
        &list(vec![s("a\"b"), s("c\\d"), s("e\nf\t\r\0"), s("")]),
        r#"("a\"b" "c\\d" "e\nf\t\r\0" "")"#,
    );
    assert_prints(
        &list(vec![
            LispVal::Char(b'a'),
            LispVal::Char(b'\n'),
            LispVal::Char(b'\''),
        ]),
        r"('a' '\n' '\'')",
    );
    assert_prints(
        &list(vec![LispVal::Float(3.0), LispVal::Float(-0.5), n(-7)]),
        "(3.0 -0.5 -7)",
    );
}

#[test]
fn quote_forms_print_unchanged() {
    let env = Environment::new_with_builtins();
    for (src, expected) in [
        ("'x", "(QUOTE X)"),
        ("''x", "(QUOTE (QUOTE X))"),
        (
            "`(a ,b ,@c)",
            "(QUASIQUOTE (A (UNQUOTE B) (UNQUOTE-SPLICING C)))",
        ),
        ("'(a . b)", "(QUOTE (A . B))"),
    ] {
        let form = read(src, &env).expect("read");
        assert_prints(&form, expected);
    }
}

#[test]
fn records_and_errors_print_unchanged() {
    let rec = LispVal::Struct(Shared::new(StructObj {
        type_name: "PT".to_string(),
        fields: vec![n(3), list(vec![s("a\"b"), s("c")]), cons(n(1), n(2))],
    }));
    assert_eq!(print(&rec), r#"#S(PT 3 ("a\"b" "c") (1 . 2))"#);
    let empty = LispVal::Struct(Shared::new(StructObj {
        type_name: "EMPTY".to_string(),
        fields: vec![],
    }));
    assert_eq!(print(&empty), "#S(EMPTY)");
    assert_prints(
        &list(vec![rec.clone(), n(1)]),
        r#"(#S(PT 3 ("a\"b" "c") (1 . 2)) 1)"#,
    );

    let err = |data| {
        LispVal::Error(Shared::new(ErrorObj {
            message: "boom".to_string(),
            data,
        }))
    };
    assert_eq!(print(&err(LispVal::Nil)), "#<error \"boom\">");
    assert_eq!(
        print(&err(list(vec![n(1), cons(n(2), n(3))]))),
        "#<error \"boom\" (1 (2 . 3))>"
    );
}

#[test]
fn defrecord_values_print_unchanged() {
    let env = env_with_stdlib();
    eval_line("(defrecord pt (x int64) (tags (list string)))", &env);
    assert_eq!(
        eval_line("(make-pt 3 (list \"a\\\"b\" \"c\"))", &env),
        r#"#S(PT 3 ("a\"b" "c"))"#
    );
    assert_eq!(
        eval_line("(list (make-pt 1 nil) (cons 2 3))", &env),
        "(#S(PT 1 ()) (2 . 3))"
    );
}

/// Builds (0 1 ... len-1 . tail) iteratively.
fn long_list(len: i64, tail: LispVal) -> LispVal {
    (0..len).rev().fold(tail, |acc, i| cons(n(i), acc))
}

fn expected_long(len: i64, tail: &str) -> String {
    let mut out = String::from("(");
    for i in 0..len {
        if i > 0 {
            out.push(' ');
        }
        out.push_str(&i.to_string());
    }
    out.push_str(tail);
    out.push(')');
    out
}

#[test]
fn million_element_list_prints_in_well_under_a_second() {
    // The large stack is for constructing and dropping the million-cons
    // chain (`Rc` drop recurses down the spine); printing itself must not
    // need it, and is timed alone.
    let elapsed = lamedh::with_large_stack(|| {
        let len = 1_000_000;
        let val = long_list(len, LispVal::Nil);
        let start = Instant::now();
        let out = print(&val);
        let elapsed = start.elapsed();
        assert_eq!(out, expected_long(len, ""));
        elapsed
    });
    assert!(
        elapsed < Duration::from_secs(1),
        "printing a 10^6-element list took {elapsed:?}"
    );
}

#[test]
fn long_dotted_list_prints_its_tail() {
    let out = lamedh::with_large_stack(|| print(&long_list(200_000, n(-1))));
    assert_eq!(out, expected_long(200_000, " . -1"));
}

#[test]
fn long_list_spine_does_not_consume_rust_stack() {
    // A 64 KiB thread stack cannot hold one Rust frame per cons of a
    // 100k-element spine; the loop-based printer needs only constant stack
    // for the spine. `forget` avoids the recursive `Rc` drop on this thread.
    let out = std::thread::Builder::new()
        .stack_size(64 * 1024)
        .spawn(|| {
            let val = long_list(100_000, LispVal::Nil);
            let out = print(&val);
            std::mem::forget(val);
            out.len()
        })
        .expect("spawn")
        .join()
        .expect("printing a long list overflowed a small stack");
    assert_eq!(out, expected_long(100_000, "").len());
}
