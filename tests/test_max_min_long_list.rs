use lamedh::environment::Environment;
use lamedh::evaluator::eval;
use lamedh::reader::read;

fn run(src: &str) -> Result<String, String> {
    let src = src.to_string();
    lamedh::with_large_stack(move || {
        let env = Environment::with_stdlib();
        let expr = read(&src, &env).map_err(|e| e.to_string())?;
        eval(&expr, &env)
            .map(|v| lamedh::printer::print(&v))
            .map_err(|e| format!("{e:?}"))
    })
}

#[test]
fn max_min_long_argument_list_does_not_overflow() {
    let build = "(let ((l nil) (i 0)) (while (< i 200000) (setq l (cons i l)) (setq i (+ i 1))) l)";
    assert_eq!(run(&format!("(apply #'max {build})")).unwrap(), "199999");
    assert_eq!(run(&format!("(apply #'min {build})")).unwrap(), "0");
}

#[test]
fn max_min_small_and_error_cases() {
    assert_eq!(run("(max 3 9 2)").unwrap(), "9");
    assert_eq!(run("(min 3 9 2)").unwrap(), "2");
    assert_eq!(run("(max 5)").unwrap(), "5");
    assert!(run("(max)").is_err());
    assert!(run("(min 1 'a)").is_err());
}
