use super::*;
// ---------------------------------------------------------------------------
// The function cell.
// ---------------------------------------------------------------------------

/// A typed function: its signature (the ABI), the typed core (reference
/// interpreter), and an optional hot-swappable compiled edition.
pub struct TypedFn {
    pub name: String,
    /// This function's own id in the registry — needed at compile time to
    /// recognize a *self* tail call (issue #133 Tier 1) as distinct from an
    /// ordinary call to another function.
    pub(super) id: usize,
    pub(super) params: RefCell<Vec<(String, Ty)>>,
    pub(super) ret: RefCell<Ty>,
    pub(super) core: RefCell<Option<Core>>,
    pub(super) slots: Cell<usize>,
    pub(super) compiled: RefCell<Option<Compiled>>,
    /// Native (Cranelift) edition. Like `compiled`, a call pins (`Rc`-clones) it,
    /// so a redefinition that swaps it out keeps the old code mapped until
    /// in-flight callers return (the `NativeEdition` owns its `JITModule`).
    #[cfg(feature = "jit")]
    pub(super) native: RefCell<Option<Rc<native::NativeEdition>>>,
    /// Stable heap word holding this function's current native entry pointer (or
    /// `0`). Other compiled functions bake this cell's *address* and load it to
    /// make direct calls; it is updated on (re)compile and cleared on deopt. A
    /// heap `Box` so the address is stable across registry `Vec` growth.
    #[cfg(feature = "jit")]
    pub(super) entry: Box<Cell<usize>>,
    pub(super) generation: Cell<u64>,
}

impl std::fmt::Debug for TypedFn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TypedFn")
            .field("name", &self.name)
            .field("params", &self.params.borrow())
            .field("ret", &self.ret.borrow())
            .field("defined", &self.core.borrow().is_some())
            .field("compiled", &self.compiled.borrow().is_some())
            .field("generation", &self.generation.get())
            .finish()
    }
}

impl TypedFn {
    fn placeholder(id: usize, name: String, params: Vec<(String, Ty)>, ret: Ty) -> TypedFn {
        let slots = params.len();
        TypedFn {
            name,
            id,
            params: RefCell::new(params),
            ret: RefCell::new(ret),
            core: RefCell::new(None),
            slots: Cell::new(slots),
            compiled: RefCell::new(None),
            #[cfg(feature = "jit")]
            native: RefCell::new(None),
            #[cfg(feature = "jit")]
            entry: Box::new(Cell::new(0)),
            generation: Cell::new(0),
        }
    }

    /// Address of this function's native-entry cell (stable; baked into other
    /// functions' compiled code for direct calls).
    #[cfg(feature = "jit")]
    fn entry_cell_addr(&self) -> usize {
        &*self.entry as *const Cell<usize> as usize
    }

    pub fn ret(&self) -> Ty {
        self.ret.borrow().clone()
    }
    pub fn params(&self) -> Vec<(String, Ty)> {
        self.params.borrow().clone()
    }
    pub fn is_compiled(&self) -> bool {
        self.compiled.borrow().is_some()
    }
    pub fn is_defined(&self) -> bool {
        self.core.borrow().is_some()
    }
    pub fn generation(&self) -> u64 {
        self.generation.get()
    }
    /// The number of slots in this function's activation frame (params + lets).
    pub fn n_slots(&self) -> usize {
        self.slots.get()
    }
    /// A clone of this function's typed-core IR, for structural inspection/verification.
    pub fn core_clone(&self) -> Option<Core> {
        self.core.borrow().clone()
    }

    #[cfg_attr(not(feature = "jit"), allow(unused_variables))]
    pub(super) fn compile_now(&self, funcs: &[Rc<TypedFn>]) {
        let c = self.core.borrow();
        if let Some(core) = c.as_ref() {
            *self.compiled.borrow_mut() = Some(compile_with_tco(core, self.id));
            // With the `jit` feature, also build a native edition. If Cranelift
            // codegen fails for any reason, fall back to the closure edition
            // rather than failing the definition. The entry cell is updated so
            // other compiled functions call this one's native code directly.
            #[cfg(feature = "jit")]
            {
                let n_params = self.params.borrow().len();
                let cell_addrs: Vec<usize> = funcs.iter().map(|f| f.entry_cell_addr()).collect();
                let param_counts: Vec<usize> =
                    funcs.iter().map(|f| f.params.borrow().len()).collect();
                match native::compile_native(
                    core,
                    self.id,
                    n_params,
                    self.slots.get(),
                    &cell_addrs,
                    &param_counts,
                ) {
                    Ok(ed) => {
                        self.entry.set(ed.entry_addr());
                        *self.native.borrow_mut() = Some(Rc::new(ed));
                    }
                    Err(_) => {
                        self.entry.set(0);
                        *self.native.borrow_mut() = None;
                    }
                }
            }
            self.generation.set(self.generation.get() + 1);
        }
    }

    pub(super) fn deoptimize(&self) {
        *self.compiled.borrow_mut() = None;
        #[cfg(feature = "jit")]
        {
            *self.native.borrow_mut() = None;
            self.entry.set(0);
        }
    }

    /// Invoke with already-unboxed words. Runs this function's own edition
    /// once, then drives the cross-function tail-call trampoline (issue
    /// #133 Tier 2a) until a call completes without leaving a pending tail
    /// call behind — O(1) native Rust stack for arbitrarily deep mutual/
    /// general tail recursion, since every iteration is a fresh top-level
    /// dispatch through [`Ctx::call`]'s own machinery (redefinition safety,
    /// entry-cell pinning, edition selection — all reused unchanged; no
    /// `Ctx`-level state persists between iterations except the one pending
    /// call being handed off).
    pub(super) fn invoke(&self, args: &[u64], ctx: &Ctx) -> u64 {
        let mut result = self.invoke_once(args, ctx);
        while let Some((id, tail_args)) = ctx.take_pending_tail() {
            result = ctx.funcs[id].invoke_once(&tail_args, ctx);
        }
        result
    }

