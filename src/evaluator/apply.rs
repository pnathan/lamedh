use super::*;
use crate::environment::DynamicBinding;
use smallvec::SmallVec;
pub(super) fn apply(
    func: &LispVal,
    args: &[LispVal],
    env: &Shared<Environment>,
) -> Result<LispVal, LispError> {
    match func {
        LispVal::Builtin(builtin) => match builtin {
            BuiltinFunc::Plus
            | BuiltinFunc::Minus
            | BuiltinFunc::Multiply
            | BuiltinFunc::Divide => apply_math_op(builtin, args, env),
            BuiltinFunc::Lessp
            | BuiltinFunc::Greaterp
            | BuiltinFunc::Zerop
            | BuiltinFunc::Remainder
            | BuiltinFunc::Expt => apply_numeric_primitives(builtin, args, env),
            BuiltinFunc::Car | BuiltinFunc::Cdr | BuiltinFunc::Cons => apply_list_op(builtin, args),
            // (append l1 l2 ... tail) — variadic (0.3 regularity), kernel-
            // fast (append is on stdlib hot paths). All but the last
            // argument must be proper lists; the last is shared as the tail
            // (so (append xs) = xs and (append) = ()).
            BuiltinFunc::Append => {
                if args.is_empty() {
                    return Ok(LispVal::Nil);
                }
                let mut items: Vec<LispVal> = Vec::new();
                for a in &args[..args.len() - 1] {
                    let mut cur = a;
                    loop {
                        match cur {
                            LispVal::Nil => break,
                            LispVal::Cons { car, cdr } => {
                                items.push(car.as_ref().clone());
                                cur = cdr.as_ref();
                            }
                            other => {
                                return Err(LispError::Generic(format!(
                                    "APPEND: expected a proper list, got tail {}",
                                    err_val(other)
                                )));
                            }
                        }
                    }
                }
                let mut out = args[args.len() - 1].clone();
                for v in items.into_iter().rev() {
                    out = LispVal::Cons {
                        car: Shared::new(v),
                        cdr: Shared::new(out),
                    };
                }
                Ok(out)
            }
            BuiltinFunc::Concat | BuiltinFunc::Index => apply_string_op(builtin, args),
            BuiltinFunc::Sort => apply_sort(args, env),
            BuiltinFunc::Sqrt
            | BuiltinFunc::Sin
            | BuiltinFunc::Cos
            | BuiltinFunc::Tan
            | BuiltinFunc::Log
            | BuiltinFunc::Exp
            | BuiltinFunc::Floor
            | BuiltinFunc::Ceiling
            | BuiltinFunc::Round
            | BuiltinFunc::Truncate
            | BuiltinFunc::Gcd
            | BuiltinFunc::Lcm
            | BuiltinFunc::Isqrt
            | BuiltinFunc::Signum => apply_math_lib(builtin, args, env),
            BuiltinFunc::StringLength
            | BuiltinFunc::Substring
            | BuiltinFunc::CharCode
            | BuiltinFunc::CodeChar
            | BuiltinFunc::MakeChar
            | BuiltinFunc::StringToNumber
            | BuiltinFunc::NumberToString
            | BuiltinFunc::Prin1ToString
            | BuiltinFunc::PrincToString => apply_string_lib(builtin, args),
            BuiltinFunc::ReadFromString => {
                // (read-from-string "(+ 1 2)") — parse one s-expression into
                // data via the reader (issue #245). Enables Lisp-side tooling
                // (config parsing, code manipulation) without file I/O.
                if args.len() != 1 {
                    return Err(LispError::Generic(
                        "read-from-string takes exactly one argument".to_string(),
                    ));
                }
                match &args[0] {
                    LispVal::String(s) => {
                        crate::reader::read_with_depth_limit(s, env, env.reader_depth_limit())
                            .map_err(LispError::Generic)
                    }
                    other => Err(LispError::Generic(format!(
                        "READ-FROM-STRING: expected a string, got {}",
                        err_val(other)
                    ))),
                }
            }
            BuiltinFunc::MakeError
            | BuiltinFunc::ErrorP
            | BuiltinFunc::ErrorMessage
            | BuiltinFunc::ErrorData => apply_error_value_op(builtin, args, env),
            BuiltinFunc::Evlis => {
                // evlis[m;a] — evaluate each element of m in environment a
                let (list, eval_env) = match args.len() {
                    1 => (&args[0], env.clone()),
                    2 => {
                        if let LispVal::Environment(e) = &args[1] {
                            (&args[0], e.clone())
                        } else {
                            return Err(LispError::Generic(format!(
                                "EVLIS: second argument must be an environment, got {}",
                                err_val(&args[1])
                            )));
                        }
                    }
                    _ => {
                        return Err(LispError::Generic(
                            "evlis takes 1 or 2 arguments".to_string(),
                        ));
                    }
                };
                let mut result = vec![];
                for form in list_to_vec(list)? {
                    result.push(eval(&form, &eval_env)?);
                }
                let mut out = LispVal::Nil;
                for v in result.into_iter().rev() {
                    out = LispVal::Cons {
                        car: Shared::new(v),
                        cdr: Shared::new(out),
                    };
                }
                Ok(out)
            }
            BuiltinFunc::Evcon => {
                // evcon[c;a] — evaluate clauses until one passes, return its value
                // Clauses: ((test value) ...) evaluated in env a
                let (clauses, eval_env) = match args.len() {
                    1 => (&args[0], env.clone()),
                    2 => {
                        if let LispVal::Environment(e) = &args[1] {
                            (&args[0], e.clone())
                        } else {
                            return Err(LispError::Generic(format!(
                                "EVCON: second argument must be an environment, got {}",
                                err_val(&args[1])
                            )));
                        }
                    }
                    _ => {
                        return Err(LispError::Generic(
                            "evcon takes 1 or 2 arguments".to_string(),
                        ));
                    }
                };
                let mut cur = clauses.clone();
                loop {
                    match cur {
                        LispVal::Nil => return Ok(LispVal::Nil),
                        LispVal::Cons { car, cdr } => {
                            let clause = list_to_vec(&car)?;
                            if clause.len() != 2 {
                                return Err(LispError::Generic(format!(
                                    "EVCON: each clause must be (test value), got {}",
                                    err_val(&car)
                                )));
                            }
                            let test = eval(&clause[0], &eval_env)?;
                            if test != LispVal::Nil {
                                return eval(&clause[1], &eval_env);
                            }
                            cur = cdr.as_ref().clone();
                        }
                        other => {
                            return Err(LispError::Generic(format!(
                                "EVCON: clauses must be a proper list, got tail {}",
                                err_val(&other)
                            )));
                        }
                    }
                }
            }
            BuiltinFunc::Eval => match args.len() {
                1 => eval(&args[0], env),
                2 => {
                    if let LispVal::Environment(eval_env) = &args[1] {
                        eval(&args[0], eval_env)
                    } else {
                        Err(LispError::Generic(format!(
                            "EVAL: second argument must be an environment, got {}",
                            err_val(&args[1])
                        )))
                    }
                }
                _ => Err(LispError::Generic(
                    "eval takes 1 or 2 arguments".to_string(),
                )),
            },
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
            BuiltinFunc::Read
            | BuiltinFunc::Prin1
            | BuiltinFunc::Princ
            | BuiltinFunc::Terpri
            | BuiltinFunc::Spaces => apply_io_op(builtin, args, env),

            // Process control: terminate with an optional exit code.  This is
            // deliberately not capability-gated — ending the process is not an
            // escape from the sandbox, and scripts/CI need it to report status
            // (issue #241).
            BuiltinFunc::Exit => {
                let code = match args {
                    [] => 0,
                    [LispVal::Number(n)] => *n as i32,
                    _ => {
                        return Err(LispError::Generic(
                            "exit takes an optional integer exit code".to_string(),
                        ));
                    }
                };
                use std::io::Write;
                let _ = std::io::stdout().flush();
                std::process::exit(code);
            }

            // Error handling
            BuiltinFunc::Error | BuiltinFunc::Errorset => apply_error_op(builtin, args, env),

            // List processing
            BuiltinFunc::Subst
            | BuiltinFunc::Sublis
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
            BuiltinFunc::Charp => {
                if args.len() != 1 {
                    return Err(LispError::Generic(
                        "charp requires exactly one argument".to_string(),
                    ));
                }
                match &args[0] {
                    LispVal::Char(_) => Ok(LispVal::Symbol(env.intern_symbol("T"))),
                    _ => Ok(LispVal::Nil),
                }
            }
            BuiltinFunc::HashTablep => {
                if args.len() != 1 {
                    return Err(LispError::Generic(
                        "hash-table-p requires exactly one argument".to_string(),
                    ));
                }
                match &args[0] {
                    LispVal::HashTable(_) => Ok(LispVal::Symbol(env.intern_symbol("T"))),
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
            | BuiltinFunc::Macrop
            | BuiltinFunc::Arrayp
            | BuiltinFunc::Extensionp => apply_type_predicates(builtin, args, env),
            BuiltinFunc::ExtensionTypeName => {
                if args.len() != 1 {
                    return Err(LispError::Generic(
                        "extension-type takes exactly one argument".to_string(),
                    ));
                }
                if let LispVal::Extension(e) = &args[0] {
                    Ok(LispVal::String(e.type_name().to_string()))
                } else {
                    Err(LispError::Generic(format!(
                        "EXTENSION-TYPE: argument must be an extension value, got {}",
                        err_val(&args[0])
                    )))
                }
            }

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
            | BuiltinFunc::Random
            | BuiltinFunc::RandomSeed => apply_new_numeric_ops(builtin, args, env),

            // New bitwise operations
            BuiltinFunc::Ash | BuiltinFunc::Lognot | BuiltinFunc::Rot => {
                apply_new_bitwise_ops(builtin, args, env)
            }

            // Function operations
            BuiltinFunc::Funcall | BuiltinFunc::Macroexpand => {
                apply_function_ops(builtin, args, env)
            }

            BuiltinFunc::RecordRef | BuiltinFunc::RecordWith => {
                apply_record_field_op(builtin, args, env)
            }
            BuiltinFunc::RecordDeclare
            | BuiltinFunc::RecordNew
            | BuiltinFunc::RecordBrand
            | BuiltinFunc::RecordCompiledP
            | BuiltinFunc::RecordFields
            | BuiltinFunc::VariantDeclare => apply_record_type_op(builtin, args, env),
            // (set sym val) — set SYM's GLOBAL value; both arguments
            // evaluated (Lisp 1.5 / CL SET). The value-level twin of the
            // quoting CSET macro, for computed symbols.
            BuiltinFunc::SetValue => {
                if args.len() != 2 {
                    return Err(LispError::Generic(
                        "set requires exactly two arguments: symbol value".to_string(),
                    ));
                }
                let LispVal::Symbol(s) = &args[0] else {
                    return Err(LispError::Generic(format!(
                        "SET: first argument must be a symbol, got {}",
                        err_val(&args[0])
                    )));
                };
                let name = s.borrow().name.clone();
                crate::evaluator::core::check_bindable(&name, "SET")?;
                env.global_set(&name, args[1].clone());
                Ok(args[1].clone())
            }
            // (declare-instance! 'name '(-> (shape ...) ret)) — register one
            // protocol instance scheme (0.3 typed protocols).
            BuiltinFunc::DeclareInstance => {
                if args.len() != 2 {
                    return Err(LispError::Generic(
                        "declare-instance! requires exactly two arguments: name scheme".to_string(),
                    ));
                }
                let LispVal::Symbol(s) = &args[0] else {
                    return Err(LispError::Generic(
                        "DECLARE-INSTANCE!: name must be a symbol".to_string(),
                    ));
                };
                let name = s.borrow().name.clone();
                let rendered = env
                    .jit_declare_instance(&name, &args[1])
                    .map_err(LispError::Generic)?;
                Ok(LispVal::String(rendered))
            }
            // (declare-protocol-dispatch! 'name idx) — which argument
            // position the protocol dispatches on (fn-first protocols
            // like MAP dispatch on 1; the default is 0).
            BuiltinFunc::DeclareProtocolDispatch => {
                if args.len() != 2 {
                    return Err(LispError::Generic(
                        "declare-protocol-dispatch! requires exactly two arguments: name index"
                            .to_string(),
                    ));
                }
                let LispVal::Symbol(s) = &args[0] else {
                    return Err(LispError::Generic(
                        "DECLARE-PROTOCOL-DISPATCH!: name must be a symbol".to_string(),
                    ));
                };
                let LispVal::Number(n) = &args[1] else {
                    return Err(LispError::Generic(
                        "DECLARE-PROTOCOL-DISPATCH!: index must be an integer".to_string(),
                    ));
                };
                if *n < 0 {
                    return Err(LispError::Generic(
                        "DECLARE-PROTOCOL-DISPATCH!: index must be non-negative".to_string(),
                    ));
                }
                let name = s.borrow().name.clone();
                env.jit_declare_protocol_dispatch(&name, *n as usize);
                Ok(args[0].clone())
            }
            // (capability-mask-allows-p 'name) — read-only introspection of
            // the dynamic capability mask (#320): T when no fence is active
            // or the active mask includes NAME. Used by the Lisp layer to
            // attenuate CUSTOM capabilities (which are not env features).
            BuiltinFunc::CapMaskAllowsP => {
                if args.len() != 1 {
                    return Err(LispError::Generic(
                        "capability-mask-allows-p takes exactly one argument".to_string(),
                    ));
                }
                let LispVal::Symbol(sym) = &args[0] else {
                    return Err(LispError::Generic(
                        "CAPABILITY-MASK-ALLOWS-P: argument must be a symbol".to_string(),
                    ));
                };
                let name = sym.borrow().name.clone();
                if crate::evaluator::core::cap_mask_allows(&name) {
                    Ok(LispVal::Symbol(env.intern_symbol("T")))
                } else {
                    Ok(LispVal::Nil)
                }
            }
            // (monotonic-micros) — microseconds since an arbitrary process
            // epoch; the timing primitive under (time ...) in lib/26.
            BuiltinFunc::MonotonicMicros => {
                if !args.is_empty() {
                    return Err(LispError::Generic(
                        "monotonic-micros takes no arguments".to_string(),
                    ));
                }
                static EPOCH: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
                let epoch = EPOCH.get_or_init(std::time::Instant::now);
                Ok(LispVal::Number(epoch.elapsed().as_micros() as i64))
            }
            // (last-backtrace) — the frames of the most recently CAUGHT
            // error (innermost first), as a list of symbols.
            BuiltinFunc::LastBacktrace => {
                if !args.is_empty() {
                    return Err(LispError::Generic(
                        "last-backtrace takes no arguments".to_string(),
                    ));
                }
                Ok(vec_to_list(
                    crate::evaluator::core::bt_last()
                        .into_iter()
                        .map(|n| LispVal::Symbol(env.intern_symbol(&n)))
                        .collect(),
                ))
            }

            // Introspection
            BuiltinFunc::Describe
            | BuiltinFunc::SeeSource
            | BuiltinFunc::SeeType
            | BuiltinFunc::ExplainCompile
            | BuiltinFunc::ReadString
            | BuiltinFunc::DeclareType
            | BuiltinFunc::Disassemble => apply_introspection(builtin, args, env),

            // String/Symbol operations
            BuiltinFunc::Explode
            | BuiltinFunc::Implode
            | BuiltinFunc::Maknam
            | BuiltinFunc::Gensym
            | BuiltinFunc::Intern
            | BuiltinFunc::Plist => apply_string_symbol_ops(builtin, args, env),
            // Float comparisons (handle -0.0 vs 0.0 correctly)
            BuiltinFunc::FloatEqual => {
                if args.len() != 2 {
                    return Err(LispError::Generic(
                        "float= requires exactly two arguments".to_string(),
                    ));
                }
                let f1 = match &args[0] {
                    LispVal::Float(f) => *f,
                    LispVal::Number(n) => *n as f64,
                    _ => {
                        return Err(LispError::Generic(format!(
                            "FLOAT=: expected numeric arguments, got {}",
                            err_val(&args[0])
                        )));
                    }
                };
                let f2 = match &args[1] {
                    LispVal::Float(f) => *f,
                    LispVal::Number(n) => *n as f64,
                    _ => {
                        return Err(LispError::Generic(format!(
                            "FLOAT=: expected numeric arguments, got {}",
                            err_val(&args[1])
                        )));
                    }
                };
                // Use bitwise equality to distinguish -0.0 from 0.0
                if f1.to_bits() == f2.to_bits() {
                    Ok(LispVal::Symbol(env.intern_symbol("T")))
                } else {
                    Ok(LispVal::Nil)
                }
            }

            BuiltinFunc::FloatLessp => {
                if args.len() != 2 {
                    return Err(LispError::Generic(
                        "float< requires exactly two arguments".to_string(),
                    ));
                }
                let f1 = match &args[0] {
                    LispVal::Float(f) => *f,
                    LispVal::Number(n) => *n as f64,
                    _ => {
                        return Err(LispError::Generic(format!(
                            "FLOAT<: expected numeric arguments, got {}",
                            err_val(&args[0])
                        )));
                    }
                };
                let f2 = match &args[1] {
                    LispVal::Float(f) => *f,
                    LispVal::Number(n) => *n as f64,
                    _ => {
                        return Err(LispError::Generic(format!(
                            "FLOAT<: expected numeric arguments, got {}",
                            err_val(&args[1])
                        )));
                    }
                };
                if f1 < f2 {
                    Ok(LispVal::Symbol(env.intern_symbol("T")))
                } else {
                    Ok(LispVal::Nil)
                }
            }

            BuiltinFunc::FloatGreaterp => {
                if args.len() != 2 {
                    return Err(LispError::Generic(
                        "float> requires exactly two arguments".to_string(),
                    ));
                }
                let f1 = match &args[0] {
                    LispVal::Float(f) => *f,
                    LispVal::Number(n) => *n as f64,
                    _ => {
                        return Err(LispError::Generic(format!(
                            "FLOAT>: expected numeric arguments, got {}",
                            err_val(&args[0])
                        )));
                    }
                };
                let f2 = match &args[1] {
                    LispVal::Float(f) => *f,
                    LispVal::Number(n) => *n as f64,
                    _ => {
                        return Err(LispError::Generic(format!(
                            "FLOAT>: expected numeric arguments, got {}",
                            err_val(&args[1])
                        )));
                    }
                };
                if f1 > f2 {
                    Ok(LispVal::Symbol(env.intern_symbol("T")))
                } else {
                    Ok(LispVal::Nil)
                }
            }

            BuiltinFunc::LoadFile => {
                require_read_fs(env)?;
                if args.len() != 1 {
                    return Err(LispError::Generic(
                        "load-file requires exactly one argument".to_string(),
                    ));
                }

                let filename = if let LispVal::String(path) = &args[0] {
                    path.clone()
                } else {
                    return Err(LispError::Generic(format!(
                        "LOAD-FILE: expected a string filename, got {}",
                        err_val(&args[0])
                    )));
                };

                crate::load_file(&filename, env)?;
                Ok(LispVal::Symbol(env.intern_symbol("T")))
            }

            BuiltinFunc::ReadFile => {
                require_read_fs(env)?;
                if args.len() != 1 {
                    return Err(LispError::Generic(
                        "read-file requires exactly one argument".to_string(),
                    ));
                }
                let path = match &args[0] {
                    LispVal::String(s) => s.clone(),
                    _ => {
                        return Err(LispError::Generic(format!(
                            "READ-FILE: path must be a string, got {}",
                            err_val(&args[0])
                        )));
                    }
                };
                let contents = std::fs::read_to_string(&path)
                    .map_err(|e| LispError::Generic(format!("read-file: {e}")))?;
                Ok(LispVal::String(contents))
            }

            BuiltinFunc::ReadFileByte => {
                require_read_fs(env)?;
                if args.len() != 2 {
                    return Err(LispError::Generic(
                        "read-file-byte requires exactly two arguments: path offset".to_string(),
                    ));
                }
                let path = match &args[0] {
                    LispVal::String(s) => s.clone(),
                    _ => {
                        return Err(LispError::Generic(format!(
                            "READ-FILE-BYTE: path must be a string, got {}",
                            err_val(&args[0])
                        )));
                    }
                };
                let offset = match &args[1] {
                    LispVal::Number(n) if *n >= 0 => *n as u64,
                    _ => {
                        return Err(LispError::Generic(format!(
                            "READ-FILE-BYTE: offset must be a non-negative integer, got {}",
                            err_val(&args[1])
                        )));
                    }
                };
                use std::io::{Read, Seek, SeekFrom};
                let mut file = std::fs::File::open(&path)
                    .map_err(|e| LispError::Generic(format!("read-file-byte: {e}")))?;
                file.seek(SeekFrom::Start(offset))
                    .map_err(|e| LispError::Generic(format!("read-file-byte: seek: {e}")))?;
                let mut buf = [0u8; 1];
                let n = file
                    .read(&mut buf)
                    .map_err(|e| LispError::Generic(format!("read-file-byte: {e}")))?;
                if n == 0 {
                    Ok(LispVal::Nil)
                } else {
                    Ok(LispVal::Number(buf[0] as i64))
                }
            }

            BuiltinFunc::ReadFileSection => {
                require_read_fs(env)?;
                if args.len() != 3 {
                    return Err(LispError::Generic(
                        "read-file-section requires exactly three arguments: path offset len"
                            .to_string(),
                    ));
                }
                let path = match &args[0] {
                    LispVal::String(s) => s.clone(),
                    _ => {
                        return Err(LispError::Generic(format!(
                            "READ-FILE-SECTION: path must be a string, got {}",
                            err_val(&args[0])
                        )));
                    }
                };
                let offset = match &args[1] {
                    LispVal::Number(n) if *n >= 0 => *n as u64,
                    _ => {
                        return Err(LispError::Generic(format!(
                            "READ-FILE-SECTION: offset must be a non-negative integer, got {}",
                            err_val(&args[1])
                        )));
                    }
                };
                let len = match &args[2] {
                    LispVal::Number(n) if *n >= 0 => *n as usize,
                    _ => {
                        return Err(LispError::Generic(format!(
                            "READ-FILE-SECTION: len must be a non-negative integer, got {}",
                            err_val(&args[2])
                        )));
                    }
                };
                use std::io::{Read, Seek, SeekFrom};
                let mut file = std::fs::File::open(&path)
                    .map_err(|e| LispError::Generic(format!("read-file-section: {e}")))?;
                file.seek(SeekFrom::Start(offset))
                    .map_err(|e| LispError::Generic(format!("read-file-section: seek: {e}")))?;
                let mut buf = vec![0u8; len];
                let n = file
                    .read(&mut buf)
                    .map_err(|e| LispError::Generic(format!("read-file-section: {e}")))?;
                buf.truncate(n);
                Ok(LispVal::String(String::from_utf8_lossy(&buf).into_owned()))
            }

            BuiltinFunc::WriteFile => {
                require_create_fs(env)?;
                if args.len() != 2 {
                    return Err(LispError::Generic(
                        "write-file requires exactly two arguments: path content".to_string(),
                    ));
                }
                let path = match &args[0] {
                    LispVal::String(s) => s.clone(),
                    _ => {
                        return Err(LispError::Generic(format!(
                            "WRITE-FILE: path must be a string, got {}",
                            err_val(&args[0])
                        )));
                    }
                };
                let content = match &args[1] {
                    LispVal::String(s) => s.clone(),
                    _ => {
                        return Err(LispError::Generic(format!(
                            "WRITE-FILE: content must be a string, got {}",
                            err_val(&args[1])
                        )));
                    }
                };
                std::fs::write(&path, content.as_bytes())
                    .map_err(|e| LispError::Generic(format!("write-file: {e}")))?;
                Ok(LispVal::Symbol(env.intern_symbol("T")))
            }

            // ── File metadata predicates ────────────────────────────────────
            BuiltinFunc::FileExistsP => {
                require_read_fs(env)?;
                if args.len() != 1 {
                    return Err(LispError::Generic(
                        "file-exists-p requires exactly one argument".to_string(),
                    ));
                }
                let path = match &args[0] {
                    LispVal::String(s) => s.clone(),
                    _ => {
                        return Err(LispError::Generic(format!(
                            "FILE-EXISTS-P: path must be a string, got {}",
                            err_val(&args[0])
                        )));
                    }
                };
                if std::path::Path::new(&path).exists() {
                    Ok(LispVal::Symbol(env.intern_symbol("T")))
                } else {
                    Ok(LispVal::Nil)
                }
            }

            BuiltinFunc::DirectoryP => {
                require_read_fs(env)?;
                if args.len() != 1 {
                    return Err(LispError::Generic(
                        "directory-p requires exactly one argument".to_string(),
                    ));
                }
                let path = match &args[0] {
                    LispVal::String(s) => s.clone(),
                    _ => {
                        return Err(LispError::Generic(format!(
                            "DIRECTORY-P: path must be a string, got {}",
                            err_val(&args[0])
                        )));
                    }
                };
                if std::path::Path::new(&path).is_dir() {
                    Ok(LispVal::Symbol(env.intern_symbol("T")))
                } else {
                    Ok(LispVal::Nil)
                }
            }

            BuiltinFunc::FileP => {
                require_read_fs(env)?;
                if args.len() != 1 {
                    return Err(LispError::Generic(
                        "file-p requires exactly one argument".to_string(),
                    ));
                }
                let path = match &args[0] {
                    LispVal::String(s) => s.clone(),
                    _ => {
                        return Err(LispError::Generic(format!(
                            "FILE-P: path must be a string, got {}",
                            err_val(&args[0])
                        )));
                    }
                };
                if std::path::Path::new(&path).is_file() {
                    Ok(LispVal::Symbol(env.intern_symbol("T")))
                } else {
                    Ok(LispVal::Nil)
                }
            }

            BuiltinFunc::FileReadableP => {
                require_read_fs(env)?;
                if args.len() != 1 {
                    return Err(LispError::Generic(
                        "file-readable-p requires exactly one argument".to_string(),
                    ));
                }
                let path = match &args[0] {
                    LispVal::String(s) => s.clone(),
                    _ => {
                        return Err(LispError::Generic(format!(
                            "FILE-READABLE-P: path must be a string, got {}",
                            err_val(&args[0])
                        )));
                    }
                };
                // Opening for read is the most reliable check with std-only.
                if std::fs::File::open(&path).is_ok() {
                    Ok(LispVal::Symbol(env.intern_symbol("T")))
                } else {
                    Ok(LispVal::Nil)
                }
            }

            BuiltinFunc::FileWritableP => {
                require_read_fs(env)?;
                if args.len() != 1 {
                    return Err(LispError::Generic(
                        "file-writable-p requires exactly one argument".to_string(),
                    ));
                }
                let path = match &args[0] {
                    LispVal::String(s) => s.clone(),
                    _ => {
                        return Err(LispError::Generic(format!(
                            "FILE-WRITABLE-P: path must be a string, got {}",
                            err_val(&args[0])
                        )));
                    }
                };
                let writable = std::fs::metadata(&path)
                    .map(|m| !m.permissions().readonly())
                    .unwrap_or(false);
                if writable {
                    Ok(LispVal::Symbol(env.intern_symbol("T")))
                } else {
                    Ok(LispVal::Nil)
                }
            }

            BuiltinFunc::FileExecutableP => {
                require_read_fs(env)?;
                if args.len() != 1 {
                    return Err(LispError::Generic(
                        "file-executable-p requires exactly one argument".to_string(),
                    ));
                }
                let path = match &args[0] {
                    LispVal::String(s) => s.clone(),
                    _ => {
                        return Err(LispError::Generic(format!(
                            "FILE-EXECUTABLE-P: path must be a string, got {}",
                            err_val(&args[0])
                        )));
                    }
                };
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    let executable = std::fs::metadata(&path)
                        .map(|m| m.permissions().mode() & 0o111 != 0)
                        .unwrap_or(false);
                    Ok(if executable {
                        LispVal::Symbol(env.intern_symbol("T"))
                    } else {
                        LispVal::Nil
                    })
                }
                #[cfg(not(unix))]
                Ok(LispVal::Nil)
            }

            BuiltinFunc::FileSize => {
                require_read_fs(env)?;
                if args.len() != 1 {
                    return Err(LispError::Generic(
                        "file-size requires exactly one argument".to_string(),
                    ));
                }
                let path = match &args[0] {
                    LispVal::String(s) => s.clone(),
                    _ => {
                        return Err(LispError::Generic(format!(
                            "FILE-SIZE: path must be a string, got {}",
                            err_val(&args[0])
                        )));
                    }
                };
                let size = std::fs::metadata(&path)
                    .map_err(|e| LispError::Generic(format!("file-size: {e}")))?
                    .len();
                Ok(LispVal::Number(size as i64))
            }

            BuiltinFunc::DirectoryFiles => {
                require_read_fs(env)?;
                if args.len() != 1 {
                    return Err(LispError::Generic(
                        "directory-files requires exactly one argument".to_string(),
                    ));
                }
                let path = match &args[0] {
                    LispVal::String(s) => s.clone(),
                    _ => {
                        return Err(LispError::Generic(format!(
                            "DIRECTORY-FILES: path must be a string, got {}",
                            err_val(&args[0])
                        )));
                    }
                };
                let mut names: Vec<String> = std::fs::read_dir(&path)
                    .map_err(|e| LispError::Generic(format!("directory-files: {e}")))?
                    .filter_map(|entry| entry.ok().and_then(|e| e.file_name().into_string().ok()))
                    .collect();
                names.sort();
                let list = names
                    .into_iter()
                    .rev()
                    .fold(LispVal::Nil, |cdr, name| LispVal::Cons {
                        car: Shared::new(LispVal::String(name)),
                        cdr: Shared::new(cdr),
                    });
                Ok(list)
            }

            BuiltinFunc::FileNewerP => {
                require_read_fs(env)?;
                if args.len() != 2 {
                    return Err(LispError::Generic(
                        "file-newer-p requires exactly two arguments: path1 path2".to_string(),
                    ));
                }
                let (p1, p2) = match (&args[0], &args[1]) {
                    (LispVal::String(a), LispVal::String(b)) => (a.clone(), b.clone()),
                    _ => {
                        return Err(LispError::Generic(format!(
                            "FILE-NEWER-P: both arguments must be strings, got {} and {}",
                            err_val(&args[0]),
                            err_val(&args[1])
                        )));
                    }
                };
                let mtime1 = std::fs::metadata(&p1)
                    .and_then(|m| m.modified())
                    .map_err(|e| LispError::Generic(format!("file-newer-p: {p1}: {e}")))?;
                let mtime2 = std::fs::metadata(&p2)
                    .and_then(|m| m.modified())
                    .map_err(|e| LispError::Generic(format!("file-newer-p: {p2}: {e}")))?;
                if mtime1 > mtime2 {
                    Ok(LispVal::Symbol(env.intern_symbol("T")))
                } else {
                    Ok(LispVal::Nil)
                }
            }

            // ── File mutation ───────────────────────────────────────────────
            BuiltinFunc::Chmod => {
                require_create_fs(env)?;
                if args.len() != 2 {
                    return Err(LispError::Generic(
                        "chmod requires exactly two arguments: path mode".to_string(),
                    ));
                }
                let path = match &args[0] {
                    LispVal::String(s) => s.clone(),
                    _ => {
                        return Err(LispError::Generic(format!(
                            "CHMOD: path must be a string, got {}",
                            err_val(&args[0])
                        )));
                    }
                };
                // Mode: integer (use directly) or octal string like "755".
                let mode: u32 = match &args[1] {
                    LispVal::Number(n) if *n >= 0 => *n as u32,
                    LispVal::String(s) => u32::from_str_radix(s, 8).map_err(|_| {
                        LispError::Generic(format!("chmod: cannot parse \"{s}\" as an octal mode"))
                    })?,
                    _ => {
                        return Err(LispError::Generic(format!(
                            "CHMOD: mode must be an integer or octal string, got {}",
                            err_val(&args[1])
                        )));
                    }
                };
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    let perms = std::fs::Permissions::from_mode(mode);
                    std::fs::set_permissions(&path, perms)
                        .map_err(|e| LispError::Generic(format!("chmod: {e}")))?;
                    Ok(LispVal::Symbol(env.intern_symbol("T")))
                }
                #[cfg(not(unix))]
                Err(LispError::Generic(
                    "chmod is only supported on Unix platforms".to_string(),
                ))
            }

            BuiltinFunc::CreateDirectory => {
                require_create_fs(env)?;
                if args.len() != 1 {
                    return Err(LispError::Generic(
                        "create-directory requires exactly one argument".to_string(),
                    ));
                }
                let path = match &args[0] {
                    LispVal::String(s) => s.clone(),
                    _ => {
                        return Err(LispError::Generic(format!(
                            "CREATE-DIRECTORY: path must be a string, got {}",
                            err_val(&args[0])
                        )));
                    }
                };
                std::fs::create_dir_all(&path)
                    .map_err(|e| LispError::Generic(format!("create-directory: {e}")))?;
                Ok(LispVal::Symbol(env.intern_symbol("T")))
            }

            BuiltinFunc::DeleteFile => {
                require_create_fs(env)?;
                if args.len() != 1 {
                    return Err(LispError::Generic(
                        "delete-file requires exactly one argument".to_string(),
                    ));
                }
                let path = match &args[0] {
                    LispVal::String(s) => s.clone(),
                    _ => {
                        return Err(LispError::Generic(format!(
                            "DELETE-FILE: path must be a string, got {}",
                            err_val(&args[0])
                        )));
                    }
                };
                std::fs::remove_file(&path)
                    .map_err(|e| LispError::Generic(format!("delete-file: {e}")))?;
                Ok(LispVal::Symbol(env.intern_symbol("T")))
            }

            BuiltinFunc::RenameFile => {
                // Renaming both observes the source path (existence probing
                // via error messages) and mutates the filesystem, so it
                // needs READ-FS in addition to CREATE-FS (issue #273).
                require_read_fs(env)?;
                require_create_fs(env)?;
                if args.len() != 2 {
                    return Err(LispError::Generic(
                        "rename-file requires exactly two arguments: from to".to_string(),
                    ));
                }
                let (from, to) = match (&args[0], &args[1]) {
                    (LispVal::String(a), LispVal::String(b)) => (a.clone(), b.clone()),
                    _ => {
                        return Err(LispError::Generic(format!(
                            "RENAME-FILE: both arguments must be strings, got {} and {}",
                            err_val(&args[0]),
                            err_val(&args[1])
                        )));
                    }
                };
                std::fs::rename(&from, &to)
                    .map_err(|e| LispError::Generic(format!("rename-file: {e}")))?;
                Ok(LispVal::Symbol(env.intern_symbol("T")))
            }

            // ── Temp filesystem ─────────────────────────────────────────────
            BuiltinFunc::MakeTempFile => {
                require_temp_fs(env)?;
                let prefix = match args.first() {
                    Some(LispVal::String(s)) => s.clone(),
                    None => String::new(),
                    Some(other) => {
                        return Err(LispError::Generic(format!(
                            "MAKE-TEMP-FILE: optional prefix must be a string, got {}",
                            err_val(other)
                        )));
                    }
                };
                let path = make_temp_path(&prefix, "");
                // Create the file atomically; fail if it somehow already exists.
                std::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&path)
                    .map_err(|e| LispError::Generic(format!("make-temp-file: {e}")))?;
                Ok(LispVal::String(path.to_string_lossy().into_owned()))
            }

            BuiltinFunc::MakeTempDirectory => {
                require_temp_fs(env)?;
                let prefix = match args.first() {
                    Some(LispVal::String(s)) => s.clone(),
                    None => String::new(),
                    Some(other) => {
                        return Err(LispError::Generic(format!(
                            "MAKE-TEMP-DIRECTORY: optional prefix must be a string, got {}",
                            err_val(other)
                        )));
                    }
                };
                let path = make_temp_path(&prefix, "");
                std::fs::create_dir(&path)
                    .map_err(|e| LispError::Generic(format!("make-temp-directory: {e}")))?;
                Ok(LispVal::String(path.to_string_lossy().into_owned()))
            }

            // Condition flags
            BuiltinFunc::SetFlag => {
                if args.len() != 1 {
                    return Err(LispError::Generic(
                        "set-flag requires exactly one argument".to_string(),
                    ));
                }
                let flag_name = match &args[0] {
                    LispVal::Symbol(s) => s.borrow().name.clone(),
                    LispVal::String(s) => s.clone(),
                    _ => {
                        return Err(LispError::Generic(format!(
                            "SET-FLAG: expected a symbol or string, got {}",
                            err_val(&args[0])
                        )));
                    }
                };
                env.set_flag(&flag_name);
                Ok(LispVal::Symbol(env.intern_symbol("T")))
            }

            BuiltinFunc::ClearFlag => {
                if args.len() != 1 {
                    return Err(LispError::Generic(
                        "clear-flag requires exactly one argument".to_string(),
                    ));
                }
                let flag_name = match &args[0] {
                    LispVal::Symbol(s) => s.borrow().name.clone(),
                    LispVal::String(s) => s.clone(),
                    _ => {
                        return Err(LispError::Generic(format!(
                            "CLEAR-FLAG: expected a symbol or string, got {}",
                            err_val(&args[0])
                        )));
                    }
                };
                env.clear_flag(&flag_name);
                Ok(LispVal::Symbol(env.intern_symbol("T")))
            }

            BuiltinFunc::FlagSetP => {
                if args.len() != 1 {
                    return Err(LispError::Generic(
                        "flag-set-p requires exactly one argument".to_string(),
                    ));
                }
                let flag_name = match &args[0] {
                    LispVal::Symbol(s) => s.borrow().name.clone(),
                    LispVal::String(s) => s.clone(),
                    _ => {
                        return Err(LispError::Generic(format!(
                            "FLAG-SET-P: expected a symbol or string, got {}",
                            err_val(&args[0])
                        )));
                    }
                };
                if env.flag_set(&flag_name) {
                    Ok(LispVal::Symbol(env.intern_symbol("T")))
                } else {
                    Ok(LispVal::Nil)
                }
            }

            BuiltinFunc::ClearAllFlags => {
                if !args.is_empty() {
                    return Err(LispError::Generic(
                        "clear-all-flags takes no arguments".to_string(),
                    ));
                }
                env.clear_all_flags();
                Ok(LispVal::Symbol(env.intern_symbol("T")))
            }

            BuiltinFunc::KernelFuelSet => {
                // (kernel-fuel-set! n-or-nil) — arm/disarm the current
                // thread's kernel step budget; returns the previous state
                // (a count or NIL) so callers can restore it on exit.
                if args.len() != 1 {
                    return Err(LispError::Generic(
                        "kernel-fuel-set! takes exactly one argument (a non-negative integer or nil)"
                            .to_string(),
                    ));
                }
                let requested = match &args[0] {
                    LispVal::Nil => None,
                    LispVal::Number(n) if *n >= 0 => Some(*n as u64),
                    other => {
                        return Err(LispError::Generic(format!(
                            "KERNEL-FUEL-SET!: expected a non-negative integer or nil, got {}",
                            err_val(other)
                        )));
                    }
                };
                // NARROW-ONLY when armed (#320): inside an active fuel
                // fence a program may lower its own budget but never raise
                // or disarm it — the only widening path is the WITH-FUEL
                // special form's own Rust-side restore.
                if crate::evaluator::core::fuel_fenced()
                    && let Some(current) = crate::evaluator::kernel_fuel_remaining()
                {
                    match requested {
                        Some(n) if n <= current => {}
                        _ => {
                            return Err(LispError::Generic(
                                "kernel-fuel-set!: cannot widen or disarm inside a fuel fence"
                                    .to_string(),
                            ));
                        }
                    }
                }
                match crate::evaluator::set_kernel_fuel(requested) {
                    Some(prev) => Ok(LispVal::Number(prev.min(i64::MAX as u64) as i64)),
                    None => Ok(LispVal::Nil),
                }
            }

            BuiltinFunc::KernelFuelRemaining => {
                if !args.is_empty() {
                    return Err(LispError::Generic(
                        "kernel-fuel-remaining takes no arguments".to_string(),
                    ));
                }
                match crate::evaluator::kernel_fuel_remaining() {
                    Some(n) => Ok(LispVal::Number(n.min(i64::MAX as u64) as i64)),
                    None => Ok(LispVal::Nil),
                }
            }

            BuiltinFunc::ReadAllPositioned => {
                // (read-all-positioned "src") — parse every top-level form in
                // the string, returning a list of (form line col) triples
                // with 1-based reader positions (issue #171 phase 2a: the
                // substrate for SGREP-FILE's file:line hits). Pure parsing:
                // no capability required; pair with READ-FILE (READ-FS) to
                // read source from disk. Honors the environment's reader
                // depth limit like the other evaluator-facing entry points.
                if args.len() != 1 {
                    return Err(LispError::Generic(
                        "read-all-positioned takes exactly one argument (a string)".to_string(),
                    ));
                }
                let LispVal::String(src) = &args[0] else {
                    return Err(LispError::Generic(format!(
                        "READ-ALL-POSITIONED: expected a string, got {}",
                        err_val(&args[0])
                    )));
                };
                let src = crate::reader::strip_shebang(src);
                let limit = env.reader_depth_limit();
                let mut triples: Vec<LispVal> = Vec::new();
                let mut rest = src;
                loop {
                    rest = crate::reader::skip_ws(rest);
                    let offset = src.len() - rest.len();
                    match crate::reader::read_next_with_depth_limit(rest, env, limit) {
                        Ok(None) => break,
                        Ok(Some((form, rem))) => {
                            let (line, col) = crate::reader::position_of(src, offset);
                            triples.push(vec_to_list(vec![
                                form,
                                LispVal::Number(line as i64),
                                LispVal::Number(col as i64),
                            ]));
                            rest = rem;
                        }
                        Err((err_offset, detail)) => {
                            return Err(LispError::Generic(crate::reader::format_parse_error(
                                src,
                                offset + err_offset,
                                &detail,
                            )));
                        }
                    }
                }
                let mut out = LispVal::Nil;
                for t in triples.into_iter().rev() {
                    out = LispVal::Cons {
                        car: Shared::new(t),
                        cdr: Shared::new(out),
                    };
                }
                Ok(out)
            }

            // Capabilities / features (read-only from Lisp)
            BuiltinFunc::FeatureEnabledP => {
                // Mask-aware (#320): inside a WITH-CAPABILITIES extent an
                // attenuated capability reads as disabled, so the Lisp-level
                // capability introspection follows the dynamic fence.
                let name = feature_name_arg(args, "feature-enabled-p")?;
                if env.feature_enabled(&name) && crate::evaluator::core::cap_mask_allows(&name) {
                    Ok(LispVal::Symbol(env.intern_symbol("T")))
                } else {
                    Ok(LispVal::Nil)
                }
            }
            BuiltinFunc::Features => {
                if !args.is_empty() {
                    return Err(LispError::Generic(
                        "features takes no arguments".to_string(),
                    ));
                }
                let mut names = env.features_list();
                names.sort();
                let list = names
                    .into_iter()
                    .rev()
                    .fold(LispVal::Nil, |cdr, n| LispVal::Cons {
                        car: Shared::new(LispVal::String(n)),
                        cdr: Shared::new(cdr),
                    });
                Ok(list)
            }
            BuiltinFunc::Shell => apply_shell(args, env),
            BuiltinFunc::TheEnvironment => {
                if !args.is_empty() {
                    return Err(LispError::Generic(
                        "the-environment takes no arguments".to_string(),
                    ));
                }
                Ok(LispVal::Environment(env.clone()))
            }
            BuiltinFunc::MakeEnvironment => match args.len() {
                0 => Ok(LispVal::Environment(Environment::new_with_builtins())),
                1 => {
                    if let LispVal::Environment(parent) = &args[0] {
                        Ok(LispVal::Environment(Environment::new_child(parent)))
                    } else {
                        Err(LispError::Generic(format!(
                            "MAKE-ENVIRONMENT: argument must be an environment, got {}",
                            err_val(&args[0])
                        )))
                    }
                }
                _ => Err(LispError::Generic(
                    "make-environment takes 0 or 1 arguments".to_string(),
                )),
            },
            BuiltinFunc::Optimize => {
                if args.len() != 1 {
                    return Err(LispError::Generic(
                        "optimize takes exactly one argument".to_string(),
                    ));
                }
                Ok(crate::optimizer::optimize(&args[0]))
            }
            // ── Arrays ─────────────────────────────────────────────────────
            BuiltinFunc::MakeArray => {
                if args.len() != 1 {
                    return Err(LispError::Generic(
                        "array takes exactly one argument".to_string(),
                    ));
                }
                const MAX_ARRAY: i64 = 16 * 1024 * 1024; // 16 M elements
                let n = match &args[0] {
                    LispVal::Number(n) if *n >= 0 && *n <= MAX_ARRAY => *n as usize,
                    LispVal::Number(n) if *n > MAX_ARRAY => {
                        return Err(LispError::Generic(format!(
                            "array: size {n} exceeds maximum of {MAX_ARRAY}"
                        )));
                    }
                    _ => {
                        return Err(LispError::Generic(format!(
                            "ARRAY: size must be a non-negative integer, got {}",
                            err_val(&args[0])
                        )));
                    }
                };
                let v = vec![LispVal::Nil; n];
                Ok(LispVal::Array(Shared::new(SharedCell::new(v))))
            }
            BuiltinFunc::ArrayFetch => {
                if args.len() != 2 {
                    return Err(LispError::Generic(
                        "fetch takes exactly two arguments".to_string(),
                    ));
                }
                if let LispVal::Array(a) = &args[0] {
                    let idx = match &args[1] {
                        LispVal::Number(n) if *n >= 0 => *n as usize,
                        _ => {
                            return Err(LispError::Generic(format!(
                                "FETCH: index must be a non-negative integer, got {}",
                                err_val(&args[1])
                            )));
                        }
                    };
                    let v = a.borrow();
                    if idx >= v.len() {
                        return Err(LispError::Generic(format!(
                            "fetch: index {idx} out of bounds (length {})",
                            v.len()
                        )));
                    }
                    Ok(v[idx].clone())
                } else {
                    Err(LispError::Generic(format!(
                        "FETCH: first argument must be an array, got {}",
                        err_val(&args[0])
                    )))
                }
            }
            BuiltinFunc::ArrayStore => {
                if args.len() != 3 {
                    return Err(LispError::Generic(
                        "store takes exactly three arguments".to_string(),
                    ));
                }
                if let LispVal::Array(a) = &args[0] {
                    let idx = match &args[1] {
                        LispVal::Number(n) if *n >= 0 => *n as usize,
                        _ => {
                            return Err(LispError::Generic(format!(
                                "STORE: index must be a non-negative integer, got {}",
                                err_val(&args[1])
                            )));
                        }
                    };
                    let val = args[2].clone();
                    let mut v = a.borrow_mut();
                    if idx >= v.len() {
                        return Err(LispError::Generic(format!(
                            "store: index {idx} out of bounds (length {})",
                            v.len()
                        )));
                    }
                    v[idx] = val.clone();
                    Ok(val)
                } else {
                    Err(LispError::Generic(format!(
                        "STORE: first argument must be an array, got {}",
                        err_val(&args[0])
                    )))
                }
            }
            BuiltinFunc::ArrayLength => {
                if args.len() != 1 {
                    return Err(LispError::Generic(
                        "array-length* takes exactly one argument".to_string(),
                    ));
                }
                if let LispVal::Array(a) = &args[0] {
                    Ok(LispVal::Number(a.borrow().len() as i64))
                } else {
                    Err(LispError::Generic(format!(
                        "ARRAY-LENGTH*: argument must be an array, got {}",
                        err_val(&args[0])
                    )))
                }
            }
            BuiltinFunc::Length => {
                if args.len() != 1 {
                    return Err(LispError::Generic(
                        "length takes exactly one argument".to_string(),
                    ));
                }
                // CL-style polymorphic length: proper lists, strings (in
                // characters, matching STRING-LENGTH*), and arrays (issue #245).
                match &args[0] {
                    LispVal::String(s) => Ok(LispVal::Number(s.chars().count() as i64)),
                    LispVal::Array(a) => Ok(LispVal::Number(a.borrow().len() as i64)),
                    // 0.3 census: hash tables too — one LENGTH for every
                    // sized collection.
                    LispVal::HashTable(h) => Ok(LispVal::Number(h.borrow().len() as i64)),
                    other => Ok(LispVal::Number(proper_list_len(other)? as i64)),
                }
            }
            BuiltinFunc::ListToArray => {
                if args.len() != 1 {
                    return Err(LispError::Generic(
                        "list->array takes exactly one argument".to_string(),
                    ));
                }
                Ok(LispVal::Array(Shared::new(SharedCell::new(list_to_vec(
                    &args[0],
                )?))))
            }
            BuiltinFunc::ArrayToList => {
                if args.len() != 1 {
                    return Err(LispError::Generic(
                        "array->list takes exactly one argument".to_string(),
                    ));
                }
                if let LispVal::Array(a) = &args[0] {
                    Ok(vec_to_list(a.borrow().clone()))
                } else {
                    Err(LispError::Generic(format!(
                        "ARRAY->LIST: argument must be an array, got {}",
                        err_val(&args[0])
                    )))
                }
            }

            // ── Concurrency primitives (concurrency feature) ─────────────────
            #[cfg(feature = "concurrency")]
            BuiltinFunc::MakeChannel => apply_make_channel(args),
            #[cfg(feature = "concurrency")]
            BuiltinFunc::SpawnProcess => apply_spawn(args, env),
            #[cfg(feature = "concurrency")]
            BuiltinFunc::ChannelSend => apply_channel_send(args, env),
            #[cfg(feature = "concurrency")]
            BuiltinFunc::ChannelRecv => apply_channel_recv(args, env),
            #[cfg(feature = "concurrency")]
            BuiltinFunc::ChannelRecvTimeout => apply_channel_recv_timeout(args, env),
            #[cfg(feature = "concurrency")]
            BuiltinFunc::CloneInterpreter => apply_clone_interpreter(args, env),
        },
        LispVal::Lambda(lambda) => {
            // Create new environment with:
            // - Lexical parent: lambda.env (captured closure environment)
            // - Dynamic parent: env (caller's environment for dynamic variable lookup)
            let new_env = Environment::new_child_with_dynamic(&lambda.env, env);
            let has_dyn = new_env.has_any_dynamic();
            let mut guards: Vec<DynamicBinding> = Vec::new();
            if let Some(rest_param_id) = lambda.rest_param_id {
                if args.len() < lambda.params.len() {
                    return Err(LispError::Generic(format!(
                        "lambda expected at least {} arguments, got {}",
                        lambda.params.len(),
                        args.len()
                    )));
                }
                for (id, arg) in lambda.param_ids.iter().zip(args.iter()) {
                    if has_dyn
                        && let Some(sym) = new_env.symbol_by_id(*id)
                        && sym.borrow().is_dynamic
                    {
                        guards.push(DynamicBinding::install(sym, arg.clone()));
                        continue;
                    }
                    new_env.set_id(*id, arg.clone());
                }
                let rest_args = vec_to_list(args[lambda.params.len()..].to_vec());
                if has_dyn
                    && let Some(sym) = new_env.symbol_by_id(rest_param_id)
                    && sym.borrow().is_dynamic
                {
                    guards.push(DynamicBinding::install(sym, rest_args));
                } else {
                    new_env.set_id(rest_param_id, rest_args);
                }
            } else {
                if lambda.params.len() != args.len() {
                    return Err(LispError::Generic(format!(
                        "lambda expected {} arguments, got {}",
                        lambda.params.len(),
                        args.len()
                    )));
                }
                for (id, arg) in lambda.param_ids.iter().zip(args) {
                    if has_dyn
                        && let Some(sym) = new_env.symbol_by_id(*id)
                        && sym.borrow().is_dynamic
                    {
                        guards.push(DynamicBinding::install(sym, arg.clone()));
                        continue;
                    }
                    new_env.set_id(*id, arg.clone());
                }
            }

            // Use the compiled body when available; fall back to tree-walker.
            // guards drops here, restoring any dynamic bindings.
            match &lambda.compiled {
                Some(compiled) => exec(compiled, &new_env),
                None => eval(&lambda.body, &new_env),
            }
        }
        LispVal::Native(f) => f(args, env),
        _ => Err(LispError::Generic(format!("Not a function: {func:?}"))),
    }
}

