//! `|...|` symbol escapes and case-preserving `intern` (issue #523): every
//! symbol the printer writes reads back as that same symbol, and symbols
//! that already round-tripped print exactly as before.

use lamedh::environment::Environment;
use lamedh::printer::{print, print_plain_symbols};
use lamedh::reader::{is_incomplete, read};
use lamedh::{LispVal, Shared, eval_line};

fn env() -> Shared<Environment> {
    Environment::new_with_builtins()
}

fn sym(name: &str, env: &Shared<Environment>) -> LispVal {
    LispVal::Symbol(env.intern_symbol(name))
}

/// The name of the symbol `src` reads as, or `None` if it reads as anything
/// else (or fails to read).
fn read_symbol_name(src: &str, env: &Shared<Environment>) -> Option<String> {
    match read(src, env) {
        Ok(LispVal::Symbol(s)) => Some(s.borrow().name.clone()),
        _ => None,
    }
}

#[test]
fn issue_repro_intern_with_space_round_trips() {
    let env = env();
    assert_eq!(eval_line("(intern \"a b\")", &env), "|a b|");
    assert_eq!(eval_line("(eq (intern \"a b\") '|a b|)", &env), "T");
}

#[test]
fn intern_preserves_case() {
    let env = env();
    assert_eq!(eval_line("(eq (intern \"foo\") 'foo)", &env), "()");
    assert_eq!(eval_line("(eq (intern \"FOO\") 'foo)", &env), "T");
    assert_eq!(eval_line("(eq (intern \"foo\") '|foo|)", &env), "T");
    assert_eq!(eval_line("(intern \"foo\")", &env), "|foo|");
}

#[test]
fn reader_bar_escape_takes_name_verbatim() {
    let env = env();
    assert_eq!(read_symbol_name("|a b|", &env).as_deref(), Some("a b"));
    assert_eq!(read_symbol_name("|FOO|", &env).as_deref(), Some("FOO"));
    assert_eq!(
        read_symbol_name("|(x) 'y \"z\"|", &env).as_deref(),
        Some("(x) 'y \"z\"")
    );
    assert_eq!(read_symbol_name("|12|", &env).as_deref(), Some("12"));
    assert_eq!(read_symbol_name("||", &env).as_deref(), Some(""));
    // `\|` and `\\` escape; any other backslash is kept as written.
    assert_eq!(read_symbol_name(r"|a\|b|", &env).as_deref(), Some("a|b"));
    assert_eq!(read_symbol_name(r"|a\\b|", &env).as_deref(), Some(r"a\b"));
    assert_eq!(read_symbol_name(r"|a\nb|", &env).as_deref(), Some(r"a\nb"));
    // `|NIL|` is the symbol named NIL, not the empty list.
    assert_eq!(read_symbol_name("|NIL|", &env).as_deref(), Some("NIL"));
    assert_eq!(read("NIL", &env), Ok(LispVal::Nil));
}

#[test]
fn reader_bar_escape_in_context() {
    let env = env();
    assert_eq!(print(&read("(|a||b| c)", &env).unwrap()), "(|a| |b| C)");
    assert_eq!(print(&read("'|x y|", &env).unwrap()), "(QUOTE |x y|)");
    assert_eq!(print(&read("(a . |b c|)", &env).unwrap()), "(A . |b c|)");
    // Block comments are unaffected.
    assert_eq!(print(&read("#| |q| |# FOO", &env).unwrap()), "FOO");
}

#[test]
fn reader_unterminated_bar_escape_is_an_error() {
    let env = env();
    assert!(read("|abc", &env).is_err());
    assert!(read(r"|abc\|", &env).is_err());
    assert!(read("(foo |abc)", &env).is_err());
}

#[test]
fn is_incomplete_skips_bar_escapes() {
    assert!(!is_incomplete("(foo |(|)"));
    assert!(!is_incomplete("(foo |)|)"));
    assert!(is_incomplete("(foo |)|"));
    assert!(is_incomplete("|abc"));
    assert!(is_incomplete(r"|abc\|"));
    assert!(!is_incomplete("|a b|"));
}

