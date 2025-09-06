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
