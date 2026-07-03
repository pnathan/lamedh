use super::*;
// String/Symbol operations
#[inline(never)]
pub(super) fn apply_string_symbol_ops(
    op: &BuiltinFunc,
    args: &[LispVal],
    env: &Shared<Environment>,
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
                    return Err(LispError::Generic(format!(
                        "EXPLODE: expected a symbol, string, or number, got {}",
                        err_val(&args[0])
                    )));
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
                    other => {
                        return Err(LispError::Generic(format!(
                            "IMPLODE: expected a list of symbols or strings, got {}",
                            err_val(&other)
                        )));
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
                    other => {
                        return Err(LispError::Generic(format!(
                            "MAKNAM: expected a list of symbols or strings, got {}",
                            err_val(&other)
                        )));
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
                    return Err(LispError::Generic(format!(
                        "INTERN: expected a string or symbol, got {}",
                        err_val(&args[0])
                    )));
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
                Err(LispError::Generic(format!(
                    "PLIST: expected a symbol, got {}",
                    err_val(&args[0])
                )))
            }
        }
        _ => Err(LispError::Generic(
            "Not a string/symbol operation".to_string(),
        )),
    }
}

// New bitwise operations
#[inline(never)]
pub(super) fn apply_new_bitwise_ops(
    op: &BuiltinFunc,
    args: &[LispVal],
    env: &Shared<Environment>,
) -> Result<LispVal, LispError> {
    match op {
        BuiltinFunc::Ash => {
            if args.len() != 2 {
                return Err(LispError::Generic(
                    "ash requires exactly two arguments".to_string(),
                ));
            }
            if let (LispVal::Number(n), LispVal::Number(shift)) = (&args[0], &args[1]) {
                if *shift == 0 {
                    Ok(LispVal::Number(*n))
                } else if *shift < 0 {
                    // Right shift: if -shift >= 64, sign-extend to 0 or -1
                    let rshift = -*shift;
                    if rshift >= 64 {
                        Ok(LispVal::Number(if *n < 0 { -1 } else { 0 }))
                    } else {
                        Ok(LispVal::Number(n >> (rshift as u32)))
                    }
                } else {
                    // Left shift: guard against shift >= 64
                    if *shift >= 64 {
                        env.set_flag("OVERFLOW");
                        Ok(LispVal::Number(0))
                    } else {
                        // shift is in [0, 63]; wrapping_shl never panics here
                        Ok(LispVal::Number(n.wrapping_shl(*shift as u32)))
                    }
                }
            } else {
                Err(LispError::Generic(format!(
                    "ASH: expected integer arguments, got {} and {}",
                    err_val(&args[0]),
                    err_val(&args[1])
                )))
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
                Err(LispError::Generic(format!(
                    "LOGNOT: expected an integer argument, got {}",
                    err_val(&args[0])
                )))
            }
        }
        BuiltinFunc::Rot => {
            if args.len() != 2 {
                return Err(LispError::Generic(
                    "rot requires exactly two arguments".to_string(),
                ));
            }
            if let (LispVal::Number(n), LispVal::Number(count)) = (&args[0], &args[1]) {
                // rem_euclid on i64 always returns a value in [0, 63]
                let count = count.rem_euclid(64) as u32;
                Ok(LispVal::Number(((*n as u64).rotate_left(count)) as i64))
            } else {
                Err(LispError::Generic(format!(
                    "ROT: expected integer arguments, got {} and {}",
                    err_val(&args[0]),
                    err_val(&args[1])
                )))
            }
        }
        _ => Err(LispError::Generic("Not a bitwise operation".to_string())),
    }
}

// Property list operations
#[inline(never)]
pub(super) fn apply_plist_op(
    op: &BuiltinFunc,
    args: &[LispVal],
    env: &Shared<Environment>,
) -> Result<LispVal, LispError> {
    match op {
        BuiltinFunc::Remprop => {
            if args.len() != 2 {
                return Err(LispError::Generic(
                    "remprop requires exactly two arguments".to_string(),
                ));
            }
            let prop = match &args[1] {
                LispVal::String(s) => s.clone(),
                LispVal::Symbol(s) => s.borrow().name.clone(),
                _ => {
                    return Err(LispError::Generic(format!(
                        "REMPROP: expected a symbol or string as its second argument, got {}",
                        err_val(&args[1])
                    )));
                }
            };
            if let LispVal::Symbol(s) = &args[0] {
                let removed = s.borrow_mut().plist.remove(&prop);
                if removed.is_some() {
                    Ok(LispVal::Symbol(env.intern_symbol("T")))
                } else {
                    Ok(LispVal::Nil)
                }
            } else {
                Err(LispError::Generic(format!(
                    "REMPROP: expected a symbol as its first argument, got {}",
                    err_val(&args[0])
                )))
            }
        }
        BuiltinFunc::Deflist => {
            if args.len() != 2 {
                return Err(LispError::Generic(
                    "deflist requires exactly two arguments".to_string(),
                ));
            }
            let pairs = &args[0];
            let indicator = match &args[1] {
                LispVal::String(s) => s.clone(),
                LispVal::Symbol(s) => s.borrow().name.clone(),
                _ => {
                    return Err(LispError::Generic(format!(
                        "DEFLIST: expected a symbol or string as its second argument, got {}",
                        err_val(&args[1])
                    )));
                }
            };
            let mut current = pairs;
            while let LispVal::Cons { car, cdr } = current {
                if let LispVal::Cons {
                    car: sym,
                    cdr: rest,
                } = &**car
                    && let LispVal::Symbol(s) = &**sym
                    && let LispVal::Cons { car: val, cdr: _ } = &**rest
                {
                    s.borrow_mut()
                        .plist
                        .insert(indicator.clone(), val.as_ref().clone());
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
