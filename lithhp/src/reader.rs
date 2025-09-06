use nom::{
    branch::alt,
    bytes::complete::{is_not, tag},
    character::complete::{alpha1, alphanumeric1, char, digit1, multispace1},
    combinator::{map, map_res, opt, recognize},
    multi::many0,
    sequence::{delimited, pair, preceded, terminated},
    IResult,
};

use crate::LispVal;

// A parser for a comment
fn parse_comment(input: &str) -> IResult<&str, &str> {
    recognize(pair(tag(";"), is_not("\n\r")))(input)
}

// A parser for whitespace, including comments
fn ws(input: &str) -> IResult<&str, &str> {
    recognize(many0(alt((multispace1, parse_comment))))(input)
}


fn parse_symbol(input: &str) -> IResult<&str, LispVal> {
    map(
        recognize(pair(
            alt((alpha1, tag("+"), tag("-"), tag("*"), tag("/"), tag("!"), tag("="))),
            many0(alt((alphanumeric1, tag("+"), tag("-"), tag("*"), tag("/"), tag("!"), tag("=")))),
        )),
        |s: &str| LispVal::Symbol(s.to_string()),
    )(input)
}

fn parse_number(input: &str) -> IResult<&str, LispVal> {
    map(
        map_res(recognize(pair(opt(tag("-")), digit1)), |s: &str| {
            s.parse::<i64>()
        }),
        LispVal::Number,
    )(input)
}

fn parse_atom(input: &str) -> IResult<&str, LispVal> {
    alt((parse_number, parse_symbol))(input)
}

fn parse_string(input: &str) -> IResult<&str, LispVal> {
    map(
        delimited(char('"'), is_not("\""), char('"')),
        |s: &str| LispVal::String(s.to_string()),
    )(input)
}

fn parse_list_contents(input: &str) -> IResult<&str, LispVal> {
    let (input, exprs) = many0(preceded(ws, parse_expr))(input)?;
    let (input, tail) = opt(preceded(preceded(ws, char('.')), preceded(ws, parse_expr)))(input)?;

    let end = tail.unwrap_or(LispVal::Nil);
    Ok((input, exprs.into_iter().rev().fold(end, |cdr, car| {
        LispVal::Cons { car: Box::new(car), cdr: Box::new(cdr) }
    })))
}

fn parse_list(input: &str) -> IResult<&str, LispVal> {
    delimited(
        char('('),
        parse_list_contents,
        preceded(ws, char(')'))
    )(input)
}

fn parse_quoted(input: &str) -> IResult<&str, LispVal> {
    map(
        preceded(char('\''), parse_expr),
        |expr| LispVal::Cons {
            car: Box::new(LispVal::Symbol("quote".to_string())),
            cdr: Box::new(LispVal::Cons {
                car: Box::new(expr),
                cdr: Box::new(LispVal::Nil),
            }),
        }
    )(input)
}

fn parse_quasiquoted(input: &str) -> IResult<&str, LispVal> {
    map(
        preceded(char('`'), parse_expr),
        |expr| LispVal::Cons {
            car: Box::new(LispVal::Symbol("quasiquote".to_string())),
            cdr: Box::new(LispVal::Cons {
                car: Box::new(expr),
                cdr: Box::new(LispVal::Nil),
            }),
        }
    )(input)
}

fn parse_unquoted(input: &str) -> IResult<&str, LispVal> {
    map(
        preceded(char(','), parse_expr),
        |expr| LispVal::Cons {
            car: Box::new(LispVal::Symbol("unquote".to_string())),
            cdr: Box::new(LispVal::Cons {
                car: Box::new(expr),
                cdr: Box::new(LispVal::Nil),
            }),
        }
    )(input)
}

fn parse_expr(input: &str) -> IResult<&str, LispVal> {
    preceded(ws, alt((parse_atom, parse_string, parse_list, parse_quoted, parse_quasiquoted, parse_unquoted)))(input)
}

