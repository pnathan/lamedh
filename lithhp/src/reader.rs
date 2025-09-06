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

fn parse_list(input: &str) -> IResult<&str, LispVal> {
    map(
        delimited(
            char('('),
            many0(preceded(ws, parse_expr)),
            preceded(ws, char(')')),
        ),
        LispVal::List,
    )(input)
}

fn parse_quoted(input: &str) -> IResult<&str, LispVal> {
    map(
        preceded(char('\''), parse_expr),
        |expr| LispVal::List(vec![LispVal::Symbol("quote".to_string()), expr])
    )(input)
}

fn parse_quasiquoted(input: &str) -> IResult<&str, LispVal> {
    map(
        preceded(char('`'), parse_expr),
        |expr| LispVal::List(vec![LispVal::Symbol("quasiquote".to_string()), expr])
    )(input)
}

fn parse_unquoted(input: &str) -> IResult<&str, LispVal> {
    map(
        preceded(char(','), parse_expr),
        |expr| LispVal::List(vec![LispVal::Symbol("unquote".to_string()), expr])
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

    #[test]
    fn test_parse_number() {
        assert_eq!(parse_number("123"), Ok(("", LispVal::Number(123))));
        assert_eq!(parse_number("-456"), Ok(("", LispVal::Number(-456))));
    }

    #[test]
    fn test_parse_symbol() {
        assert_eq!(
            parse_symbol("abc"),
            Ok(("", LispVal::Symbol("abc".to_string())))
        );
        assert_eq!(
            parse_symbol("+"),
            Ok(("", LispVal::Symbol("+".to_string())))
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
        assert_eq!(
            parse_list("(+ 1 2)"),
            Ok((
                "",
                LispVal::List(vec![
                    LispVal::Symbol("+".to_string()),
                    LispVal::Number(1),
                    LispVal::Number(2)
                ])
            ))
        );
    }

    #[test]
    fn test_read_simple_list() {
        let result = read("(+ 10 20)");
        assert_eq!(
            result,
            Ok(LispVal::List(vec![
                LispVal::Symbol("+".to_string()),
                LispVal::Number(10),
                LispVal::Number(20)
            ]))
        );
    }

    #[test]
    fn test_read_nested_list() {
        let result = read("(+ 10 (* 5 2))");
        assert_eq!(
            result,
            Ok(LispVal::List(vec![
                LispVal::Symbol("+".to_string()),
                LispVal::Number(10),
                LispVal::List(vec![
                    LispVal::Symbol("*".to_string()),
                    LispVal::Number(5),
                    LispVal::Number(2)
                ])
            ]))
        );
    }

    #[test]
    fn test_comment() {
        let result = read("
            ; this is a comment
            (+ 1 2) ; another comment
        ");
        assert_eq!(
            result,
            Ok(LispVal::List(vec![
                LispVal::Symbol("+".to_string()),
                LispVal::Number(1),
                LispVal::Number(2)
            ]))
        );
    }

    #[test]
    fn test_read_quoted() {
        let result = read("'a");
        assert_eq!(
            result,
            Ok(LispVal::List(vec![
                LispVal::Symbol("quote".to_string()),
                LispVal::Symbol("a".to_string())
            ]))
        );
    }

    #[test]
    fn test_read_quasiquote() {
        let result = read("`(a ,b)");
        assert_eq!(
            result,
            Ok(LispVal::List(vec![
                LispVal::Symbol("quasiquote".to_string()),
                LispVal::List(vec![
                    LispVal::Symbol("a".to_string()),
                    LispVal::List(vec![
                        LispVal::Symbol("unquote".to_string()),
                        LispVal::Symbol("b".to_string())
                    ])
                ])
            ]))
        );
    }
}
