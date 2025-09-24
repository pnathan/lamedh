use crate::LispVal;
use crate::environment::Environment;
use nom::{
    IResult,
    branch::alt,
    bytes::complete::{is_not, tag},
    character::complete::{alpha1, alphanumeric1, char, digit1, multispace1, one_of},
    combinator::{map, map_res, opt, recognize},
    multi::many0,
    sequence::{delimited, pair, preceded, terminated, tuple},
};
use std::cell::RefCell;
use std::rc::Rc;

type ParseResult<'a> = IResult<&'a str, LispVal>;

fn parse_expr(env: Rc<RefCell<Environment>>) -> impl Fn(&str) -> ParseResult {
    move |input: &str| {
        preceded(
            ws,
            alt((
                parse_atom(env.clone()),
                parse_string,
                parse_list(env.clone()),
                parse_quoted(env.clone()),
                parse_quasiquoted(env.clone()),
                parse_unquoted(env.clone()),
            )),
        )(input)
    }
}

// A parser for a comment
fn parse_comment(input: &str) -> IResult<&str, &str> {
    recognize(pair(tag(";"), is_not("\n\r")))(input)
}

// A parser for whitespace, including comments
fn ws(input: &str) -> IResult<&str, &str> {
    recognize(many0(alt((multispace1, parse_comment))))(input)
}

fn parse_float(input: &str) -> ParseResult {
    map(
        map_res(
            recognize(tuple((
                opt(tag("-")),
                digit1,
                tag("."),
                digit1,
                opt(tuple((one_of("Ee"), opt(one_of("+-")), digit1))),
            ))),
            |s: &str| s.parse::<f64>(),
        ),
        LispVal::Float,
    )(input)
}

fn parse_number(input: &str) -> ParseResult {
    alt((
        parse_float,
        map(
            map_res(recognize(pair(opt(tag("-")), digit1)), |s: &str| {
                s.parse::<i64>()
            }),
            LispVal::Number,
        ),
    ))(input)
}

fn parse_atom(env: Rc<RefCell<Environment>>) -> impl Fn(&str) -> ParseResult {
    move |input: &str| {
        alt((
            parse_number,
            map(
                recognize(pair(
                    alt((alpha1, tag("&"))),
                    many0(alt((alphanumeric1, tag("-")))),
                )),
                |s: &str| {
                    let s_upper = s.to_uppercase();
                    match s_upper.as_str() {
                        "T" => LispVal::Symbol(env.borrow_mut().intern_symbol("T")),
                        "NIL" => LispVal::Nil,
                        _ => LispVal::Symbol(env.borrow_mut().intern_symbol(&s_upper)),
                    }
                },
            ),
        ))(input)
    }
}

fn parse_string(input: &str) -> ParseResult {
    map(delimited(char('"'), is_not("\""), char('"')), |s: &str| {
        LispVal::String(s.to_string())
    })(input)
}

fn parse_list_contents(env: Rc<RefCell<Environment>>) -> impl Fn(&str) -> ParseResult {
    move |input: &str| {
        let (input, exprs) = many0(preceded(ws, parse_expr(env.clone())))(input)?;
        let (input, tail) = opt(preceded(
            preceded(ws, char('.')),
            preceded(ws, parse_expr(env.clone())),
        ))(input)?;

        let end = tail.unwrap_or(LispVal::Nil);
        Ok((
            input,
            exprs.into_iter().rev().fold(end, |cdr, car| LispVal::Cons {
                car: Box::new(car),
                cdr: Box::new(cdr),
            }),
        ))
    }
}

fn parse_list(env: Rc<RefCell<Environment>>) -> impl Fn(&str) -> ParseResult {
    move |input: &str| {
        delimited(
            char('('),
            parse_list_contents(env.clone()),
            preceded(ws, char(')')),
        )(input)
    }
}

fn parse_quoted(env: Rc<RefCell<Environment>>) -> impl Fn(&str) -> ParseResult {
    let quote_symbol = LispVal::Symbol(env.borrow_mut().intern_symbol("QUOTE"));
    move |input: &str| {
        map(preceded(char('\''), parse_expr(env.clone())), |expr| {
            LispVal::Cons {
                car: Box::new(quote_symbol.clone()),
                cdr: Box::new(LispVal::Cons {
                    car: Box::new(expr),
                    cdr: Box::new(LispVal::Nil),
                }),
            }
        })(input)
    }
}

fn parse_quasiquoted(env: Rc<RefCell<Environment>>) -> impl Fn(&str) -> ParseResult {
    let quasiquote_symbol = LispVal::Symbol(env.borrow_mut().intern_symbol("QUASIQUOTE"));
    move |input: &str| {
        map(preceded(char('`'), parse_expr(env.clone())), |expr| {
            LispVal::Cons {
                car: Box::new(quasiquote_symbol.clone()),
                cdr: Box::new(LispVal::Cons {
                    car: Box::new(expr),
                    cdr: Box::new(LispVal::Nil),
                }),
            }
        })(input)
    }
}

fn parse_unquoted(env: Rc<RefCell<Environment>>) -> impl Fn(&str) -> ParseResult {
    let unquote_symbol = LispVal::Symbol(env.borrow_mut().intern_symbol("UNQUOTE"));
    move |input: &str| {
        map(preceded(char(','), parse_expr(env.clone())), |expr| {
            LispVal::Cons {
                car: Box::new(unquote_symbol.clone()),
                cdr: Box::new(LispVal::Cons {
                    car: Box::new(expr),
                    cdr: Box::new(LispVal::Nil),
                }),
            }
        })(input)
    }
}