pub fn read(input: &str) -> Result<LispVal, String> {
    match terminated(parse_expr, ws)(input.trim()) {
        Ok(("", val)) => Ok(val),
        Ok((rem, _)) => Err(format!("Unexpected input: {rem}")),
        Err(e) => Err(e.to_string()),
    }
}

pub fn read_all(input: &str) -> Result<Vec<LispVal>, String> {
    let mut results = vec![];
    let mut current_input = input.trim();
    while !current_input.is_empty() {
        match terminated(parse_expr, ws)(current_input) {
            Ok((rem, val)) => {
                results.push(val);
                current_input = rem;
            }
            Err(e) => return Err(e.to_string()),
        }
    }
    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cons(car: LispVal, cdr: LispVal) -> LispVal {
        LispVal::Cons { car: Box::new(car), cdr: Box::new(cdr) }
    }

    fn symbol(s: &str) -> LispVal {
        LispVal::Symbol(s.to_string())
    }

    fn number(n: i64) -> LispVal {
        LispVal::Number(n)
    }

    #[test]
    fn test_parse_number() {
        assert_eq!(parse_number("123"), Ok(("", number(123))));
        assert_eq!(parse_number("-456"), Ok(("", number(-456))));
    }

    #[test]
    fn test_parse_symbol() {
        assert_eq!(parse_symbol("abc"), Ok(("", symbol("abc"))));
        assert_eq!(parse_symbol("+"), Ok(("", symbol("+"))));
    }

    #[test]
    fn test_parse_string() {
        assert_eq!(
            parse_string("\"hello world\""),
            Ok(("", LispVal::String("hello world".to_string())))
        );
    }

    #[test]
    fn test_parse_list() {
        assert_eq!(
            parse_list("(+ 1 2)"),
            Ok((
                "",
                cons(symbol("+"), cons(number(1), cons(number(2), LispVal::Nil)))
            ))
        );
    }

    #[test]
    fn test_read_simple_list() {
        let result = read("(+ 10 20)");
        assert_eq!(
            result,
            Ok(cons(symbol("+"), cons(number(10), cons(number(20), LispVal::Nil))))
        );
    }

    #[test]
    fn test_read_nested_list() {
        let result = read("(+ 10 (* 5 2))");
        assert_eq!(
            result,
            Ok(cons(
                symbol("+"),
                cons(
                    number(10),
                    cons(
                        cons(symbol("*"), cons(number(5), cons(number(2), LispVal::Nil))),
                        LispVal::Nil
                    )
                )
            ))
        );
    }

    #[test]
    fn test_read_dotted_list() {
        let result = read("(a . b)");
        assert_eq!(result, Ok(cons(symbol("a"), symbol("b"))));
    }

    #[test]
    fn test_read_complex_dotted_list() {
        let result = read("(a b . c)");
        assert_eq!(result, Ok(cons(symbol("a"), cons(symbol("b"), symbol("c")))));
    }

    #[test]
    fn test_comment() {
        let result = read("
            ; this is a comment
            (+ 1 2) ; another comment
        ");
        assert_eq!(
            result,
            Ok(cons(symbol("+"), cons(number(1), cons(number(2), LispVal::Nil))))
        );
    }

    #[test]
    fn test_read_quoted() {
        let result = read("'a");
        assert_eq!(
            result,
            Ok(cons(symbol("quote"), cons(symbol("a"), LispVal::Nil)))
        );
    }

    #[test]
    fn test_read_quasiquote() {
        let result = read("`(a ,b)");
        assert_eq!(
            result,
            Ok(cons(
                symbol("quasiquote"),
                cons(
                    cons(
                        symbol("a"),
                        cons(
                            cons(symbol("unquote"), cons(symbol("b"), LispVal::Nil)),
                            LispVal::Nil
                        )
                    ),
                    LispVal::Nil
                )
            ))
        );
    }
}
