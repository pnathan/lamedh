//! Format [`LispVal`] values as readable Lisp text.
//!
//! The single public function [`print()`] converts any [`LispVal`] to a `String`
//! suitable for display in a REPL or written to a file.  The output is valid
//! input for the [`crate::reader`] for all self-representing types (numbers,
//! strings, symbols, lists) — with the exception of opaque types like
//! `<lambda>`, `<builtin>`, and `<hash-table>` which are not readable.
//!
//! ## Format rules
//!
//! | Value | Output |
//! |-------|--------|
//! | `Symbol("FOO")` | `FOO` |
//! | `Symbol("a b")` | `\|a b\|` (any name that would not read back bare; issue #523) |
//! | `Number(42)` | `42` |
//! | `Char(97)` | `'a'` (same escapes as the reader: `\n \t \r \\ \' \0`) |
//! | `Float(3.0)` | `3.0` (always includes `.`) |
//! | `String("hi\n")` | `"hi\n"` (escaped) |
//! | `Nil` | `()` |
//! | Proper list `(a b c)` | `(A B C)` |
//! | Dotted pair `(a . b)` | `(A . B)` |
//! | `Lambda` | `<lambda>` |
//! | `Builtin` | `<builtin>` |
//! | `HashTable` | `<hash-table>` |
//! | `Array(n)` | `<array:n>` |
//! | `TypedArray(n)` | `<typed-array:elem:n>` |
//! | `Struct` | `#S(TYPE field...)` (round-trips via the reader) |
//! | `Extension` | via [`crate::LispValExtension::display`] |
//! | `Port` | `#<port:kind "name" open|closed>` |

use std::fmt::Write;

use crate::LispVal;

/// Format `val` as readable Lisp text.
///
/// The result is suitable for display in a REPL (`PRIN1` semantics: strings
/// are double-quoted with escapes).  For most self-representing types the
/// output round-trips through [`crate::reader::read`]; opaque types emit
/// non-readable tags like `<lambda>`.
pub fn print(val: &LispVal) -> String {
    let mut out = String::new();
    write_val(&mut out, val, true);
    out
}

/// [`print()`] with every symbol written as its bare name, never
/// `|...|`-escaped — the `PRINC` view of a symbol, whose name is text for a
/// human rather than for the reader. Everything else prints as [`print()`]
/// does.
pub fn print_plain_symbols(val: &LispVal) -> String {
    let mut out = String::new();
    write_val(&mut out, val, false);
    out
}

/// Appends a symbol name as readable text: bare when it reads back as
/// itself, otherwise wrapped in the reader's `|...|` escape with `|` and `\`
/// backslashed (issue #523).
fn write_symbol_name(out: &mut String, name: &str) {
    if crate::reader::symbol_reads_bare(name) {
        out.push_str(name);
        return;
    }
    out.reserve(name.len() + 2);
    out.push('|');
    for c in name.chars() {
        if c == '|' || c == '\\' {
            out.push('\\');
        }
        out.push(c);
    }
    out.push('|');
}

