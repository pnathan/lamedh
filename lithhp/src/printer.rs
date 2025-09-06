use crate::LispVal;

pub fn print(val: &LispVal) -> String {
    match val {
        LispVal::Symbol(s) => s.clone(),
        LispVal::Number(n) => n.to_string(),
        LispVal::String(s) => format!("\"{s}\""),
        LispVal::Builtin(_) => "<builtin>".to_string(),
        LispVal::Lambda(_) => "<lambda>".to_string(),
        LispVal::Fexpr(_) => "<fexpr>".to_string(),
        LispVal::List(list) => {
            if list.is_empty() {
                "()".to_string()
            } else {
                let inner: Vec<String> = list.iter().map(print).collect();
                format!("({})", inner.join(" "))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_print_nested_list() {
        let list = LispVal::List(vec![
            LispVal::Symbol("+".to_string()),
            LispVal::Number(10),
            LispVal::List(vec![
                LispVal::Symbol("*".to_string()),
                LispVal::Number(5),
                LispVal::Number(2),
            ]),
        ]);
        assert_eq!(print(&list), "(+ 10 (* 5 2))");
    }

    #[test]
    fn test_print_string() {
        let s = LispVal::String("hello world".to_string());
        assert_eq!(print(&s), "\"hello world\"");
    }
}
