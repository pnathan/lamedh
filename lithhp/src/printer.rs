use crate::LispVal;

fn print_list_contents(cdr: &LispVal) -> String {
    match cdr {
        LispVal::Cons { car, cdr } => format!(" {}", print(car)) + &print_list_contents(cdr),
        LispVal::Nil => "".to_string(),
        _ => format!(" . {}", print(cdr)),
    }
}

pub fn print(val: &LispVal) -> String {
    match val {
        LispVal::Symbol(s) => {
            let symbol = s.borrow();
            if symbol.plist.is_empty() {
                symbol.name.clone()
            } else {
                let mut plist_str = "(".to_string();
                for (key, val) in &symbol.plist {
                    plist_str.push_str(&format!("{} {}", key, print(val)));
                }
                plist_str.push(')');
                format!("#<symbol-name: {} plist: {}>", symbol.name, plist_str)
            }
        }
        LispVal::Number(n) => n.to_string(),
        LispVal::String(s) => format!("\"{s}\""),
        LispVal::Builtin(_) => "<builtin>".to_string(),
        LispVal::Lambda(_) => "<lambda>".to_string(),
        LispVal::Fexpr(_) => "<fexpr>".to_string(),
        LispVal::Macro(_) => "<macro>".to_string(),
        LispVal::HashTable(_) => "<hash-table>".to_string(),
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

    fn cons(car: LispVal, cdr: LispVal) -> LispVal {
        LispVal::Cons {
            car: Box::new(car),
            cdr: Box::new(cdr),
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
        let mut env = Environment::new();
        let s = env.intern_symbol("a");
        s.borrow_mut()
            .plist
            .insert("key".to_string(), LispVal::String("value".to_string()));
        let lisp_val = LispVal::Symbol(s);
        assert_eq!(print(&lisp_val), "#<symbol-name: a plist: (key \"value\")>");
    }
}