pub fn read(input: &str, env: &mut Environment) -> Result<LispVal, String> {
    let env_rc = Rc::new(RefCell::new(env.clone()));
    match terminated(parse_expr(env_rc), ws)(input.trim()) {
        Ok(("", val)) => Ok(val),
        Ok((rem, _)) => Err(format!("Unexpected input: {rem}")),
        Err(e) => Err(e.to_string()),
    }
}

pub fn read_all(input: &str, env: &mut Environment) -> Result<Vec<LispVal>, String> {
    let env_rc = Rc::new(RefCell::new(env.clone()));
    let mut results = vec![];
    let mut current_input = input.trim();
    while !current_input.is_empty() {
        match terminated(parse_expr(env_rc.clone()), ws)(current_input) {
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

    fn float(f: f64) -> LispVal {
        LispVal::Float(f)
    }

    #[test]
    fn test_parse_number() {
        assert_eq!(parse_number("123"), Ok(("", number(123))));
        assert_eq!(parse_number("-456"), Ok(("", number(-456))));
    }

    #[test]
    fn test_parse_float() {
        assert_eq!(parse_float("3.14"), Ok(("", float(3.14))));
        assert_eq!(parse_float("-0.5"), Ok(("", float(-0.5))));
    }

    #[test]
    fn test_parse_atom() {
        let mut env = Environment::new();
        let env_rc = Rc::new(RefCell::new(env.clone()));
        assert_eq!(
            parse_atom(env_rc.clone())("abc"),
            Ok(("", symbol("ABC", &mut env)))
        );
        assert_eq!(
            parse_atom(env_rc.clone())("with-hyphen"),
            Ok(("", symbol("WITH-HYPHEN", &mut env)))
        );
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
        let mut env = Environment::new();
        let env_rc = Rc::new(RefCell::new(env.clone()));
        assert_eq!(
            parse_list(env_rc)("(PLUS 1 2)"),
            Ok((
                "",
                cons(
                    symbol("PLUS", &mut env),
                    cons(number(1), cons(number(2), LispVal::Nil))
                )
            ))
        );
    }

    #[test]
    fn test_read_simple_list() {
        let mut env = Environment::new();
        let result = read("(PLUS 10 20)", &mut env);
        assert_eq!(
            result,
            Ok(cons(
                symbol("PLUS", &mut env),
                cons(number(10), cons(number(20), LispVal::Nil))
            ))
        );
    }

    #[test]
    fn test_read_nested_list() {
        let mut env = Environment::new();
        let result = read("(PLUS 10 (TIMES 5 2))", &mut env);
        assert_eq!(
            result,
            Ok(cons(
                symbol("PLUS", &mut env),
                cons(
                    number(10),
                    cons(
                        cons(
                            symbol("TIMES", &mut env),
                            cons(number(5), cons(number(2), LispVal::Nil))
                        ),
                        LispVal::Nil
                    )
                )
            ))
        );
    }

    #[test]
    fn test_read_dotted_list() {
        let mut env = Environment::new();
        let result = read("(a . b)", &mut env);
        assert_eq!(
            result,
            Ok(cons(symbol("A", &mut env), symbol("B", &mut env)))
        );
    }

    #[test]
    fn test_read_complex_dotted_list() {
        let mut env = Environment::new();
        let result = read("(a b . c)", &mut env);
        assert_eq!(
            result,
            Ok(cons(
                symbol("A", &mut env),
                cons(symbol("B", &mut env), symbol("C", &mut env))
            ))
        );
    }

    #[test]
    fn test_comment() {
        let mut env = Environment::new();
        let result = read(
            "
            ; this is a comment
            (PLUS 1 2) ; another comment
        ",
            &mut env,
        );
        assert_eq!(
            result,
            Ok(cons(
                symbol("PLUS", &mut env),
                cons(number(1), cons(number(2), LispVal::Nil))
            ))
        );
    }

    #[test]
    fn test_read_quoted() {
        let mut env = Environment::new();
        let result = read("'a", &mut env);
        assert_eq!(
            result,
            Ok(cons(
                symbol("QUOTE", &mut env),
                cons(symbol("A", &mut env), LispVal::Nil)
            ))
        );
    }

    #[test]
    fn test_read_quasiquote() {
        let mut env = Environment::new();
        let result = read("`(a ,b)", &mut env);
        assert_eq!(
            result,
            Ok(cons(
                symbol("QUASIQUOTE", &mut env),
                cons(
                    cons(
                        symbol("A", &mut env),
                        cons(
                            cons(
                                symbol("UNQUOTE", &mut env),
                                cons(symbol("B", &mut env), LispVal::Nil)
                            ),
                            LispVal::Nil
                        )
                    ),
                    LispVal::Nil
                )
            ))
        );
    }

    #[test]
    fn test_read_nil() {
        let mut env = Environment::new();
        assert_eq!(read("NIL", &mut env), Ok(LispVal::Nil));
    }

    #[test]
    fn test_read_t() {
        let mut env = Environment::new();
        assert_eq!(read("T", &mut env), Ok(symbol("T", &mut env)));
    }
}
