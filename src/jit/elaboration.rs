use super::*;
// ---------------------------------------------------------------------------
// Elaboration = lowering + monomorphic type checking, in one pass.
// ---------------------------------------------------------------------------

pub(super) type Scope = Vec<(String, Ty)>;

/// Source of a plain lambda's (params, body) for on-demand call-site
/// checking (#308) — backed by the environment in checker entry points.
pub(super) type LambdaSource<'a> = &'a dyn Fn(&str) -> Option<(Vec<String>, Vec<LispVal>)>;

/// Elaboration context: read-only access to the signatures of all registered
/// functions, so call sites can be type-checked (and self/forward references
/// resolved) while a body is being elaborated.
pub(super) struct Cx<'a> {
    pub(super) funcs: &'a [Rc<TypedFn>],
    pub(super) by_name: &'a HashMap<String, usize>,
    pub(super) structs: &'a HashMap<String, Rc<StructDef>>,
    /// Parametric nominals (0.3 HM generics), for construction rules.
    pub(super) generics: &'a HashMap<String, Rc<GenericDef>>,
    /// Declared schemes (experimental rows): axioms from `declare-type!`,
    /// consulted by the checker for callees the typed registry doesn't know.
    pub(super) declared: &'a HashMap<String, infer::Scheme>,
    /// The inference state for this definition: fresh variables + substitution
    /// (issue #135). Held behind a `RefCell` so the elaboration methods keep
    /// their `&self` signatures while still threading one shared substitution.
    pub(super) infer: RefCell<Infer>,
    /// Checker mode (#162): when set, elaboration types the *full* checkable
    /// lattice (lists, pairs, symbols, strings) and degrades an unknown/untyped
    /// call to `Any` (the gradual frontier) instead of rejecting it. The produced
    /// [`Core`] is *not* used (the checker never compiles), only the types are.
    /// When unset (the codegen path), behavior is unchanged: every type must be
    /// compileable and unknown calls are errors.
    pub(super) checking: bool,
    /// Call-site consultation of INFERRED schemes (#308): resolves an unknown
    /// callee that is a plain lambda to its (params, body) so its scheme can
    /// be derived on demand instead of degrading the call to `Any`. `None` in
    /// codegen mode and in contexts with no environment access.
    pub(super) resolver: Option<LambdaSource<'a>>,
    /// Per-run memo of schemes derived through `resolver`. `None` records a
    /// callee that could not be derived (variadic, failed its own check, not
    /// a lambda) so it is not re-attempted; such calls stay gradual — the
    /// callee's own error is reported at its own definition, not here.
    pub(super) derived: RefCell<HashMap<String, Option<infer::Scheme>>>,
    /// Monotype arrow assumptions for callees currently being checked
    /// up-stack (self/mutual recursion), consulted before re-entering the
    /// resolver so cycles terminate.
    pub(super) assumptions: RefCell<HashMap<String, Ty>>,
    /// Type-variable ids of enclosing in-flight checks. A nested callee's
    /// scheme is generalized *avoiding* these — see
    /// [`Infer::generalize_avoiding`].
    pub(super) avoid_gen: RefCell<Vec<u32>>,
}

