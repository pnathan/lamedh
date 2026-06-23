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
use std::rc::Rc;

type ParseResult<'a> = IResult<&'a str, LispVal>;

fn parse_expr(env: Rc<Environment>) -> impl Fn(&str) -> ParseResult {
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
                parse_function_shorthand(env.clone()),
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

fn parse_float(input: &str) -> ParseResult<'_> {
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

fn parse_integer_or_overflow_float(input: &str) -> ParseResult<'_> {
    let (rest, s) = recognize(pair(opt(tag("-")), digit1))(input)?;
    if let Ok(n) = s.parse::<i64>() {
        Ok((rest, LispVal::Number(n)))
    } else if let Ok(f) = s.parse::<f64>() {
        Ok((rest, LispVal::Float(f)))
    } else {
        Err(nom::Err::Error(nom::error::Error::new(
            input,
            nom::error::ErrorKind::Digit,
        )))
    }
}

fn parse_number(input: &str) -> ParseResult<'_> {
    alt((parse_float, parse_integer_or_overflow_float))(input)
}

fn parse_one_plus_minus(env: Rc<Environment>) -> impl Fn(&str) -> ParseResult {
    move |input: &str| {
        let (rest, sym) = alt((tag("1+"), tag("1-")))(input)?;
        Ok((rest, LispVal::Symbol(env.intern_symbol(sym))))
    }
}

fn parse_atom(env: Rc<Environment>) -> impl Fn(&str) -> ParseResult {
    move |input: &str| {
        alt((
            // Parse special numeric symbols like 1+ and 1- BEFORE numbers
            parse_one_plus_minus(env.clone()),
            parse_number,
            // Parse earmuff symbols (*name*) - dynamic variable naming convention
            // Must come before regular symbols and operators
            parse_earmuff_symbol(env.clone()),
            map(
                recognize(pair(
                    alt((alpha1, tag("&"))),
                    many0(alt((alphanumeric1, tag("-")))),
                )),
                |s: &str| {
                    let s_upper = s.to_uppercase();
                    match s_upper.as_str() {
                        "T" => LispVal::Symbol(env.intern_symbol("T")),
                        "NIL" => LispVal::Nil,
                        _ => LispVal::Symbol(env.intern_symbol(&s_upper)),
                    }
                },
            ),
            // Parse operator symbols (+, -, *, /, =, etc.) - after attempting number/alpha parse
            map(
                alt((
                    tag("+"),
                    tag("-"),
                    tag("*"),
                    tag("/"),
                    tag("="),
                    tag("<"),
                    tag(">"),
                )),
                |s: &str| LispVal::Symbol(env.intern_symbol(s)),
            ),
        ))(input)
    }
}

/// Parse earmuff symbols: *name* (dynamic variable naming convention)
/// Examples: *debug*, *print-level*, *foo123*
fn parse_earmuff_symbol(env: Rc<Environment>) -> impl Fn(&str) -> ParseResult {
    move |input: &str| {
        map(
            recognize(tuple((
                tag("*"),
                alpha1,
                many0(alt((alphanumeric1, tag("-")))),
                tag("*"),
            ))),
            |s: &str| LispVal::Symbol(env.intern_symbol(&s.to_uppercase())),
        )(input)
    }
}

fn parse_string(input: &str) -> ParseResult<'_> {
    map(delimited(char('"'), is_not("\""), char('"')), |s: &str| {
        LispVal::String(s.to_string())
    })(input)
}

