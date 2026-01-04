#![allow(clippy::mutable_key_type)]
use crate::{BuiltinFunc, LispError, LispVal, environment::Environment};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

// Helper function to convert a Lisp list (Cons chain) to a Rust Vec.
fn list_to_vec(list: &LispVal) -> Result<Vec<LispVal>, LispError> {
    let mut vec = Vec::new();
    let mut current = list;
    while let LispVal::Cons { car, cdr } = current {
        vec.push(*car.clone());
        current = cdr;
    }
    if *current != LispVal::Nil {
        return Err(LispError::Generic(
            "list_to_vec: not a proper list".to_string(),
        ));
    }
    Ok(vec)
}

// Helper function to convert a Rust Vec to a Lisp list.
fn vec_to_list(vec: Vec<LispVal>) -> LispVal {
    vec.into_iter()
        .rev()
        .fold(LispVal::Nil, |cdr, car| LispVal::Cons {
            car: Box::new(car),
            cdr: Box::new(cdr),
        })
}

fn apply_apply(args: &[LispVal], env: &Rc<Environment>) -> Result<LispVal, LispError> {
    if args.len() != 2 {
        return Err(LispError::Generic(
            "APPLY requires exactly two arguments".to_string(),
        ));
    }
    let func_arg = &args[0];
    let arg_list = &args[1];

    let func = match func_arg {
        LispVal::Symbol(s) => env
            .get(&s.borrow().name)
            .ok_or_else(|| LispError::Generic(format!("Function not found: {}", s.borrow().name))),
        _ => Ok(func_arg.clone()),
    }?;

    let unpacked_args = match list_to_vec(arg_list) {
        Ok(vec) => vec,
        Err(_) => {
            return Err(LispError::Generic(
                "APPLY second argument must be a proper list".to_string(),
            ));
        }
    };

    match &func {
        LispVal::Macro(m) => {
            let expanded = expand_macro(m, &unpacked_args, env)?;
            eval(&expanded, env)
        }
        LispVal::Fexpr(f) => {
            if f.params.len() != 1 {
                return Err(LispError::Generic(
                    "APPLY: fexpr must have exactly one parameter for the list of arguments"
                        .to_string(),
                ));
            }
            let new_env = Environment::new_child(&f.env);
            let fexpr_arg_list = vec_to_list(unpacked_args);
            new_env.set(f.params[0].clone(), fexpr_arg_list);
            eval(&f.body, &new_env)
        }
        _ => apply(&func, &unpacked_args, env),
    }
}

fn is_truthy(val: &LispVal) -> bool {
    !matches!(val, LispVal::Nil)
}

fn apply_math_op(op: &BuiltinFunc, args: &[LispVal]) -> Result<LispVal, LispError> {
    let nums: Result<Vec<i64>, LispError> = args
        .iter()
        .map(|arg| match arg {
            LispVal::Number(n) => Ok(*n),
            _ => Err(LispError::Generic(
                "Math functions only accept numbers".to_string(),
            )),
        })
        .collect();
    let nums = nums?;

    match op {
        BuiltinFunc::Plus => Ok(LispVal::Number(nums.iter().sum())),
        BuiltinFunc::Minus => {
            if nums.is_empty() {
                return Err(LispError::Generic(
                    "- requires at least one argument".to_string(),
                ));
            }
            if nums.len() == 1 {
                Ok(LispVal::Number(-nums[0]))
            } else {
                let mut result = nums[0];
                for &num in &nums[1..] {
                    result -= num;
                }
                Ok(LispVal::Number(result))
            }
        }
        BuiltinFunc::Multiply => Ok(LispVal::Number(nums.iter().product())),
        BuiltinFunc::Divide => {
            if nums.len() != 2 {
                return Err(LispError::Generic(
                    "/ requires exactly two arguments".to_string(),
                ));
            }
            if nums[1] == 0 {
                return Err(LispError::Generic("Division by zero".to_string()));
            }
            Ok(LispVal::Number(nums[0] / nums[1]))
        }
        _ => Err(LispError::Generic("Not a math operation".to_string())),
    }
}

fn apply_list_op(op: &BuiltinFunc, args: &[LispVal]) -> Result<LispVal, LispError> {
    match op {
        BuiltinFunc::Car => {
            if args.len() != 1 {
                return Err(LispError::Generic(
                    "car requires exactly one argument".to_string(),
                ));
            }
            match &args[0] {
                LispVal::Cons { car, .. } => Ok(*car.clone()),
                LispVal::Nil => Ok(LispVal::Nil),
                _ => Err(LispError::Generic("car requires a list".to_string())),
            }
        }
        BuiltinFunc::Cdr => {
            if args.len() != 1 {
                return Err(LispError::Generic(
                    "cdr requires exactly one argument".to_string(),
                ));
            }
            match &args[0] {
                LispVal::Cons { cdr, .. } => Ok(*cdr.clone()),
                LispVal::Nil => Ok(LispVal::Nil),
                _ => Err(LispError::Generic("cdr requires a list".to_string())),
            }
        }
        BuiltinFunc::Cons => {
            if args.len() != 2 {
                return Err(LispError::Generic(
                    "cons requires exactly two arguments".to_string(),
                ));
            }
            Ok(LispVal::Cons {
                car: Box::new(args[0].clone()),
                cdr: Box::new(args[1].clone()),
            })
        }
        _ => Err(LispError::Generic("Not a list operation".to_string())),
    }
}

fn apply_string_op(op: &BuiltinFunc, args: &[LispVal]) -> Result<LispVal, LispError> {
    match op {
        BuiltinFunc::Concat => {
            let strs: Result<Vec<String>, LispError> = args
                .iter()
                .map(|arg| match arg {
                    LispVal::String(s) => Ok(s.clone()),
                    _ => Err(LispError::Generic(
                        "concat only accepts strings".to_string(),
                    )),
                })
                .collect();
            Ok(LispVal::String(strs?.concat()))
        }
        BuiltinFunc::Index => {
            if args.len() != 2 {
                return Err(LispError::Generic(
                    "index requires exactly two arguments".to_string(),
                ));
            }
            let s = if let LispVal::String(s) = &args[0] {
                s
            } else {
                return Err(LispError::Generic(
                    "index requires a string as its first argument".to_string(),
                ));
            };
            let i = if let LispVal::Number(n) = &args[1] {
                *n as usize
            } else {
                return Err(LispError::Generic(
                    "index requires a number as its second argument".to_string(),
                ));
            };
            if let Some(ch) = s.chars().nth(i) {
                Ok(LispVal::String(ch.to_string()))
            } else {
                Err(LispError::Generic("index out of bounds".to_string()))
            }
        }
        _ => Err(LispError::Generic("Not a string operation".to_string())),
    }
}

fn apply_numeric_primitives(
    op: &BuiltinFunc,
    args: &[LispVal],
    env: &Rc<Environment>,
) -> Result<LispVal, LispError> {
    match op {
        BuiltinFunc::Lessp => {
            if args.len() != 2 {
                return Err(LispError::Generic("lessp requires 2 args".to_string()));
            }
            if let (LispVal::Number(x), LispVal::Number(y)) = (&args[0], &args[1]) {
                Ok(if x < y {
                    LispVal::Symbol(env.intern_symbol("T"))
                } else {
                    LispVal::Nil
                })
            } else {
                Err(LispError::Generic("lessp requires numbers".to_string()))
            }
        }
        BuiltinFunc::Greaterp => {
            if args.len() != 2 {
                return Err(LispError::Generic("greaterp requires 2 args".to_string()));
            }
            if let (LispVal::Number(x), LispVal::Number(y)) = (&args[0], &args[1]) {
                Ok(if x > y {
                    LispVal::Symbol(env.intern_symbol("T"))
                } else {
                    LispVal::Nil
                })
            } else {
                Err(LispError::Generic("greaterp requires numbers".to_string()))
            }
        }
        BuiltinFunc::Zerop => {
            if args.len() != 1 {
                return Err(LispError::Generic("zerop requires 1 arg".to_string()));
            }
            if let LispVal::Number(x) = &args[0] {
                Ok(if *x == 0 {
                    LispVal::Symbol(env.intern_symbol("T"))
                } else {
                    LispVal::Nil
                })
            } else {
                Err(LispError::Generic("zerop requires number".to_string()))
            }
        }
        BuiltinFunc::Remainder => {
            if args.len() != 2 {
                return Err(LispError::Generic("remainder requires 2 args".to_string()));
            }
            if let (LispVal::Number(x), LispVal::Number(y)) = (&args[0], &args[1]) {
                if *y == 0 {
                    return Err(LispError::Generic("Division by zero".to_string()));
                }
                Ok(LispVal::Number(x % y))
            } else {
                Err(LispError::Generic("remainder requires numbers".to_string()))
            }
        }
        BuiltinFunc::Expt => {
            if args.len() != 2 {
                return Err(LispError::Generic("expt requires 2 args".to_string()));
            }
            if let (LispVal::Number(base), LispVal::Number(exp)) = (&args[0], &args[1]) {
                if *exp < 0 {
                    return Err(LispError::Generic(
                        "negative exponent not supported".to_string(),
                    ));
                }
                Ok(LispVal::Number(base.pow(*exp as u32)))
            } else {
                Err(LispError::Generic("expt requires numbers".to_string()))
            }
        }
        _ => Err(LispError::Generic("Not a numeric primitive".to_string())),
    }
}

