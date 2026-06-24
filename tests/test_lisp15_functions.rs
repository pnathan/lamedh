use lamedh::{environment::Environment, eval_str};

fn env() -> std::rc::Rc<Environment> {
    Environment::with_stdlib()
}

// PROG2

#[test]
fn test_prog2_returns_second() {
    let e = env();
    let v = eval_str("(prog2 1 2 3)", &e).unwrap();
    assert_eq!(v, lamedh::LispVal::Number(2));
}

#[test]
fn test_prog2_evaluates_first() {
    let e = env();
    eval_str("(def counter 0)", &e).unwrap();
    eval_str("(prog2 (setq counter (+ counter 1)) 99)", &e).unwrap();
    let v = eval_str("counter", &e).unwrap();
    assert_eq!(v, lamedh::LispVal::Number(1));
}

#[test]
fn test_prog2_two_args() {
    let e = env();
    let v = eval_str("(prog2 10 20)", &e).unwrap();
    assert_eq!(v, lamedh::LispVal::Number(20));
}

// NCONC

#[test]
fn test_nconc_two_lists() {
    let e = env();
    let v = eval_str("(nconc '(1 2) '(3 4))", &e).unwrap();
    let expected = eval_str("'(1 2 3 4)", &e).unwrap();
    assert_eq!(v, expected);
}

#[test]
fn test_nconc_empty_first() {
    let e = env();
    let v = eval_str("(nconc '() '(1 2))", &e).unwrap();
    let expected = eval_str("'(1 2)", &e).unwrap();
    assert_eq!(v, expected);
}

// COPY

#[test]
fn test_copy_flat_list() {
    let e = env();
    let v = eval_str("(copy '(1 2 3))", &e).unwrap();
    let expected = eval_str("'(1 2 3)", &e).unwrap();
    assert_eq!(v, expected);
}

#[test]
fn test_copy_nested_list() {
    let e = env();
    let v = eval_str("(copy '(1 (2 3) 4))", &e).unwrap();
    let expected = eval_str("'(1 (2 3) 4)", &e).unwrap();
    assert_eq!(v, expected);
}

#[test]
fn test_copy_atom() {
    let e = env();
    let v = eval_str("(copy 'x)", &e).unwrap();
    let expected = eval_str("'x", &e).unwrap();
    assert_eq!(v, expected);
}

// SASSOC

#[test]
fn test_sassoc_found() {
    let e = env();
    let v = eval_str("(sassoc 'b '((a . 1) (b . 2)) (lambda () 'notfound))", &e).unwrap();
    let expected = eval_str("'(b . 2)", &e).unwrap();
    assert_eq!(v, expected);
}

#[test]
fn test_sassoc_not_found_calls_fn() {
    let e = env();
    let v = eval_str("(sassoc 'x '((a . 1) (b . 2)) (lambda () 'notfound))", &e).unwrap();
    let expected = eval_str("'notfound", &e).unwrap();
    assert_eq!(v, expected);
}

#[test]
fn test_sassoc_empty_alist() {
    let e = env();
    let v = eval_str("(sassoc 'x '() (lambda () 42))", &e).unwrap();
    assert_eq!(v, lamedh::LispVal::Number(42));
}

// MAPC

#[test]
fn test_mapc_returns_list() {
    let e = env();
    eval_str("(def result '())", &e).unwrap();
    let v = eval_str(
        "(mapc '(1 2 3) (lambda (x) (setq result (cons x result))))",
        &e,
    )
    .unwrap();
    let expected = eval_str("'(1 2 3)", &e).unwrap();
    assert_eq!(v, expected);
}

#[test]
fn test_mapc_side_effects() {
    let e = env();
    eval_str("(def acc 0)", &e).unwrap();
    eval_str("(mapc '(1 2 3) (lambda (x) (setq acc (+ acc x))))", &e).unwrap();
    let v = eval_str("acc", &e).unwrap();
    assert_eq!(v, lamedh::LispVal::Number(6));
}

#[test]
fn test_mapc_empty_list() {
    let e = env();
    let v = eval_str("(mapc '() (lambda (x) x))", &e).unwrap();
    assert_eq!(v, lamedh::LispVal::Nil);
}

// MAPCON

#[test]
fn test_mapcon_collects_tails() {
    let e = env();
    let v = eval_str("(mapcon '(1 2 3) (lambda (x) (list (car x))))", &e).unwrap();
    let expected = eval_str("'(1 2 3)", &e).unwrap();
    assert_eq!(v, expected);
}

#[test]
fn test_mapcon_empty() {
    let e = env();
    let v = eval_str("(mapcon '() (lambda (x) x))", &e).unwrap();
    assert_eq!(v, lamedh::LispVal::Nil);
}

// CSET / CSETQ

#[test]
fn test_cset_defines_var() {
    let e = env();
    eval_str("(cset myvar 99)", &e).unwrap();
    let v = eval_str("myvar", &e).unwrap();
    assert_eq!(v, lamedh::LispVal::Number(99));
}

#[test]
fn test_csetq_alias() {
    let e = env();
    eval_str("(csetq another 77)", &e).unwrap();
    let v = eval_str("another", &e).unwrap();
    assert_eq!(v, lamedh::LispVal::Number(77));
}