// Appends `val` to `out`.  Everything is written into the one buffer, so
// printing is linear in the output size; a list's spine is walked in a loop
// and only `car`s recurse, so a long list costs no Rust stack (issue #509).
// Writing to a `String` cannot fail, so `write!` results are discarded.
// `escape` selects readable symbol names ([`print`]) over bare ones
// ([`print_plain_symbols`]).
fn write_val(out: &mut String, val: &LispVal, escape: bool) {
    match val {
        // Just the symbol name, regardless of plist.
        LispVal::Symbol(s) if escape => write_symbol_name(out, &s.borrow().name),
        LispVal::Symbol(s) => out.push_str(&s.borrow().name),
        LispVal::Number(n) => {
            let _ = write!(out, "{}", n);
        }
        LispVal::Char(b) => match b {
            b'\n' => out.push_str("'\\n'"),
            b'\t' => out.push_str("'\\t'"),
            b'\r' => out.push_str("'\\r'"),
            b'\\' => out.push_str("'\\\\'"),
            b'\'' => out.push_str("'\\''"),
            b'\0' => out.push_str("'\\0'"),
            _ => {
                let _ = write!(out, "'{}'", *b as char);
            }
        },
        LispVal::Float(f) => {
            let s = f.to_string();
            out.push_str(&s);
            if !(s.contains('.')
                || s.contains('e')
                || s.contains('E')
                || s.contains("inf")
                || s.contains("NaN"))
            {
                out.push_str(".0");
            }
        }
        LispVal::String(s) => {
            out.reserve(s.len() + 2);
            out.push('"');
            for c in s.chars() {
                match c {
                    '"' => out.push_str("\\\""),
                    '\\' => out.push_str("\\\\"),
                    '\n' => out.push_str("\\n"),
                    '\t' => out.push_str("\\t"),
                    '\r' => out.push_str("\\r"),
                    '\0' => out.push_str("\\0"),
                    _ => out.push(c),
                }
            }
            out.push('"');
        }
        LispVal::Builtin(_) => out.push_str("<builtin>"),
        LispVal::Lambda(_) => out.push_str("<lambda>"),
        LispVal::Fexpr(_) => out.push_str("<fexpr>"),
        LispVal::Macro(_) => out.push_str("<macro>"),
        LispVal::Vau(_) => out.push_str("<vau>"),
        LispVal::HashTable(_) => out.push_str("<hash-table>"),
        LispVal::Array(a) => {
            let _ = write!(out, "<array:{}>", a.borrow().len());
        }
        LispVal::TypedArray(a) => {
            let _ = write!(out, "<typed-array:{}:{}>", a.elem, a.len());
        }
        // Readable record syntax (issue #308 stage D): field values in
        // declaration order, each printed readably, so the output round-trips
        // through the reader's #S literal (spawn/channel serialization).
        LispVal::Struct(s) => {
            let _ = write!(out, "#S({}", s.type_name);
            for f in &s.fields {
                out.push(' ');
                write_val(out, f, escape);
            }
            out.push(')');
        }
        LispVal::Extension(e) => out.push_str(&e.display()),
        LispVal::Error(e) => {
            let _ = write!(out, "#<error {:?}", e.message);
            if e.data != LispVal::Nil {
                out.push(' ');
                write_val(out, &e.data, escape);
            }
            out.push('>');
        }
        LispVal::Native(_) => out.push_str("<native>"),
        LispVal::Environment(_) => out.push_str("<environment>"),
        LispVal::Port(p) => {
            let _ = write!(
                out,
                "#<port:{} {:?} {}>",
                p.kind,
                p.name,
                if p.is_open() { "open" } else { "closed" }
            );
        }
        LispVal::NetHandle(h) => {
            let _ = write!(
                out,
                "#<net:{} {:?} {}>",
                h.kind,
                h.name,
                if h.is_open() { "open" } else { "closed" }
            );
        }
        LispVal::OsChild(c) => {
            let _ = write!(
                out,
                "#<process {:?} {}>",
                c.name,
                if c.is_open() { "running" } else { "reaped" }
            );
        }
        #[cfg(feature = "concurrency")]
        LispVal::Channel(_) => out.push_str("<channel>"),
        LispVal::Nil => out.push_str("()"),
        LispVal::Cons { car, cdr } => {
            out.push('(');
            write_val(out, car, escape);
            let mut rest: &LispVal = cdr;
            loop {
                match rest {
                    LispVal::Cons { car, cdr } => {
                        out.push(' ');
                        write_val(out, car, escape);
                        rest = cdr;
                    }
                    LispVal::Nil => break,
                    tail => {
                        out.push_str(" . ");
                        write_val(out, tail, escape);
                        break;
                    }
                }
            }
            out.push(')');
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Shared;
    use crate::environment::Environment;

    fn cons(car: LispVal, cdr: LispVal) -> LispVal {
        LispVal::Cons {
            car: Shared::new(car),
            cdr: Shared::new(cdr),
        }
    }

    fn symbol(s: &str, env: &mut Environment) -> LispVal {
        LispVal::Symbol(env.intern_symbol(s))
    }

    fn number(n: i64) -> LispVal {
        LispVal::Number(n)
    }

    #[test]
    fn test_print_nested_list() {
        let mut env = Environment::new();
        let list = cons(
            symbol("+", &mut env),
            cons(
                number(10),
                cons(
                    cons(
                        symbol("*", &mut env),
                        cons(number(5), cons(number(2), LispVal::Nil)),
                    ),
                    LispVal::Nil,
                ),
            ),
        );
        assert_eq!(print(&list), "(+ 10 (* 5 2))");
    }

    #[test]
    fn test_print_string() {
        let s = LispVal::String("hello world".to_string());
        assert_eq!(print(&s), "\"hello world\"");
    }

    #[test]
    fn test_print_dotted_list() {
        let mut env = Environment::new();
        let list = cons(symbol("A", &mut env), symbol("B", &mut env));
        assert_eq!(print(&list), "(A . B)");
    }

    #[test]
    fn test_print_complex_dotted_list() {
        let mut env = Environment::new();
        let list = cons(
            symbol("A", &mut env),
            cons(symbol("B", &mut env), symbol("C", &mut env)),
        );
        assert_eq!(print(&list), "(A B . C)");
    }

    #[test]
    fn test_print_nil() {
        assert_eq!(print(&LispVal::Nil), "()");
    }

    #[test]
    fn test_print_symbol_with_plist() {
        let env = Environment::new();
        let s = env.intern_symbol("A");
        s.borrow_mut()
            .plist
            .insert("key".to_string(), LispVal::String("value".to_string()));
        let lisp_val = LispVal::Symbol(s);
        // Symbols always print as just their name, regardless of plist
        assert_eq!(print(&lisp_val), "A");
    }
}