fn apply_logical_op(
    op: &BuiltinFunc,
    args: &[LispVal],
    env: &Rc<Environment>,
) -> Result<LispVal, LispError> {
    match op {
        BuiltinFunc::Eq => {
            if args.len() != 2 {
                return Err(LispError::Generic(
                    "eq requires exactly two arguments".to_string(),
                ));
            }
            if args[0] == args[1] {
                Ok(LispVal::Symbol(env.intern_symbol("T")))
            } else {
                Ok(LispVal::Nil)
            }
        }
        BuiltinFunc::Not => {
            if args.len() != 1 {
                return Err(LispError::Generic(
                    "not requires exactly one argument".to_string(),
                ));
            }
            if is_truthy(&args[0]) {
                Ok(LispVal::Nil)
            } else {
                Ok(LispVal::Symbol(env.intern_symbol("T")))
            }
        }
        BuiltinFunc::NumericEquals => {
            if args.len() != 2 {
                return Err(LispError::Generic(
                    "= requires exactly two arguments".to_string(),
                ));
            }
            if let (LispVal::Number(a), LispVal::Number(b)) = (&args[0], &args[1]) {
                if a == b {
                    Ok(LispVal::Symbol(env.intern_symbol("T")))
                } else {
                    Ok(LispVal::Nil)
                }
            } else {
                Err(LispError::Generic(
                    "= requires numeric arguments".to_string(),
                ))
            }
        }
        _ => Err(LispError::Generic("Not a logical operation".to_string())),
    }
}

fn apply_hashtable_op(
    op: &BuiltinFunc,
    args: &[LispVal],
    env: &Rc<Environment>,
) -> Result<LispVal, LispError> {
    match op {
        BuiltinFunc::MakeHashTable => {
            if !args.is_empty() {
                return Err(LispError::Generic(
                    "make-hash-table takes no arguments".to_string(),
                ));
            }
            Ok(LispVal::HashTable(Rc::new(RefCell::new(HashMap::new()))))
        }
        BuiltinFunc::Set => {
            if args.len() != 3 {
                return Err(LispError::Generic(
                    "set! takes exactly three arguments".to_string(),
                ));
            }
            if let LispVal::HashTable(h) = &args[0] {
                let key = args[1].clone();
                let val = args[2].clone();
                h.borrow_mut().insert(key, val);
                Ok(LispVal::Symbol(env.intern_symbol("T")))
            } else {
                Err(LispError::Generic(
                    "set! requires a hash table as its first argument".to_string(),
                ))
            }
        }
        BuiltinFunc::Get => {
            if args.len() != 2 {
                return Err(LispError::Generic(
                    "get takes exactly two arguments".to_string(),
                ));
            }
            if let LispVal::HashTable(h) = &args[0] {
                let key = &args[1];
                if let Some(val) = h.borrow().get(key) {
                    Ok(val.clone())
                } else {
                    Ok(LispVal::Nil)
                }
            } else {
                Err(LispError::Generic(
                    "get requires a hash table as its first argument".to_string(),
                ))
            }
        }
        BuiltinFunc::DeleteKey => {
            if args.len() != 2 {
                return Err(LispError::Generic(
                    "delete-key! takes exactly two arguments".to_string(),
                ));
            }
            if let LispVal::HashTable(h) = &args[0] {
                let key = &args[1];
                h.borrow_mut().remove(key);
                Ok(LispVal::Symbol(env.intern_symbol("T")))
            } else {
                Err(LispError::Generic(
                    "delete-key! requires a hash table as its first argument".to_string(),
                ))
            }
        }
        BuiltinFunc::CurrentEnvironment => {
            if !args.is_empty() {
                return Err(LispError::Generic(
                    "current-environment takes no arguments".to_string(),
                ));
            }
            let bindings = env.all_bindings();
            let mut hash_map = HashMap::new();
            for (k, v) in bindings {
                hash_map.insert(LispVal::Symbol(env.intern_symbol(&k)), v);
            }
            Ok(LispVal::HashTable(Rc::new(RefCell::new(hash_map))))
        }
        BuiltinFunc::Keys => {
            if args.len() != 1 {
                return Err(LispError::Generic(
                    "keys requires exactly one argument".to_string(),
                ));
            }
            if let LispVal::HashTable(h) = &args[0] {
                let keys = h.borrow().keys().cloned().collect();
                Ok(vec_to_list(keys))
            } else {
                Err(LispError::Generic(
                    "keys requires a hash table as its first argument".to_string(),
                ))
            }
        }
        _ => Err(LispError::Generic("Not a hash table operation".to_string())),
    }
}

