use lithhp::repl_loop;
use std::io::{Cursor, BufReader, BufWriter};

#[test]
fn test_add_two_numbers() {
    let input = "(+ 1 2)\n";
    let mut reader = BufReader::new(Cursor::new(input));
    let mut writer = BufWriter::new(Vec::new());
    repl_loop(&mut reader, &mut writer).unwrap();

    let output = String::from_utf8(writer.into_inner().unwrap()).unwrap();
    assert_eq!(output, "3\n");
}

#[test]
fn test_define_and_call_function() {
    let input = "(defun square (x) (* x x))\n(square 5)\n";
    let mut reader = BufReader::new(Cursor::new(input));
    let mut writer = BufWriter::new(Vec::new());
    repl_loop(&mut reader, &mut writer).unwrap();

    let output = String::from_utf8(writer.into_inner().unwrap()).unwrap();
    assert_eq!(output, "square\n25\n");
}

#[test]
fn test_let_binding() {
    let input = "(let ((x 10)) (* x 2))\n";
    let mut reader = BufReader::new(Cursor::new(input));
    let mut writer = BufWriter::new(Vec::new());
    repl_loop(&mut reader, &mut writer).unwrap();

    let output = String::from_utf8(writer.into_inner().unwrap()).unwrap();
    assert_eq!(output, "20\n");
}

#[test]
fn test_eq() {
    let input = "(eq 1 1)\n(eq 1 2)\n(eq \"a\" \"a\")\n(eq \"a\" \"b\")\n(eq t t)\n(eq nil nil)\n(eq t nil)\n";
    let mut reader = BufReader::new(Cursor::new(input));
    let mut writer = BufWriter::new(Vec::new());
    repl_loop(&mut reader, &mut writer).unwrap();

    let output = String::from_utf8(writer.into_inner().unwrap()).unwrap();
    assert_eq!(output, "t\n()\nt\n()\nt\nt\n()\n");
}

#[test]
fn test_logical_ops() {
    let input = "(not t)\n(not nil)\n(and t t)\n(and t nil)\n(or t nil)\n(or nil nil)\n";
    let mut reader = BufReader::new(Cursor::new(input));
    let mut writer = BufWriter::new(Vec::new());
    repl_loop(&mut reader, &mut writer).unwrap();

    let output = String::from_utf8(writer.into_inner().unwrap()).unwrap();
    assert_eq!(output, "()\nt\nt\n()\nt\n()\n");
}

#[test]
fn test_if_with_t_nil() {
    let input = "(if t 1 2)\n(if nil 1 2)\n";
    let mut reader = BufReader::new(Cursor::new(input));
    let mut writer = BufWriter::new(Vec::new());
    repl_loop(&mut reader, &mut writer).unwrap();

    let output = String::from_utf8(writer.into_inner().unwrap()).unwrap();
    assert_eq!(output, "1\n2\n");
}
