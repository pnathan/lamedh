/// Reproduction test for the embedded-env dotted-pair-ending-in-lambda bug.
/// See issue-embedded-env-dotted-pair-lambda-0.3.md in the Tupshar workspace.
use lamedh::environment::Environment;
use lamedh::{LispVal, Shared, eval_all, with_large_stack};

#[test]
fn test_register_fn_recursive_scan() {
    with_large_stack(|| {
        let env = Environment::with_stdlib_fresh();

        // Shadow CONCAT and PRINC-TO-STRING like the game does
        env.register_fn("CONCAT", |args, _env| {
            let mut result = String::new();
            for arg in args {
                match arg {
                    LispVal::String(s) => result.push_str(s),
                    other => result.push_str(&lamedh::printer::print(other)),
                }
            }
            Ok(LispVal::String(result))
        });
        env.register_fn("PRINC-TO-STRING", |args, _env| {
            if let Some(v) = args.first() {
                Ok(LispVal::String(lamedh::printer::print(v)))
            } else {
                Ok(LispVal::String(String::new()))
            }
        });

        env.register_fn("ACTOR-COUNT", |_args, _env| Ok(LispVal::Float(3.0)));
        env.register_fn("ACTOR-ID", |args, _env| {
            let i = match &args[0] {
                LispVal::Number(n) => *n,
                LispVal::Float(f) => *f as i64,
                _ => 0,
            };
            Ok(LispVal::String(match i {
                0 => "pc".to_string(),
                1 => "wolf-1".to_string(),
                _ => "wolf-2".to_string(),
            }))
        });
        env.register_fn("ACTOR-POS", |_args, _env| Ok(LispVal::list([0.0f64, 0.0])));
        env.register_fn("DISTANCE", |args, _env| {
            let x1 = match &args[0] {
                LispVal::Float(f) => *f,
                LispVal::Number(n) => *n as f64,
                _ => 0.0,
            };
            let y1 = match &args[1] {
                LispVal::Float(f) => *f,
                LispVal::Number(n) => *n as f64,
                _ => 0.0,
            };
            let x2 = match &args[2] {
                LispVal::Float(f) => *f,
                LispVal::Number(n) => *n as f64,
                _ => 0.0,
            };
            let y2 = match &args[3] {
                LispVal::Float(f) => *f,
                LispVal::Number(n) => *n as f64,
                _ => 0.0,
            };
            let dx = x1 - x2;
            let dy = y1 - y2;
            Ok(LispVal::Float((dx * dx + dy * dy).sqrt()))
        });
        env.register_fn("ACTIONABLE-ACTOR-DESC", |args, _env| {
            if let LispVal::String(id) = &args[0]
                && id == "wolf-1"
            {
                let kind = LispVal::Cons {
                    car: Shared::new(LispVal::String("kind".to_string())),
                    cdr: Shared::new(LispVal::String("talk".to_string())),
                };
                let label = LispVal::Cons {
                    car: Shared::new(LispVal::String("label".to_string())),
                    cdr: Shared::new(LispVal::String("Talk".to_string())),
                };
                return Ok(LispVal::list(vec![kind, label]));
            }
            Ok(LispVal::Nil)
        });

        let code = r#"
(defun scan-actors (pp i best bestd)
  (if (float-greaterp i (- (actor-count) 1)) best
    (let ((id (actor-id i)))
      (let ((desc (actionable-actor-desc id)))
        (if (null desc) (scan-actors pp (+ i 1) best bestd)
          (let ((ap (actor-pos id)))
            (let ((d (distance (car pp) (cadr pp) (car ap) (cadr ap))))
              (if (and (float-lessp d 1.8) (float-lessp d bestd))
                  (scan-actors pp (+ i 1) (list desc d) d)
                  (scan-actors pp (+ i 1) best bestd)))))))))

(let ((pp (list 0.0 0.0)))
  (scan-actors pp 0 nil 99.0))
"#;
        let result = eval_all(code, &env);
        match result {
            Ok(vals) => {
                let last = vals.last().unwrap();
                let printed = lamedh::printer::print(last);
                eprintln!("Result: {printed}");
                assert!(
                    !printed.contains("lambda"),
                    "should not contain lambda in result: {printed}"
                );
            }
            Err(e) => {
                panic!("scan-actors failed: {e:?}");
            }
        }
    });
}

#[test]
fn test_register_fn_shadowing_concat() {
    with_large_stack(|| {
        let env = Environment::with_stdlib_fresh();

        env.register_fn("CONCAT", |args, _env| {
            let mut result = String::new();
            for arg in args {
                match arg {
                    LispVal::String(s) => result.push_str(s),
                    other => result.push_str(&lamedh::printer::print(other)),
                }
            }
            Ok(LispVal::String(result))
        });

        let result = eval_all(r#"(concat "hello" " " "world")"#, &env);
        match result {
            Ok(vals) => {
                let printed = lamedh::printer::print(vals.last().unwrap());
                assert_eq!(printed, "\"hello world\"", "got: {printed}");
            }
            Err(e) => panic!("concat shadow test failed: {e:?}"),
        }

        let result = eval_all(r#"(string-upcase "hello")"#, &env);
        match result {
            Ok(vals) => {
                let printed = lamedh::printer::print(vals.last().unwrap());
                assert_eq!(printed, "\"HELLO\"", "stdlib should still work: {printed}");
            }
            Err(e) => panic!("stdlib with shadowed concat failed: {e:?}"),
        }
    });
}