fn apply(func: &LispVal, args: &[LispVal], env: &Rc<Environment>) -> Result<LispVal, LispError> {
    match func {
        LispVal::Builtin(builtin) => match builtin {
            BuiltinFunc::Plus
            | BuiltinFunc::Minus
            | BuiltinFunc::Multiply
            | BuiltinFunc::Divide => apply_math_op(builtin, args),
            BuiltinFunc::Lessp
            | BuiltinFunc::Greaterp
            | BuiltinFunc::Zerop
            | BuiltinFunc::Remainder
            | BuiltinFunc::Expt => apply_numeric_primitives(builtin, args, env),
            BuiltinFunc::Car | BuiltinFunc::Cdr | BuiltinFunc::Cons => apply_list_op(builtin, args),
            BuiltinFunc::Concat | BuiltinFunc::Index => apply_string_op(builtin, args),
            BuiltinFunc::Eval => {
                if args.len() != 1 {
                    return Err(LispError::Generic(
                        "eval takes exactly one argument".to_string(),
                    ));
                }
                eval(&args[0], env)
            }
            BuiltinFunc::Eq | BuiltinFunc::Not | BuiltinFunc::NumericEquals => {
                apply_logical_op(builtin, args, env)
            }
            BuiltinFunc::MakeHashTable
            | BuiltinFunc::Get
            | BuiltinFunc::Set
            | BuiltinFunc::DeleteKey
            | BuiltinFunc::CurrentEnvironment
            | BuiltinFunc::Keys => apply_hashtable_op(builtin, args, env),
            BuiltinFunc::Atom => {
                if args.len() != 1 {
                    return Err(LispError::Generic(
                        "atom requires exactly one argument".to_string(),
                    ));
                }
                match &args[0] {
                    LispVal::Cons { .. } => Ok(LispVal::Nil),
                    _ => Ok(LispVal::Symbol(env.intern_symbol("T"))),
                }
            }
            BuiltinFunc::Print => {
                for arg in args {
                    print!("{}", crate::printer::print(arg));
                }
                println!();
                Ok(LispVal::Nil)
            }
            BuiltinFunc::GetP | BuiltinFunc::PutP => apply_symbol_op(builtin, args, env),
            BuiltinFunc::Stringp => {
                if args.len() != 1 {
                    return Err(LispError::Generic(
                        "stringp requires exactly one argument".to_string(),
                    ));
                }
                match &args[0] {
                    LispVal::String(_) => Ok(LispVal::Symbol(env.intern_symbol("T"))),
                    _ => Ok(LispVal::Nil),
                }
            }
            BuiltinFunc::Numberp => {
                if args.len() != 1 {
                    return Err(LispError::Generic(
                        "numberp requires exactly one argument".to_string(),
                    ));
                }
                match &args[0] {
                    LispVal::Number(_) | LispVal::Float(_) => {
                        Ok(LispVal::Symbol(env.intern_symbol("T")))
                    }
                    _ => Ok(LispVal::Nil),
                }
            }

            BuiltinFunc::Apply => apply_apply(args, env),

            // I/O functions
            BuiltinFunc::Read | BuiltinFunc::Prin1 | BuiltinFunc::Princ | BuiltinFunc::Terpri => {
                apply_io_op(builtin, args, env)
            }

            // Error handling
            BuiltinFunc::Error | BuiltinFunc::Errorset => apply_error_op(builtin, args, env),

            // List processing
            BuiltinFunc::Subst
            | BuiltinFunc::Assoc
            | BuiltinFunc::Maplist
            | BuiltinFunc::Mapcar
            | BuiltinFunc::Rplaca
            | BuiltinFunc::Rplacd => apply_list_processing(builtin, args, env),

            // Bitwise operations
            BuiltinFunc::Logor
            | BuiltinFunc::Logand
            | BuiltinFunc::Logxor
            | BuiltinFunc::Leftshift => apply_bitwise_op(builtin, args, env),

            // Property list functions
            BuiltinFunc::Remprop | BuiltinFunc::Deflist => apply_plist_op(builtin, args, env),

            // Type predicates
            BuiltinFunc::Fixp => {
                if args.len() != 1 {
                    return Err(LispError::Generic(
                        "fixp requires exactly one argument".to_string(),
                    ));
                }
                match &args[0] {
                    LispVal::Number(_) => Ok(LispVal::Symbol(env.intern_symbol("T"))),
                    _ => Ok(LispVal::Nil),
                }
            }
            BuiltinFunc::Floatp => {
                if args.len() != 1 {
                    return Err(LispError::Generic(
                        "floatp requires exactly one argument".to_string(),
                    ));
                }
                match &args[0] {
                    LispVal::Float(_) => Ok(LispVal::Symbol(env.intern_symbol("T"))),
                    _ => Ok(LispVal::Nil),
                }
            }

            // New type predicates
            BuiltinFunc::Symbolp
            | BuiltinFunc::Boundp
            | BuiltinFunc::Functionp
            | BuiltinFunc::Macrop => apply_type_predicates(builtin, args, env),

            // New list operations
            BuiltinFunc::List
            | BuiltinFunc::Last
            | BuiltinFunc::Nth
            | BuiltinFunc::Nthcdr
            | BuiltinFunc::Efface => apply_new_list_ops(builtin, args, env),

            // New numeric operations
            BuiltinFunc::Mod
            | BuiltinFunc::Plusp
            | BuiltinFunc::Evenp
            | BuiltinFunc::Oddp
            | BuiltinFunc::Add1
            | BuiltinFunc::Sub1
            | BuiltinFunc::Random => apply_new_numeric_ops(builtin, args, env),

            // New bitwise operations
            BuiltinFunc::Ash | BuiltinFunc::Lognot | BuiltinFunc::Rot => {
                apply_new_bitwise_ops(builtin, args, env)
            }

            // Function operations
            BuiltinFunc::Funcall | BuiltinFunc::Macroexpand => {
                apply_function_ops(builtin, args, env)
            }

            // String/Symbol operations
            BuiltinFunc::Explode
            | BuiltinFunc::Implode
            | BuiltinFunc::Maknam
            | BuiltinFunc::Gensym
            | BuiltinFunc::Intern
            | BuiltinFunc::Plist => apply_string_symbol_ops(builtin, args, env),

            BuiltinFunc::LoadFile => {
                if args.len() != 1 {
                    return Err(LispError::Generic(
                        "load-file requires exactly one argument".to_string(),
                    ));
                }

                let filename = if let LispVal::String(path) = &args[0] {
                    path.clone()
                } else {
                    return Err(LispError::Generic(
                        "load-file requires a string filename".to_string(),
                    ));
                };

                crate::load_file(&filename, env)?;
                Ok(LispVal::Symbol(env.intern_symbol("T")))
            }
        },
        LispVal::Lambda(lambda) => {
            let new_env = Environment::new_child(&lambda.env);
            if let Some(rest_param_name) = &lambda.rest_param {
                if args.len() < lambda.params.len() {
                    return Err(LispError::Generic(format!(
                        "lambda expected at least {} arguments, got {}",
                        lambda.params.len(),
                        args.len()
                    )));
                }
                for (param, arg) in lambda.params.iter().zip(args.iter()) {
                    new_env.set(param.clone(), arg.clone());
                }
                let rest_args = vec_to_list(args[lambda.params.len()..].to_vec());
                new_env.set(rest_param_name.clone(), rest_args);
            } else {
                if lambda.params.len() != args.len() {
                    return Err(LispError::Generic(format!(
                        "lambda expected {} arguments, got {}",
                        lambda.params.len(),
                        args.len()
                    )));
                }
                for (param, arg) in lambda.params.iter().zip(args) {
                    new_env.set(param.clone(), arg.clone());
                }
            }

            eval(&lambda.body, &new_env)
        }
        _ => Err(LispError::Generic(format!("Not a function: {func:?}"))),
    }
}

fn make_lambda(
    params: &LispVal,
    body: &LispVal,
    env: &Rc<Environment>,
) -> Result<LispVal, LispError> {
    let p_list = list_to_vec(params)?;
    let mut params_vec = Vec::new();
    let mut rest_param = None;
    let mut iter = p_list.iter();

    while let Some(p) = iter.next() {
        if let LispVal::Symbol(s) = p {
            if s.borrow().name == "&REST" {
                if let Some(LispVal::Symbol(rest_p_sym)) = iter.next() {
                    if iter.next().is_some() {
                        return Err(LispError::Generic(
                            "Only one symbol can follow &rest".to_string(),
                        ));
                    }
                    rest_param = Some(rest_p_sym.borrow().name.clone());
                    break; // No more params after &rest
                } else {
                    return Err(LispError::Generic(
                        "&rest must be followed by a symbol".to_string(),
                    ));
                }
            } else {
                params_vec.push(s.borrow().name.clone());
            }
        } else {
            return Err(LispError::Generic(
                "lambda parameters must be symbols".to_string(),
            ));
        }
    }

    Ok(LispVal::Lambda(crate::Lambda {
        params: params_vec,
        rest_param,
        body: Box::new(body.clone()),
        env: env.clone(),
    }))
}

fn make_fexpr(
    params: &LispVal,
    body: &LispVal,
    env: &Rc<Environment>,
) -> Result<LispVal, LispError> {
    let p_list = list_to_vec(params)?;
    let params_vec: Result<Vec<String>, _> = p_list
        .iter()
        .map(|p| {
            if let LispVal::Symbol(s) = p {
                Ok(s.borrow().name.clone())
            } else {
                Err(LispError::Generic(
                    "fexpr parameters must be symbols".to_string(),
                ))
            }
        })
        .collect();

    Ok(LispVal::Fexpr(crate::Fexpr {
        params: params_vec?,
        body: Box::new(body.clone()),
        env: env.clone(),
    }))
}

fn make_macro(
    params: &LispVal,
    body: &LispVal,
    env: &Rc<Environment>,
) -> Result<LispVal, LispError> {
    let p_list = list_to_vec(params)?;
    let mut params_vec = Vec::new();
    let mut rest_param = None;
    let mut iter = p_list.iter();

    while let Some(p) = iter.next() {
        if let LispVal::Symbol(s) = p {
            if s.borrow().name == "&REST" {
                if let Some(LispVal::Symbol(rest_p_sym)) = iter.next() {
                    if iter.next().is_some() {
                        return Err(LispError::Generic(
                            "Only one symbol can follow &rest".to_string(),
                        ));
                    }
                    rest_param = Some(rest_p_sym.borrow().name.clone());
                    break; // No more params after &rest
                } else {
                    return Err(LispError::Generic(
                        "&rest must be followed by a symbol".to_string(),
                    ));
                }
            } else {
                params_vec.push(s.borrow().name.clone());
            }
        } else {
            return Err(LispError::Generic(
                "macro parameters must be symbols".to_string(),
            ));
        }
    }

    Ok(LispVal::Macro(crate::Macro {
        params: params_vec,
        rest_param,
        body: Box::new(body.clone()),
        env: env.clone(),
    }))
}

fn expand_macro(
    m: &crate::Macro,
    args: &[LispVal],
    _env: &Rc<Environment>,
) -> Result<LispVal, LispError> {
    let macro_env = Environment::new_child(&m.env);

    if let Some(rest_param_name) = &m.rest_param {
        if args.len() < m.params.len() {
            return Err(LispError::Generic(format!(
                "macro expected at least {} arguments, got {}",
                m.params.len(),
                args.len()
            )));
        }
        for (param, arg) in m.params.iter().zip(args.iter()) {
            macro_env.set(param.clone(), arg.clone());
        }
        let rest_args = vec_to_list(args[m.params.len()..].to_vec());
        macro_env.set(rest_param_name.clone(), rest_args);
    } else {
        if m.params.len() != args.len() {
            return Err(LispError::Generic(format!(
                "macro expected {} arguments, got {}",
                m.params.len(),
                args.len()
            )));
        }
        for (param, arg) in m.params.iter().zip(args) {
            macro_env.set(param.clone(), arg.clone());
        }
    }

    eval(&m.body, &macro_env)
}

