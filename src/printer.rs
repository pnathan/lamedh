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

use crate::LispVal;

fn print_list_contents(cdr: &LispVal, escape: bool) -> String {
    match cdr {
        LispVal::Cons { car, cdr } => {
            format!(" {}", render(car, escape)) + &print_list_contents(cdr, escape)
        }
        LispVal::Nil => "".to_string(),
        _ => format!(" . {}", render(cdr, escape)),
    }
}

/// A symbol name as readable text: bare when it reads back as itself,
/// otherwise wrapped in the reader's `|...|` escape with `|` and `\`
/// backslashed (issue #523).
fn print_symbol_name(name: &str) -> String {
    if crate::reader::symbol_reads_bare(name) {
        return name.to_string();
    }
    let mut out = String::with_capacity(name.len() + 2);
    out.push('|');
    for c in name.chars() {
        if c == '|' || c == '\\' {
            out.push('\\');
        }
        out.push(c);
    }
    out.push('|');
    out
}

/// Format `val` as readable Lisp text.
///
/// The result is suitable for display in a REPL (`PRIN1` semantics: strings
/// are double-quoted with escapes).  For most self-representing types the
/// output round-trips through [`crate::reader::read`]; opaque types emit
/// non-readable tags like `<lambda>`.
pub fn print(val: &LispVal) -> String {
    render(val, true)
}

/// [`print()`] with every symbol written as its bare name, never
/// `|...|`-escaped — the `PRINC` view of a symbol, whose name is text for a
/// human rather than for the reader. Everything else prints as [`print()`]
/// does.
pub fn print_plain_symbols(val: &LispVal) -> String {
    render(val, false)
}

fn render(val: &LispVal, escape: bool) -> String {
    match val {
        // Just the symbol name, regardless of plist.
        LispVal::Symbol(s) if escape => print_symbol_name(&s.borrow().name),
        LispVal::Symbol(s) => s.borrow().name.clone(),
        LispVal::Number(n) => n.to_string(),
        LispVal::Char(b) => match b {
            b'\n' => "'\\n'".to_string(),
            b'\t' => "'\\t'".to_string(),
            b'\r' => "'\\r'".to_string(),
            b'\\' => "'\\\\'".to_string(),
            b'\'' => "'\\''".to_string(),
            b'\0' => "'\\0'".to_string(),
            _ => format!("'{}'", *b as char),
        },
        LispVal::Float(f) => {
            let s = f.to_string();
            if s.contains('.')
                || s.contains('e')
                || s.contains('E')
                || s.contains("inf")
                || s.contains("NaN")
            {
                s
            } else {
                format!("{}.0", s)
            }
        }
        LispVal::String(s) => {
            let mut out = String::with_capacity(s.len() + 2);
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
            out
        }
        LispVal::Builtin(_) => "<builtin>".to_string(),
        LispVal::Lambda(_) => "<lambda>".to_string(),
        LispVal::Fexpr(_) => "<fexpr>".to_string(),
        LispVal::Macro(_) => "<macro>".to_string(),
        LispVal::Vau(_) => "<vau>".to_string(),
        LispVal::HashTable(_) => "<hash-table>".to_string(),
        LispVal::Array(a) => format!("<array:{}>", a.borrow().len()),
        LispVal::TypedArray(a) => format!("<typed-array:{}:{}>", a.elem, a.len()),
        // Readable record syntax (issue #308 stage D): field values in
        // declaration order, each printed readably, so the output round-trips
        // through the reader's #S literal (spawn/channel serialization).
        LispVal::Struct(s) => {
            let mut out = format!("#S({}", s.type_name);
            for f in &s.fields {
                out.push(' ');
                out.push_str(&render(f, escape));
            }
            out.push(')');
            out
        }
        LispVal::Extension(e) => e.display(),
        LispVal::Error(e) => {
            if e.data == LispVal::Nil {
                format!("#<error {:?}>", e.message)
            } else {
                format!("#<error {:?} {}>", e.message, render(&e.data, escape))
            }
        }
        LispVal::Native(_) => "<native>".to_string(),
        LispVal::Environment(_) => "<environment>".to_string(),
        LispVal::Port(p) => format!(
            "#<port:{} {:?} {}>",
            p.kind,
            p.name,
            if p.is_open() { "open" } else { "closed" }
        ),
        LispVal::NetHandle(h) => format!(
            "#<net:{} {:?} {}>",
            h.kind,
            h.name,
            if h.is_open() { "open" } else { "closed" }
        ),
        LispVal::OsChild(c) => format!(
            "#<process {:?} {}>",
            c.name,
            if c.is_open() { "running" } else { "reaped" }
        ),
        #[cfg(feature = "concurrency")]
        LispVal::Channel(_) => "<channel>".to_string(),
        LispVal::Nil => "()".to_string(),
        LispVal::Cons { car, cdr } => {
            format!(
                "({}{})",
                render(car, escape),
                print_list_contents(cdr, escape)
            )
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
