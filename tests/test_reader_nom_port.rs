//! Characterization of reader behaviour across the nom 7 -> 8 port (#631).
//! The snapshot in `tests/data/reader_nom_snapshot.txt` was recorded on nom 7;
//! the port must reproduce it byte for byte: every accepted form, every
//! printed AST, every error offset and detail, and every `is_incomplete` verdict.
//! Regenerate deliberately with `UPDATE_READER_SNAPSHOT=1`.
use lamedh::Shared;
use lamedh::environment::Environment;
use lamedh::printer::print;
use lamedh::reader::{is_incomplete, read_all, read_next, read_next_with_depth_limit};

const CORPUS: &[&str] = &[
    "",
    "   ",
    "; c\n",
    "#| b |#",
    "#| #| n |# |# 1",
    "#| open",
    "(",
    ")",
    "(1 2",
    "(1 . 2)",
    "(1 . )",
    "(. 1)",
    "(1 . 2 3)",
    "'a",
    "'(a b)",
    "`(a ,b ,@c)",
    "#'car",
    "'",
    ",",
    ",@",
    "\"str\"",
    "\"a\\nb\"",
    "\"unterminated",
    "\"bad\\q\"",
    "#\\a",
    "#\\Space",
    "#\\",
    "'a'",
    "'ab",
    "123",
    "-123",
    "+5",
    "1.5",
    "-1.5e3",
    "1e5",
    "1.5e",
    "123abc",
    "2.0.3",
    "1+",
    "1-",
    "1q",
    "17Q",
    "ffH",
    "-ffH",
    "#xff",
    "#b101",
    "#o17",
    "#x",
    "#xZZ",
    "#b2",
    "99999999999999999999",
    "-9223372036854775808",
    "9223372036854775808",
    "foo",
    "FOO-bar",
    "a.b",
    "a?",
    "<=",
    "!x",
    "_x",
    "?_",
    "nil",
    "()",
    "(a (b (c)))",
    "(a ; c\n b)",
    "(a #| c |# b)",
    "#(1 2 3)",
    "#S(pt 1 2)",
    "#S",
    "#S(",
    "#+foo 1 2",
    "#-foo 1 2",
    "#+(and a b) 1 2",
    "#+",
    "#+x",
    "#+x (",
    "#!/bin/x\n1",
    "(1 2) (3",
    "a b c",
    "(a . b) (c . d)",
    "'''a",
    "#:g",
    "#",
    "#z",
    "{",
    "}",
    "[",
    "é",
    "(é)",
    "\"é\"",
    "1 2 ) 3",
    "(quote . a)",
    "( 1 )",
    "#\\(",
    "#\\)",
    "1.",
    ".5",
    "-",
    "+",
    "-.5",
    "1e+5",
];

fn snapshot() -> String {
    let mut out = String::new();
    for src in CORPUS {
        out.push_str(&format!("== {src:?}\n"));
        let env = Shared::new(Environment::new());
        match read_all(src, &env) {
            Ok(forms) => out.push_str(&format!(
                "all: {}\n",
                forms.iter().map(print).collect::<Vec<_>>().join(" | ")
            )),
            Err(e) => out.push_str(&format!("all-err: {e}\n")),
        }
        match read_next(src, &env) {
            Ok(Some((v, rest))) => out.push_str(&format!("next: {} rest={rest:?}\n", print(&v))),
            Ok(None) => out.push_str("next: none\n"),
            Err((off, d)) => out.push_str(&format!("next-err: {off} {d}\n")),
        }
        match read_next_with_depth_limit(src, &env, 3) {
            Ok(Some((v, rest))) => out.push_str(&format!("d3: {} rest={rest:?}\n", print(&v))),
            Ok(None) => out.push_str("d3: none\n"),
            Err((off, d)) => out.push_str(&format!("d3-err: {off} {d}\n")),
        }
        out.push_str(&format!("incomplete: {}\n", is_incomplete(src)));
    }
    let deep = "(".repeat(600);
    let env = Shared::new(Environment::new());
    match read_next(&deep, &env) {
        Err((off, d)) => out.push_str(&format!("deep: {off} {d}\n")),
        other => out.push_str(&format!("deep: unexpected {}\n", other.is_ok())),
    }
    out
}

#[test]
fn reader_matches_nom7_snapshot() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/data/reader_nom_snapshot.txt"
    );
    let got = snapshot();
    if std::env::var_os("UPDATE_READER_SNAPSHOT").is_some() {
        std::fs::write(path, &got).unwrap();
        return;
    }
    let want = std::fs::read_to_string(path).expect("snapshot file");
    assert_eq!(got, want);
}