pub fn eval(val: &LispVal, env: &Rc<Environment>) -> Result<LispVal, LispError> {
    match val {
        LispVal::Nil => Ok(LispVal::Nil),
        LispVal::Symbol(s) => env
            .get(&s.borrow().name)
            .ok_or_else(|| LispError::Generic(format!("Unbound variable: {}", s.borrow().name))),
        LispVal::Number(_)
        | LispVal::Float(_)
        | LispVal::String(_)
        | LispVal::Builtin(_)
        | LispVal::Lambda(_)
        | LispVal::Fexpr(_)
        | LispVal::Macro(_)
        | LispVal::HashTable(_) => Ok(val.clone()),

        LispVal::Cons {
            car: first,
            cdr: rest,
        } => {
            if let LispVal::Symbol(s) = &**first {
                match s.borrow().name.as_str() {
                    "QUOTE" => {
                        if let LispVal::Cons { car, cdr } = &**rest
                            && **cdr == LispVal::Nil
                        {
                            return Ok(*car.clone());
                        }
                        Err(LispError::Generic(
                            "quote takes exactly one argument".to_string(),
                        ))
                    }
                    "QUASIQUOTE" => {
                        if let LispVal::Cons { car, cdr } = &**rest
                            && **cdr == LispVal::Nil
                        {
                            return quasiquote_eval(car, env);
                        }
                        Err(LispError::Generic(
                            "quasiquote takes exactly one argument".to_string(),
                        ))
                    }
                    "COND" => {
                        let mut current_clause = &**rest;
                        while let LispVal::Cons {
                            car: clause,
                            cdr: next_clauses,
                        } = current_clause
                        {
                            if let LispVal::Cons {
                                car: predicate,
                                cdr: expressions,
                            } = &**clause
                            {
                                let predicate_result = eval(predicate, env)?;
                                if is_truthy(&predicate_result) {
                                    if **expressions == LispVal::Nil {
                                        return Ok(predicate_result);
                                    } else {
                                        let mut last_val = LispVal::Nil;
                                        let mut current_expr = &**expressions;
                                        while let LispVal::Cons {
                                            car: expr,
                                            cdr: next_exprs,
                                        } = current_expr
                                        {
                                            last_val = eval(expr, env)?;
                                            current_expr = next_exprs;
                                        }
                                        return Ok(last_val);
                                    }
                                }
                            } else {
                                return Err(LispError::Generic(
                                    "cond clauses must be lists".to_string(),
                                ));
                            }
                            current_clause = next_clauses;
                        }
                        Ok(LispVal::Nil) // No clause was true
                    }
                    "IF" => {
                        let args = list_to_vec(rest)?;
                        if args.len() != 3 {
                            return Err(LispError::Generic(
                                "if takes exactly three arguments".to_string(),
                            ));
                        }
                        let cond_result = eval(&args[0], env)?;
                        if is_truthy(&cond_result) {
                            eval(&args[1], env)
                        } else {
                            eval(&args[2], env)
                        }
                    }
                    "AND" => {
                        let mut last_val = LispVal::Symbol(env.intern_symbol("T"));
                        let mut current = &**rest;
                        while let LispVal::Cons { car, cdr } = current {
                            last_val = eval(car, env)?;
                            if !is_truthy(&last_val) {
                                return Ok(LispVal::Nil);
                            }
                            current = cdr;
                        }
                        Ok(last_val)
                    }
                    "OR" => {
                        let mut current = &**rest;
                        while let LispVal::Cons { car, cdr } = current {
                            let val = eval(car, env)?;
                            if is_truthy(&val) {
                                return Ok(val);
                            }
                            current = cdr;
                        }
                        Ok(LispVal::Nil)
                    }
                    "DEF" => {
                        let args = list_to_vec(rest)?;
                        if args.len() != 2 && args.len() != 3 {
                            return Err(LispError::Generic(
                                "def takes two or three arguments".to_string(),
                            ));
                        }
                        if let LispVal::Symbol(s) = &args[0] {
                            let name = s.borrow().name.clone();
                            let val = eval(&args[1], env)?;
                            if args.len() == 3 {
                                if let LispVal::String(doc) = &args[2] {
                                    s.borrow_mut().plist.insert(
                                        "docstring".to_string(),
                                        LispVal::String(doc.clone()),
                                    );
                                } else {
                                    return Err(LispError::Generic(
                                        "docstring must be a string".to_string(),
                                    ));
                                }
                            }
                            env.set(name, val);
                            Ok(LispVal::Symbol(s.clone()))
                        } else {
                            Err(LispError::Generic(
                                "def requires a symbol as its first argument".to_string(),
                            ))
                        }
                    }
                    "LAMBDA" => {
                        if let LispVal::Cons {
                            car: params,
                            cdr: body_list,
                        } = &**rest
                        {
                            let body_exprs = list_to_vec(body_list)?;
                            let final_body = if body_exprs.len() == 1 {
                                body_exprs[0].clone()
                            } else {
                                let progn_sym = LispVal::Symbol(env.intern_symbol("PROGN"));
                                vec_to_list([vec![progn_sym], body_exprs].concat())
                            };
                            return make_lambda(params, &final_body, env);
                        }
                        Err(LispError::Generic(
                            "lambda requires params and at least one body expression".to_string(),
                        ))
                    }
                    "FUNCTION" => {
                        let args = list_to_vec(rest)?;
                        if args.len() != 1 {
                            return Err(LispError::Generic(
                                "FUNCTION takes exactly one argument".to_string(),
                            ));
                        }
                        let arg = &args[0];

                        // Case 1: Argument is a literal LAMBDA expression
                        if let LispVal::Cons {
                            car: lambda_sym,
                            cdr: lambda_body,
                        } = arg
                            && let LispVal::Symbol(s) = &**lambda_sym
                            && s.borrow().name == "LAMBDA"
                            && let LispVal::Cons {
                                car: params,
                                cdr: body_list,
                            } = &**lambda_body
                        {
                            let body_exprs = list_to_vec(body_list)?;
                            let final_body = if body_exprs.len() == 1 {
                                body_exprs[0].clone()
                            } else {
                                let progn_sym = LispVal::Symbol(env.intern_symbol("PROGN"));
                                vec_to_list([vec![progn_sym], body_exprs].concat())
                            };
                            return make_lambda(params, &final_body, env);
                        }

                        // Case 2: Argument is a symbol bound to a function
                        if let LispVal::Symbol(s) = arg {
                            let func = env.get(&s.borrow().name).ok_or_else(|| {
                                LispError::Generic(format!(
                                    "Undefined function: {}",
                                    s.borrow().name
                                ))
                            })?;

                            match func {
                                LispVal::Lambda(_)
                                | LispVal::Builtin(_)
                                | LispVal::Fexpr(_)
                                | LispVal::Macro(_) => return Ok(func),
                                _ => {
                                    return Err(LispError::Generic(format!(
                                        "Symbol '{}' is not bound to a function",
                                        s.borrow().name
                                    )));
                                }
                            }
                        }

                        Err(LispError::Generic(
                            "FUNCTION argument must be a LAMBDA expression or a symbol bound to a function"
                                .to_string(),
                        ))
                    }
                    "LABEL" => {
                        let args = list_to_vec(rest)?;
                        if args.len() != 2 {
                            return Err(LispError::Generic(
                                "LABEL requires a name and an expression".to_string(),
                            ));
                        }
                        let name_val = &args[0];
                        let expr_val = &args[1];

                        if let LispVal::Symbol(name_sym) = name_val {
                            let new_env = Environment::new_child(env);
                            let label_expr = LispVal::Cons {
                                car: Box::new(LispVal::Symbol(env.intern_symbol("LABEL"))),
                                cdr: rest.clone(),
                            };
                            new_env.set(name_sym.borrow().name.clone(), label_expr);
                            eval(expr_val, &new_env)
                        } else {
                            Err(LispError::Generic(
                                "LABEL name must be a symbol".to_string(),
                            ))
                        }
                    }
                    "DEFINE" => {
                        let defs = list_to_vec(rest)?;
                        if defs.len() != 1 {
                            return Err(LispError::Generic(
                                "DEFINE takes a list of definitions".to_string(),
                            ));
                        }
                        let def_list = list_to_vec(&defs[0])?;
                        let mut defined_names = vec![];
                        for def in def_list {
                            let def_pair = list_to_vec(&def)?;
                            if def_pair.len() != 2 {
                                return Err(LispError::Generic(
                                    "Each definition must be a pair of name and value".to_string(),
                                ));
                            }
                            if let LispVal::Symbol(s) = &def_pair[0] {
                                let name = s.borrow().name.clone();
                                let val = &def_pair[1];
                                env.set(name, val.clone());
                                defined_names.push(LispVal::Symbol(s.clone()));
                            } else {
                                return Err(LispError::Generic(
                                    "Definition name must be a symbol".to_string(),
                                ));
                            }
                        }
                        Ok(vec_to_list(defined_names))
                    }
                    "DEFEXPR" | "DEFMACRO" => {
                        let args = list_to_vec(rest)?;
                        if args.len() < 3 || args.len() > 4 {
                            return Err(LispError::Generic(
                                format!("{} takes three or four arguments", s.borrow().name)
                                    .to_string(),
                            ));
                        }
                        if let LispVal::Symbol(name_sym) = &args[0] {
                            let params = &args[1];
                            let mut body_idx = 2;
                            if args.len() == 4 {
                                if let LispVal::String(doc) = &args[2] {
                                    name_sym.borrow_mut().plist.insert(
                                        "docstring".to_string(),
                                        LispVal::String(doc.clone()),
                                    );
                                    body_idx = 3;
                                } else {
                                    return Err(LispError::Generic(
                                        "docstring must be a string".to_string(),
                                    ));
                                }
                            }
                            let body = &args[body_idx];
                            let func = if s.borrow().name == "DEFEXPR" {
                                make_fexpr(params, body, env)?
                            } else {
                                make_macro(params, body, env)?
                            };
                            env.set(name_sym.borrow().name.clone(), func);
                            Ok(LispVal::Symbol(name_sym.clone()))
                        } else {
                            Err(LispError::Generic(
                                format!(
                                    "{} requires a symbol as its first argument",
                                    s.borrow().name
                                )
                                .to_string(),
                            ))
                        }
                    }
                    "PROGN" => {
                        let mut last_val = LispVal::Nil;
                        let mut current = &**rest;
                        while let LispVal::Cons { car, cdr } = current {
                            last_val = eval(car, env)?;
                            current = cdr;
                        }
                        Ok(last_val)
                    }
                    "SETQ" => {
                        let args_vec = list_to_vec(rest)?;
                        if args_vec.len() % 2 != 0 {
                            return Err(LispError::Generic(
                                "SETQ requires an even number of arguments".to_string(),
                            ));
                        }
                        let mut last_val = LispVal::Nil;
                        for chunk in args_vec.chunks(2) {
                            let var = &chunk[0];
                            let val_expr = &chunk[1];
                            if let LispVal::Symbol(s) = var {
                                let val = eval(val_expr, env)?;
                                Environment::update(env, &s.borrow().name, val.clone());
                                last_val = val;
                            } else {
                                return Err(LispError::Generic(
                                    "SETQ variable name must be a symbol".to_string(),
                                ));
                            }
                        }
                        Ok(last_val)
                    }
                    "PROG" => {
                        let args = list_to_vec(rest)?;
                        if args.is_empty() {
                            return Err(LispError::Generic(
                                "PROG requires at least a var list".to_string(),
                            ));
                        }

                        let var_list = list_to_vec(&args[0])?;
                        let body = &args[1..];

                        let prog_env = Environment::new_child(env);

                        for var in var_list {
                            if let LispVal::Symbol(s) = var {
                                prog_env.set(s.borrow().name.clone(), LispVal::Nil);
                            } else {
                                return Err(LispError::Generic(
                                    "PROG variable list must contain only symbols".to_string(),
                                ));
                            }
                        }

                        let mut labels = HashMap::new();
                        for (i, item) in body.iter().enumerate() {
                            if let LispVal::Symbol(s) = item {
                                labels.insert(s.borrow().name.clone(), i);
                            }
                        }

                        let mut pc = 0;
                        loop {
                            if pc >= body.len() {
                                break Ok(LispVal::Nil); // Fell off the end
                            }

                            let item = &body[pc];

                            // If it's a label, just skip it.
                            if let LispVal::Symbol(_) = item {
                                pc += 1;
                                continue;
                            }

                            match eval(item, &prog_env) {
                                Ok(_) => {
                                    pc += 1;
                                }
                                Err(LispError::Return(val)) => {
                                    break Ok(*val);
                                }
                                Err(LispError::Go(label)) => {
                                    if let Some(new_pc) = labels.get(&label) {
                                        pc = *new_pc;
                                    } else {
                                        break Err(LispError::Generic(format!(
                                            "GO: label not found in PROG: {label}"
                                        )));
                                    }
                                }
                                Err(e) => {
                                    break Err(e);
                                }
                            }
                        }
                    }
                    "RETURN" => {
                        let args = list_to_vec(rest)?;
                        if args.len() != 1 {
                            return Err(LispError::Generic(
                                "RETURN takes exactly one argument".to_string(),
                            ));
                        }
                        let retval = eval(&args[0], env)?;
                        Err(LispError::Return(Box::new(retval)))
                    }
                    "GO" => {
                        let args = list_to_vec(rest)?;
                        if args.len() != 1 {
                            return Err(LispError::Generic(
                                "GO takes exactly one argument".to_string(),
                            ));
                        }
                        if let LispVal::Symbol(s) = &args[0] {
                            Err(LispError::Go(s.borrow().name.clone()))
                        } else {
                            Err(LispError::Generic(
                                "GO argument must be a symbol".to_string(),
                            ))
                        }
                    }
                    "LET" => {
                        let args = list_to_vec(rest)?;
                        if args.len() != 2 {
                            return Err(LispError::Generic(
                                "let takes exactly two arguments".to_string(),
                            ));
                        }
                        let bindings_vec = list_to_vec(&args[0])?;
                        let body = &args[1];

                        let mut params = vec![];
                        let mut arg_exprs = vec![];
                        for binding in bindings_vec {
                            let pair = list_to_vec(&binding)?;
                            if pair.len() != 2 {
                                return Err(LispError::Generic(
                                    "let binding must be a pair".to_string(),
                                ));
                            }
                            params.push(pair[0].clone());
                            arg_exprs.push(pair[1].clone());
                        }

                        let lambda = make_lambda(&vec_to_list(params), body, env)?;
                        let mut application = vec![lambda];
                        application.extend(arg_exprs);
                        eval(&vec_to_list(application), env)
                    }
                    _ => {
                        // Function call
                        let func = eval(first, env)?;
                        let args_list = list_to_vec(rest)?;
                        if let LispVal::Macro(m) = &func {
                            let expanded = expand_macro(m, &args_list, env)?;
                            return eval(&expanded, env);
                        }
                        if let LispVal::Fexpr(fexpr) = &func {
                            if fexpr.params.len() != 1 {
                                return Err(LispError::Generic("fexpr must have exactly one parameter for the list of arguments".to_string()));
                            }
                            let new_env = Environment::new_child(&fexpr.env);
                            new_env.set(fexpr.params[0].clone(), *rest.clone());
                            return eval(&fexpr.body, &new_env);
                        }

                        let eval_args: Result<Vec<LispVal>, LispError> =
                            args_list.iter().map(|arg| eval(arg, env)).collect();
                        apply(&func, &eval_args?, env)
                    }
                }
            } else {
                let func = eval(first, env)?;
                let args_list = list_to_vec(rest)?;
                let eval_args: Result<Vec<LispVal>, LispError> =
                    args_list.iter().map(|arg| eval(arg, env)).collect();
                apply(&func, &eval_args?, env)
            }
        }
    }
}

