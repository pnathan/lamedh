mod test_helpers;
use lamedh::eval_line;
use test_helpers::env_with_stdlib;

#[test]
fn test_load_file() {
    let env = env_with_stdlib();
    // Enable READ-FS so the feature gate passes.
    env.enable_feature("READ-FS");
    let load_output = eval_line("(load-file \"tests/load_file_test_sample.lisp\")", &env);
    assert_eq!(load_output, "T");
    let output = eval_line("(loaded-function)", &env);
    assert_eq!(output, "42");
}
