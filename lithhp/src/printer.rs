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
        LispVal::Symbol(s) => s.clone(),
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

    fn cons(car: LispVal, cdr: LispVal) -> LispVal {
        LispVal::Cons {
            car: Box::new(car),
            cdr: Box::new(cdr),
        }
    }

    fn symbol(s: &str) -> LispVal {
        LispVal::Symbol(s.to_string())
    }

    fn number(n: i64) -> LispVal {
        LispVal::Number(n)
    }

    #[test]
    fn test_print_nested_list() {
        let list = cons(
            symbol("+"),
            cons(
                number(10),
                cons(
                    cons(symbol("*"), cons(number(5), cons(number(2), LispVal::Nil))),
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
        let list = cons(symbol("a"), symbol("b"));
        assert_eq!(print(&list), "(a . b)");
    }

    #[test]
    fn test_print_complex_dotted_list() {
        let list = cons(symbol("a"), cons(symbol("b"), symbol("c")));
        assert_eq!(print(&list), "(a b . c)");
    }

    #[test]
    fn test_print_nil() {
        assert_eq!(print(&LispVal::Nil), "()");
    }
}