fn quasiquote_eval(val: &LispVal, env: &Rc<Environment>) -> Result<LispVal, LispError> {
    if let LispVal::Cons { car, cdr } = val {
        if let LispVal::Symbol(s) = &**car
            && s.borrow().name == "UNQUOTE"
        {
            if let LispVal::Cons {
                car: unquoted_val,
                cdr: rest,
            } = &**cdr
                && **rest == LispVal::Nil
            {
                return eval(unquoted_val, env);
            }
            return Err(LispError::Generic(
                "unquote takes exactly one argument".to_string(),
            ));
        }
        let car_eval = quasiquote_eval(car, env)?;
        let cdr_eval = quasiquote_eval(cdr, env)?;
        Ok(LispVal::Cons {
            car: Box::new(car_eval),
            cdr: Box::new(cdr_eval),
        })
    } else {
        Ok(val.clone())
    }
}

fn apply_symbol_op(
    op: &BuiltinFunc,
    args: &[LispVal],
    env: &Rc<Environment>,
) -> Result<LispVal, LispError> {
    match op {
        BuiltinFunc::GetP => {
            if args.len() != 2 {
                return Err(LispError::Generic(
                    "get-p takes exactly two arguments".to_string(),
                ));
            }
            if let LispVal::Symbol(s) = &args[0] {
                if let LispVal::String(prop) = &args[1] {
                    if let Some(val) = s.borrow().plist.get(prop) {
                        Ok(val.clone())
                    } else {
                        Ok(LispVal::Nil)
                    }
                } else {
                    Err(LispError::Generic(
                        "get-p requires a string as its second argument".to_string(),
                    ))
                }
            } else {
                Err(LispError::Generic(
                    "get-p requires a symbol as its first argument".to_string(),
                ))
            }
        }
        BuiltinFunc::PutP => {
            if args.len() != 3 {
                return Err(LispError::Generic(
                    "put-p takes exactly three arguments".to_string(),
                ));
            }
            if let LispVal::Symbol(s) = &args[0] {
                if let LispVal::String(prop) = &args[1] {
                    let val = args[2].clone();
                    s.borrow_mut().plist.insert(prop.clone(), val);
                    Ok(LispVal::Symbol(env.intern_symbol("T")))
                } else {
                    Err(LispError::Generic(
                        "put-p requires a string as its second argument".to_string(),
                    ))
                }
            } else {
                Err(LispError::Generic(
                    "put-p requires a symbol as its first argument".to_string(),
                ))
            }
        }
        _ => Err(LispError::Generic("Not a symbol operation".to_string())),
    }
}