/// Apply a callable to an **owned** argument vector, moving each value into the
/// lambda's environment frame instead of cloning it.
///
/// Called from `eval_application` in `eval_step` for builtin/native callables
/// (the path taken when the head is neither a Lambda nor a Fexpr/Vau/Macro).
/// For `LispVal::Lambda` the args are consumed via `into_iter()`/`drain` so
/// no per-argument clone is needed. All other callables (builtins, natives,
/// fexprs, macros) fall through to [`apply`] via a slice reference.
pub(super) fn apply_owned(
    func: &LispVal,
    args: SmallVec<[LispVal; 4]>,
    env: &Shared<Environment>,
) -> Result<LispVal, LispError> {
    match func {
        LispVal::Lambda(lambda) => {
            // Create new environment with:
            // - Lexical parent: lambda.env (captured closure environment)
            // - Dynamic parent: env (caller's environment for dynamic variable lookup)
            let new_env = Environment::new_child_with_dynamic(&lambda.env, env);
            let has_dyn = new_env.has_any_dynamic();
            let mut guards: Vec<DynamicBinding> = Vec::new();
            if let Some(rest_param_id) = lambda.rest_param_id {
                if args.len() < lambda.params.len() {
                    return Err(LispError::Generic(format!(
                        "lambda expected at least {} arguments, got {}",
                        lambda.params.len(),
                        args.len()
                    )));
                }
                // Move fixed args into the frame, keep the rest for the &rest list.
                let n_fixed = lambda.params.len();
                let mut args = args;
                for (id, arg) in lambda.param_ids.iter().zip(args.drain(..n_fixed)) {
                    if has_dyn
                        && let Some(sym) = new_env.symbol_by_id(*id)
                        && sym.borrow().is_dynamic
                    {
                        guards.push(DynamicBinding::install(sym, arg));
                        continue;
                    }
                    new_env.set_id(*id, arg);
                }
                let rest_args = vec_to_list(args.into_vec());
                if has_dyn
                    && let Some(sym) = new_env.symbol_by_id(rest_param_id)
                    && sym.borrow().is_dynamic
                {
                    guards.push(DynamicBinding::install(sym, rest_args));
                } else {
                    new_env.set_id(rest_param_id, rest_args);
                }
            } else {
                if lambda.params.len() != args.len() {
                    return Err(LispError::Generic(format!(
                        "lambda expected {} arguments, got {}",
                        lambda.params.len(),
                        args.len()
                    )));
                }
                // Move every arg directly into the frame — no clone.
                for (id, arg) in lambda.param_ids.iter().zip(args) {
                    if has_dyn
                        && let Some(sym) = new_env.symbol_by_id(*id)
                        && sym.borrow().is_dynamic
                    {
                        guards.push(DynamicBinding::install(sym, arg));
                        continue;
                    }
                    new_env.set_id(*id, arg);
                }
            }
            // Use the compiled body when available; fall back to tree-walker.
            // guards drops here, restoring any dynamic bindings.
            match &lambda.compiled {
                Some(compiled) => exec(compiled, &new_env),
                None => eval(&lambda.body, &new_env),
            }
        }
        // For all other callables (builtins, natives, fexprs, macros) the
        // existing borrowed-slice path is correct.
        other => apply(other, &args, env),
    }
}