    /// Run this function's own edition (native/closure/interpreter) exactly
    /// once, without following any pending cross-function tail call it may
    /// leave behind — that is [`TypedFn::invoke`]'s job. Builds the callee
    /// frame, dispatches to the compiled edition if present (pinning it for
    /// the call), else interprets.
    fn invoke_once(&self, args: &[u64], ctx: &Ctx) -> u64 {
        // Native edition first (pinned for the call so a redefinition can't free
        // the code out from under us). `args` are the parameter words directly;
        // the native function builds its own local frame.
        #[cfg(feature = "jit")]
        {
            let native = self.native.borrow().clone();
            if let Some(ed) = native {
                // The native prologue reads exactly `n_params` words from the
                // args pointer.  A stale caller compiled against an old signature
                // may pass the wrong count; skip native (fall through to the
                // interpreter edition) instead of reading out-of-bounds.
                if args.len() == self.params.borrow().len() {
                    let ctx_ptr = ctx as *const Ctx as *const core::ffi::c_void;
                    return unsafe { ed.call(args, ctx_ptr) };
                }
            }
        }
        // Interpreter/closure fallthrough. Guard against exactly the same
        // stale-arity hazard the native path above guards against (issue
        // #271): without this, a redefinition that changed this function's
        // arity could leave an old compiled caller (native or closure) still
        // passing the previous argument count until it is recompiled
        // (`recompile_all_except`), and `env[..args.len()].copy_from_slice`
        // below would index out of bounds and panic.
        if args.len() != self.params.borrow().len() {
            ctx.set_pending_error(format!(
                "{}: expected {} argument(s), got {} (stale call site after redefinition)",
                self.name,
                self.params.borrow().len(),
                args.len()
            ));
            return ctx.alloc_buffer(0) as u64;
        }
        let mut env = vec![0u64; self.slots.get()];
        env[..args.len()].copy_from_slice(args);
        let edition = self.compiled.borrow().clone();
        match edition {
            Some(f) => f(&mut env, ctx),
            None => {
                let core = self.core.borrow();
                match core.as_ref() {
                    Some(core) => eval_core(core, &mut env, ctx, self.id),
                    None => {
                        // Reached only for a `declare-typed`d forward
                        // reference that was never actually defined (issue
                        // #271); the public membrane's `call_inner` already
                        // guards this with `is_defined()`, but an internal
                        // `ctx.call` from another typed function's body has
                        // no such check.
                        ctx.set_pending_error(format!(
                            "typed function `{}` called before it was defined",
                            self.name
                        ));
                        ctx.alloc_buffer(0) as u64
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// The registry / front end.
// ---------------------------------------------------------------------------

/// A registry of typed functions, with the define/declare front end. Functions
/// are addressed by a stable id so calls survive redefinition.
#[derive(Default, Debug)]
pub struct Jit {
    pub(super) funcs: Vec<Rc<TypedFn>>,
    pub(super) by_name: HashMap<String, usize>,
    /// Registered typed struct definitions, by (uppercased) name. A struct name
    /// is usable as a type in `defun-typed` signatures, and its accessor
    /// functions (`make-NAME`, `NAME-FIELD`, `set-NAME-FIELD`) are generated as
    /// ordinary typed functions over the [`Core`] struct ops.
    pub(super) structs: HashMap<String, Rc<StructDef>>,
    /// Parametric nominals (0.3 HM generics): records and variant
    /// constructors with type parameters, plus parametric variants
    /// themselves. Uses appear as [`Ty::App`].
    pub(super) generics: HashMap<String, Rc<GenericDef>>,
    /// Declared sum types (variants): name -> closed constructor-brand set.
    /// A variant name is denotable in declared schemes; its constructors are
    /// ordinary registered records that absorb into the variant on unify.
    pub(super) variants: HashMap<String, Rc<VariantDef>>,
    /// **Declared** type schemes (experimental rows, `declare-type!`): axioms
    /// asserted by the Lisp layer for dynamically-implemented functions (e.g.
    /// concept accessors declared row-polymorphic). Consulted by the *checker*
    /// when a callee is otherwise unknown; never by codegen, so nothing here
    /// can reach the native tier.
    pub(super) declared: HashMap<String, infer::Scheme>,
}

impl Jit {
    pub fn new() -> Jit {
        Jit::default()
    }

    /// Register a **declared** scheme for `name` from its surface form, e.g.
    /// `(forall (r) (-> ((record ((amount int64)) r)) int64))`. Returns the
    /// rendered scheme. The declaration is an axiom, not a checked fact — the
    /// caller (the condensation layer) is responsible for generating the
    /// implementation in lockstep.
    pub fn declare_scheme(&mut self, name: &str, form: &LispVal) -> Result<String, String> {
        let scheme = parse_scheme_form(form, &self.structs, &self.variants, &self.generics)?;
        let rendered = infer::scheme_name(&scheme);
        self.declared.insert(name.to_string(), scheme);
        Ok(rendered)
    }

    /// Register a record type WITHOUT installing typed functions (issue
    /// #308 stage B): the branded StructDef enters the type language — the
    /// name becomes denotable, nominal in the checker, and row-subsumable
    /// (#299) — while field types may be ANY checkable type (list, pair,
    /// nested record, ...), not just the natively-storable set. Records
    /// whose fields all happen to be storable should use `define_struct`
    /// (the compiled tier) instead; `is_compileable` reports which tier a
    /// registered def actually supports.
    pub fn declare_record(&mut self, name: &str, field_specs: &LispVal) -> Result<(), String> {
        // Two-phase registration (recursive records): first register a
        // provisional def for the record itself and a forward stub for any
        // bare-symbol field type that is neither a builtin type word nor a
        // known type, so self- and mutual references resolve NOMINALLY
        // instead of degrading. Unification is by name and struct-into-row
        // expansion re-resolves through the registry snapshot, so the
        // provisional defs embedded in recursive field types stay honest.
        let specs = list_to_vec(field_specs);
        self.structs.entry(name.to_string()).or_insert_with(|| {
            Rc::new(StructDef {
                name: name.to_string(),
                fields: Vec::new(),
            })
        });
        for f in &specs {
            if let [_, LispVal::Symbol(t)] = list_to_vec(f).as_slice() {
                let tn = t.borrow().name.clone();
                // Words parse_declared_ty resolves itself (scalars, string,
                // symbol, any) and structural heads must never become stubs.
                let reserved = matches!(
                    tn.as_str(),
                    "INT64"
                        | "FLOAT64"
                        | "BOOL"
                        | "CHAR"
                        | "U8"
                        | "BYTE"
                        | "SYMBOL"
                        | "STRING"
                        | "ANY"
                        | "LIST"
                        | "ARRAY"
                        | "PAIR"
                        | "RECORD"
                );
                if !reserved
                    && !self.structs.contains_key(&tn)
                    && !self.variants.contains_key(&tn)
                    && !self.generics.contains_key(&tn)
                {
                    self.structs.insert(
                        tn.clone(),
                        Rc::new(StructDef {
                            name: tn,
                            fields: Vec::new(),
                        }),
                    );
                }
            }
        }
        let mut fields: Vec<(String, Ty)> = Vec::new();
        let vars = HashMap::new();
        for f in &specs {
            let parts = list_to_vec(f);
            match parts.as_slice() {
                [LispVal::Symbol(fname), fty] => {
                    let ty = parse_declared_ty(
                        fty,
                        &vars,
                        &self.structs,
                        &self.variants,
                        &self.generics,
                    )
                    .map_err(|e| format!("record `{name}`: {e}"))?;
                    fields.push((fname.borrow().name.clone(), ty));
                }
                _ => return Err(format!("record `{name}`: each field must be (name type)")),
            }
        }
        self.structs.insert(
            name.to_string(),
            Rc::new(StructDef {
                name: name.to_string(),
                fields,
            }),
        );
        Ok(())
    }

    /// Declare a PARAMETRIC record or variant constructor (0.3 HM
    /// generics): `params` are the type-parameter names (canonical ids in
    /// field types by position). Two-phase like `declare_record`, so
    /// self-referential fields ((defrecord (node a) ... (next (node a))))
    /// resolve. If `name` is a constructor of an already-declared
    /// parametric variant, the back-reference is recorded (App-into-App
    /// absorption).
    pub fn declare_generic_record(
        &mut self,
        name: &str,
        params: &[String],
        field_specs: &LispVal,
    ) -> Result<(), String> {
        reject_reserved_type_name(name)?;
        let arity = params.len();
        let variant = self
            .generics
            .values()
            .find(|g| !g.ctors.is_empty() && g.ctors.iter().any(|c| c == name))
            .map(|g| g.name.clone());
        // Provisional def: self-references resolve by name.
        self.generics.entry(name.to_string()).or_insert_with(|| {
            Rc::new(GenericDef {
                name: name.to_string(),
                arity,
                fields: Vec::new(),
                ctors: Vec::new(),
                variant: variant.clone(),
            })
        });
        let mut vars: HashMap<String, u32> = HashMap::new();
        for (i, p) in params.iter().enumerate() {
            if vars.insert(p.clone(), i as u32).is_some() {
                return Err(format!("record `{name}`: duplicate type parameter {p}"));
            }
        }
        let mut fields: Vec<(String, Ty)> = Vec::new();
        for f in list_to_vec(field_specs) {
            let parts = list_to_vec(&f);
            match parts.as_slice() {
                [LispVal::Symbol(fname), fty] => {
                    let ty = self
                        .parse_ty_with_generics(fty, &vars)
                        .map_err(|e| format!("record `{name}`: {e}"))?;
                    fields.push((fname.borrow().name.clone(), ty));
                }
                _ => return Err(format!("record `{name}`: each field must be (name type)")),
            }
        }
        self.generics.insert(
            name.to_string(),
            Rc::new(GenericDef {
                name: name.to_string(),
                arity,
                fields,
                ctors: Vec::new(),
                variant,
            }),
        );
        Ok(())
    }

    /// Declare a PARAMETRIC variant: the union name with its arity and
    /// constructor brands. Constructors are declared separately (with the
    /// same parameter list) via [`Self::declare_generic_record`].
    pub fn declare_generic_variant(
        &mut self,
        name: &str,
        arity: usize,
        ctors: Vec<String>,
    ) -> Result<(), String> {
        reject_reserved_type_name(name)?;
        for c in &ctors {
            reject_reserved_type_name(c)?;
        }
        if self.structs.contains_key(name) || self.variants.contains_key(name) {
            return Err(format!(
                "generic variant `{name}` collides with an existing type"
            ));
        }
        self.generics.insert(
            name.to_string(),
            Rc::new(GenericDef {
                name: name.to_string(),
                arity,
                fields: Vec::new(),
                ctors,
                variant: None,
            }),
        );
        Ok(())
    }

    /// `parse_declared_ty` with this registry's maps (helper for the
    /// generic-declaration path).
    fn parse_ty_with_generics(
        &self,
        form: &LispVal,
        vars: &HashMap<String, u32>,
    ) -> Result<Ty, String> {
        parse_declared_ty(form, vars, &self.structs, &self.variants, &self.generics)
    }

    /// Declare a sum type: `name` becomes the checker-level union of the
    /// constructor brands `ctors`. Constructors are registered separately as
    /// records (before or after — membership is by name). The variant name
    /// must not collide with a record name.
    pub fn declare_variant(&mut self, name: &str, ctors: Vec<String>) -> Result<(), String> {
        if self.structs.contains_key(name) {
            return Err(format!(
                "variant `{name}` collides with an existing record type"
            ));
        }
        self.variants.insert(
            name.to_string(),
            Rc::new(VariantDef {
                name: name.to_string(),
                ctors,
            }),
        );
        Ok(())
    }

    /// Constructor brand names of the registered variant `name`, if any.
    pub fn variant_ctors(&self, name: &str) -> Option<Vec<String>> {
        self.variants.get(name).map(|v| v.ctors.clone())
    }

    /// Whether the registered record/struct `name` is natively compileable
    /// (every field storable in the typed island) — the tier introspection
    /// behind `record-compiled-p`.
    pub fn record_compileable(&self, name: &str) -> Option<bool> {
        self.structs
            .get(name)
            .map(|d| is_compileable(&Ty::Struct(d.clone())))
            .or_else(|| self.generics.get(name).map(|_| false))
    }

    /// Ordered field names of the registered struct/record `name`, if any
    /// (issue #308: the field table behind the `record-ref` primitive).
    pub fn struct_field_names(&self, name: &str) -> Option<Vec<String>> {
        self.structs
            .get(name)
            .map(|def| def.fields.iter().map(|(n, _)| n.clone()).collect())
            .or_else(|| {
                self.generics
                    .get(name)
                    .map(|def| def.fields.iter().map(|(n, _)| n.clone()).collect())
            })
    }

    /// The rendered declared scheme for `name`, if one was registered.
    pub fn declared_scheme_name(&self, name: &str) -> Option<String> {
        self.declared.get(name).map(infer::scheme_name)
    }

    /// Register (or update) the signature for `name`.  Returns `(id, arity_changed)`.
    /// `arity_changed` is true only when this is a *redefinition* of an existing
    /// function whose parameter count differs — the caller must then recompile all
    /// other typed functions so they rebuild their call-site argument buffers with
    /// the correct size (see `recompile_all_except`).
    fn intern(&mut self, name: &str, params: Vec<(String, Ty)>, ret: Ty) -> (usize, bool) {
        if let Some(&id) = self.by_name.get(name) {
            let f = &self.funcs[id];
            let arity_changed = f.params.borrow().len() != params.len();
            *f.params.borrow_mut() = params;
            *f.ret.borrow_mut() = ret;
            if arity_changed {
                // Zero the entry cell immediately so any compiled caller that
                // tries the fast path before its own recompilation falls through
                // to the trampoline instead of calling native code with the wrong
                // number of argument words in the stack buffer.
                f.deoptimize();
            }
            (id, arity_changed)
        } else {
            let id = self.funcs.len();
            self.funcs.push(Rc::new(TypedFn::placeholder(
                id,
                name.to_string(),
                params,
                ret,
            )));
            self.by_name.insert(name.to_string(), id);
            (id, false)
        }
    }

    /// Deopt and recompile every typed function except `skip_id`.
    ///
    /// Called when `skip_id`'s arity changed on redefinition.  Every other compiled
    /// function may have baked a fixed-size argument buffer sized for the old arity
    /// into its native code (`emit_call` allocates `argc * 8` bytes on the stack);
    /// recompiling with the updated signature visible produces correct buffers and
    /// eliminates the out-of-bounds read that would otherwise occur.
    ///
    /// This is O(n_typed_fns) but redefinitions with arity changes are rare REPL
    /// events, not hot-path operations.
    fn recompile_all_except(&self, skip_id: usize) {
        let funcs: Vec<Rc<TypedFn>> = self.funcs.to_vec();
        for (id, f) in funcs.iter().enumerate() {
            if id != skip_id && f.core.borrow().is_some() {
                f.deoptimize();
                f.compile_now(&funcs);
            }
        }
    }

    /// Forward-declare a signature so mutually-recursive functions can reference
    /// each other before their bodies exist.
    pub fn declare(&mut self, name: &str, params: &[(&str, Ty)], ret: Ty) -> usize {
        let params = params
            .iter()
            .map(|(n, t)| ((*n).to_string(), t.clone()))
            .collect();
        // Declarations never compile; arity_changed is irrelevant here.
        self.intern(name, params, ret).0
    }

    /// Forward-declare from a `(declare-typed (name ret) ((arg ty)...))` form.
    /// Returns the (uppercased) name.
    pub fn declare_form(&mut self, form: &LispVal) -> Result<String, String> {
        let items = list_to_vec(form);
        match items.first() {
            Some(LispVal::Symbol(s)) if s.borrow().name == "DECLARE-TYPED" => {}
            _ => return Err("expected a (declare-typed ...) form".to_string()),
        }
        if items.len() != 3 {
            return Err("declare-typed: (declare-typed (name ret) ((arg ty)...))".to_string());
        }
        // A forward declaration has no body to infer from, so its types must be
        // concrete — reject a bare `array` (unpinned element) here.
        let mut infer = Infer::new();
        let (name, ret, params) = parse_signature(&items, &mut infer, &self.structs)?;
        if infer.resolve(&ret).is_err() || params.iter().any(|(_, t)| infer.resolve(t).is_err()) {
            return Err(format!(
                "{name}: a declaration needs concrete types; pin array elements as `(array T)`"
            ));
        }
        self.intern(&name, params, ret); // arity_changed ignored — no compilation at declaration time
        Ok(name)
    }

    /// The (parameter types, return type) of a registered function.
    pub fn signature(&self, name: &str) -> Option<(Vec<Ty>, Ty)> {
        let id = self.id(name)?;
        let f = &self.funcs[id];
        let ptys = f.params.borrow().iter().map(|(_, t)| t.clone()).collect();
        Some((ptys, f.ret.borrow().clone()))
    }

    pub fn named_signature(&self, name: &str) -> Option<(Vec<(String, Ty)>, Ty)> {
        let id = self.id(name)?;
        let f = &self.funcs[id];
        Some((f.params.borrow().clone(), f.ret.borrow().clone()))
    }

    /// Type-check and (eagerly) compile a `(defun-typed ...)` form. Returns the
    /// stable function id.
    pub fn define(&mut self, form: &LispVal) -> Result<usize, String> {
        let items = list_to_vec(form);
        match items.first() {
            Some(LispVal::Symbol(s)) if s.borrow().name == "DEFUN-TYPED" => {}
            _ => return Err("expected a (defun-typed ...) form".to_string()),
        }
        if items.len() < 4 {
            return Err("defun-typed: (defun-typed (name ret) ((arg ty)...) body...)".to_string());
        }

        // One inference state spans signature parsing and the body, so an array
        // parameter's element type (a fresh variable from `parse_ty`) is unified
        // by `fetch`/`store` in the body and resolved into the stored signature.
        let mut infer = Infer::new();
        let (name, ret, params) = parse_signature(&items, &mut infer, &self.structs)?;
        let mut scope: Scope = params.clone();

        // Register the signature *before* elaborating the body so a function can
        // call itself (and any already-declared peer).
        let (id, arity_changed) = self.intern(&name, params.clone(), ret.clone());

        let mut max_slots = scope.len();
        let (core, resolved_params, resolved_ret) = {
            let cx = Cx {
                declared: &self.declared,
                funcs: &self.funcs,
                by_name: &self.by_name,
                structs: &self.structs,
                generics: &self.generics,
                infer: RefCell::new(infer),
                checking: false,
                resolver: None,
                derived: RefCell::new(HashMap::new()),
                assumptions: RefCell::new(HashMap::new()),
                avoid_gen: RefCell::new(Vec::new()),
            };
            let (core, body_ty) = cx.elab_body(&items[3..], &mut scope, &mut max_slots)?;
            // The declared return type is a principal-type pin: the body's
            // inferred type must unify with it.
            if cx.unify(&body_ty, &ret).is_err() {
                return Err(format!(
                    "{name}: declared return {ret:?} but body has type {:?}",
                    cx.walk(&body_ty)
                ));
            }
            // Final resolve: drive the signature (params + return) to concrete
            // types, baking any inferred array element types. A still-ambiguous
            // type (e.g. an array param the body never indexes) is rejected here.
            let resolved_ret = cx
                .resolve(&ret)
                .map_err(|e| format!("{name} return type: {e}"))?;
            let mut resolved_params = Vec::with_capacity(params.len());
            for (pn, pt) in &params {
                let rt = cx
                    .resolve(pt)
                    .map_err(|e| format!("{name} parameter `{pn}`: {e}"))?;
                resolved_params.push((pn.clone(), rt));
            }
            (core, resolved_params, resolved_ret)
        };

        // Bake the resolved (concrete) signature so the membrane and callers see
        // monomorphic types, never variables.
        let f = self.funcs[id].clone();
        *f.params.borrow_mut() = resolved_params;
        *f.ret.borrow_mut() = resolved_ret;
        f.slots.set(max_slots);
        *f.core.borrow_mut() = Some(core);
        f.compile_now(&self.funcs);
        // If the arity changed on redefinition, every other compiled function may
        // have baked a now-wrong argument-buffer size for calls to this one.
        // Recompile them with the updated signature visible so they produce
        // correctly-sized buffers (and correct direct-native entry pointers).
        if arity_changed {
            self.recompile_all_except(id);
        }
        Ok(id)
    }

    /// Install a typed function from a *prebuilt* core (no surface elaboration),
    /// used for generated struct accessors. Eagerly compiles like `define`.
    fn install(
        &mut self,
        name: &str,
        params: Vec<(String, Ty)>,
        ret: Ty,
        core: Core,
        slots: usize,
    ) -> usize {
        let (id, arity_changed) = self.intern(name, params, ret);
        let f = self.funcs[id].clone();
        f.slots.set(slots);
        *f.core.borrow_mut() = Some(core);
        f.compile_now(&self.funcs);
        if arity_changed {
            self.recompile_all_except(id);
        }
        id
    }

    /// Attempt to type and compile an **un-annotated** function — HM firing
    /// invisibly under `defun` (#134 stretch goal). Every parameter starts as a
    /// fresh type variable; the body is elaborated under inference and the
    /// parameter + return types are resolved. It succeeds *only* if the whole
    /// body is a fully-inferable typed island (scalars/arrays/structs, arithmetic,
    /// and calls to already-typed functions) and every type resolves to a
    /// concrete monomorphic type. Anything outside the island — an untyped call,
    /// a `cons`, an ambiguous (polymorphic) numeric type — makes it fail, and the
    /// caller keeps the dynamic definition.
    ///
    /// On failure the registry is left exactly as it was (the provisional
    /// self-reference registration is rolled back), so this is a safe, silent,
    /// best-effort optimization.
    pub fn infer_untyped(
        &mut self,
        name: &str,
        params: &[String],
        body: &[LispVal],
    ) -> Result<usize, String> {
        if body.is_empty() {
            return Err("empty body".to_string());
        }
        let mut infer = Infer::new()
            .with_structs(std::rc::Rc::new(self.structs.clone()))
            .with_generics(std::rc::Rc::new(self.generics.clone()));
        let param_tys: Vec<(String, Ty)> =
            params.iter().map(|p| (p.clone(), infer.fresh())).collect();
        let ret_var = infer.fresh();

        // Provisionally register under the name so a self-recursive call resolves
        // during elaboration. A fresh func id is pushed; `prev` lets us roll the
        // name binding back on failure (the orphaned id is simply never reached).
        let new_id = self.funcs.len();
        self.funcs.push(Rc::new(TypedFn::placeholder(
            new_id,
            name.to_string(),
            param_tys.clone(),
            ret_var.clone(),
        )));
        let prev = self.by_name.insert(name.to_string(), new_id);

        let mut scope: Scope = param_tys.clone();
        let mut max_slots = scope.len();
        // (core, resolved params, resolved return) of a successful inference.
        type Inferred = (Core, Vec<(String, Ty)>, Ty);
        let outcome: Result<Inferred, String> = (|| {
            let cx = Cx {
                declared: &self.declared,
                funcs: &self.funcs,
                by_name: &self.by_name,
                structs: &self.structs,
                generics: &self.generics,
                infer: RefCell::new(infer),
                checking: false,
                resolver: None,
                derived: RefCell::new(HashMap::new()),
                assumptions: RefCell::new(HashMap::new()),
                avoid_gen: RefCell::new(Vec::new()),
            };
            let (core, body_ty) = cx.elab_body(body, &mut scope, &mut max_slots)?;
            cx.unify(&body_ty, &ret_var)
                .map_err(|_| "return type mismatch".to_string())?;
            let resolved_ret = cx.resolve(&ret_var)?;
            let mut resolved_params = Vec::with_capacity(param_tys.len());
            for (pn, pt) in &param_tys {
                resolved_params.push((pn.clone(), cx.resolve(pt)?));
            }
            Ok((core, resolved_params, resolved_ret))
        })();

        match outcome {
            Ok((core, resolved_params, resolved_ret)) => {
                let f = self.funcs[new_id].clone();
                *f.params.borrow_mut() = resolved_params;
                *f.ret.borrow_mut() = resolved_ret;
                f.slots.set(max_slots);
                *f.core.borrow_mut() = Some(core);
                f.compile_now(&self.funcs);
                Ok(new_id)
            }
            Err(e) => {
                // Roll back the name binding; the pushed func id is orphaned.
                match prev {
                    Some(p) => {
                        self.by_name.insert(name.to_string(), p);
                    }
                    None => {
                        self.by_name.remove(name);
                    }
                }
                Err(e)
            }
        }
    }

    /// The CODEGEN-path verdict for an un-annotated function WITHOUT
    /// installing anything: `Ok(())` when it would compile natively,
    /// `Err(reason)` with the concrete blocker otherwise. The dry-run twin
    /// of [`Self::infer_untyped`], powering `explain-compile`.
    pub fn compile_reason(
        &mut self,
        name: &str,
        params: &[String],
        body: &[LispVal],
    ) -> Result<(), String> {
        if body.is_empty() {
            return Err("empty body".to_string());
        }
        let mut infer = Infer::new()
            .with_structs(std::rc::Rc::new(self.structs.clone()))
            .with_generics(std::rc::Rc::new(self.generics.clone()));
        let param_tys: Vec<(String, Ty)> =
            params.iter().map(|p| (p.clone(), infer.fresh())).collect();
        let ret_var = infer.fresh();
        let new_id = self.funcs.len();
        self.funcs.push(Rc::new(TypedFn::placeholder(
            new_id,
            name.to_string(),
            param_tys.clone(),
            ret_var.clone(),
        )));
        let prev = self.by_name.insert(name.to_string(), new_id);
        let mut scope: Scope = param_tys.clone();
        let mut max_slots = scope.len();
        let outcome: Result<(), String> = (|| {
            let cx = Cx {
                declared: &self.declared,
                funcs: &self.funcs,
                by_name: &self.by_name,
                structs: &self.structs,
                generics: &self.generics,
                infer: RefCell::new(infer),
                checking: false,
                resolver: None,
                derived: RefCell::new(HashMap::new()),
                assumptions: RefCell::new(HashMap::new()),
                avoid_gen: RefCell::new(Vec::new()),
            };
            let (_core, body_ty) = cx.elab_body(body, &mut scope, &mut max_slots)?;
            cx.unify(&body_ty, &ret_var)
                .map_err(|_| "return type mismatch".to_string())?;
            cx.resolve(&ret_var)?;
            for (_, pt) in &param_tys {
                cx.resolve(pt)?;
            }
            Ok(())
        })();
        // ALWAYS roll back — explanation is side-effect-free.
        match prev {
            Some(p) => {
                self.by_name.insert(name.to_string(), p);
            }
            None => {
                self.by_name.remove(name);
            }
        }
        outcome
    }

    /// Like [`Self::infer_untyped`] but accepts **partial type hints** per param and
    /// for the return type. `Some(ty)` pins the slot to that type; `None` inserts
    /// a fresh inference variable. This is the compilation back-end for `defun*`.
    ///
    /// Returns `(id, sig_string)` on success, where `sig_string` is the resolved
    /// full signature (params + return) formatted as surface-syntax text, so the
    /// caller can emit a note when types were inferred. Rolls back on failure.
    pub fn define_partial(
        &mut self,
        name: &str,
        params: &[(String, Option<Ty>)],
        ret_hint: Option<Ty>,
        body: &[LispVal],
    ) -> Result<(usize, String), String> {
        if body.is_empty() {
            return Err("empty body".to_string());
        }
        let mut infer = Infer::new();
        // Pin specified slots; fresh var for unspecified slots.
        let param_tys: Vec<(String, Ty)> = params
            .iter()
            .map(|(n, opt)| (n.clone(), opt.clone().unwrap_or_else(|| infer.fresh())))
            .collect();
        let ret_var = ret_hint.unwrap_or_else(|| infer.fresh());

        let new_id = self.funcs.len();
        self.funcs.push(Rc::new(TypedFn::placeholder(
            new_id,
            name.to_string(),
            param_tys.clone(),
            ret_var.clone(),
        )));
        let prev = self.by_name.insert(name.to_string(), new_id);

        let mut scope: Scope = param_tys.clone();
        let mut max_slots = scope.len();
        type Inferred = (Core, Vec<(String, Ty)>, Ty);
        let outcome: Result<Inferred, String> = (|| {
            let cx = Cx {
                declared: &self.declared,
                funcs: &self.funcs,
                by_name: &self.by_name,
                structs: &self.structs,
                generics: &self.generics,
                infer: RefCell::new(infer),
                checking: false,
                resolver: None,
                derived: RefCell::new(HashMap::new()),
                assumptions: RefCell::new(HashMap::new()),
                avoid_gen: RefCell::new(Vec::new()),
            };
            let (core, body_ty) = cx.elab_body(body, &mut scope, &mut max_slots)?;
            cx.unify(&body_ty, &ret_var)
                .map_err(|_| "return type mismatch".to_string())?;
            let resolved_ret = cx.resolve(&ret_var)?;
            let mut resolved_params = Vec::with_capacity(param_tys.len());
            for (pn, pt) in &param_tys {
                resolved_params.push((pn.clone(), cx.resolve(pt)?));
            }
            Ok((core, resolved_params, resolved_ret))
        })();

        match outcome {
            Ok((core, resolved_params, resolved_ret)) => {
                let sig = resolved_params
                    .iter()
                    .map(|(n, t)| format!("({n} {})", ty_name(t)))
                    .collect::<Vec<_>>()
                    .join(" ");
                let sig_str = format!("{sig} -> {}", ty_name(&resolved_ret));
                let f = self.funcs[new_id].clone();
                *f.params.borrow_mut() = resolved_params;
                *f.ret.borrow_mut() = resolved_ret;
                f.slots.set(max_slots);
                *f.core.borrow_mut() = Some(core);
                f.compile_now(&self.funcs);
                Ok((new_id, sig_str))
            }
            Err(e) => {
                match prev {
                    Some(p) => {
                        self.by_name.insert(name.to_string(), p);
                    }
                    None => {
                        self.by_name.remove(name);
                    }
                }
                Err(e)
            }
        }
    }

    /// **Type-check** an un-annotated function without compiling it (#162) — the
    /// non-compiled checker. Runs full HM inference (checker mode: lists, pairs,
    /// symbols, strings, and a gradual `Any` at the operative/untyped frontier)
    /// and returns the function's generalized type as a printable scheme, or a
    /// type error. Catches software type errors even when the type is *not*
    /// compileable. Installs nothing and leaves the registry untouched (the
    /// provisional self-reference is rolled back).
    pub fn check_untyped(
        &mut self,
        name: &str,
        params: &[String],
        body: &[LispVal],
        resolver: Option<super::elaboration::LambdaSource<'_>>,
    ) -> Result<String, String> {
        if body.is_empty() {
            return Err("empty body".to_string());
        }
        let mut infer = Infer::new();
        let param_tys: Vec<(String, Ty)> =
            params.iter().map(|p| (p.clone(), infer.fresh())).collect();
        let ret_var = infer.fresh();

        // Provisional self-reference so recursive calls type against this function
        // (rolled back unconditionally — the checker never installs anything).
        let new_id = self.funcs.len();
        self.funcs.push(Rc::new(TypedFn::placeholder(
            new_id,
            name.to_string(),
            param_tys.clone(),
            ret_var.clone(),
        )));
        let prev = self.by_name.insert(name.to_string(), new_id);

        let mut scope: Scope = param_tys.clone();
        let mut max_slots = scope.len();
        let outcome: Result<String, String> = (|| {
            // This function's own in-flight variables seed `avoid_gen` so a
            // callee checked on demand (via `resolver`) never quantifies them.
            let own_vars: Vec<u32> = param_tys
                .iter()
                .map(|(_, t)| t)
                .chain(std::iter::once(&ret_var))
                .filter_map(|t| match t {
                    Ty::Var(v) => Some(*v),
                    _ => None,
                })
                .collect();
            let cx = Cx {
                declared: &self.declared,
                funcs: &self.funcs,
                by_name: &self.by_name,
                structs: &self.structs,
                generics: &self.generics,
                infer: RefCell::new(infer),
                checking: true,
                resolver,
                derived: RefCell::new(HashMap::new()),
                assumptions: RefCell::new(HashMap::new()),
                avoid_gen: RefCell::new(own_vars),
            };
            let (_core, body_ty) = cx.elab_body(body, &mut scope, &mut max_slots)?;
            cx.unify(&body_ty, &ret_var)
                .map_err(|_| "return type mismatch across branches".to_string())?;
            let inf = cx.infer.borrow();
            let arrow = Ty::Fn(
                param_tys.iter().map(|(_, t)| inf.zonk(t)).collect(),
                Box::new(inf.zonk(&ret_var)),
            );
            Ok(infer::scheme_name(&inf.generalize(&arrow)))
        })();

        // Always roll back — checking is side-effect-free.
        match prev {
            Some(p) => {
                self.by_name.insert(name.to_string(), p);
            }
            None => {
                self.by_name.remove(name);
            }
        }
        outcome
    }

    /// One-pass analysis of an un-annotated function (#162 stage 4): run the
    /// **checker** first (so a genuine type error is reported even when nothing
    /// compiles), then gate native codegen on compileability. The two pipelines
    /// are intentionally separate — the checker is permissive (gradual `Any`,
    /// lists), the compiler is strict (compileable monomorphic types only) — so a
    /// function can be *checked* without being *compiled*.
    pub fn analyze_untyped(
        &mut self,
        name: &str,
        params: &[String],
        body: &[LispVal],
        resolver: Option<super::elaboration::LambdaSource<'_>>,
    ) -> Analysis {
        match self.check_untyped(name, params, body, resolver) {
            Err(e) => Analysis::TypeError(e),
            Ok(scheme) => {
                if self.infer_untyped(name, params, body).is_ok() {
                    Analysis::Native(scheme)
                } else {
                    Analysis::Checked(scheme)
                }
            }
        }
    }

    /// Type-check a **single expression** without wrapping it in a function.
    /// Elaborates `expr` in checker mode with an empty scope and returns its
    /// inferred type as a human-readable string (e.g. `"int64"`, `"float64"`,
    /// `"(forall (a) (list a))"`) or an error. Used by `(check-type <expr>)`.
    pub fn check_expr(
        &mut self,
        expr: &LispVal,
        resolver: Option<super::elaboration::LambdaSource<'_>>,
    ) -> Result<String, String> {
        let infer = Infer::new()
            .with_structs(std::rc::Rc::new(self.structs.clone()))
            .with_generics(std::rc::Rc::new(self.generics.clone()));
        let mut scope = Scope::new();
        let mut max_slots = 0;
        let cx = Cx {
            declared: &self.declared,
            funcs: &self.funcs,
            by_name: &self.by_name,
            structs: &self.structs,
            generics: &self.generics,
            infer: RefCell::new(infer),
            checking: true,
            resolver,
            derived: RefCell::new(HashMap::new()),
            assumptions: RefCell::new(HashMap::new()),
            avoid_gen: RefCell::new(Vec::new()),
        };
        let (_, ty) = cx.elab_body(std::slice::from_ref(expr), &mut scope, &mut max_slots)?;
        let resolved = cx.infer.borrow().zonk(&ty);
        let scheme = cx.infer.borrow().generalize(&resolved);
        Ok(super::infer::scheme_name(&scheme))
    }

    /// Define a typed struct from `(defstruct-typed Name (field type)...)`.
    /// Registers the struct type (usable in `defun-typed` signatures) and
    /// generates its accessor functions over the [`Core`] struct ops:
    /// `make-NAME`, `NAME-FIELD` (getter), `set-NAME-FIELD` (setter). Returns the
    /// generated (uppercased) function names so the caller can install membrane
    /// entries. Fields are laid out as a flat one-word-per-field buffer.
    pub fn define_struct(&mut self, form: &LispVal) -> Result<Vec<String>, String> {
        let items = list_to_vec(form);
        match items.first() {
            Some(LispVal::Symbol(s)) if s.borrow().name == "DEFSTRUCT-TYPED" => {}
            _ => return Err("expected a (defstruct-typed ...) form".to_string()),
        }
        if items.len() < 2 {
            return Err("defstruct-typed: (defstruct-typed Name (field type)...)".to_string());
        }
        let name = match &items[1] {
            LispVal::Symbol(s) => s.borrow().name.clone(),
            _ => return Err("defstruct-typed: struct name must be a symbol".to_string()),
        };
        // Field types must be concrete (a struct's layout is fixed); arrays may
        // pin or be elided, but an elided element here has nothing to infer it.
        let mut infer = Infer::new();
        let mut fields: Vec<(String, Ty)> = Vec::new();
        for f in &items[2..] {
            let parts = list_to_vec(f);
            match parts.as_slice() {
                [LispVal::Symbol(fname), fty] => {
                    let ty = parse_ty(fty, &mut infer, &self.structs)?;
                    let ty = infer.resolve(&ty).map_err(|_| {
                        format!(
                            "struct `{name}` field `{}` needs a concrete type",
                            fname.borrow().name
                        )
                    })?;
                    fields.push((fname.borrow().name.clone(), ty));
                }
                _ => return Err("each field must be (field-name type)".to_string()),
            }
        }
        if fields.is_empty() {
            return Err(format!("struct `{name}` must have at least one field"));
        }

        let def = Rc::new(StructDef {
            name: name.clone(),
            fields: fields.clone(),
        });
        self.structs.insert(name.clone(), def.clone());
        let struct_ty = Ty::Struct(def);

        let mut generated = Vec::new();

        // Constructor: make-NAME : (f0 .. fn) -> NAME.
        let ctor = format!("MAKE-{name}");
        let ctor_core = Core::StructNew((0..fields.len()).map(Core::Var).collect());
        self.install(
            &ctor,
            fields.clone(),
            struct_ty.clone(),
            ctor_core,
            fields.len(),
        );
        generated.push(ctor);

        // Per-field getter NAME-FIELD and setter set-NAME-FIELD.
        for (i, (fname, fty)) in fields.iter().enumerate() {
            let getter = format!("{name}-{fname}");
            self.install(
                &getter,
                vec![("SELF".to_string(), struct_ty.clone())],
                fty.clone(),
                Core::FieldGet(Box::new(Core::Var(0)), i),
                1,
            );
            generated.push(getter);

            let setter = format!("SET-{name}-{fname}");
            self.install(
                &setter,
                vec![
                    ("SELF".to_string(), struct_ty.clone()),
                    ("V".to_string(), fty.clone()),
                ],
                fty.clone(),
                Core::FieldSet(Box::new(Core::Var(0)), i, Box::new(Core::Var(1))),
                2,
            );
            generated.push(setter);
        }
        Ok(generated)
    }

    pub fn id(&self, name: &str) -> Option<usize> {
        // Names are case-normalized to uppercase (matching the reader), so
        // callers may use either case.
        self.by_name.get(&name.to_uppercase()).copied()
    }

    /// The (uppercased) name of the function with the given id.
    pub fn name_of(&self, id: usize) -> Option<String> {
        self.funcs.get(id).map(|f| f.name.clone())
    }
    pub fn get(&self, name: &str) -> Option<&Rc<TypedFn>> {
        self.id(name).map(|i| &self.funcs[i])
    }

    pub(super) fn ctx(&self) -> Ctx<'_> {
        Ctx {
            funcs: &self.funcs,
            arena: RefCell::new(Vec::new()),
            pending_tail: RefCell::new(None),
            overflow: Cell::new(false),
            div_by_zero: Cell::new(false),
            depth: Cell::new(0),
            pending_error: RefCell::new(None),
        }
    }

    /// Call a function by name with boxed [`Value`]s; type-checks the arguments
    /// against the signature and re-boxes the result. This is the public membrane.
    pub fn call(&self, name: &str, args: &[Value]) -> Result<Value, String> {
        Ok(self.call_inner(name, args)?.0)
    }

    /// Like [`Jit::call`], but also reads back the post-call contents of any
    /// flat-scalar-array argument (issue #216: `LispVal::Array` has interior
    /// mutability and `STORE`'s docs promise in-place mutation for all
    /// references, but the typed runtime's arena buffer is a *copy* of the
    /// caller's array by construction — `Value::to_word`'s `Array` arm
    /// always allocates a fresh arena buffer — so without this, a
    /// `store`/`aset` inside a `defun-typed` body silently never reached the
    /// caller). `updated[i]` is `Some(new_value)` whenever argument `i`'s
    /// *type* is a flat scalar array (see `is_flat_scalar_array`); it is
    /// populated whether or not the callee actually mutated that argument (a
    /// redundant copy-out for a pure-reader function — simpler and still
    /// correct, at a small extra-copy cost). Callers holding the original
    /// backing store (e.g. a `LispVal`) decide whether *their* specific
    /// argument is actually alias-eligible — e.g. a `LispVal::String` passed
    /// to an `(array char)` parameter type-checks the same as a genuine
    /// `LispVal::Array` here, but has no interior mutability to write back
    /// into, so the caller must skip it.
    pub fn call_with_array_writeback(&self, name: &str, args: &[Value]) -> WritebackResult {
        self.call_inner(name, args)
    }

    fn call_inner(&self, name: &str, args: &[Value]) -> WritebackResult {
        let id = self
            .id(name)
            .ok_or_else(|| format!("unknown function `{name}`"))?;
        let f = &self.funcs[id];
        if !f.is_defined() {
            return Err(format!("{name}: declared but not defined"));
        }
        let params = f.params.borrow();
        if args.len() != params.len() {
            return Err(format!(
                "{name}: expected {} args, got {}",
                params.len(),
                args.len()
            ));
        }
        // The arena (in `ctx`) must outlive both arg lowering (which allocates
        // compound buffers into it) and result/write-back reading (which
        // copies them out), so it is created up front and dropped only at
        // return.
        let ctx = self.ctx();
        let mut words = Vec::with_capacity(args.len());
        let mut tys = Vec::with_capacity(args.len());
        for (a, (_, ty)) in args.iter().zip(params.iter()) {
            words.push(a.to_word(ty, &ctx)?);
            tys.push(ty.clone());
        }
        let ret = f.ret.borrow().clone();
        drop(params);
        let w = f.invoke(&words, &ctx);
        // A reachable-panic condition (issue #271: oversized allocation,
        // recursion cap, undefined callee, stale-arity call site) takes
        // priority over the ordinary overflow/div-by-zero flags below — it
        // means the computed word `w` is a meaningless placeholder, not a
        // real (if flagged) result.
        if let Some(msg) = ctx.pending_error.borrow_mut().take() {
            return Err(msg);
        }
        let flags = JitFlags {
            overflow: ctx.overflow.get(),
            div_by_zero: ctx.div_by_zero.get(),
        };
        let result = Value::from_word(w, &ret);
        let updated = words
            .iter()
            .zip(tys.iter())
            .map(|(w, ty)| is_flat_scalar_array(ty).then(|| Value::from_word(*w, ty)))
            .collect();
        Ok((result, updated, flags))
    }

    /// Convenience for callers holding `LispVal`s: maps `Number`/`Float` to
    /// [`Value`], calls, and re-boxes to `Number`/`Float`/(`Number 0/1` for bool).
    ///
    /// Issue #216: like the interpreter's own typed membrane
    /// (`make_typed_native` in `src/evaluator/functions.rs`), this writes a
    /// mutated flat-scalar-array argument back into the caller's original
    /// `LispVal::Array` in place — see `Jit::call_with_array_writeback` and
    /// `is_flat_scalar_array` for the exact scope and the `LispVal::String`
    /// (no interior mutability) exclusion.
    pub fn call_lisp(&self, name: &str, args: &[LispVal]) -> Result<LispVal, String> {
        let (ptys, ret) = self
            .signature(name)
            .ok_or_else(|| format!("unknown function `{name}`"))?;
        if args.len() != ptys.len() {
            return Err(format!(
                "{name}: expected {} args, got {}",
                ptys.len(),
                args.len()
            ));
        }
        let mut vals = Vec::with_capacity(args.len());
        for (a, ty) in args.iter().zip(ptys.iter()) {
            vals.push(lispval_to_value(a, ty)?);
        }
        let (result, updated, _flags) = self.call_with_array_writeback(name, &vals)?;
        for (orig, upd) in args.iter().zip(updated) {
            if let (LispVal::Array(rc), Some(Value::Array(items))) = (orig, upd) {
                // Every item here is scalar (`is_flat_scalar_array` excludes
                // nested compounds), so this doesn't need a `Ty` to
                // disambiguate the char-array-as-string case the way
                // `value_to_lispval`'s `Value::Array` arm does.
                let new_items: Vec<LispVal> = items
                    .into_iter()
                    .map(|it| match it {
                        Value::Int(n) => LispVal::Number(n),
                        Value::Float(f) => LispVal::Float(f),
                        Value::Bool(b) => LispVal::Number(b as i64),
                        Value::Char(b) => LispVal::Char(b),
                        Value::Array(_) | Value::Struct(_) => {
                            unreachable!("flat scalar array write-back produced a compound element")
                        }
                    })
                    .collect();
                *rc.borrow_mut() = new_items;
            }
        }
        Ok(value_to_lispval(&result, &ret))
    }

    /// Trace a call through the **reference (typed-core) interpreter**, returning
    /// the boxed result alongside a step-by-step [`TraceStep`] log of the callee's
    /// own body (nested calls run normally and are summarised by a single `call`
    /// step). This is the debug stepper/examiner: it lets a correctness suite
    /// inspect *how* a typed function computed its result, and assert invariants
    /// (determinism, result agreement, slot-bound safety) over the trace.
    ///
    /// Independent of whether a compiled edition exists — it always interprets so
    /// the trace reflects the reference semantics that compiled editions must match.
    pub fn trace_call(
        &self,
        name: &str,
        args: &[Value],
    ) -> Result<(Value, Vec<TraceStep>), String> {
        let id = self
            .id(name)
            .ok_or_else(|| format!("unknown function `{name}`"))?;
        let f = &self.funcs[id];
        let core = f
            .core_clone()
            .ok_or_else(|| format!("{name}: declared but not defined"))?;
        let params = f.params.borrow().clone();
        if args.len() != params.len() {
            return Err(format!(
                "{name}: expected {} args, got {}",
                params.len(),
                args.len()
            ));
        }
        let ctx = self.ctx();
        let mut frame = vec![0u64; f.slots.get()];
        for (i, (a, (_, ty))) in args.iter().zip(params.iter()).enumerate() {
            frame[i] = a.to_word(ty, &ctx)?;
        }
        let mut log = Vec::new();
        let w = eval_core_traced(&core, &mut frame, &ctx, 0, &mut log);
        // See the identical check in `call_inner` (issue #271): a nested call
        // reached via `Ctx::call` may have recorded a reachable-panic
        // condition instead of a real result.
        if let Some(msg) = ctx.pending_error.borrow_mut().take() {
            return Err(msg);
        }
        let ret = f.ret.borrow().clone();
        Ok((Value::from_word(w, &ret), log))
    }

    /// Drop every compiled edition (force the interpreter path). Test/diagnostic.
    pub fn deoptimize_all(&self) {
        for f in &self.funcs {
            f.deoptimize();
        }
    }
    /// (Re)compile every defined function.
    pub fn compile_all(&self) {
        for f in &self.funcs {
            f.compile_now(&self.funcs);
        }
    }

    /// Render the typed-core IR of `name` as a flat pseudo-assembly listing.
    ///
    /// This is the "what is actually being run" view for a jotted (typed)
    /// function: the typed core is the reference program every compiled edition
    /// must match, lowered here to a linear, register-and-label instruction
    /// stream. Returns `None` if no typed function by that name is registered.
    pub fn disassemble(&self, name: &str) -> Option<String> {
        let f = self.get(name)?;
        let params = f.params();
        let ret = f.ret();
        let mut s = String::new();
        let sig = params
            .iter()
            .map(|(_, t)| ty_name(t))
            .collect::<Vec<_>>()
            .join(" ");
        s.push_str(&format!(
            "; typed function {} ({sig}) -> {}\n",
            f.name,
            ty_name(&ret)
        ));
        if params.is_empty() {
            s.push_str("; slots: (none)\n");
        } else {
            s.push_str("; slots:\n");
            for (i, (pname, ty)) in params.iter().enumerate() {
                s.push_str(&format!(";   slot{i} = {pname} : {}\n", ty_name(ty)));
            }
        }
        s.push_str(&format!(
            "; compiled edition: {}\n",
            if f.is_compiled() {
                "yes"
            } else {
                "no (interpreted)"
            }
        ));
        match f.core_clone() {
            None => s.push_str("    ; declared but not yet defined\n"),
            Some(core) => {
                let mut out = Vec::new();
                let mut reg = 0usize;
                let mut lab = 0usize;
                self.dis_emit(&core, "rv", &mut out, &mut reg, &mut lab);
                for line in out {
                    s.push_str(&line);
                    s.push('\n');
                }
                s.push_str("    ret rv\n");
            }
        }
        Some(s)
    }

    /// Linearize `core` into instructions writing their result into register
    /// `dst`, appending textual lines to `out`. `reg`/`lab` are monotonic
    /// counters for fresh temporaries and branch labels.
    fn dis_emit(
        &self,
        core: &Core,
        dst: &str,
        out: &mut Vec<String>,
        reg: &mut usize,
        lab: &mut usize,
    ) {
        let fresh = |reg: &mut usize| {
            let r = format!("r{}", *reg);
            *reg += 1;
            r
        };
        let fresh_lab = |lab: &mut usize, base: &str| {
            let l = format!(".{base}{}", *lab);
            *lab += 1;
            l
        };
        match core {
            Core::LitI(n) => out.push(format!("    {dst} = li   {n}")),
            Core::LitF(x) => out.push(format!("    {dst} = lf   {x}")),
            Core::Var(i) => out.push(format!("    {dst} = ld   slot{i}")),
            Core::ToChar(a) => {
                self.dis_emit(a, dst, out, reg, lab);
                out.push(format!("    {dst} = and  {dst}, 0xff        ; code-char"));
            }
            Core::Not(a) => {
                self.dis_emit(a, dst, out, reg, lab);
                out.push(format!("    {dst} = not  {dst}"));
            }
            Core::Bin(k, op, a, b) => {
                let t1 = fresh(reg);
                let t2 = fresh(reg);
                self.dis_emit(a, &t1, out, reg, lab);
                self.dis_emit(b, &t2, out, reg, lab);
                out.push(format!("    {dst} = {} {t1}, {t2}", bin_mnemonic(*k, *op)));
            }
            Core::Cmp(k, op, a, b) => {
                let t1 = fresh(reg);
                let t2 = fresh(reg);
                self.dis_emit(a, &t1, out, reg, lab);
                self.dis_emit(b, &t2, out, reg, lab);
                out.push(format!("    {dst} = {} {t1}, {t2}", cmp_mnemonic(*k, *op)));
            }
            Core::And(a, b) => {
                self.dis_emit(a, dst, out, reg, lab);
                let end = fresh_lab(lab, "and_end");
                out.push(format!("    brz  {dst}, {end}          ; short-circuit"));
                self.dis_emit(b, dst, out, reg, lab);
                out.push(format!("{end}:"));
            }
            Core::Or(a, b) => {
                self.dis_emit(a, dst, out, reg, lab);
                let end = fresh_lab(lab, "or_end");
                out.push(format!("    brnz {dst}, {end}          ; short-circuit"));
                self.dis_emit(b, dst, out, reg, lab);
                out.push(format!("{end}:"));
            }
            Core::If(c, t, e) => {
                let tc = fresh(reg);
                self.dis_emit(c, &tc, out, reg, lab);
                let l_else = fresh_lab(lab, "else");
                let l_end = fresh_lab(lab, "endif");
                out.push(format!("    brz  {tc}, {l_else}"));
                self.dis_emit(t, dst, out, reg, lab);
                out.push(format!("    br   {l_end}"));
                out.push(format!("{l_else}:"));
                self.dis_emit(e, dst, out, reg, lab);
                out.push(format!("{l_end}:"));
            }
            Core::Let(slot, val, body) => {
                let tv = fresh(reg);
                self.dis_emit(val, &tv, out, reg, lab);
                out.push(format!("    st   slot{slot}, {tv}"));
                self.dis_emit(body, dst, out, reg, lab);
            }
            Core::Call(id, args) => {
                let mut argregs = Vec::with_capacity(args.len());
                for a in args {
                    let t = fresh(reg);
                    self.dis_emit(a, &t, out, reg, lab);
                    argregs.push(t);
                }
                let callee = self.name_of(*id).unwrap_or_else(|| format!("fn#{id}"));
                out.push(format!("    {dst} = call {callee}({})", argregs.join(", ")));
            }
            Core::ArrayNew(n) => {
                let t = fresh(reg);
                self.dis_emit(n, &t, out, reg, lab);
                out.push(format!("    {dst} = alloc {t}        ; array"));
            }
            Core::ArrayGet(a, i) => {
                let (ta, ti) = (fresh(reg), fresh(reg));
                self.dis_emit(a, &ta, out, reg, lab);
                self.dis_emit(i, &ti, out, reg, lab);
                out.push(format!(
                    "    {dst} = ldelem {ta}[{ti}]   ; fetch (bounds-checked)"
                ));
            }
            Core::ArraySet(a, i, v) => {
                let (ta, ti, tv) = (fresh(reg), fresh(reg), fresh(reg));
                self.dis_emit(a, &ta, out, reg, lab);
                self.dis_emit(i, &ti, out, reg, lab);
                self.dis_emit(v, &tv, out, reg, lab);
                out.push(format!(
                    "    stelem {ta}[{ti}], {tv}   ; store (bounds-checked)"
                ));
                out.push(format!("    {dst} = mov {tv}"));
            }
            Core::ArrayLen(a) => {
                let t = fresh(reg);
                self.dis_emit(a, &t, out, reg, lab);
                out.push(format!("    {dst} = ldlen {t}"));
            }
            Core::StructNew(inits) => {
                let mut regs = Vec::with_capacity(inits.len());
                for c in inits {
                    let t = fresh(reg);
                    self.dis_emit(c, &t, out, reg, lab);
                    regs.push(t);
                }
                out.push(format!(
                    "    {dst} = struct {{{}}}      ; make struct",
                    regs.join(", ")
                ));
            }
            Core::FieldGet(s, idx) => {
                let t = fresh(reg);
                self.dis_emit(s, &t, out, reg, lab);
                out.push(format!("    {dst} = ldfld {t}.{idx}"));
            }
            Core::FieldSet(s, idx, v) => {
                let (ts, tv) = (fresh(reg), fresh(reg));
                self.dis_emit(s, &ts, out, reg, lab);
                self.dis_emit(v, &tv, out, reg, lab);
                out.push(format!("    stfld {ts}.{idx}, {tv}"));
                out.push(format!("    {dst} = mov {tv}"));
            }
            Core::Seq(forms) => {
                for f in forms {
                    self.dis_emit(f, dst, out, reg, lab);
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Declared-scheme parsing (experimental rows, `declare-type!`).
// ---------------------------------------------------------------------------

/// Parse a surface scheme form: `(forall (vars...) ty)` or a bare `ty`.
fn parse_scheme_form(
    form: &LispVal,
    structs: &HashMap<String, Rc<StructDef>>,
    variants: &HashMap<String, Rc<VariantDef>>,
    generics: &HashMap<String, Rc<GenericDef>>,
) -> Result<infer::Scheme, String> {
    let parts = list_to_vec(form);
    let (var_names, body) = match parts.as_slice() {
        [LispVal::Symbol(s), vars_form, body] if s.borrow().name == "FORALL" => {
            let mut names = Vec::new();
            for v in list_to_vec(vars_form) {
                match v {
                    LispVal::Symbol(sym) => names.push(sym.borrow().name.clone()),
                    other => {
                        return Err(format!("declare-type!: bad type variable {other:?}"));
                    }
                }
            }
            (names, body.clone())
        }
        _ => (Vec::new(), form.clone()),
    };
    let mut vars: HashMap<String, u32> = HashMap::new();
    for (i, n) in var_names.iter().enumerate() {
        if vars.insert(n.clone(), i as u32).is_some() {
            return Err(format!("declare-type!: duplicate type variable {n}"));
        }
    }
    let ty = parse_declared_ty(&body, &vars, structs, variants, generics)?;
    Ok(infer::Scheme {
        vars: (0..var_names.len() as u32).collect(),
        ty,
    })
}

/// Names that are built-in type constructors or type words: a user nominal
/// with one of these names would silently shadow (or be shadowed by) the
/// built-in meaning in type surfaces — reject at declaration.
fn reject_reserved_type_name(name: &str) -> Result<(), String> {
    match name {
        "LIST" | "ARRAY" | "PAIR" | "RECORD" | "->" | "FORALL" | "INT64" | "FLOAT64" | "BOOL"
        | "CHAR" | "U8" | "BYTE" | "SYMBOL" | "STRING" | "ANY" => Err(format!(
            "`{}` is a built-in type name and cannot name a record or variant",
            name.to_lowercase()
        )),
        _ => Ok(()),
    }
}

fn parse_declared_ty(
    form: &LispVal,
    vars: &HashMap<String, u32>,
    structs: &HashMap<String, Rc<StructDef>>,
    variants: &HashMap<String, Rc<VariantDef>>,
    generics: &HashMap<String, Rc<GenericDef>>,
) -> Result<Ty, String> {
    match form {
        LispVal::Symbol(s) => {
            let n = s.borrow().name.clone();
            if let Some(id) = vars.get(&n) {
                return Ok(Ty::Var(*id));
            }
            match n.as_str() {
                "INT64" => Ok(Ty::Int64),
                "FLOAT64" => Ok(Ty::Float64),
                "BOOL" => Ok(Ty::Bool),
                "CHAR" => Ok(Ty::Char),
                "SYMBOL" => Ok(Ty::Symbol),
                "STRING" => Ok(Ty::Str),
                "ANY" => Ok(Ty::Any),
                other => structs
                    .get(other)
                    .map(|d| Ty::Struct(d.clone()))
                    .or_else(|| variants.get(other).map(|v| Ty::Variant(v.clone())))
                    // A BARE generic name is sugar for the all-ANY
                    // application: `option` means `(option any)` — the
                    // gradual reading, and what pre-parametric code wrote.
                    .or_else(|| {
                        generics
                            .get(other)
                            .map(|g| Ty::App(g.clone(), vec![Ty::Any; g.arity]))
                    })
                    .ok_or_else(|| format!("declare-type!: unknown type `{other}`")),
            }
        }
        LispVal::Cons { .. } => {
            let parts = list_to_vec(form);
            let head = match parts.first() {
                Some(LispVal::Symbol(s)) => s.borrow().name.clone(),
                _ => return Err("declare-type!: malformed compound type".to_string()),
            };
            let sub = |f: &LispVal| parse_declared_ty(f, vars, structs, variants, generics);
            match head.as_str() {
                "LIST" if parts.len() == 2 => Ok(Ty::List(Box::new(sub(&parts[1])?))),
                "ARRAY" if parts.len() == 2 => Ok(Ty::Array(Box::new(sub(&parts[1])?))),
                "PAIR" if parts.len() == 3 => Ok(Ty::Pair(
                    Box::new(sub(&parts[1])?),
                    Box::new(sub(&parts[2])?),
                )),
                "->" if parts.len() == 3 => {
                    let mut args = Vec::new();
                    for a in list_to_vec(&parts[1]) {
                        args.push(sub(&a)?);
                    }
                    Ok(Ty::Fn(args, Box::new(sub(&parts[2])?)))
                }
                "RECORD" if parts.len() == 2 || parts.len() == 3 => {
                    let mut fields = Vec::new();
                    for f in list_to_vec(&parts[1]) {
                        let fp = list_to_vec(&f);
                        match fp.as_slice() {
                            [LispVal::Symbol(l), t] => {
                                fields.push((l.borrow().name.clone(), sub(t)?));
                            }
                            _ => {
                                return Err(
                                    "declare-type!: record field must be (label type)".to_string()
                                );
                            }
                        }
                    }
                    fields.sort_by(|a, b| a.0.cmp(&b.0));
                    for w in fields.windows(2) {
                        if w[0].0 == w[1].0 {
                            return Err(format!(
                                "declare-type!: duplicate record label {}",
                                w[0].0
                            ));
                        }
                    }
                    let rest = if parts.len() == 3 {
                        let tail = sub(&parts[2])?;
                        if !matches!(tail, Ty::Var(_)) {
                            return Err("declare-type!: record row tail must be a type variable"
                                .to_string());
                        }
                        Some(Box::new(tail))
                    } else {
                        None
                    };
                    Ok(Ty::Record(fields, rest))
                }
                // A registered parametric nominal applied to arguments
                // (0.3 HM generics): (option a), (pair int64 string).
                other => match generics.get(other) {
                    Some(def) if parts.len() - 1 == def.arity => {
                        let mut args = Vec::with_capacity(def.arity);
                        for a in &parts[1..] {
                            args.push(sub(a)?);
                        }
                        Ok(Ty::App(def.clone(), args))
                    }
                    Some(def) => Err(format!(
                        "declare-type!: `{other}` takes {} type argument(s), got {}",
                        def.arity,
                        parts.len() - 1
                    )),
                    None => Err(format!("declare-type!: unknown type constructor `{other}`")),
                },
            }
        }
        other => Err(format!("declare-type!: malformed type {other:?}")),
    }
}
