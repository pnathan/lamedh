use super::*;
/// Mnemonic for an arithmetic [`BinOp`] at numeric kind `k` (`i*` vs `f*`).
pub(super) fn bin_mnemonic(k: NumKind, op: BinOp) -> &'static str {
    let p = matches!(k, NumKind::I);
    match (op, p) {
        (BinOp::Add, true) => "iadd",
        (BinOp::Add, false) => "fadd",
        (BinOp::Sub, true) => "isub",
        (BinOp::Sub, false) => "fsub",
        (BinOp::Mul, true) => "imul",
        (BinOp::Mul, false) => "fmul",
        (BinOp::Div, true) => "idiv",
        (BinOp::Div, false) => "fdiv",
        (BinOp::Mod, true) => "imod",
        (BinOp::Mod, false) => "fmod",
    }
}

/// Mnemonic for a comparison [`CmpOp`] at numeric kind `k` (`icmp.*`/`fcmp.*`).
pub(super) fn cmp_mnemonic(k: NumKind, op: CmpOp) -> &'static str {
    let p = matches!(k, NumKind::I);
    match (op, p) {
        (CmpOp::Lt, true) => "icmp.lt",
        (CmpOp::Lt, false) => "fcmp.lt",
        (CmpOp::Gt, true) => "icmp.gt",
        (CmpOp::Gt, false) => "fcmp.gt",
        (CmpOp::Le, true) => "icmp.le",
        (CmpOp::Le, false) => "fcmp.le",
        (CmpOp::Ge, true) => "icmp.ge",
        (CmpOp::Ge, false) => "fcmp.ge",
        (CmpOp::Eq, true) => "icmp.eq",
        (CmpOp::Eq, false) => "fcmp.eq",
        (CmpOp::Ne, true) => "icmp.ne",
        (CmpOp::Ne, false) => "fcmp.ne",
    }
}

/// A parsed typed signature: function name, return type, parameter `(name, type)`s.
pub(super) type ParsedSig = (String, Ty, Vec<(String, Ty)>);

/// Parse a type annotation: a scalar keyword, the bare `array` keyword (element
/// type left as a fresh inference variable), or `(array T)` with the element
/// pinned. The `infer` supplies fresh variables for unpinned array elements.
pub(super) fn parse_ty(
    form: &LispVal,
    infer: &mut Infer,
    structs: &HashMap<String, Rc<StructDef>>,
) -> Result<Ty, String> {
    match form {
        LispVal::Symbol(s) => {
            let name = s.borrow().name.clone();
            if name == "ARRAY" {
                return Ok(Ty::Array(Box::new(infer.fresh())));
            }
            if let Some(def) = structs.get(&name) {
                return Ok(Ty::Struct(def.clone()));
            }
            Ty::parse(&name).ok_or_else(|| format!("unknown type `{name}`"))
        }
        LispVal::Cons { .. } => {
            let items = list_to_vec(form);
            match items.as_slice() {
                [LispVal::Symbol(h), elem] if h.borrow().name == "ARRAY" => {
                    Ok(Ty::Array(Box::new(parse_ty(elem, infer, structs)?)))
                }
                _ => Err("type must be a scalar, struct, `array`, or `(array T)`".to_string()),
            }
        }
        other => Err(format!("bad type annotation: {other:?}")),
    }
}

/// Parse `items[1]` = `(name ret)` and `items[2]` = `((arg ty)...)` shared by
/// `deffun-typed` and `declare-typed`. Array element types may be inferred, so
/// the `infer` provides fresh variables; `define` resolves them after the body.
pub(super) fn parse_signature(
    items: &[LispVal],
    infer: &mut Infer,
    structs: &HashMap<String, Rc<StructDef>>,
) -> Result<ParsedSig, String> {
    let sig = list_to_vec(&items[1]);
    let (name, ret) = match sig.as_slice() {
        [LispVal::Symbol(n), rty] => {
            let ret = parse_ty(rty, infer, structs).map_err(|e| match rty {
                LispVal::Symbol(r) => format!("unknown return type `{}`", r.borrow().name),
                _ => e,
            })?;
            (n.borrow().name.clone(), ret)
        }
        _ => return Err("typed signature must be (name return-type)".to_string()),
    };
    let mut params = Vec::new();
    for p in list_to_vec(&items[2]) {
        let parts = list_to_vec(&p);
        match parts.as_slice() {
            [LispVal::Symbol(a), t] => {
                let pt = parse_ty(t, infer, structs).map_err(|e| match t {
                    LispVal::Symbol(s) => format!("unknown param type `{}`", s.borrow().name),
                    _ => e,
                })?;
                params.push((a.borrow().name.clone(), pt));
            }
            _ => return Err("each parameter must be (name type)".to_string()),
        }
    }
    Ok((name, ret, params))
}