fn parse_list_contents(env: Rc<Environment>) -> impl Fn(&str) -> ParseResult {
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

fn parse_list(env: Rc<Environment>) -> impl Fn(&str) -> ParseResult {
    move |input: &str| {
        delimited(
            char('('),
            parse_list_contents(env.clone()),
            preceded(ws, char(')')),
        )(input)
    }
}

fn parse_quoted(env: Rc<Environment>) -> impl Fn(&str) -> ParseResult {
    let quote_symbol = LispVal::Symbol(env.intern_symbol("QUOTE"));
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

fn parse_quasiquoted(env: Rc<Environment>) -> impl Fn(&str) -> ParseResult {
    let quasiquote_symbol = LispVal::Symbol(env.intern_symbol("QUASIQUOTE"));
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

fn parse_function_shorthand(env: Rc<Environment>) -> impl Fn(&str) -> ParseResult {
    let function_symbol = LispVal::Symbol(env.intern_symbol("FUNCTION"));
    move |input: &str| {
        map(preceded(tag("#'"), parse_expr(env.clone())), |expr| {
            LispVal::Cons {
                car: Box::new(function_symbol.clone()),
                cdr: Box::new(LispVal::Cons {
                    car: Box::new(expr),
                    cdr: Box::new(LispVal::Nil),
                }),
            }
        })(input)
    }
}

fn parse_unquoted(env: Rc<Environment>) -> impl Fn(&str) -> ParseResult {
    let unquote_symbol = LispVal::Symbol(env.intern_symbol("UNQUOTE"));
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

pub fn read(input: &str, env: &Rc<Environment>) -> Result<LispVal, String> {
    match terminated(parse_expr(env.clone()), ws)(input.trim()) {
        Ok(("", val)) => Ok(val),
        Ok((rem, _)) => Err(format!("Unexpected input: {rem}")),
        Err(e) => Err(e.to_string()),
    }
}

pub fn read_all(input: &str, env: &Rc<Environment>) -> Result<Vec<LispVal>, String> {
    let mut results = vec![];
    let mut current_input = input.trim();
    while !current_input.is_empty() {
        match terminated(parse_expr(env.clone()), ws)(current_input) {
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

    fn symbol(s: &str, env: &Rc<Environment>) -> LispVal {
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
        let env = Rc::new(Environment::new());
        assert_eq!(
            parse_atom(env.clone())("abc"),
            Ok(("", symbol("ABC", &env)))
        );
        assert_eq!(
            parse_atom(env.clone())("with-hyphen"),
            Ok(("", symbol("WITH-HYPHEN", &env)))
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
        let env = Rc::new(Environment::new());
        assert_eq!(
            parse_list(env.clone())("(PLUS 1 2)"),
            Ok((
                "",
                cons(
                    symbol("PLUS", &env),
                    cons(number(1), cons(number(2), LispVal::Nil))
                )
            ))
        );
    }

    #[test]
    fn test_read_simple_list() {
        let env = Rc::new(Environment::new());
        let result = read("(PLUS 10 20)", &env);
        assert_eq!(
            result,
            Ok(cons(
                symbol("PLUS", &env),
                cons(number(10), cons(number(20), LispVal::Nil))
            ))
        );
    }

    #[test]
    fn test_read_nested_list() {
        let env = Rc::new(Environment::new());
        let result = read("(PLUS 10 (TIMES 5 2))", &env);
        assert_eq!(
            result,
            Ok(cons(
                symbol("PLUS", &env),
                cons(
                    number(10),
                    cons(
                        cons(
                            symbol("TIMES", &env),
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
        let env = Rc::new(Environment::new());
        let result = read("(a . b)", &env);
        assert_eq!(result, Ok(cons(symbol("A", &env), symbol("B", &env))));
    }

    #[test]
    fn test_read_complex_dotted_list() {
        let env = Rc::new(Environment::new());
        let result = read("(a b . c)", &env);
        assert_eq!(
            result,
            Ok(cons(
                symbol("A", &env),
                cons(symbol("B", &env), symbol("C", &env))
            ))
        );
    }

    #[test]
    fn test_comment() {
        let env = Rc::new(Environment::new());
        let result = read(
            "
            ; this is a comment
            (PLUS 1 2) ; another comment
        ",
            &env,
        );
        assert_eq!(
            result,
            Ok(cons(
                symbol("PLUS", &env),
                cons(number(1), cons(number(2), LispVal::Nil))
            ))
        );
    }

    #[test]
    fn test_read_quoted() {
        let env = Rc::new(Environment::new());
        let result = read("'a", &env);
        assert_eq!(
            result,
            Ok(cons(
                symbol("QUOTE", &env),
                cons(symbol("A", &env), LispVal::Nil)
            ))
        );
    }

    #[test]
    fn test_read_quasiquote() {
        let env = Rc::new(Environment::new());
        let result = read("`(a ,b)", &env);
        assert_eq!(
            result,
            Ok(cons(
                symbol("QUASIQUOTE", &env),
                cons(
                    cons(
                        symbol("A", &env),
                        cons(
                            cons(
                                symbol("UNQUOTE", &env),
                                cons(symbol("B", &env), LispVal::Nil)
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
        let env = Rc::new(Environment::new());
        assert_eq!(read("NIL", &env), Ok(LispVal::Nil));
    }

    #[test]
    fn test_read_t() {
        let env = Rc::new(Environment::new());
        assert_eq!(read("T", &env), Ok(symbol("T", &env)));
    }
}