// I/O operations
fn apply_io_op(
    op: &BuiltinFunc,
    args: &[LispVal],
    env: &Rc<Environment>,
) -> Result<LispVal, LispError> {
    match op {
        BuiltinFunc::Read => {
            if !args.is_empty() {
                return Err(LispError::Generic("read takes no arguments".to_string()));
            }
            use std::io::{self, BufRead};
            let stdin = io::stdin();
            let mut line = String::new();
            stdin
                .lock()
                .read_line(&mut line)
                .map_err(|e| LispError::Generic(format!("Failed to read input: {}", e)))?;
            crate::reader::read(&line, env)
                .map_err(|e| LispError::Generic(format!("Failed to parse input: {}", e)))
        }
        BuiltinFunc::Prin1 => {
            if args.len() != 1 {
                return Err(LispError::Generic(
                    "prin1 requires exactly one argument".to_string(),
                ));
            }
            print!("{}", crate::printer::print(&args[0]));
            use std::io::{self, Write};
            io::stdout()
                .flush()
                .map_err(|e| LispError::Generic(format!("Failed to flush output: {}", e)))?;
            Ok(args[0].clone())
        }
        BuiltinFunc::Princ => {
            if args.len() != 1 {
                return Err(LispError::Generic(
                    "princ requires exactly one argument".to_string(),
                ));
            }
            let output = match &args[0] {
                LispVal::String(s) => s.clone(),
                other => crate::printer::print(other),
            };
            print!("{}", output);
            use std::io::{self, Write};
            io::stdout()
                .flush()
                .map_err(|e| LispError::Generic(format!("Failed to flush output: {}", e)))?;
            Ok(args[0].clone())
        }
        BuiltinFunc::Terpri => {
            if !args.is_empty() {
                return Err(LispError::Generic("terpri takes no arguments".to_string()));
            }
            println!();
            Ok(LispVal::Nil)
        }
        _ => Err(LispError::Generic("Not an I/O operation".to_string())),
    }
}

// Error handling operations
fn apply_error_op(
    op: &BuiltinFunc,
    args: &[LispVal],
    env: &Rc<Environment>,
) -> Result<LispVal, LispError> {
    match op {
        BuiltinFunc::Error => {
            if args.is_empty() {
                return Err(LispError::Generic("Error".to_string()));
            }
            let msg = if let LispVal::String(s) = &args[0] {
                s.clone()
            } else {
                crate::printer::print(&args[0])
            };
            Err(LispError::Generic(msg))
        }
        BuiltinFunc::Errorset => {
            if args.len() != 1 && args.len() != 2 {
                return Err(LispError::Generic(
                    "errorset requires one or two arguments".to_string(),
                ));
            }
            let form = &args[0];
            match eval(form, env) {
                Ok(result) => Ok(vec_to_list(vec![result])),
                Err(_) => Ok(LispVal::Nil),
            }
        }
        _ => Err(LispError::Generic(
            "Not an error handling operation".to_string(),
        )),
    }
}

// List processing operations
fn apply_list_processing(
    op: &BuiltinFunc,
    args: &[LispVal],
    env: &Rc<Environment>,
) -> Result<LispVal, LispError> {
    match op {
        BuiltinFunc::Subst => {
            if args.len() != 3 {
                return Err(LispError::Generic(
                    "subst requires exactly three arguments".to_string(),
                ));
            }
            let new_val = &args[0];
            let old_val = &args[1];
            let tree = &args[2];
            fn subst_helper(new: &LispVal, old: &LispVal, tree: &LispVal) -> LispVal {
                if tree == old {
                    new.clone()
                } else if let LispVal::Cons { car, cdr } = tree {
                    LispVal::Cons {
                        car: Box::new(subst_helper(new, old, car)),
                        cdr: Box::new(subst_helper(new, old, cdr)),
                    }
                } else {
                    tree.clone()
                }
            }
            Ok(subst_helper(new_val, old_val, tree))
        }
        BuiltinFunc::Assoc => {
            if args.len() != 2 {
                return Err(LispError::Generic(
                    "assoc requires exactly two arguments".to_string(),
                ));
            }
            let key = &args[0];
            let mut alist = &args[1];
            while let LispVal::Cons { car, cdr } = alist {
                if let LispVal::Cons {
                    car: pair_car,
                    cdr: _,
                } = &**car
                {
                    if **pair_car == *key {
                        return Ok(*car.clone());
                    }
                }
                alist = cdr;
            }
            Ok(LispVal::Nil)
        }
        BuiltinFunc::Maplist => {
            if args.len() != 2 {
                return Err(LispError::Generic(
                    "maplist requires exactly two arguments".to_string(),
                ));
            }
            let list = &args[0];
            let func = &args[1];
            let mut result = Vec::new();
            let mut current = list.clone();
            while let LispVal::Cons { car: _, cdr } = &current {
                let applied = apply(func, &[current.clone()], env)?;
                result.push(applied);
                current = *cdr.clone();
            }
            Ok(vec_to_list(result))
        }
        BuiltinFunc::Mapcar => {
            if args.len() != 2 {
                return Err(LispError::Generic(
                    "mapcar requires exactly two arguments".to_string(),
                ));
            }
            let list = &args[0];
            let func = &args[1];
            let mut result = Vec::new();
            let mut current = list;
            while let LispVal::Cons { car, cdr } = current {
                let applied = apply(func, &[*car.clone()], env)?;
                result.push(applied);
                current = cdr;
            }
            Ok(vec_to_list(result))
        }
        BuiltinFunc::Rplaca => {
            if args.len() != 2 {
                return Err(LispError::Generic(
                    "rplaca requires exactly two arguments".to_string(),
                ));
            }
            if let LispVal::Cons { car: _, cdr } = &args[0] {
                Ok(LispVal::Cons {
                    car: Box::new(args[1].clone()),
                    cdr: cdr.clone(),
                })
            } else {
                Err(LispError::Generic(
                    "rplaca requires a cons cell as its first argument".to_string(),
                ))
            }
        }
        BuiltinFunc::Rplacd => {
            if args.len() != 2 {
                return Err(LispError::Generic(
                    "rplacd requires exactly two arguments".to_string(),
                ));
            }
            if let LispVal::Cons { car, cdr: _ } = &args[0] {
                Ok(LispVal::Cons {
                    car: car.clone(),
                    cdr: Box::new(args[1].clone()),
                })
            } else {
                Err(LispError::Generic(
                    "rplacd requires a cons cell as its first argument".to_string(),
                ))
            }
        }
        _ => Err(LispError::Generic(
            "Not a list processing operation".to_string(),
        )),
    }
}

// Bitwise operations
fn apply_bitwise_op(
    op: &BuiltinFunc,
    args: &[LispVal],
    _env: &Rc<Environment>,
) -> Result<LispVal, LispError> {
    match op {
        BuiltinFunc::Logor => {
            let mut result = 0i64;
            for arg in args {
                if let LispVal::Number(n) = arg {
                    result |= n;
                } else {
                    return Err(LispError::Generic(
                        "logor requires integer arguments".to_string(),
                    ));
                }
            }
            Ok(LispVal::Number(result))
        }
        BuiltinFunc::Logand => {
            if args.is_empty() {
                return Ok(LispVal::Number(-1));
            }
            let mut result = if let LispVal::Number(n) = &args[0] {
                *n
            } else {
                return Err(LispError::Generic(
                    "logand requires integer arguments".to_string(),
                ));
            };
            for arg in &args[1..] {
                if let LispVal::Number(n) = arg {
                    result &= n;
                } else {
                    return Err(LispError::Generic(
                        "logand requires integer arguments".to_string(),
                    ));
                }
            }
            Ok(LispVal::Number(result))
        }
        BuiltinFunc::Logxor => {
            let mut result = 0i64;
            for arg in args {
                if let LispVal::Number(n) = arg {
                    result ^= n;
                } else {
                    return Err(LispError::Generic(
                        "logxor requires integer arguments".to_string(),
                    ));
                }
            }
            Ok(LispVal::Number(result))
        }
        BuiltinFunc::Leftshift => {
            if args.len() != 2 {
                return Err(LispError::Generic(
                    "leftshift requires exactly two arguments".to_string(),
                ));
            }
            if let (LispVal::Number(n), LispVal::Number(shift)) = (&args[0], &args[1]) {
                if *shift < 0 {
                    Ok(LispVal::Number(n >> (-shift)))
                } else {
                    Ok(LispVal::Number(n << shift))
                }
            } else {
                Err(LispError::Generic(
                    "leftshift requires integer arguments".to_string(),
                ))
            }
        }
        _ => Err(LispError::Generic("Not a bitwise operation".to_string())),
    }
}

