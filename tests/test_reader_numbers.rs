//! Issue #503: a token that starts like a number but is not a complete one
//! used to be split into a number plus a symbol (`123abc` read as `123 ABC`,
//! `2.0.3` as the dotted pair `2.0 . 3`, `1e5` as `1 E5`, `-8000000000000000H`
//! as decimal `-8000000000000000` plus `H`). Every number parser now requires
//! the literal to end at a delimiter, so such a token is a parse error; `1+`
//! and `1-` alone remain symbols. Also: a lower-case `q` octal suffix,
//! exponent-only floats, and Q/H/`#x` literals parsed with the sign attached
//! (i64::MIN is representable; an overflow is an error, never a fall-through
//! to another base).
use lamedh::environment::Environment;
use lamedh::printer::print;
use lamedh::reader::read_all;
use lamedh::{LispVal, Shared};

fn read_printed(src: &str) -> Result<String, String> {
    let env = Shared::new(Environment::new());
    read_all(src, &env).map(|forms| forms.iter().map(print).collect::<Vec<_>>().join(" "))
}

enum Want {
    /// Reads, and prints as exactly this (all forms, space-separated).
    Reads(&'static str),
    /// Is a parse error whose message contains this text.
    Error(&'static str),
}
use Want::*;

#[test]
fn number_tokens_table() {
    const OUT_OF_RANGE: &str = "number literal out of range or with an invalid digit";
    let table: &[(&str, Want)] = &[
        // Issue repros: malformed tokens are errors, not splits.
        ("123abc", Error("near '123abc'")),
        ("1.5x", Error("near '1.5x'")),
        ("2.0.3", Error("near '2.0.3'")),
        ("1-2", Error("near '1-2'")),
        ("1+2", Error("near '1+2'")),
        ("(quote (123abc 1.5x 2.0.3))", Error("column 9")),
        ("(quote (1-2))", Error("column 9")),
        ("(quote (123abc 1.5x 2.0.3 1-2))", Error("column 9")),
        ("1.", Error("near '1.'")),
        ("1.5e", Error("near '1.5e'")),
        ("12#x", Error("near '12#x'")),
        ("0ffhx", Error("near '0ffhx'")),
        ("#b102", Error("near '#b102'")),
        ("#xFG", Error("near '#xFG'")),
        ("177Qx", Error("near '177Qx'")),
        // `1+` / `1-` are stdlib function names and stay symbols.
        ("1+", Reads("1+")),
        ("1-", Reads("1-")),
        ("(1+ 1- (1+) (1-))", Reads("(1+ 1- (1+) (1-))")),
        // Exponent-only floats.
        ("1e5", Reads("100000.0")),
        ("1E5", Reads("100000.0")),
        ("1e+5", Reads("100000.0")),
        ("-2E-3", Reads("-0.002")),
        ("1e21", Reads("1000000000000000000000.0")),
        ("1e400", Reads("inf")),
        ("1.5e3", Reads("1500.0")),
        ("(quote (1e5))", Reads("(QUOTE (100000.0))")),
        // A leading-digit token ending in H is still hex, even with an `e`.
        ("1e5h", Reads("485")),
        // Octal: either suffix case; a non-octal digit is an error.
        ("177q", Reads("127")),
        ("177Q", Reads("127")),
        ("-10q", Reads("-8")),
        ("8Q", Error(OUT_OF_RANGE)),
        ("(quote (8Q))", Error("column 9")),
        // i64::MIN in every signed notation.
        ("-8000000000000000H", Reads("-9223372036854775808")),
        ("-8000000000000000h", Reads("-9223372036854775808")),
        ("#x-8000000000000000", Reads("-9223372036854775808")),
        ("-1000000000000000000000Q", Reads("-9223372036854775808")),
        ("-9223372036854775808", Reads("-9223372036854775808")),
        ("7FFFFFFFFFFFFFFFH", Reads("9223372036854775807")),
        // One past the range is an error, not a re-read in another base.
        ("8000000000000000H", Error(OUT_OF_RANGE)),
        ("-8000000000000001H", Error(OUT_OF_RANGE)),
        ("#x8000000000000000", Error(OUT_OF_RANGE)),
        ("1000000000000000000000Q", Error(OUT_OF_RANGE)),
        // An oversized decimal integer still reads as a float (unchanged).
        ("99999999999999999999", Reads("100000000000000000000.0")),
        // Delimiters end a number token.
        ("(1 2)", Reads("(1 2)")),
        ("(1 . 2)", Reads("(1 . 2)")),
        ("(1.5)", Reads("(1.5)")),
        ("1;c", Reads("1")),
        ("1\"s\"", Reads("1 \"s\"")),
        ("(0ffh)", Reads("(255)")),
        ("(#x1F)", Reads("(31)")),
        ("(177q)", Reads("(127)")),
    ];
    let mut failures = Vec::new();
    for (src, want) in table {
        let got = read_printed(src);
        let ok = match (want, &got) {
            (Reads(expected), Ok(printed)) => printed == expected,
            (Error(needle), Err(msg)) => msg.contains(needle),
            _ => false,
        };
        if !ok {
            let expected = match want {
                Reads(e) => format!("reads {e}"),
                Error(e) => format!("error containing {e:?}"),
            };
            failures.push(format!("{src:?}: expected {expected}, got {got:?}"));
        }
    }
    assert!(failures.is_empty(), "\n{}", failures.join("\n"));
}

#[test]
fn i64_min_hex_is_a_number() {
    let env = Shared::new(Environment::new());
    let forms = read_all("-8000000000000000H", &env).unwrap();
    assert!(matches!(forms.as_slice(), [LispVal::Number(i64::MIN)]));
}
