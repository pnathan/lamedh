use lamedh::LispVal;
use lamedh::environment::Environment;
use lamedh::evaluator::eval;
use lamedh::reader::read;

#[test]
fn deffun_typed_opt_optimizes_then_compiles() {
    let env = Environment::with_stdlib();
    let def = read(
        "(defun-typed-opt (plus-zero int64) ((x int64)) (if t (+ x 0) 99))",
        &env,
    )
    .unwrap();

    eval(&def, &env).expect("optimized typed definition should compile");

    let call = read("(plus-zero 7)", &env).unwrap();
    assert_eq!(eval(&call, &env).unwrap(), LispVal::Number(7));

    let dis = env
        .jit_disassemble("PLUS-ZERO")
        .expect("optimized typed definition should install a typed edition");
    assert!(
        dis.contains("compiled edition: yes"),
        "expected compiled typed edition:\n{dis}"
    );
    assert!(
        !dis.contains("iadd"),
        "optimizer should remove (+ x 0) before typed codegen:\n{dis}"
    );
}