// New list operations
fn apply_new_list_ops(
    op: &BuiltinFunc,
    args: &[LispVal],
    _env: &Rc<Environment>,
) -> Result<LispVal, LispError> {
    match op {
        BuiltinFunc::List => Ok(vec_to_list(args.to_vec())),
        BuiltinFunc::Last => {
            if args.len() != 1 {
                return Err(LispError::Generic(
                    "last requires exactly one argument".to_string(),
                ));
            }
            let mut current = &args[0];
            while let LispVal::Cons { car: _, cdr } = current {
                if **cdr == LispVal::Nil {
                    return Ok(current.clone());
                }
                current = cdr;
            }
            Ok(LispVal::Nil)
        }
        BuiltinFunc::Nth => {
            if args.len() != 2 {
                return Err(LispError::Generic(
                    "nth requires exactly two arguments".to_string(),
                ));
            }
            let n = if let LispVal::Number(n) = &args[0] {
                *n as usize
            } else {
                return Err(LispError::Generic(
                    "nth requires a number as first argument".to_string(),
                ));
            };
            let mut current = &args[1];
            for _ in 0..n {
                if let LispVal::Cons { car: _, cdr } = current {
                    current = cdr;
                } else {
                    return Ok(LispVal::Nil);
                }
            }
            if let LispVal::Cons { car, cdr: _ } = current {
                Ok(*car.clone())
            } else {
                Ok(LispVal::Nil)
            }
        }
        BuiltinFunc::Nthcdr => {
            if args.len() != 2 {
                return Err(LispError::Generic(
                    "nthcdr requires exactly two arguments".to_string(),
                ));
            }
            let n = if let LispVal::Number(n) = &args[0] {
                *n as usize
            } else {
                return Err(LispError::Generic(
                    "nthcdr requires a number as first argument".to_string(),
                ));
            };
            let mut current = args[1].clone();
            for _ in 0..n {
                if let LispVal::Cons { car: _, cdr } = current {
                    current = *cdr;
                } else {
                    return Ok(LispVal::Nil);
                }
            }
            Ok(current)
        }
        BuiltinFunc::Efface => {
            if args.len() != 2 {
                return Err(LispError::Generic(
                    "efface requires exactly two arguments".to_string(),
                ));
            }
            let item = &args[0];
            let list = &args[1];

            // Build a new list without the first occurrence of item
            let items = list_to_vec(list)?;
            let mut found = false;
            let result: Vec<LispVal> = items
                .into_iter()
                .filter(|x| {
                    if !found && x == item {
                        found = true;
                        false
                    } else {
                        true
                    }
                })
                .collect();
            Ok(vec_to_list(result))
        }
        _ => Err(LispError::Generic("Not a list operation".to_string())),
    }
}

// New numeric operations
fn apply_new_numeric_ops(
    op: &BuiltinFunc,
    args: &[LispVal],
    env: &Rc<Environment>,
) -> Result<LispVal, LispError> {
    match op {
        BuiltinFunc::Mod => {
            if args.len() != 2 {
                return Err(LispError::Generic(
                    "mod requires exactly two arguments".to_string(),
                ));
            }
            if let (LispVal::Number(x), LispVal::Number(y)) = (&args[0], &args[1]) {
                if *y == 0 {
                    return Err(LispError::Generic("Division by zero".to_string()));
                }
                // MOD uses floored division (different from remainder for negative numbers)
                Ok(LispVal::Number(x.rem_euclid(*y)))
            } else {
                Err(LispError::Generic(
                    "mod requires integer arguments".to_string(),
                ))
            }
        }
        BuiltinFunc::Plusp => {
            if args.len() != 1 {
                return Err(LispError::Generic(
                    "plusp requires exactly one argument".to_string(),
                ));
            }
            match &args[0] {
                LispVal::Number(n) => {
                    if *n > 0 {
                        Ok(LispVal::Symbol(env.intern_symbol("T")))
                    } else {
                        Ok(LispVal::Nil)
                    }
                }
                LispVal::Float(f) => {
                    if *f > 0.0 {
                        Ok(LispVal::Symbol(env.intern_symbol("T")))
                    } else {
                        Ok(LispVal::Nil)
                    }
                }
                _ => Err(LispError::Generic("plusp requires a number".to_string())),
            }
        }
        BuiltinFunc::Evenp => {
            if args.len() != 1 {
                return Err(LispError::Generic(
                    "evenp requires exactly one argument".to_string(),
                ));
            }
            if let LispVal::Number(n) = &args[0] {
                if n % 2 == 0 {
                    Ok(LispVal::Symbol(env.intern_symbol("T")))
                } else {
                    Ok(LispVal::Nil)
                }
            } else {
                Err(LispError::Generic("evenp requires an integer".to_string()))
            }
        }
        BuiltinFunc::Oddp => {
            if args.len() != 1 {
                return Err(LispError::Generic(
                    "oddp requires exactly one argument".to_string(),
                ));
            }
            if let LispVal::Number(n) = &args[0] {
                if n % 2 != 0 {
                    Ok(LispVal::Symbol(env.intern_symbol("T")))
                } else {
                    Ok(LispVal::Nil)
                }
            } else {
                Err(LispError::Generic("oddp requires an integer".to_string()))
            }
        }
        BuiltinFunc::Add1 => {
            if args.len() != 1 {
                return Err(LispError::Generic(
                    "add1 requires exactly one argument".to_string(),
                ));
            }
            match &args[0] {
                LispVal::Number(n) => Ok(LispVal::Number(n + 1)),
                LispVal::Float(f) => Ok(LispVal::Float(f + 1.0)),
                _ => Err(LispError::Generic("add1 requires a number".to_string())),
            }
        }
        BuiltinFunc::Sub1 => {
            if args.len() != 1 {
                return Err(LispError::Generic(
                    "sub1 requires exactly one argument".to_string(),
                ));
            }
            match &args[0] {
                LispVal::Number(n) => Ok(LispVal::Number(n - 1)),
                LispVal::Float(f) => Ok(LispVal::Float(f - 1.0)),
                _ => Err(LispError::Generic("sub1 requires a number".to_string())),
            }
        }
        BuiltinFunc::Random => {
            if args.len() != 1 {
                return Err(LispError::Generic(
                    "random requires exactly one argument".to_string(),
                ));
            }
            if let LispVal::Number(n) = &args[0] {
                if *n <= 0 {
                    return Err(LispError::Generic(
                        "random requires a positive integer".to_string(),
                    ));
                }
                // Simple linear congruential generator using system time as seed
                use std::time::{SystemTime, UNIX_EPOCH};
                let seed = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos() as u64;
                let random_val = (seed % (*n as u64)) as i64;
                Ok(LispVal::Number(random_val))
            } else {
                Err(LispError::Generic("random requires an integer".to_string()))
            }
        }
        _ => Err(LispError::Generic("Not a numeric operation".to_string())),
    }
}

// Type predicate operations
fn apply_type_predicates(
    op: &BuiltinFunc,
    args: &[LispVal],
    env: &Rc<Environment>,
) -> Result<LispVal, LispError> {
    if args.len() != 1 {
        return Err(LispError::Generic(
            "Type predicate requires exactly one argument".to_string(),
        ));
    }
    let arg = &args[0];
    let result = match op {
        BuiltinFunc::Symbolp => matches!(arg, LispVal::Symbol(_)),
        BuiltinFunc::Boundp => {
            if let LispVal::Symbol(s) = arg {
                env.is_bound(&s.borrow().name)
            } else {
                return Err(LispError::Generic("boundp requires a symbol".to_string()));
            }
        }
        BuiltinFunc::Functionp => matches!(
            arg,
            LispVal::Lambda(_) | LispVal::Builtin(_) | LispVal::Fexpr(_)
        ),
        BuiltinFunc::Macrop => matches!(arg, LispVal::Macro(_)),
        _ => return Err(LispError::Generic("Not a type predicate".to_string())),
    };
    if result {
        Ok(LispVal::Symbol(env.intern_symbol("T")))
    } else {
        Ok(LispVal::Nil)
    }
}