/// `record-ref` / `record-with` (issue #308): by-name record field access
/// over the one record representation (`LispVal::Struct`), via the
/// registry's field table for the value's brand — this is the primitive
/// whose absence was issue #305 (native structs were reachable only through
/// nominal accessors).
///
/// `(record-ref v 'field)` reads; `(record-with v 'field x)` returns a new
/// record with the field replaced (functional update — records are values).
fn apply_record_field_op(
    builtin: &BuiltinFunc,
    args: &[LispVal],
    env: &Shared<Environment>,
) -> Result<LispVal, LispError> {
    let is_ref = matches!(builtin, BuiltinFunc::RecordRef);
    let (op, want) = if is_ref {
        ("record-ref", 2)
    } else {
        ("record-with", 3)
    };
    if args.len() != want {
        return Err(LispError::Generic(format!(
            "{op} requires exactly {want} arguments"
        )));
    }
    let field = match &args[1] {
        LispVal::Symbol(s) => s.borrow().name.clone(),
        other => {
            return Err(LispError::Generic(format!(
                "{}: field must be a symbol, got {}",
                op.to_uppercase(),
                err_val(other)
            )));
        }
    };

    // Resolve (brand, field index, arity) for either representation.
    match &args[0] {
        LispVal::Struct(obj) => {
            let names = env.jit_struct_field_names(&obj.type_name).ok_or_else(|| {
                LispError::Generic(format!("{op}: unknown record type {}", obj.type_name))
            })?;
            let idx = names.iter().position(|n| *n == field).ok_or_else(|| {
                LispError::Generic(format!(
                    "{op}: record {} has no field {}",
                    obj.type_name,
                    field.to_lowercase()
                ))
            })?;
            if is_ref {
                Ok(obj.fields[idx].clone())
            } else {
                let mut fields = obj.fields.clone();
                fields[idx] = args[2].clone();
                Ok(LispVal::Struct(Shared::new(crate::StructObj {
                    type_name: obj.type_name.clone(),
                    fields,
                })))
            }
        }
        other => Err(LispError::Generic(format!(
            "{}: not a record value, got {}",
            op.to_uppercase(),
            err_val(other)
        ))),
    }
}