#[test]
fn printer_escapes_names_that_would_not_read_back() {
    let env = env();
    for (name, printed) in [
        ("a b", "|a b|"),
        ("a\tb", "|a\tb|"),
        ("(", "|(|"),
        (")", "|)|"),
        ("'x", "|'x|"),
        ("\"x\"", "|\"x\"|"),
        ("foo", "|foo|"),
        ("Foo", "|Foo|"),
        ("12", "|12|"),
        ("-3", "|-3|"),
        ("1.5", "|1.5|"),
        ("0FFH", "|0FFH|"),
        ("177Q", "|177Q|"),
        ("#X1F", "|#X1F|"),
        (".", "|.|"),
        ("NIL", "|NIL|"),
        ("", "||"),
        ("A.B", "|A.B|"),
        ("*A", "|*A|"),
        ("*A*B*", "|*A*B*|"),
        (":A:B", "|:A:B|"),
        ("1+X", "|1+X|"),
        ("É", "|É|"),
        ("a|b", r"|a\|b|"),
        (r"a\b", r"|a\\b|"),
    ] {
        let s = sym(name, &env);
        assert_eq!(print(&s), printed, "printing symbol {name:?}");
        assert_eq!(read_symbol_name(printed, &env).as_deref(), Some(name));
    }
}

#[test]
fn ordinary_symbols_print_bare() {
    let env = env();
    for name in [
        "FOO",
        "T",
        "+",
        "-",
        "*",
        "/",
        ">=",
        "/=",
        "!=",
        "~",
        "1+",
        "1-",
        "*X*",
        "*PRINT-LEVEL*",
        "+HOST-TRAITS+",
        ":KEY",
        ":&REST",
        "?X",
        "??XS",
        "?_",
        "&REST",
        "$DEFUN",
        "MOD:SYM",
        "SET-CAR!",
        "A1",
        "NULL?",
        "<=>",
        "X_Y",
    ] {
        assert_eq!(print(&sym(name, &env)), name);
        assert_eq!(read_symbol_name(name, &env).as_deref(), Some(name));
    }
}

#[test]
fn princ_writes_bare_symbol_names() {
    let env = env();
    let s = sym("a b", &env);
    assert_eq!(print_plain_symbols(&s), "a b");
    assert_eq!(
        eval_line("(princ-to-string (intern \"a b\"))", &env),
        "\"a b\""
    );
    assert_eq!(
        eval_line("(prin1-to-string (intern \"a b\"))", &env),
        "\"|a b|\""
    );
}

/// Every symbol interned by loading the standard library (almost all of them
/// read from source) prints bare exactly when its bare name reads back as
/// itself — so no existing symbol changes how it prints.
#[test]
fn stdlib_symbols_print_as_before() {
    lamedh::with_large_stack(|| {
        let env = Environment::with_stdlib();
        let names: Vec<String> = env
            .all_symbols()
            .iter()
            .map(|s| s.borrow().name.clone())
            .collect();
        assert!(names.len() > 1000);
        for name in names {
            let s = sym(&name, &env);
            let bare_round_trips = read_symbol_name(&name, &env).as_deref() == Some(name.as_str());
            assert_eq!(
                print(&s) == name,
                bare_round_trips,
                "symbol {name:?} printed as {}",
                print(&s)
            );
        }
    });
}

/// Property: for pseudo-random names over an alphabet heavy in reader
/// syntax, the printed symbol reads back as the same symbol, and a name
/// prints bare exactly when its bare spelling already reads back as itself.
#[test]
fn every_printed_symbol_reads_back_as_itself() {
    const ALPHABET: &[char] = &[
        'A', 'B', 'Z', 'Q', 'H', 'E', 'X', 'a', 'z', 'q', 'h', '0', '1', '7', '9', ' ', '\t', '\n',
        '(', ')', '\'', '"', '`', ',', '@', ';', '#', '|', '\\', '.', ':', '-', '+', '*', '/', '=',
        '<', '>', '!', '~', '?', '&', '$', '_', '%', '[', ']', 'é', 'λ',
    ];
    let env = env();
    let mut state: u64 = 0x9E37_79B9_7F4A_7C15;
    let mut next = || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };
    for _ in 0..50_000 {
        let len = (next() % 9) as usize;
        // Half the names draw only from upper-case letters and constituents,
        // so the bare path is exercised as often as the escaped one.
        let pool = if next().is_multiple_of(2) {
            ALPHABET
        } else {
            &ALPHABET[..7]
        };
        let name: String = (0..len)
            .map(|_| {
                let r = next() as usize;
                if pool.len() == 7 && r.is_multiple_of(3) {
                    ['-', '*', '+', ':', '?', '1', '='][r / 3 % 7]
                } else {
                    pool[r % pool.len()]
                }
            })
            .collect();
        let printed = print(&sym(&name, &env));
        assert_eq!(
            read_symbol_name(&printed, &env).as_deref(),
            Some(name.as_str()),
            "symbol {name:?} printed as {printed:?}"
        );
        let bare_round_trips = read_symbol_name(&name, &env).as_deref() == Some(name.as_str());
        assert_eq!(
            printed == name,
            bare_round_trips,
            "symbol {name:?} printed as {printed:?}"
        );
    }
}