/// Type-directed `LispVal` → [`Value`] at the convenience membrane
/// ([`Jit::call_lisp`]). A `(array char)` accepts a string; arrays/structs
/// convert element/field-wise. `bool` follows Lisp truthiness.
pub(super) fn char_byte_from_number(n: i64, context: &str) -> Result<u8, String> {
    u8::try_from(n).map_err(|_| format!("{context}: {n} out of range 0-255"))
}

pub(super) fn lispval_to_value(lv: &LispVal, ty: &Ty) -> Result<Value, String> {
    match ty {
        Ty::Int64 => match lv {
            LispVal::Number(n) => Ok(Value::Int(*n)),
            other => Err(format!("expected int64, got {other:?}")),
        },
        Ty::Float64 => match lv {
            LispVal::Float(f) => Ok(Value::Float(*f)),
            LispVal::Number(n) => Ok(Value::Float(*n as f64)),
            other => Err(format!("expected float64, got {other:?}")),
        },
        Ty::Bool => Ok(Value::Bool(!matches!(lv, LispVal::Nil))),
        Ty::Char => match lv {
            LispVal::Char(b) => Ok(Value::Char(*b)),
            LispVal::Number(n) => Ok(Value::Char(char_byte_from_number(*n, "char")?)),
            other => Err(format!("expected char, got {other:?}")),
        },
        Ty::Array(elem) => match lv {
            LispVal::String(s) if matches!(**elem, Ty::Char) => {
                Ok(Value::Array(s.bytes().map(Value::Char).collect()))
            }
            LispVal::Array(a) => {
                let mut out = Vec::new();
                for it in a.borrow().iter() {
                    out.push(lispval_to_value(it, elem)?);
                }
                Ok(Value::Array(out))
            }
            other => Err(format!("expected array, got {other:?}")),
        },
        Ty::Struct(def) => match lv {
            LispVal::Struct(obj) => {
                if obj.type_name != def.name {
                    return Err(format!(
                        "expected struct {}, got {}",
                        def.name, obj.type_name
                    ));
                }
                if obj.fields.len() != def.fields.len() {
                    return Err(format!(
                        "expected {} fields for struct {}, got {}",
                        def.fields.len(),
                        def.name,
                        obj.fields.len()
                    ));
                }
                let mut out = Vec::new();
                for (it, (_, ft)) in obj.fields.iter().zip(def.fields.iter()) {
                    out.push(lispval_to_value(it, ft)?);
                }
                Ok(Value::Struct(out))
            }
            other => Err(format!("expected struct {}, got {other:?}", def.name)),
        },
        // Non-compileable types (#162) never back a native edition.
        _ => Err(format!("type {} is not compileable", ty_name(ty))),
    }
}

/// Type-directed [`Value`] → `LispVal` ([`Jit::call_lisp`]). `bool` maps to
/// `0`/`1` (no environment here for `T`); `(array char)` becomes a string.
pub(super) fn value_to_lispval(v: &Value, ty: &Ty) -> LispVal {
    match v {
        Value::Int(n) => LispVal::Number(*n),
        Value::Float(f) => LispVal::Float(*f),
        Value::Bool(b) => LispVal::Number(*b as i64),
        Value::Char(b) => LispVal::Char(*b),
        Value::Array(items) => match ty {
            Ty::Array(elem) if matches!(**elem, Ty::Char) => {
                let bytes: Vec<u8> = items
                    .iter()
                    .map(|x| match x {
                        Value::Char(b) => *b,
                        Value::Int(n) => *n as u8,
                        _ => 0,
                    })
                    .collect();
                LispVal::String(String::from_utf8_lossy(&bytes).into_owned())
            }
            Ty::Array(elem) => LispVal::Array(Rc::new(RefCell::new(
                items.iter().map(|x| value_to_lispval(x, elem)).collect(),
            ))),
            _ => LispVal::Nil,
        },
        Value::Struct(fields) => match ty {
            Ty::Struct(def) => LispVal::Struct(Rc::new(StructObj {
                type_name: def.name.clone(),
                fields: fields
                    .iter()
                    .zip(def.fields.iter())
                    .map(|(fv, (_, ft))| value_to_lispval(fv, ft))
                    .collect(),
            })),
            _ => LispVal::Nil,
        },
    }
}

/// Collect a proper list into a vector (improper tails are ignored).
pub(super) fn list_to_vec(list: &LispVal) -> Vec<LispVal> {
    let mut out = Vec::new();
    let mut cur = list;
    while let LispVal::Cons { car, cdr } = cur {
        out.push(car.as_ref().clone());
        cur = cdr;
    }
    out
}