/// Record type operations (issue #308 stage B): the kernel quartet behind
/// the one-door `defrecord`. `record-declare` registers a branded record
/// type whose fields may be ANY checkable type (the name becomes denotable
/// and nominal in the checker, row-subsumable via #299) without installing
/// typed functions; `record-new` constructs a StructObj value with arity
/// checking; `record-brand` reports a value's brand (both representations);
/// `record-compiled-p` reports whether a registered record's fields are all
/// natively storable (the tier introspection).
fn apply_record_type_op(
    builtin: &BuiltinFunc,
    args: &[LispVal],
    env: &Shared<Environment>,
) -> Result<LispVal, LispError> {
    match builtin {
        BuiltinFunc::RecordDeclare => {
            if args.len() != 2 {
                return Err(LispError::Generic(
                    "record-declare requires exactly two arguments: brand field-specs".to_string(),
                ));
            }
            // Parametric form (0.3 HM generics): (record-declare '(name p...)
            // '((field ty)...)) declares a generic record/ctor.
            if let LispVal::Cons { .. } = &args[0] {
                let parts = list_to_vec(&args[0])?;
                let mut names = Vec::with_capacity(parts.len());
                for p in &parts {
                    let LispVal::Symbol(s) = p else {
                        return Err(LispError::Generic(
                            "RECORD-DECLARE: generic head must be (name param...) of symbols"
                                .to_string(),
                        ));
                    };
                    names.push(s.borrow().name.clone());
                }
                let (name, params) = names.split_first().ok_or_else(|| {
                    LispError::Generic("RECORD-DECLARE: empty generic head".to_string())
                })?;
                env.jit_declare_generic_record(name, params, &args[1])
                    .map_err(LispError::Generic)?;
                return Ok(LispVal::Symbol(env.intern_symbol(name)));
            }
            let LispVal::Symbol(brand) = &args[0] else {
                return Err(LispError::Generic(format!(
                    "RECORD-DECLARE: brand must be a symbol, got {}",
                    err_val(&args[0])
                )));
            };
            let name = brand.borrow().name.clone();
            env.jit_declare_record(&name, &args[1])
                .map_err(LispError::Generic)?;
            Ok(LispVal::Symbol(brand.clone()))
        }
        BuiltinFunc::RecordNew => {
            // Zero field values are legal: a nullary variant constructor
            // like (none) builds an empty branded record.
            if args.is_empty() {
                return Err(LispError::Generic(
                    "record-new requires a brand".to_string(),
                ));
            }
            let LispVal::Symbol(brand) = &args[0] else {
                return Err(LispError::Generic(format!(
                    "RECORD-NEW: brand must be a symbol, got {}",
                    err_val(&args[0])
                )));
            };
            let name = brand.borrow().name.clone();
            let field_names = env.jit_struct_field_names(&name).ok_or_else(|| {
                LispError::Generic(format!("record-new: unknown record type {name}"))
            })?;
            if args.len() - 1 != field_names.len() {
                return Err(LispError::Generic(format!(
                    "record-new: {name} has {} field(s), got {} value(s)",
                    field_names.len(),
                    args.len() - 1
                )));
            }
            Ok(LispVal::Struct(Shared::new(crate::StructObj {
                type_name: name,
                fields: args[1..].to_vec(),
            })))
        }
        BuiltinFunc::RecordBrand => {
            if args.len() != 1 {
                return Err(LispError::Generic(
                    "record-brand requires exactly one argument".to_string(),
                ));
            }
            match &args[0] {
                LispVal::Struct(obj) => Ok(LispVal::Symbol(env.intern_symbol(&obj.type_name))),
                _ => Ok(LispVal::Nil),
            }
        }
        BuiltinFunc::RecordCompiledP => {
            if args.len() != 1 {
                return Err(LispError::Generic(
                    "record-compiled-p requires exactly one argument".to_string(),
                ));
            }
            let LispVal::Symbol(brand) = &args[0] else {
                return Err(LispError::Generic(format!(
                    "RECORD-COMPILED-P: brand must be a symbol, got {}",
                    err_val(&args[0])
                )));
            };
            let name = brand.borrow().name.clone();
            match env.jit_record_compileable(&name) {
                Some(true) => Ok(LispVal::Symbol(env.intern_symbol("T"))),
                Some(false) => Ok(LispVal::Nil),
                None => Err(LispError::Generic(format!(
                    "record-compiled-p: unknown record type {name}"
                ))),
            }
        }
        BuiltinFunc::RecordFields => {
            if args.len() != 1 {
                return Err(LispError::Generic(
                    "record-fields requires exactly one argument".to_string(),
                ));
            }
            match &args[0] {
                LispVal::Struct(obj) => Ok(vec_to_list(obj.fields.clone())),
                other => Err(LispError::Generic(format!(
                    "RECORD-FIELDS: not a record value, got {}",
                    err_val(other)
                ))),
            }
        }
        // (variant-declare 'name '(ctor ...)) — register the checker-level
        // union of constructor brands (#312 sums).
        BuiltinFunc::VariantDeclare => {
            if args.len() != 2 {
                return Err(LispError::Generic(
                    "variant-declare requires exactly two arguments: name ctors".to_string(),
                ));
            }
            // Parametric form: (variant-declare '(name p...) '(ctor...)).
            if let LispVal::Cons { .. } = &args[0] {
                let parts = list_to_vec(&args[0])?;
                let mut names = Vec::with_capacity(parts.len());
                for p in &parts {
                    let LispVal::Symbol(s) = p else {
                        return Err(LispError::Generic(
                            "VARIANT-DECLARE: generic head must be (name param...) of symbols"
                                .to_string(),
                        ));
                    };
                    names.push(s.borrow().name.clone());
                }
                let (name, params) = names.split_first().ok_or_else(|| {
                    LispError::Generic("VARIANT-DECLARE: empty generic head".to_string())
                })?;
                let ctors = list_to_vec(&args[1])?
                    .into_iter()
                    .map(|c| match c {
                        LispVal::Symbol(s) => Ok(s.borrow().name.clone()),
                        _ => Err(LispError::Generic(
                            "VARIANT-DECLARE: ctors must be symbols".to_string(),
                        )),
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                env.jit_declare_generic_variant(name, params.len(), ctors)
                    .map_err(LispError::Generic)?;
                return Ok(LispVal::Symbol(env.intern_symbol(name)));
            }
            let LispVal::Symbol(name) = &args[0] else {
                return Err(LispError::Generic(
                    "VARIANT-DECLARE: name must be a symbol".to_string(),
                ));
            };
            let mut ctors = Vec::new();
            let mut cur = args[1].clone();
            loop {
                match cur {
                    LispVal::Nil => break,
                    LispVal::Cons { car, cdr } => {
                        let LispVal::Symbol(c) = car.as_ref() else {
                            return Err(LispError::Generic(
                                "VARIANT-DECLARE: ctors must be symbols".to_string(),
                            ));
                        };
                        ctors.push(c.borrow().name.clone());
                        cur = cdr.as_ref().clone();
                    }
                    _ => {
                        return Err(LispError::Generic(
                            "VARIANT-DECLARE: ctors must be a proper list".to_string(),
                        ));
                    }
                }
            }
            if ctors.is_empty() {
                return Err(LispError::Generic(
                    "VARIANT-DECLARE: a variant needs at least one constructor".to_string(),
                ));
            }
            env.jit_declare_variant(&name.borrow().name.clone(), ctors)
                .map_err(LispError::Generic)?;
            Ok(LispVal::Symbol(name.clone()))
        }
        _ => unreachable!("routed only for record type ops"),
    }
}