impl Cx<'_> {
    /// A fresh type variable from this definition's inference state.
    fn fresh(&self) -> Ty {
        self.infer.borrow_mut().fresh()
    }

    /// A resolved operand type the EVALUATOR would reject for arithmetic /
    /// numeric comparison (#322): known non-numerics fail the check early;
    /// variables and Any stay gradual. Char is numeric (byte arithmetic).
    fn known_non_numeric(w: &Ty) -> bool {
        matches!(
            w,
            Ty::Str
                | Ty::Symbol
                | Ty::Bool
                | Ty::List(_)
                | Ty::Pair(_, _)
                | Ty::Record(_, _)
                | Ty::Struct(_)
                | Ty::Variant(_)
        )
    }
    /// Unify two types, extending the substitution (or report the clash).
    pub(super) fn unify(&self, a: &Ty, b: &Ty) -> Result<(), String> {
        self.infer.borrow_mut().unify(a, b)
    }
    /// Read a type's current representative under the substitution (for
    /// diagnostics; may still be a variable).
    pub(super) fn walk(&self, t: &Ty) -> Ty {
        self.infer.borrow().walk(t)
    }
    /// Resolve a type to a concrete type, erroring if it is still ambiguous.
    pub(super) fn resolve(&self, t: &Ty) -> Result<Ty, String> {
        self.infer.borrow().resolve(t)
    }

    fn elab(
        &self,
        form: &LispVal,
        scope: &mut Scope,
        max: &mut usize,
    ) -> Result<(Core, Ty), String> {
        match form {
            LispVal::Number(n) => Ok((Core::LitI(*n), Ty::Int64)),
            LispVal::Float(f) => Ok((Core::LitF(*f), Ty::Float64)),
            LispVal::Symbol(s) => {
                let name = &s.borrow().name;
                if name == "TRUE" {
                    return Ok((Core::LitI(1), Ty::Bool));
                }
                if name == "FALSE" {
                    return Ok((Core::LitI(0), Ty::Bool));
                }
                match scope.iter().rposition(|(n, _)| n == name) {
                    Some(slot) => Ok((Core::Var(slot), scope[slot].1.clone())),
                    // A free symbol in checker mode is some global we don't track
                    // — the gradual frontier (`Any`). The codegen path rejects it.
                    None if self.checking => Ok((Core::LitI(0), Ty::Any)),
                    None => Err(format!("unbound variable: {name}")),
                }
            }
            // Checker-only literals (#162): a boxed string is `string`, `nil`/`()`
            // is an empty list of some element type. The codegen path rejects them.
            LispVal::String(_) if self.checking => Ok((Core::LitI(0), Ty::Str)),
            LispVal::Char(_) if self.checking => Ok((Core::LitI(0), Ty::Char)),
            LispVal::Nil if self.checking => Ok((Core::LitI(0), Ty::List(Box::new(self.fresh())))),
            LispVal::Cons { .. } => {
                let items = list_to_vec(form);
                let head = match items.first() {
                    Some(LispVal::Symbol(s)) => s.borrow().name.clone(),
                    _ => return Err("typed core: call head must be a symbol".to_string()),
                };
                let args = &items[1..];
                match head.as_str() {
                    "+" | "-" | "*" | "/" | "MOD" => self.elab_bin(&head, args, scope, max),
                    "<" | ">" | "<=" | ">=" | "=" | "/=" => self.elab_cmp(&head, args, scope, max),
                    "NOT" => self.elab_not(args, scope, max),
                    "AND" | "OR" => self.elab_logic(&head, args, scope, max),
                    "IF" => self.elab_if(args, scope, max),
                    "LET-TYPED" => self.elab_let(args, scope, max),
                    "CHAR-CODE" => self.elab_char_code(args, scope, max),
                    "CODE-CHAR" => self.elab_code_char(args, scope, max),
                    "ARRAY" | "MAKE-ARRAY" => self.elab_array_new(args, scope, max),
                    "FETCH" | "AREF" => self.elab_fetch(args, scope, max),
                    "STORE" | "ASET" => self.elab_store(args, scope, max),
                    "ARRAY-LENGTH" => self.elab_array_len(args, scope, max),
                    // Checker-only forms (#162): list/pair processing + the
                    // untyped `let`/`progn`/`quote` that real `defun` bodies use.
                    "CONS" if self.checking => self.elab_cons(args, scope, max),
                    "CAR" | "FIRST" if self.checking => self.elab_car(args, scope, max),
                    "CDR" | "REST" if self.checking => self.elab_cdr(args, scope, max),
                    "LIST" if self.checking => self.elab_list(args, scope, max),
                    "NULL" | "NULL?" | "ENDP" if self.checking => self.elab_null(args, scope, max),
                    "RECORD-REF" if self.checking => self.elab_record_ref(args, scope, max),
                    "RECORD-NEW" if self.checking => self.elab_record_new(args, scope, max),
                    "RECORD-WITH" if self.checking => self.elab_record_with(args, scope, max),
                    "LET" if self.checking => self.elab_let(args, scope, max),
                    "PROGN" if self.checking => self.elab_body(args, scope, max),
                    "QUOTE" if self.checking => self.elab_quote(args),
                    "COND" if self.checking => self.elab_cond(args, scope, max),
                    "WHEN" | "UNLESS" if self.checking => self.elab_when(args, scope, max),
                    _ => self.elab_call(&head, args, scope, max),
                }
            }
            other if self.checking => {
                // Any other literal the checker doesn't model is gradually `Any`.
                let _ = other;
                Ok((Core::LitI(0), Ty::Any))
            }
            other => Err(format!("typed core: unsupported literal {other:?}")),
        }
    }

    fn elab_bin(
        &self,
        op: &str,
        args: &[LispVal],
        scope: &mut Scope,
        max: &mut usize,
    ) -> Result<(Core, Ty), String> {
        // `+` and `*` support 0–N args; `-` requires at least 1 (unary
        // negate, or N-ary left-fold). `/` and `MOD` are strictly BINARY in
        // the evaluator (`BuiltinFunc::Divide` in `apply_math_op`,
        // `builtins_core.rs`, and `mod` in `builtins_extra.rs` both reject
        // anything but exactly 2 arguments — no unary reciprocal, no
        // variadic chain division/modulus) and must be rejected at every
        // other arity here too, rather than silently accepted with a
        // made-up unary/N-ary meaning the evaluator doesn't have (issue
        // #209 — the same "checker/compiler disagrees with the evaluator on
        // what's valid" bug class as #202, but in the opposite,
        // over-permissive direction).
        let bop = match op {
            "+" => BinOp::Add,
            "-" => BinOp::Sub,
            "*" => BinOp::Mul,
            "/" => BinOp::Div,
            _ => BinOp::Mod,
        };
        if matches!(bop, BinOp::Div | BinOp::Mod) && args.len() != 2 {
            return Err(format!(
                "`{op}` requires exactly 2 arguments, got {}",
                args.len()
            ));
        }
        if matches!(bop, BinOp::Sub) && args.is_empty() {
            return Err(format!("`{op}` requires at least 1 argument"));
        }

        // 0-arg identity: (+ ) = 0, (* ) = 1
        if args.is_empty() {
            let identity: i64 = if matches!(bop, BinOp::Mul) { 1 } else { 0 };
            if self.checking {
                return Ok((Core::LitI(identity), Ty::Int64));
            }
            return Ok((Core::LitI(identity), Ty::Int64));
        }

        // 1-arg: unary identity — (+ x) = x, (* x) = x, (- x) = (- 0 x).
        // (`/`/`MOD` can never reach here: pinned to exactly 2 args above.)
        if args.len() == 1 {
            let (a, ta) = self.elab(&args[0], scope, max)?;
            if self.checking {
                return Ok((Core::LitI(0), self.walk(&ta)));
            }
            if matches!(bop, BinOp::Sub) {
                // negate: (- x) => (0 - x)
                let rt = self
                    .resolve(&ta)
                    .map_err(|_| format!("`{op}`: cannot infer operand type"))?;
                let num = rt
                    .as_num()
                    .ok_or_else(|| format!("`{op}` expects a numeric operand, got {rt:?}"))?;
                return Ok((
                    Core::Bin(num.into(), BinOp::Sub, Box::new(Core::LitI(0)), Box::new(a)),
                    rt,
                ));
            }
            // (+ x), (* x) — just return the arg
            return Ok((a, ta));
        }

        // ≥2 args: elaborate all, unify types pairwise, left-fold into BinOp
        // tree. For `/`/`MOD` this loop runs exactly once (arity pinned to 2
        // above); only `+`/`-`/`*` ever reach a 3+-ary fold here.
        let (mut acc, mut ty) = self.elab(&args[0], scope, max)?;
        for arg in &args[1..] {
            let (b, tb) = self.elab(arg, scope, max)?;
            if self.unify(&ty, &tb).is_err() {
                return Err(format!(
                    "`{op}` operands disagree: {:?} vs {:?}",
                    self.walk(&ty),
                    self.walk(&tb)
                ));
            }
            ty = self.walk(&ty);
            if self.checking {
                // In checker mode no codegen; keep accumulating the type.
                acc = Core::LitI(0);
                continue;
            }
            let rt = self
                .resolve(&ty)
                .map_err(|_| format!("`{op}`: cannot infer operand type"))?;
            let num = rt
                .as_num()
                .ok_or_else(|| format!("`{op}` expects numeric operands, got {rt:?}"))?;
            if matches!(bop, BinOp::Mod) && !matches!(num, NumTy::I) {
                return Err("`mod` is int64-only".to_string());
            }
            ty = rt.clone();
            acc = Core::Bin(num.into(), bop, Box::new(acc), Box::new(b));
        }
        if self.checking {
            let w = self.walk(&ty);
            // #322: the evaluator rejects non-numeric operands at runtime;
            // reject the KNOWN cases statically ((+ "a" "b") was `string`).
            if Self::known_non_numeric(&w) {
                return Err(format!(
                    "`{op}` expects numeric operands, got {}",
                    super::ty_name(&w)
                ));
            }
            return Ok((Core::LitI(0), w));
        }
        Ok((acc, ty))
    }

    fn elab_cmp(
        &self,
        op: &str,
        args: &[LispVal],
        scope: &mut Scope,
        max: &mut usize,
    ) -> Result<(Core, Ty), String> {
        if args.len() != 2 {
            return Err(format!("`{op}` expects 2 args, got {}", args.len()));
        }
        let (a, ta) = self.elab(&args[0], scope, max)?;
        let (b, tb) = self.elab(&args[1], scope, max)?;
        if self.unify(&ta, &tb).is_err() {
            return Err(format!(
                "`{op}` operands disagree: {:?} vs {:?}",
                self.walk(&ta),
                self.walk(&tb)
            ));
        }
        // Checker mode: comparison is `bool`; known non-comparable operand
        // kinds are rejected like the evaluator would at runtime (#322).
        if self.checking {
            let w = self.walk(&ta);
            if Self::known_non_numeric(&w) {
                return Err(format!(
                    "`{op}` expects comparable (numeric or char) operands, got {}",
                    super::ty_name(&w)
                ));
            }
            return Ok((Core::LitI(0), Ty::Bool));
        }
        let rt = self
            .resolve(&ta)
            .map_err(|_| format!("`{op}`: cannot infer operand type"))?;
        let num = rt
            .cmp_num()
            .ok_or_else(|| format!("`{op}` expects comparable operands, got {rt:?}"))?;
        let cop = match op {
            "<" => CmpOp::Lt,
            ">" => CmpOp::Gt,
            "<=" => CmpOp::Le,
            ">=" => CmpOp::Ge,
            "=" => CmpOp::Eq,
            _ => CmpOp::Ne,
        };
        Ok((
            Core::Cmp(num.into(), cop, Box::new(a), Box::new(b)),
            Ty::Bool,
        ))
    }

    fn elab_not(
        &self,
        args: &[LispVal],
        scope: &mut Scope,
        max: &mut usize,
    ) -> Result<(Core, Ty), String> {
        if args.len() != 1 {
            return Err(format!("`not` expects 1 arg, got {}", args.len()));
        }
        let (a, ta) = self.elab(&args[0], scope, max)?;
        // In checker mode `not` follows Lisp truthiness (any operand → bool); the
        // codegen path requires a real `bool`.
        if !self.checking && self.unify(&ta, &Ty::Bool).is_err() {
            return Err(format!("`not` expects bool, got {:?}", self.walk(&ta)));
        }
        Ok((Core::Not(Box::new(a)), Ty::Bool))
    }

    /// `AND`/`OR` are fully variadic special forms in the evaluator
    /// (`SpecialForm::And`/`Or`, `src/evaluator/special_forms.rs`: zero or
    /// more operands, short-circuiting left to right). This must accept any
    /// arity — rejecting `(and a b c)` as a type error would violate the
    /// checker's "never reject a program the interpreter runs" contract
    /// (issue #202) even though the compileable core only has a *binary*
    /// `Core::And`/`Or` node: 3+ operands fold right-associatively into
    /// nested binary nodes (`(and a b c)` → `And(a, And(b, c))`), which
    /// preserves the evaluator's short-circuit order exactly (evaluate left
    /// to right, stop at the first falsy/truthy operand). Every operand is
    /// required to be strictly `bool`-typed here (unlike the untyped
    /// evaluator's "returns the actual last value" semantics), so the folded
    /// result is that last operand's *boolean* value — observationally the
    /// same thing, since a `bool` word is already exactly 0 or 1.
    fn elab_logic(
        &self,
        op: &str,
        args: &[LispVal],
        scope: &mut Scope,
        max: &mut usize,
    ) -> Result<(Core, Ty), String> {
        // Checker mode follows Lisp truthiness: `and`/`or` take any number of
        // operands of any type and the result is one of them (heterogeneous)
        // → `any`. Still elaborate every operand so a genuine type error
        // nested inside one of them is reported. The codegen path below
        // requires real `bool` operands and yields `bool`.
        if self.checking {
            for a in args {
                self.elab(a, scope, max)?;
            }
            return Ok((Core::LitI(0), Ty::Any));
        }
        match args {
            // Vacuous identity: `(and)` is truthy, `(or)` is falsy — matches
            // the evaluator's zero-operand result (`T` / `NIL`).
            [] => {
                let lit = if op == "AND" { 1 } else { 0 };
                Ok((Core::LitI(lit), Ty::Bool))
            }
            // A single operand's (already-bool) value passes straight
            // through — `(and a)` and `(or a)` are both just `a`.
            [only] => {
                let (a, ta) = self.elab(only, scope, max)?;
                if self.unify(&ta, &Ty::Bool).is_err() {
                    return Err(format!(
                        "`{op}` expects bool operands, got {:?}",
                        self.walk(&ta)
                    ));
                }
                Ok((a, Ty::Bool))
            }
            [first, rest @ ..] => {
                let (a, ta) = self.elab(first, scope, max)?;
                if self.unify(&ta, &Ty::Bool).is_err() {
                    return Err(format!(
                        "`{op}` expects bool operands, got {:?}",
                        self.walk(&ta)
                    ));
                }
                // Recursively fold the remainder; bottoms out at the `[only]`
                // arm once one operand is left.
                let (b, _tb) = self.elab_logic(op, rest, scope, max)?;
                let node = if op == "AND" {
                    Core::And(Box::new(a), Box::new(b))
                } else {
                    Core::Or(Box::new(a), Box::new(b))
                };
                Ok((node, Ty::Bool))
            }
        }
    }

    fn elab_if(
        &self,
        args: &[LispVal],
        scope: &mut Scope,
        max: &mut usize,
    ) -> Result<(Core, Ty), String> {
        if args.len() != 3 {
            return Err(format!(
                "`if` expects (if cond then else), got {} args",
                args.len()
            ));
        }
        let (c, tc) = self.elab(&args[0], scope, max)?;
        // Checker mode follows Lisp truthiness: any condition is allowed. The
        // codegen path requires a real `bool`.
        if !self.checking && self.unify(&tc, &Ty::Bool).is_err() {
            return Err(format!(
                "`if` condition must be bool, got {:?}",
                self.walk(&tc)
            ));
        }
        let saved = scope.len();
        let (t, tt) = self.elab(&args[1], scope, max)?;
        scope.truncate(saved);
        let (e, te) = self.elab(&args[2], scope, max)?;
        scope.truncate(saved);
        if self.unify(&tt, &te).is_err() {
            return Err(format!(
                "`if` branches disagree: {:?} vs {:?}",
                self.walk(&tt),
                self.walk(&te)
            ));
        }
        Ok((
            Core::If(Box::new(c), Box::new(t), Box::new(e)),
            self.walk(&tt),
        ))
    }

    fn elab_let(
        &self,
        args: &[LispVal],
        scope: &mut Scope,
        max: &mut usize,
    ) -> Result<(Core, Ty), String> {
        let bindings = args
            .first()
            .map(list_to_vec)
            .ok_or_else(|| "`let-typed` needs a binding list".to_string())?;
        let body = &args[1..];
        if body.is_empty() {
            return Err("`let-typed` needs a body".to_string());
        }
        let saved = scope.len();
        let mut writes: Vec<(usize, Core)> = Vec::with_capacity(bindings.len());
        for b in &bindings {
            let parts = list_to_vec(b);
            // Two binding shapes: `(name type init)` pins the type explicitly,
            // `(name init)` leaves it to be inferred from the initializer
            // (issue #135 — the one surface-compatible inferable position).
            let (name, declared, init) = match parts.as_slice() {
                [LispVal::Symbol(n), declared_ty, init] => {
                    let mut infer = self.infer.borrow_mut();
                    let ty = parse_ty(declared_ty, &mut infer, self.structs)
                        .map_err(|e| format!("binding `{}`: {e}", n.borrow().name))?;
                    (n.borrow().name.clone(), Some(ty), init)
                }
                [LispVal::Symbol(n), init] => (n.borrow().name.clone(), None, init),
                _ => {
                    return Err(
                        "`let-typed` binding must be (name type init) or (name init)".to_string(),
                    );
                }
            };
            let (init_core, init_ty) = self.elab(init, scope, max)?;
            // An explicit annotation is a principal-type pin: unify it with the
            // inferred initializer type (must agree). Omitting it makes a fresh
            // variable that the initializer constrains — the inference path.
            let ty = match declared {
                Some(d) => {
                    if self.unify(&d, &init_ty).is_err() {
                        return Err(format!(
                            "binding `{name}` declared {d:?} but init is {:?}",
                            self.walk(&init_ty)
                        ));
                    }
                    d
                }
                None => {
                    // A fresh variable constrained by the initializer. It is NOT
                    // resolved here: an array binding's element type is only fixed
                    // by later `store`/`fetch` in the body, so resolution is
                    // deferred (the var flows via `walk` to every reference).
                    let v = self.fresh();
                    self.unify(&v, &init_ty)
                        .expect("fresh variable always unifies");
                    v
                }
            };
            let slot = scope.len();
            scope.push((name, ty));
            *max = (*max).max(scope.len());
            writes.push((slot, init_core));
        }
        let (body_core, body_ty) = self.elab_body(body, scope, max)?;
        scope.truncate(saved);
        let mut acc = body_core;
        for (slot, init_core) in writes.into_iter().rev() {
            acc = Core::Let(slot, Box::new(init_core), Box::new(acc));
        }
        Ok((acc, body_ty))
    }

    /// The derived-scheme path for an unknown callee (#308). Returns
    /// `Ok(None)` when nothing can be derived (the caller stays gradual).
    fn derived_call(
        &self,
        name: &str,
        args: &[LispVal],
        scope: &mut Scope,
        max: &mut usize,
    ) -> Result<Option<(Core, Ty)>, String> {
        // A callee currently being checked up-stack: use its in-flight
        // monotype arrow (standard monomorphic-recursion assumption).
        let assumed = self.assumptions.borrow().get(name).cloned();
        if let Some(Ty::Fn(ps, ret)) = assumed {
            if ps.len() != args.len() {
                return Ok(None);
            }
            for (a, p) in args.iter().zip(ps.iter()) {
                let (_, at) = self.elab(a, scope, max)?;
                self.unify(&at, p)
                    .map_err(|e| format!("in call to `{name}`: {e}"))?;
            }
            return Ok(Some((Core::LitI(0), self.walk(&ret))));
        }
        let memo = self.derived.borrow().get(name).cloned();
        let scheme = match memo {
            Some(cached) => cached,
            None => {
                let Some(resolver) = self.resolver else {
                    return Ok(None);
                };
                let derived = match resolver(name) {
                    Some((params, body)) => self.check_callee(name, &params, &body).ok(),
                    None => None,
                };
                self.derived
                    .borrow_mut()
                    .insert(name.to_string(), derived.clone());
                derived
            }
        };
        let Some(scheme) = scheme else {
            return Ok(None);
        };
        let inst = self.infer.borrow_mut().instantiate(&scheme);
        match inst {
            Ty::Fn(ps, ret) => {
                if ps.len() != args.len() {
                    return Err(format!(
                        "`{name}` expects {} args, got {} (inferred type)",
                        ps.len(),
                        args.len()
                    ));
                }
                for (a, p) in args.iter().zip(ps.iter()) {
                    let (_, at) = self.elab(a, scope, max)?;
                    self.unify(&at, p)
                        .map_err(|e| format!("in call to `{name}`: {e}"))?;
                }
                Ok(Some((Core::LitI(0), *ret)))
            }
            _ => Ok(None),
        }
    }

    /// Check a callee's own body inside this run and return its generalized
    /// scheme. The callee's fresh variables join `avoid_gen` for the duration
    /// (so deeper callees don't quantify them), and its arrow is generalized
    /// avoiding every *enclosing* in-flight variable.
    fn check_callee(
        &self,
        name: &str,
        params: &[String],
        body: &[LispVal],
    ) -> Result<infer::Scheme, String> {
        if body.is_empty() {
            return Err("empty body".to_string());
        }
        let ptys: Vec<Ty> = params.iter().map(|_| self.fresh()).collect();
        let ret = self.fresh();
        let own_vars: Vec<u32> = ptys
            .iter()
            .chain(std::iter::once(&ret))
            .filter_map(|t| match t {
                Ty::Var(v) => Some(*v),
                _ => None,
            })
            .collect();
        let arrow = Ty::Fn(ptys.clone(), Box::new(ret.clone()));
        self.assumptions
            .borrow_mut()
            .insert(name.to_string(), arrow.clone());
        self.avoid_gen.borrow_mut().extend(own_vars.iter().copied());
        let mut scope: Scope = params.iter().cloned().zip(ptys).collect();
        let mut max = scope.len();
        let outcome = self
            .elab_body(body, &mut scope, &mut max)
            .and_then(|(_, bt)| {
                self.unify(&bt, &ret)
                    .map_err(|_| "return type mismatch across branches".to_string())
            });
        self.assumptions.borrow_mut().remove(name);
        {
            let mut avoid = self.avoid_gen.borrow_mut();
            avoid.retain(|v| !own_vars.contains(v));
        }
        outcome?;
        let inf = self.infer.borrow();
        Ok(inf.generalize_avoiding(&arrow, &self.avoid_gen.borrow()))
    }

    fn elab_call(
        &self,
        name: &str,
        args: &[LispVal],
        scope: &mut Scope,
        max: &mut usize,
    ) -> Result<(Core, Ty), String> {
        let id = match self.by_name.get(name) {
            Some(id) => *id,
            // A **declared** scheme (experimental rows): the Lisp layer asserted
            // this callee's type (e.g. a row-polymorphic concept accessor), so
            // instantiate it, demand it of the arguments, and yield its result
            // type. Checker-only: codegen still rejects unknown calls.
            None if self.checking && self.declared.contains_key(name) => {
                let inst = {
                    let mut inf = self.infer.borrow_mut();
                    inf.instantiate(&self.declared[name])
                };
                match inst {
                    Ty::Fn(ps, ret) => {
                        if ps.len() != args.len() {
                            return Err(format!(
                                "`{name}` expects {} args, got {} (declared type)",
                                ps.len(),
                                args.len()
                            ));
                        }
                        for (a, p) in args.iter().zip(ps.iter()) {
                            let (_, at) = self.elab(a, scope, max)?;
                            self.unify(&at, p)
                                .map_err(|e| format!("in call to `{name}`: {e}"))?;
                        }
                        return Ok((Core::LitI(0), *ret));
                    }
                    // A declared non-arrow type says nothing useful about a
                    // call; stay gradual.
                    _ => {
                        for a in args {
                            self.elab(a, scope, max)?;
                        }
                        return Ok((Core::LitI(0), Ty::Any));
                    }
                }
            }
            // Derived schemes (#308): before conceding the gradual frontier,
            // try to derive the callee's own scheme — a recursion assumption,
            // a memoized result, or an on-demand check of its lambda body via
            // the resolver. This is what lets row types flow through helper
            // functions with no declare-type! axioms.
            None if self.checking => {
                if let Some(res) = self.derived_call(name, args, scope, max)? {
                    return Ok(res);
                }
                // Gradual frontier (#162): an unknown/untyped callee yields
                // `Any`. We still elaborate the arguments so type errors
                // *inside* them surface, but leave them unconstrained (the
                // callee makes no demand). The codegen path keeps rejecting
                // unknown calls.
                for a in args {
                    self.elab(a, scope, max)?;
                }
                return Ok((Core::LitI(0), Ty::Any));
            }
            // FUNCALL/APPLY are higher-order; they can't be compiled to native code
            // because the callee is a runtime value with no static type. Bounce the
            // call: elaborate args so inner type errors surface, yield Any. In
            // codegen mode the containing defun-typed will fail to resolve Any to a
            // concrete return type — use an untyped defun wrapper instead.
            None if matches!(name, "FUNCALL" | "APPLY") => {
                for a in args {
                    self.elab(a, scope, max)?;
                }
                return Ok((Core::LitI(0), Ty::Any));
            }
            None => return Err(format!("call to unknown function `{name}`")),
        };
        let callee = &self.funcs[id];
        let params = callee.params.borrow().clone();
        let ret = callee.ret.borrow().clone();
        if args.len() != params.len() {
            return Err(format!(
                "`{name}` expects {} args, got {}",
                params.len(),
                args.len()
            ));
        }
        let mut arg_cores = Vec::with_capacity(args.len());
        for (i, a) in args.iter().enumerate() {
            let (ac, at) = self.elab(a, scope, max)?;
            if self.unify(&at, &params[i].1).is_err() {
                return Err(format!(
                    "`{name}` arg {i} expects {}, got {}",
                    ty_name(&params[i].1),
                    ty_name(&self.walk(&at))
                ));
            }
            arg_cores.push(ac);
        }
        Ok((Core::Call(id, arg_cores), ret))
    }

    /// `(char-code c)` : char -> int64. Widening: the char word already holds
    /// the byte value, so the core is reused unchanged, only its type changes.
    fn elab_char_code(
        &self,
        args: &[LispVal],
        scope: &mut Scope,
        max: &mut usize,
    ) -> Result<(Core, Ty), String> {
        if args.len() != 1 {
            return Err(format!("`char-code` expects 1 arg, got {}", args.len()));
        }
        let (a, ta) = self.elab(&args[0], scope, max)?;
        // The evaluator's CHAR-CODE (`src/evaluator/builtins_core.rs`) accepts
        // a char *or* any non-empty string (the first char's code point). In
        // checker mode (`check-type`) that generosity must be preserved so the
        // checker never rejects a program the interpreter would run (#202,
        // #281): also accept a string / `(array char)` operand. In codegen
        // mode only a scalar `char` lowers to an unboxed word (a boxed
        // `LispVal::String`/array is not a scalar char), so the strict
        // char-only requirement stays there.
        if self.checking {
            let w = self.walk(&ta);
            let accepts = match &w {
                Ty::Char | Ty::Str | Ty::Any => true,
                Ty::Array(e) => matches!(**e, Ty::Char | Ty::Any | Ty::Var(_)),
                // An unconstrained variable defaults to char, as before.
                Ty::Var(_) => self.unify(&ta, &Ty::Char).is_ok(),
                _ => false,
            };
            if !accepts {
                return Err(format!(
                    "`char-code` expects char or string, got {:?}",
                    self.walk(&ta)
                ));
            }
            return Ok((a, Ty::Int64));
        }
        if self.unify(&ta, &Ty::Char).is_err() {
            return Err(format!(
                "`char-code` expects char, got {:?}",
                self.walk(&ta)
            ));
        }
        Ok((a, Ty::Int64))
    }

    /// `(code-char n)` : int64 -> char. Narrowing: mask the word to a byte.
    fn elab_code_char(
        &self,
        args: &[LispVal],
        scope: &mut Scope,
        max: &mut usize,
    ) -> Result<(Core, Ty), String> {
        if args.len() != 1 {
            return Err(format!("`code-char` expects 1 arg, got {}", args.len()));
        }
        let (a, ta) = self.elab(&args[0], scope, max)?;
        if self.unify(&ta, &Ty::Int64).is_err() {
            return Err(format!(
                "`code-char` expects int64, got {:?}",
                self.walk(&ta)
            ));
        }
        Ok((Core::ToChar(Box::new(a)), Ty::Char))
    }

    /// `(array n)` / `(make-array n)` : int64 -> (array α). The element type is a
    /// fresh variable, unified at each `fetch`/`store` site and resolved before
    /// codegen (#137/#138 — element types are inferred, never annotated here).
    fn elab_array_new(
        &self,
        args: &[LispVal],
        scope: &mut Scope,
        max: &mut usize,
    ) -> Result<(Core, Ty), String> {
        if args.len() != 1 {
            return Err(format!("`array` expects 1 arg (size), got {}", args.len()));
        }
        let (n, tn) = self.elab(&args[0], scope, max)?;
        if self.unify(&tn, &Ty::Int64).is_err() {
            return Err(format!(
                "`array` size must be int64, got {:?}",
                self.walk(&tn)
            ));
        }
        let elem = self.fresh();
        Ok((Core::ArrayNew(Box::new(n)), Ty::Array(Box::new(elem))))
    }

    /// `(fetch a i)` : (array α) int64 -> α. Bounds-checked at runtime.
    fn elab_fetch(
        &self,
        args: &[LispVal],
        scope: &mut Scope,
        max: &mut usize,
    ) -> Result<(Core, Ty), String> {
        if args.len() != 2 {
            return Err(format!("`fetch` expects 2 args, got {}", args.len()));
        }
        let (a, ta) = self.elab(&args[0], scope, max)?;
        let (i, ti) = self.elab(&args[1], scope, max)?;
        let elem = self.fresh();
        if self.unify(&ta, &Ty::Array(Box::new(elem.clone()))).is_err() {
            return Err(format!(
                "`fetch` expects an array, got {:?}",
                self.walk(&ta)
            ));
        }
        if self.unify(&ti, &Ty::Int64).is_err() {
            return Err(format!(
                "`fetch` index must be int64, got {:?}",
                self.walk(&ti)
            ));
        }
        let rt = self.walk(&elem);
        Ok((Core::ArrayGet(Box::new(a), Box::new(i)), rt))
    }

    /// `(store a i v)` : (array α) int64 α -> α. Evaluates to the stored value.
    fn elab_store(
        &self,
        args: &[LispVal],
        scope: &mut Scope,
        max: &mut usize,
    ) -> Result<(Core, Ty), String> {
        if args.len() != 3 {
            return Err(format!("`store` expects 3 args, got {}", args.len()));
        }
        let (a, ta) = self.elab(&args[0], scope, max)?;
        let (i, ti) = self.elab(&args[1], scope, max)?;
        let (v, tv) = self.elab(&args[2], scope, max)?;
        let elem = self.fresh();
        if self.unify(&ta, &Ty::Array(Box::new(elem.clone()))).is_err() {
            return Err(format!(
                "`store` expects an array, got {:?}",
                self.walk(&ta)
            ));
        }
        if self.unify(&ti, &Ty::Int64).is_err() {
            return Err(format!(
                "`store` index must be int64, got {:?}",
                self.walk(&ti)
            ));
        }
        if self.unify(&tv, &elem).is_err() {
            return Err(format!(
                "`store` value type {:?} does not match element type {:?}",
                self.walk(&tv),
                self.walk(&elem)
            ));
        }
        let rt = self.walk(&elem);
        Ok((Core::ArraySet(Box::new(a), Box::new(i), Box::new(v)), rt))
    }

    /// `(array-length a)` : (array α) -> int64.
    fn elab_array_len(
        &self,
        args: &[LispVal],
        scope: &mut Scope,
        max: &mut usize,
    ) -> Result<(Core, Ty), String> {
        if args.len() != 1 {
            return Err(format!("`array-length` expects 1 arg, got {}", args.len()));
        }
        let (a, ta) = self.elab(&args[0], scope, max)?;
        let elem = self.fresh();
        if self.unify(&ta, &Ty::Array(Box::new(elem))).is_err() {
            return Err(format!(
                "`array-length` expects an array, got {:?}",
                self.walk(&ta)
            ));
        }
        Ok((Core::ArrayLen(Box::new(a)), Ty::Int64))
    }

    // --- checker-only list/pair forms (#162) -------------------------------
    // These run only in `checking` mode; the produced `Core` is a placeholder
    // (`LitI(0)`) since the checker never compiles — only the *types* matter.

    /// `(cons x xs)` : α (list α) -> (list α). The list-cons view (lamedh lists
    /// are nested conses); proper homogeneous lists are the useful case to check.
    fn elab_cons(
        &self,
        args: &[LispVal],
        scope: &mut Scope,
        max: &mut usize,
    ) -> Result<(Core, Ty), String> {
        if args.len() != 2 {
            return Err(format!("`cons` expects 2 args, got {}", args.len()));
        }
        let (_, tx) = self.elab(&args[0], scope, max)?;
        let (_, txs) = self.elab(&args[1], scope, max)?;
        let lst = Ty::List(Box::new(tx));
        if self.unify(&txs, &lst).is_err() {
            return Err(format!(
                "`cons`: tail {:?} is not a list of the head's type",
                self.walk(&txs)
            ));
        }
        Ok((Core::LitI(0), self.walk(&lst)))
    }

    /// `(car xs)` : (list α) -> α.
    fn elab_car(
        &self,
        args: &[LispVal],
        scope: &mut Scope,
        max: &mut usize,
    ) -> Result<(Core, Ty), String> {
        if args.len() != 1 {
            return Err(format!("`car` expects 1 arg, got {}", args.len()));
        }
        let (_, txs) = self.elab(&args[0], scope, max)?;
        let elem = self.fresh();
        if self.unify(&txs, &Ty::List(Box::new(elem.clone()))).is_err() {
            return Err(format!("`car` expects a list, got {:?}", self.walk(&txs)));
        }
        Ok((Core::LitI(0), self.walk(&elem)))
    }

    /// `(cdr xs)` : (list α) -> (list α).
    fn elab_cdr(
        &self,
        args: &[LispVal],
        scope: &mut Scope,
        max: &mut usize,
    ) -> Result<(Core, Ty), String> {
        if args.len() != 1 {
            return Err(format!("`cdr` expects 1 arg, got {}", args.len()));
        }
        let (_, txs) = self.elab(&args[0], scope, max)?;
        let lst = Ty::List(Box::new(self.fresh()));
        if self.unify(&txs, &lst).is_err() {
            return Err(format!("`cdr` expects a list, got {:?}", self.walk(&txs)));
        }
        Ok((Core::LitI(0), self.walk(&lst)))
    }

    /// `(list e0 e1 …)` : all elements unified to α -> (list α). Empty → (list α).
    fn elab_list(
        &self,
        args: &[LispVal],
        scope: &mut Scope,
        max: &mut usize,
    ) -> Result<(Core, Ty), String> {
        let elem = self.fresh();
        for (i, a) in args.iter().enumerate() {
            let (_, ta) = self.elab(a, scope, max)?;
            if self.unify(&ta, &elem).is_err() {
                return Err(format!(
                    "`list` element {i} has type {:?}, expected {:?}",
                    self.walk(&ta),
                    self.walk(&elem)
                ));
            }
        }
        Ok((Core::LitI(0), Ty::List(Box::new(self.walk(&elem)))))
    }

    /// `(null xs)` : (list α) -> bool.
    fn elab_null(
        &self,
        args: &[LispVal],
        scope: &mut Scope,
        max: &mut usize,
    ) -> Result<(Core, Ty), String> {
        if args.len() != 1 {
            return Err(format!("`null` expects 1 arg, got {}", args.len()));
        }
        let (_, txs) = self.elab(&args[0], scope, max)?;
        if self.unify(&txs, &Ty::List(Box::new(self.fresh()))).is_err() {
            return Err(format!("`null` expects a list, got {:?}", self.walk(&txs)));
        }
        Ok((Core::LitI(0), Ty::Bool))
    }

    /// Extract the quoted field symbol from a `record-ref`/`record-with`
    /// field position: `(quote f)`. A computed field expression falls back
    /// to the dynamic path (return None -> caller degrades to `Any`).
    fn quoted_field(arg: &LispVal) -> Option<String> {
        let items = list_to_vec(arg);
        match items.as_slice() {
            [LispVal::Symbol(q), LispVal::Symbol(f)] if q.borrow().name == "QUOTE" => {
                Some(f.borrow().name.clone())
            }
            _ => None,
        }
    }

    /// `(record-new 'brand v1 … vn)` : the branded constructor rule (issue
    /// #308) — looks the brand up in the registry, unifies each argument
    /// with its field type, and returns the NOMINAL Ty::Struct. This is
    /// what makes `record-new` values carry their brand in checked code:
    /// passing a same-shaped-but-differently-branded record where a
    /// specific brand is demanded is a static error. An unquoted/unknown
    /// brand degrades to `Any` (the dynamic frontier).
    fn elab_record_new(
        &self,
        args: &[LispVal],
        scope: &mut Scope,
        max: &mut usize,
    ) -> Result<(Core, Ty), String> {
        if args.is_empty() {
            return Err("`record-new` expects a brand and field values".to_string());
        }
        let Some(brand) = Self::quoted_field(&args[0]) else {
            for a in &args[1..] {
                self.elab(a, scope, max)?;
            }
            return Ok((Core::LitI(0), Ty::Any));
        };
        let Some(def) = self.structs.get(&brand).cloned() else {
            // Parametric brand (0.3 HM generics): instantiate fresh type
            // arguments, demand the substituted field types, return the
            // application — `(record-new 'some v)` : (some α) with α = v's
            // type, absorbing into (option α) where demanded.
            if let Some(gdef) = self.generics.get(&brand).cloned() {
                if args.len() - 1 != gdef.fields.len() {
                    return Err(format!(
                        "`record-new`: {} has {} field(s), got {} value(s)",
                        brand,
                        gdef.fields.len(),
                        args.len() - 1
                    ));
                }
                let fresh: Vec<Ty> = (0..gdef.arity).map(|_| self.fresh()).collect();
                let m: HashMap<u32, Ty> =
                    (0..gdef.arity as u32).zip(fresh.iter().cloned()).collect();
                for (arg, (fname, fty)) in args[1..].iter().zip(gdef.fields.iter()) {
                    let (_, ta) = self.elab(arg, scope, max)?;
                    let want = Infer::subst_vars(fty, &m);
                    self.unify(&ta, &want).map_err(|e| {
                        format!("`record-new`: field {}: {e}", fname.to_lowercase())
                    })?;
                }
                return Ok((Core::LitI(0), Ty::App(gdef, fresh)));
            }
            // Unregistered at check time (e.g. forward use): dynamic frontier.
            for a in &args[1..] {
                self.elab(a, scope, max)?;
            }
            return Ok((Core::LitI(0), Ty::Any));
        };
        if args.len() - 1 != def.fields.len() {
            return Err(format!(
                "`record-new`: {} has {} field(s), got {} value(s)",
                brand,
                def.fields.len(),
                args.len() - 1
            ));
        }
        for (arg, (fname, fty)) in args[1..].iter().zip(def.fields.iter()) {
            let (_, ta) = self.elab(arg, scope, max)?;
            self.unify(&ta, fty)
                .map_err(|e| format!("`record-new`: field {}: {e}", fname.to_lowercase()))?;
        }
        Ok((Core::LitI(0), Ty::Struct(def)))
    }

    /// `(record-ref x 'f)` : (record ((f α)) ρ) → α — the checker-native row
    /// rule (issue #308). This is what makes row types DERIVED end-to-end:
    /// any function reading a field through the primitive infers an open
    /// record requirement with no declare-type! axioms. A struct argument
    /// satisfies it via subsumption (#299); a computed (non-quoted) field
    /// name degrades to `Any` like other dynamic frontiers.
    fn elab_record_ref(
        &self,
        args: &[LispVal],
        scope: &mut Scope,
        max: &mut usize,
    ) -> Result<(Core, Ty), String> {
        if args.len() != 2 {
            return Err(format!("`record-ref` expects 2 args, got {}", args.len()));
        }
        let (_, tx) = self.elab(&args[0], scope, max)?;
        let Some(field) = Self::quoted_field(&args[1]) else {
            return Ok((Core::LitI(0), Ty::Any));
        };
        let alpha = self.fresh();
        let rho = self.fresh();
        self.unify(
            &tx,
            &Ty::Record(vec![(field, alpha.clone())], Some(Box::new(rho))),
        )
        .map_err(|e| format!("`record-ref`: {e}"))?;
        Ok((Core::LitI(0), self.walk(&alpha)))
    }

    /// `(record-with x 'f v)` : (record ((f α)) ρ) α → same record type —
    /// typed functional update (issue #308).
    fn elab_record_with(
        &self,
        args: &[LispVal],
        scope: &mut Scope,
        max: &mut usize,
    ) -> Result<(Core, Ty), String> {
        if args.len() != 3 {
            return Err(format!("`record-with` expects 3 args, got {}", args.len()));
        }
        let (_, tx) = self.elab(&args[0], scope, max)?;
        let (_, tv) = self.elab(&args[2], scope, max)?;
        let Some(field) = Self::quoted_field(&args[1]) else {
            return Ok((Core::LitI(0), Ty::Any));
        };
        let alpha = self.fresh();
        let rho = self.fresh();
        self.unify(
            &tx,
            &Ty::Record(vec![(field, alpha.clone())], Some(Box::new(rho))),
        )
        .map_err(|e| format!("`record-with`: {e}"))?;
        self.unify(&tv, &alpha)
            .map_err(|e| format!("`record-with`: replacement value: {e}"))?;
        Ok((Core::LitI(0), self.walk(&tx)))
    }

    /// `(cond (test body…) …)` : every clause body unifies to one result type;
    /// tests follow Lisp truthiness (any type). With no clause, `any`.
    fn elab_cond(
        &self,
        clauses: &[LispVal],
        scope: &mut Scope,
        max: &mut usize,
    ) -> Result<(Core, Ty), String> {
        let result = self.fresh();
        let mut had_clause = false;
        for clause in clauses {
            let parts = list_to_vec(clause);
            if parts.is_empty() {
                continue;
            }
            let saved = scope.len();
            let (_, test_ty) = self.elab(&parts[0], scope, max)?;
            // A clause with no body yields the test value; otherwise the body.
            let bt = if parts.len() == 1 {
                test_ty
            } else {
                self.elab_body(&parts[1..], scope, max)?.1
            };
            scope.truncate(saved);
            if self.unify(&bt, &result).is_err() {
                return Err(format!(
                    "`cond` clauses disagree: {:?} vs {:?}",
                    self.walk(&bt),
                    self.walk(&result)
                ));
            }
            had_clause = true;
        }
        if had_clause {
            Ok((Core::LitI(0), self.walk(&result)))
        } else {
            Ok((Core::LitI(0), Ty::Any))
        }
    }

    /// `(when test body…)` / `(unless test body…)`: the test follows truthiness
    /// and the body is checked, but the value is the body *or* `nil`
    /// (heterogeneous), so the result is `any`.
    fn elab_when(
        &self,
        args: &[LispVal],
        scope: &mut Scope,
        max: &mut usize,
    ) -> Result<(Core, Ty), String> {
        if args.is_empty() {
            return Err("`when`/`unless` need a condition".to_string());
        }
        self.elab(&args[0], scope, max)?;
        if args.len() > 1 {
            self.elab_body(&args[1..], scope, max)?;
        }
        Ok((Core::LitI(0), Ty::Any))
    }

    /// `(quote x)`: a quoted symbol is `symbol`, quoted `()` is a list, any other
    /// quoted datum is `any` (the checker does not model quoted structure).
    fn elab_quote(&self, args: &[LispVal]) -> Result<(Core, Ty), String> {
        let ty = match args.first() {
            Some(LispVal::Symbol(_)) => Ty::Symbol,
            Some(LispVal::Nil) => Ty::List(Box::new(self.fresh())),
            _ => Ty::Any,
        };
        Ok((Core::LitI(0), ty))
    }

    pub(super) fn elab_body(
        &self,
        forms: &[LispVal],
        scope: &mut Scope,
        max: &mut usize,
    ) -> Result<(Core, Ty), String> {
        if forms.is_empty() {
            return Err("empty body".to_string());
        }
        let mut cores = Vec::with_capacity(forms.len());
        let mut last_ty = Ty::Int64;
        for f in forms {
            let (c, t) = self.elab(f, scope, max)?;
            cores.push(c);
            last_ty = t;
        }
        // A single form needs no wrapper; multiple forms sequence (earlier ones
        // run for their side effects, e.g. `store`), yielding the last's type.
        let core = if cores.len() == 1 {
            cores.pop().unwrap()
        } else {
            Core::Seq(cores)
        };
        Ok((core, last_ty))
    }
}
