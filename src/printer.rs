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
//! | `Array` | `#(1 2 3)` (round-trips via the reader's `#(...)` literal) |
//! | `TypedArray` | `#<typed-array:int64 1 2 3>` (not readable) |
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
    write_val(&mut out, val);
    out
}

// Appends `val` to `out`.  Everything is written into the one buffer, so
// printing is linear in the output size; a list's spine is walked in a loop
// and only `car`s recurse, so a long list costs no Rust stack (issue #509).
// Writing to a `String` cannot fail, so `write!` results are discarded.
fn write_val(out: &mut String, val: &LispVal) {
    match val {
        LispVal::Symbol(s) => {
            // Always print just the symbol name, regardless of plist
            out.push_str(&s.borrow().name)
        }
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
        // Contents, not just the length (issue #527). See `print_array`.
        LispVal::Array(a) => out.push_str(&print_array(a)),
        LispVal::TypedArray(a) => out.push_str(&print_typed_array(a)),
        // Readable record syntax (issue #308 stage D): field values in
        // declaration order, each printed readably, so the output round-trips
        // through the reader's #S literal (spawn/channel serialization).
        LispVal::Struct(s) => {
            let _ = write!(out, "#S({}", s.type_name);
            for f in &s.fields {
                out.push(' ');
                write_val(out, f);
            }
            out.push(')');
        }
        LispVal::Extension(e) => out.push_str(&e.display()),
        LispVal::Error(e) => {
            let _ = write!(out, "#<error {:?}", e.message);
            if e.data != LispVal::Nil {
                out.push(' ');
                write_val(out, &e.data);
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
            write_val(out, car);
            let mut rest: &LispVal = cdr;
            loop {
                match rest {
                    LispVal::Cons { car, cdr } => {
                        out.push(' ');
                        write_val(out, car);
                        rest = cdr;
                    }
                    LispVal::Nil => break,
                    tail => {
                        out.push_str(" . ");
                        write_val(out, tail);
                        break;
                    }
                }
            }
            out.push(')');
        }
    }
}

/// Elements shown before an array is abridged (issue #527). Beyond it the
/// printer emits a `#<...N more>` marker, which the reader rejects, so an
/// abridged array can never read back as a silently shorter one.
pub const ARRAY_PRINT_LIMIT: usize = 100;

thread_local! {
    /// `None` while inside [`print_unabridged`]; otherwise the element limit.
    static ARRAY_LIMIT: std::cell::Cell<Option<usize>> =
        const { std::cell::Cell::new(Some(ARRAY_PRINT_LIMIT)) };
    /// Arrays currently being printed on this thread, by address: an array
    /// that (transitively) contains itself prints `#<circular-array>` at the
    /// back-reference instead of recursing forever.
    static ARRAYS_IN_PROGRESS: std::cell::RefCell<Vec<usize>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

/// Like [`print()`], but arrays are never abridged, so every array whose
/// elements are readable round-trips through the reader. Used where the
/// printed text is a serialization (channel and spawn payloads) rather than
/// a display.
pub fn print_unabridged(val: &LispVal) -> String {
    struct Restore(Option<usize>);
    impl Drop for Restore {
        fn drop(&mut self) {
            ARRAY_LIMIT.with(|l| l.set(self.0));
        }
    }
    let _restore = Restore(ARRAY_LIMIT.with(|l| l.replace(None)));
    print(val)
}

/// Print `len` elements as `open e0 e1 ... close`, abridged per
/// [`ARRAY_LIMIT`], guarding against an array that contains itself.
fn print_elements(
    addr: usize,
    open: &str,
    close: char,
    len: usize,
    elem: impl Fn(usize) -> Option<LispVal>,
) -> String {
    struct Pop;
    impl Drop for Pop {
        fn drop(&mut self) {
            ARRAYS_IN_PROGRESS.with(|s| s.borrow_mut().pop());
        }
    }
    if ARRAYS_IN_PROGRESS.with(|s| s.borrow().contains(&addr)) {
        return "#<circular-array>".to_string();
    }
    ARRAYS_IN_PROGRESS.with(|s| s.borrow_mut().push(addr));
    let _pop = Pop;
    let shown = ARRAY_LIMIT.with(|l| l.get()).map_or(len, |n| n.min(len));
    let mut out = open.to_string();
    for i in 0..shown {
        // Fetch each element afresh so no borrow is held across the
        // recursive `print` (an element may be this very array).
        let Some(v) = elem(i) else { break };
        if i > 0 {
            out.push(' ');
        }
        out.push_str(&print(&v));
    }
    if shown < len {
        if shown > 0 {
            out.push(' ');
        }
        out.push_str(&format!("#<...{} more>", len - shown));
    }
    out.push(close);
    out
}

fn print_array(a: &crate::Shared<crate::SharedCell<Vec<LispVal>>>) -> String {
    let len = a.borrow().len();
    print_elements(
        crate::Shared::as_ptr(a) as *const () as usize,
        "#(",
        ')',
        len,
        |i| a.borrow().get(i).cloned(),
    )
}

fn print_typed_array(a: &crate::Shared<crate::TypedArrayObj>) -> String {
    let open = if a.is_empty() {
        format!("#<typed-array:{}", a.elem)
    } else {
        format!("#<typed-array:{} ", a.elem)
    };
    print_elements(
        crate::Shared::as_ptr(a) as *const () as usize,
        &open,
        '>',
        a.len(),
        |i| a.get(i),
    )
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
