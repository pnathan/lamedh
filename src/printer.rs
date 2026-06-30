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
//! | `Struct` | `#<struct TYPE>` |
//! | `Extension` | via [`crate::LispValExtension::display`] |

use crate::LispVal;

fn print_list_contents(cdr: &LispVal) -> String {
    match cdr {
        LispVal::Cons { car, cdr } => format!(" {}", print(car)) + &print_list_contents(cdr),
        LispVal::Nil => "".to_string(),
        _ => format!(" . {}", print(cdr)),
    }
}

/// Format `val` as readable Lisp text.
///
/// The result is suitable for display in a REPL (`PRIN1` semantics: strings
/// are double-quoted with escapes).  For most self-representing types the
/// output round-trips through [`crate::reader::read`]; opaque types emit
/// non-readable tags like `<lambda>`.
pub fn print(val: &LispVal) -> String {
    match val {
        LispVal::Symbol(s) => {
            // Always print just the symbol name, regardless of plist
            s.borrow().name.clone()
        }
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
        LispVal::Struct(s) => format!("#<struct {}>", s.type_name),
        LispVal::Extension(e) => e.display(),
        LispVal::Error(e) => {
            if e.data == LispVal::Nil {
                format!("#<error {:?}>", e.message)
            } else {
                format!("#<error {:?} {}>", e.message, print(&e.data))
            }
        }
        LispVal::Native(_) => "<native>".to_string(),
        LispVal::Environment(_) => "<environment>".to_string(),
        #[cfg(feature = "concurrency")]
        LispVal::Channel(_) => "<channel>".to_string(),
        LispVal::Nil => "()".to_string(),
        LispVal::Cons { car, cdr } => {
            format!("({}{})", print(car), print_list_contents(cdr))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::environment::Environment;
    use std::rc::Rc;

    fn cons(car: LispVal, cdr: LispVal) -> LispVal {
        LispVal::Cons {
            car: Rc::new(car),
            cdr: Rc::new(cdr),
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
        let list = cons(symbol("a", &mut env), symbol("b", &mut env));
        assert_eq!(print(&list), "(a . b)");
    }

    #[test]
    fn test_print_complex_dotted_list() {
        let mut env = Environment::new();
        let list = cons(
            symbol("a", &mut env),
            cons(symbol("b", &mut env), symbol("c", &mut env)),
        );
        assert_eq!(print(&list), "(a b . c)");
    }

    #[test]
    fn test_print_nil() {
        assert_eq!(print(&LispVal::Nil), "()");
    }

    #[test]
    fn test_print_symbol_with_plist() {
        let env = Environment::new();
        let s = env.intern_symbol("a");
        s.borrow_mut()
            .plist
            .insert("key".to_string(), LispVal::String("value".to_string()));
        let lisp_val = LispVal::Symbol(s);
        // Symbols always print as just their name, regardless of plist
        assert_eq!(print(&lisp_val), "a");
    }
}