// Function operations
fn apply_function_ops(
    op: &BuiltinFunc,
    args: &[LispVal],
    env: &Rc<Environment>,
) -> Result<LispVal, LispError> {
    match op {
        BuiltinFunc::Funcall => {
            if args.is_empty() {
                return Err(LispError::Generic(
                    "funcall requires at least one argument".to_string(),
                ));
            }
            // If the first arg is a symbol, look it up to get the function
            let func = match &args[0] {
                LispVal::Symbol(s) => env.get(&s.borrow().name).ok_or_else(|| {
                    LispError::Generic(format!("Function not found: {}", s.borrow().name))
                })?,
                other => other.clone(),
            };
            let func_args = &args[1..];
            apply(&func, func_args, env)
        }
        BuiltinFunc::Macroexpand => {
            if args.len() != 1 {
                return Err(LispError::Generic(
                    "macroexpand requires exactly one argument".to_string(),
                ));
            }
            let form = &args[0];
            if let LispVal::Cons { car, cdr } = form
                && let LispVal::Symbol(s) = &**car
                && let Some(LispVal::Macro(m)) = env.get(&s.borrow().name)
            {
                let macro_args = list_to_vec(cdr)?;
                return expand_macro(&m, &macro_args, env);
            }
            // Not a macro call, return as-is
            Ok(form.clone())
        }
        _ => Err(LispError::Generic("Not a function operation".to_string())),
    }
}

// String/Symbol operations
fn apply_string_symbol_ops(
    op: &BuiltinFunc,
    args: &[LispVal],
    env: &Rc<Environment>,
) -> Result<LispVal, LispError> {
    match op {
        BuiltinFunc::Explode => {
            if args.len() != 1 {
                return Err(LispError::Generic(
                    "explode requires exactly one argument".to_string(),
                ));
            }
            let chars: Vec<LispVal> = match &args[0] {
                LispVal::Symbol(s) => s
                    .borrow()
                    .name
                    .chars()
                    .map(|c| LispVal::Symbol(env.intern_symbol(&c.to_string())))
                    .collect(),
                LispVal::String(s) => s
                    .chars()
                    .map(|c| LispVal::Symbol(env.intern_symbol(&c.to_string())))
                    .collect(),
                LispVal::Number(n) => n
                    .to_string()
                    .chars()
                    .map(|c| LispVal::Symbol(env.intern_symbol(&c.to_string())))
                    .collect(),
                _ => {
                    return Err(LispError::Generic(
                        "explode requires a symbol, string, or number".to_string(),
                    ));
                }
            };
            Ok(vec_to_list(chars))
        }
        BuiltinFunc::Implode => {
            if args.len() != 1 {
                return Err(LispError::Generic(
                    "implode requires exactly one argument".to_string(),
                ));
            }
            let chars = list_to_vec(&args[0])?;
            let mut result = String::new();
            for ch in chars {
                match ch {
                    LispVal::Symbol(s) => result.push_str(&s.borrow().name),
                    LispVal::String(s) => result.push_str(&s),
                    _ => {
                        return Err(LispError::Generic(
                            "implode requires a list of symbols or strings".to_string(),
                        ));
                    }
                }
            }
            Ok(LispVal::Symbol(env.intern_symbol(&result)))
        }
        BuiltinFunc::Maknam => {
            // Same as implode in our implementation
            if args.len() != 1 {
                return Err(LispError::Generic(
                    "maknam requires exactly one argument".to_string(),
                ));
            }
            let chars = list_to_vec(&args[0])?;
            let mut result = String::new();
            for ch in chars {
                match ch {
                    LispVal::Symbol(s) => result.push_str(&s.borrow().name),
                    LispVal::String(s) => result.push_str(&s),
                    _ => {
                        return Err(LispError::Generic(
                            "maknam requires a list of symbols or strings".to_string(),
                        ));
                    }
                }
            }
            Ok(LispVal::Symbol(env.intern_symbol(&result)))
        }
        BuiltinFunc::Gensym => {
            if !args.is_empty() {
                return Err(LispError::Generic("gensym takes no arguments".to_string()));
            }
            Ok(LispVal::Symbol(env.gensym()))
        }
        BuiltinFunc::Intern => {
            if args.len() != 1 {
                return Err(LispError::Generic(
                    "intern requires exactly one argument".to_string(),
                ));
            }
            let name = match &args[0] {
                LispVal::String(s) => s.to_uppercase(),
                LispVal::Symbol(s) => s.borrow().name.clone(),
                _ => {
                    return Err(LispError::Generic(
                        "intern requires a string or symbol".to_string(),
                    ));
                }
            };
            Ok(LispVal::Symbol(env.intern_symbol(&name)))
        }
        BuiltinFunc::Plist => {
            if args.len() != 1 {
                return Err(LispError::Generic(
                    "plist requires exactly one argument".to_string(),
                ));
            }
            if let LispVal::Symbol(s) = &args[0] {
                let plist = &s.borrow().plist;
                let mut result = Vec::new();
                for (key, val) in plist.iter() {
                    result.push(LispVal::String(key.clone()));
                    result.push(val.clone());
                }
                Ok(vec_to_list(result))
            } else {
                Err(LispError::Generic(
                    "plist requires a symbol as its argument".to_string(),
                ))
            }
        }
        _ => Err(LispError::Generic(
            "Not a string/symbol operation".to_string(),
        )),
    }
}

// New bitwise operations
fn apply_new_bitwise_ops(
    op: &BuiltinFunc,
    args: &[LispVal],
    _env: &Rc<Environment>,
) -> Result<LispVal, LispError> {
    match op {
        BuiltinFunc::Ash => {
            if args.len() != 2 {
                return Err(LispError::Generic(
                    "ash requires exactly two arguments".to_string(),
                ));
            }
            if let (LispVal::Number(n), LispVal::Number(shift)) = (&args[0], &args[1]) {
                if *shift < 0 {
                    Ok(LispVal::Number(n >> (-shift)))
                } else {
                    Ok(LispVal::Number(n << shift))
                }
            } else {
                Err(LispError::Generic(
                    "ash requires integer arguments".to_string(),
                ))
            }
        }
        BuiltinFunc::Lognot => {
            if args.len() != 1 {
                return Err(LispError::Generic(
                    "lognot requires exactly one argument".to_string(),
                ));
            }
            if let LispVal::Number(n) = &args[0] {
                Ok(LispVal::Number(!n))
            } else {
                Err(LispError::Generic(
                    "lognot requires an integer argument".to_string(),
                ))
            }
        }
        BuiltinFunc::Rot => {
            if args.len() != 2 {
                return Err(LispError::Generic(
                    "rot requires exactly two arguments".to_string(),
                ));
            }
            if let (LispVal::Number(n), LispVal::Number(count)) = (&args[0], &args[1]) {
                // Rotate within 64 bits
                let bits = 64;
                let count = count.rem_euclid(bits);
                if count >= 0 {
                    let count = count as u32;
                    Ok(LispVal::Number(((*n as u64).rotate_left(count)) as i64))
                } else {
                    let count = (-count) as u32;
                    Ok(LispVal::Number(((*n as u64).rotate_right(count)) as i64))
                }
            } else {
                Err(LispError::Generic(
                    "rot requires integer arguments".to_string(),
                ))
            }
        }
        _ => Err(LispError::Generic("Not a bitwise operation".to_string())),
    }
}

// Property list operations
fn apply_plist_op(
    op: &BuiltinFunc,
    args: &[LispVal],
    env: &Rc<Environment>,
) -> Result<LispVal, LispError> {
    match op {
        BuiltinFunc::Remprop => {
            if args.len() != 2 {
                return Err(LispError::Generic(
                    "remprop requires exactly two arguments".to_string(),
                ));
            }
            if let LispVal::Symbol(s) = &args[0] {
                if let LispVal::String(prop) = &args[1] {
                    let removed = s.borrow_mut().plist.remove(prop);
                    if removed.is_some() {
                        Ok(LispVal::Symbol(env.intern_symbol("T")))
                    } else {
                        Ok(LispVal::Nil)
                    }
                } else {
                    Err(LispError::Generic(
                        "remprop requires a string as its second argument".to_string(),
                    ))
                }
            } else {
                Err(LispError::Generic(
                    "remprop requires a symbol as its first argument".to_string(),
                ))
            }
        }
        BuiltinFunc::Deflist => {
            if args.len() != 2 {
                return Err(LispError::Generic(
                    "deflist requires exactly two arguments".to_string(),
                ));
            }
            let pairs = &args[0];
            let indicator = if let LispVal::String(s) = &args[1] {
                s.clone()
            } else {
                return Err(LispError::Generic(
                    "deflist requires a string as its second argument".to_string(),
                ));
            };
            let mut current = pairs;
            while let LispVal::Cons { car, cdr } = current {
                if let LispVal::Cons {
                    car: sym,
                    cdr: rest,
                } = &**car
                {
                    if let LispVal::Symbol(s) = &**sym {
                        if let LispVal::Cons { car: val, cdr: _ } = &**rest {
                            s.borrow_mut().plist.insert(indicator.clone(), *val.clone());
                        }
                    }
                }
                current = cdr;
            }
            Ok(LispVal::Symbol(env.intern_symbol("T")))
        }
        _ => Err(LispError::Generic(
            "Not a property list operation".to_string(),
        )),
    }
}
