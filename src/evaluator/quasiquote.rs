use super::*;
#[inline(never)]
pub(super) fn quasiquote_eval(
    val: &LispVal,
    env: &Shared<Environment>,
) -> Result<LispVal, LispError> {
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
        // A splicing form `,@e` appearing as a list element: evaluate `e` to a
        // list and graft its elements into the surrounding list, ahead of the
        // result of processing the remaining elements (the cdr).
        if let Some(spliced) = unquote_splicing_arg(car) {
            let spliced_list = eval(spliced, env)?;
            let cdr_eval = quasiquote_eval(cdr, env)?;
            return append_lists(&spliced_list, cdr_eval);
        }
        let car_eval = quasiquote_eval(car, env)?;
        let cdr_eval = quasiquote_eval(cdr, env)?;
        Ok(LispVal::Cons {
            car: Shared::new(car_eval),
            cdr: Shared::new(cdr_eval),
        })
    } else {
        Ok(val.clone())
    }
}

/// If `val` is a well-formed `(UNQUOTE-SPLICING e)` form, return a reference to
/// `e`; otherwise return `None` (including ill-arity forms, which then fall
/// through to ordinary template processing).
pub(super) fn unquote_splicing_arg(val: &LispVal) -> Option<&LispVal> {
    if let LispVal::Cons { car, cdr } = val
        && let LispVal::Symbol(s) = &**car
        && s.borrow().name == "UNQUOTE-SPLICING"
        && let LispVal::Cons {
            car: arg,
            cdr: rest,
        } = &**cdr
        && **rest == LispVal::Nil
    {
        return Some(arg);
    }
    None
}

/// Build a fresh cons chain holding every element of the proper list `front`
/// followed by `tail`. The cons cells of `front` are copied so the original is
/// left untouched; a non-list `front` (or an improper tail) yields an error.
///
/// Iterative on purpose: the length of `front` is user data, so recursing once
/// per element would overflow the native stack on a long spliced list.
pub(super) fn append_lists(front: &LispVal, tail: LispVal) -> Result<LispVal, LispError> {
    let mut items: Vec<&Shared<LispVal>> = Vec::new();
    let mut cur = front;
    loop {
        match cur {
            LispVal::Nil => break,
            LispVal::Cons { car, cdr } => {
                items.push(car);
                cur = cdr;
            }
            other => {
                return Err(LispError::Generic(format!(
                    "UNQUOTE-SPLICING: expected a list argument, got {}",
                    err_val(other)
                )));
            }
        }
    }
    Ok(items
        .into_iter()
        .rev()
        .fold(tail, |rest, car| LispVal::Cons {
            car: car.clone(),
            cdr: Shared::new(rest),
        }))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `append_lists` once recursed per spliced element, so a long `,@` list
    /// aborted the process with a native stack overflow. Run it on a 1 MiB
    /// stack with far more elements than any per-element frame could fit.
    /// The long lists are leaked: dropping a long cons chain is itself
    /// recursive (a separate limitation) and is not what is under test.
    #[test]
    fn append_lists_is_not_recursive_in_the_spliced_length() {
        const N: i64 = 200_000;
        let handle = std::thread::Builder::new()
            .stack_size(1024 * 1024)
            .spawn(|| {
                let mut front = LispVal::Nil;
                for i in (0..N).rev() {
                    front = LispVal::Cons {
                        car: Shared::new(LispVal::Number(i)),
                        cdr: Shared::new(front),
                    };
                }
                let out = append_lists(&front, LispVal::Number(-1)).expect("proper list");
                let mut len = 0usize;
                let mut cur = &out;
                let mut last_car = None::<i64>;
                while let LispVal::Cons { car, cdr } = cur {
                    len += 1;
                    if let LispVal::Number(n) = **car {
                        last_car = Some(n);
                    }
                    cur = cdr;
                }
                let tail = match cur {
                    LispVal::Number(n) => Some(*n),
                    _ => None,
                };
                std::mem::forget(front);
                std::mem::forget(out);
                (len, last_car, tail)
            })
            .expect("spawn");
        let (len, last_car, tail) = handle.join().expect("stack overflow or panic");
        assert_eq!(len, N as usize);
        assert_eq!(last_car, Some(N - 1));
        assert_eq!(tail, Some(-1));
    }
}
