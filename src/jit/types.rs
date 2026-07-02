// ---------------------------------------------------------------------------
// Types and runtime values.
// ---------------------------------------------------------------------------

use super::*;

/// A monomorphic type in the typed core.
///
/// No longer `Copy`: arrays (#137/#138) and structs carry owned component types,
/// so the type is a small recursive tree. It is cheap to `clone` (scalars are
/// nullary; only compounds allocate).
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Ty {
    Int64,
    Float64,
    Bool,
    /// A byte / `u8` scalar (issue #136). Runtime representation is the byte
    /// value (`0..=255`) held in the low bits of the `u64` word; it slots into
    /// the unboxed scalar tier alongside `int64`/`float64`/`bool`.
    Char,
    /// A flat array of a scalar element type (#137/#138). The runtime value is a
    /// pointer to a header-prefixed `u64` buffer `[len, e0, e1, …]` rooted in the
    /// call arena (`Ctx`); every element is one `u64` word read per the element
    /// type. `(array char)` *is* a string (native byte processing, #137).
    /// The element type cannot be written in surface syntax — it is **inferred**
    /// (the reason #135 came first) — though a param/return may pin it
    /// (`(array int64)`), or leave it open with the bare `array` keyword.
    Array(Box<Ty>),
    /// A struct: a fixed record of named scalar fields, laid out as a flat
    /// `u64` buffer (one word per field) rooted in the call arena, like a
    /// fixed-shape array with named offsets.
    Struct(Rc<StructDef>),
    /// A type variable (issue #135): an as-yet-undetermined type, identified by
    /// a fresh id. Variables exist only *during* elaboration; `Infer::resolve`
    /// must drive every one to a concrete type before a definition is accepted,
    /// so no `Var` is ever stored in a function signature or reaches the runtime
    /// membrane.
    Var(u32),

    // --- checkable-but-not-compileable types (issue #162) ------------------
    // These extend the *checker's* type language past the compileable lattice:
    // the inference engine can prove a value has one of these types (catching
    // software type errors), even though codegen does not lower them to unboxed
    // native code. `is_compileable` partitions the lattice; the codegen gate
    // (`resolve_compileable`) rejects everything below this line.
    /// A homogeneous proper list of `T` (boxed `LispVal` cons chain).
    List(Box<Ty>),
    /// A cons cell `(car . cdr)` with independently-typed halves.
    Pair(Box<Ty>, Box<Ty>),
    /// An interned symbol.
    Symbol,
    /// The boxed `LispVal::String` (distinct from `(array char)`).
    Str,
    /// An arrow type `(args...) -> ret`, for higher-order checking.
    Fn(Vec<Ty>, Box<Ty>),
    /// A **row-typed record** (experimental): a set of labeled fields plus an
    /// optional row tail. `rest: None` is a closed record (exactly these
    /// fields); `rest: Some(Var)` is an open one ("these fields, and the
    /// rest is ρ"). Rows exist only in the *checker's* type language — they
    /// come from declared schemes (`declare-type!`), never from codegen, and
    /// `is_compileable` rejects them. Fields are kept sorted by label; a row
    /// tail may itself resolve to another `Record`, which unification and
    /// `zonk` flatten.
    Record(Vec<(String, Ty)>, Option<Box<Ty>>),
    /// The gradual top type: the operative/`eval`/create-on-assign frontier
    /// (Wand). It unifies with anything (absorbing) and propagates, so the
    /// checker stays sound on the applicative island and makes no claim across
    /// the membrane.
    Any,
}

/// Outcome of [`Jit::analyze_untyped`] (#162 stage 4): the checker's verdict for
/// an un-annotated function, and whether a native edition was installed.
pub enum Analysis {
    /// Well-typed *and* compileable — a native edition was installed. Carries the
    /// inferred (compileable) type, rendered.
    Native(String),
    /// Well-typed but **not** compileable — it type-checks and stays dynamic
    /// (interpreted). Carries the inferred type scheme, rendered.
    Checked(String),
    /// A genuine type error: a clash on the typed island.
    TypeError(String),
}

/// Whether `t` lies in the **compileable** sub-lattice — the types codegen can
/// lower to unboxed machine words/buffers. Checkable-but-not-compileable types
/// (issue #162) are well-typed but stay interpreted/boxed.
pub fn is_compileable(t: &Ty) -> bool {
    match t {
        Ty::Int64 | Ty::Float64 | Ty::Bool | Ty::Char => true,
        Ty::Array(e) => is_compileable(e),
        Ty::Struct(d) => d.fields.iter().all(|(_, ft)| is_compileable(ft)),
        // A variable is not (yet) compileable until resolved; the rest are
        // checkable only.
        Ty::Var(_)
        | Ty::List(_)
        | Ty::Pair(_, _)
        | Ty::Symbol
        | Ty::Str
        | Ty::Fn(_, _)
        | Ty::Record(_, _)
        | Ty::Any => false,
    }
}

/// Whether `t` is a *flat* array of scalars (`(array int64/float64/bool/char)`,
/// not an array of arrays or an array of structs) — issue #216: this is the
/// scope `Jit::call_with_array_writeback` writes a mutated argument back
/// into the caller's backing store for. Excluded on purpose:
///
/// - Nested compounds (`(array (array T))`, `(array Struct)`): `from_word`
///   would rebuild fresh inner `LispVal`s for every element, so writing the
///   outer buffer back would silently replace inner-object identity for any
///   other holder of one of those inner values. That is a genuinely new kind
///   of aliasing bug, not a fix — out of scope for this pass (see #216).
/// - Structs at the top level: `LispVal::Struct` has no interior mutability
///   in the first place (`Shared<StructObj>`, not `Shared<SharedCell<_>>>`),
///   so there is no existing in-place-mutation contract to honor there,
///   unlike arrays (`LispVal::Array` is `Shared<SharedCell<Vec<LispVal>>>>`,
///   and `STORE`'s docs explicitly promise in-place mutation).
pub(super) fn is_flat_scalar_array(t: &Ty) -> bool {
    matches!(
        t,
        Ty::Array(elem) if matches!(**elem, Ty::Int64 | Ty::Float64 | Ty::Bool | Ty::Char)
    )
}

/// A struct type definition: an ordered list of `(field-name, field-type)`. Two
/// struct types are equal iff they have the same name and field layout; the
/// `Rc` keeps clones cheap and identity stable.
#[derive(PartialEq, Eq, Debug)]
pub struct StructDef {
    pub name: String,
    pub fields: Vec<(String, Ty)>,
}

impl Ty {
    pub fn parse(name: &str) -> Option<Ty> {
        match name {
            "INT64" => Some(Ty::Int64),
            "FLOAT64" => Some(Ty::Float64),
            "BOOL" => Some(Ty::Bool),
            "CHAR" | "U8" | "BYTE" => Some(Ty::Char),
            _ => None,
        }
    }

    /// Arithmetic interpretation (`+ - * /`). `char` is intentionally excluded:
    /// byte math is done by widening to `int64` (`char-code`) and narrowing back
    /// (`code-char`).
    pub(super) fn as_num(&self) -> Option<NumTy> {
        match self {
            Ty::Int64 => Some(NumTy::I),
            Ty::Float64 => Some(NumTy::F),
            _ => None,
        }
    }

    /// Comparison interpretation. `char` compares as an unsigned-small integer
    /// (its word holds a `0..=255` byte value), so it reuses the integer path.
    pub(super) fn cmp_num(&self) -> Option<NumTy> {
        match self {
            Ty::Int64 | Ty::Char => Some(NumTy::I),
            Ty::Float64 => Some(NumTy::F),
            _ => None,
        }
    }
}

/// Surface-style name of a [`Ty`], matching the `defun-typed` syntax
/// (`int64`, `float64`, `bool`, `char`, `(array T)`, struct name). Used by
/// introspection (`describe`, `disassemble`) and diagnostics.
pub fn ty_name(t: &Ty) -> String {
    match t {
        Ty::Int64 => "int64".to_string(),
        Ty::Float64 => "float64".to_string(),
        Ty::Bool => "bool".to_string(),
        Ty::Char => "char".to_string(),
        Ty::Array(e) => format!("(array {})", ty_name(e)),
        Ty::Struct(s) => s.name.clone(),
        // A variable should never survive to a signature/introspection site.
        Ty::Var(v) => format!("?{v}"),
        Ty::List(e) => format!("(list {})", ty_name(e)),
        Ty::Pair(a, b) => format!("(pair {} {})", ty_name(a), ty_name(b)),
        Ty::Symbol => "symbol".to_string(),
        Ty::Str => "string".to_string(),
        Ty::Fn(ps, r) => {
            let args = ps.iter().map(ty_name).collect::<Vec<_>>().join(" ");
            format!("(-> ({args}) {})", ty_name(r))
        }
        Ty::Record(fields, rest) => {
            let fs = fields
                .iter()
                .map(|(n, t)| format!("({} {})", n.to_lowercase(), ty_name(t)))
                .collect::<Vec<_>>()
                .join(" ");
            match rest {
                Some(r) => format!("(record ({fs}) {})", ty_name(r)),
                None => format!("(record ({fs}))"),
            }
        }
        Ty::Any => "any".to_string(),
    }
}

/// Which machine interpretation a numeric op uses on its `u64` words.
#[derive(Clone, Copy, Debug)]
pub(super) enum NumTy {
    I,
    F,
}

/// A boxed value at the public boundary (the unboxed runtime uses raw `u64`).
#[derive(Clone, PartialEq, Debug)]
pub enum Value {
    Int(i64),
    Float(f64),
    Bool(bool),
    /// A byte value (`0..=255`) at the boundary. Boxes to/from `LispVal::Char`;
    /// in-range numbers are accepted at input for compatibility.
    Char(u8),
    /// A flat array, materialized at the boundary as its element [`Value`]s. The
    /// runtime word is a pointer into the call arena; this is the copied-out view
    /// the membrane hands back (or takes in).
    Array(Vec<Value>),
    /// A struct, materialized as its field [`Value`]s in declaration order.
    Struct(Vec<Value>),
}

/// Result of a call that also reports post-call array write-back (issue
/// #216): the ordinary boxed result, plus one entry per argument that is
/// `Some(updated_value)` when that argument's type is a flat scalar array
/// (see [`is_flat_scalar_array`]), `None` otherwise. See
/// `Jit::call_with_array_writeback`.
pub type WritebackResult = Result<(Value, Vec<Option<Value>>), String>;

impl Value {
    /// Lower a boundary value to its runtime `u64` word for a parameter of type
    /// `ty`, allocating compound values (arrays/structs) into the call arena so
    /// the resulting pointer word is rooted for the duration of the call.
    pub(super) fn to_word(&self, ty: &Ty, ctx: &Ctx) -> Result<u64, String> {
        match (self, ty) {
            (Value::Int(n), Ty::Int64) => Ok(*n as u64),
            (Value::Float(f), Ty::Float64) => Ok(f.to_bits()),
            (Value::Bool(b), Ty::Bool) => Ok(*b as u64),
            (Value::Char(b), Ty::Char) => Ok(*b as u64),
            (Value::Int(n), Ty::Char) => Ok(u8::try_from(*n)
                .map_err(|_| format!("char argument: {n} out of range 0-255"))?
                as u64),
            (Value::Array(items), Ty::Array(elem)) => {
                let buf = ctx.alloc_buffer(items.len());
                for (i, it) in items.iter().enumerate() {
                    let w = it.to_word(elem, ctx)?;
                    unsafe { *buf.add(i + 1) = w };
                }
                Ok(buf as u64)
            }
            (Value::Struct(fields), Ty::Struct(def)) => {
                if fields.len() != def.fields.len() {
                    return Err(format!(
                        "struct `{}` expects {} fields, got {}",
                        def.name,
                        def.fields.len(),
                        fields.len()
                    ));
                }
                let buf = ctx.alloc_buffer(fields.len());
                for (i, (fv, (_, ft))) in fields.iter().zip(def.fields.iter()).enumerate() {
                    let w = fv.to_word(ft, ctx)?;
                    unsafe { *buf.add(i + 1) = w };
                }
                Ok(buf as u64)
            }
            _ => Err(format!(
                "value {self:?} does not match type {}",
                ty_name(ty)
            )),
        }
    }

    /// Read a runtime word back into a boundary value of type `ty`, copying
    /// compound buffers out of the arena (so the result outlives the call).
    pub(super) fn from_word(w: u64, ty: &Ty) -> Value {
        match ty {
            Ty::Int64 => Value::Int(w as i64),
            Ty::Float64 => Value::Float(f64::from_bits(w)),
            Ty::Bool => Value::Bool(w != 0),
            Ty::Char => Value::Char(w as u8),
            Ty::Array(elem) => {
                let base = w as *const u64;
                let len = unsafe { *base } as usize;
                let mut items = Vec::with_capacity(len);
                for i in 0..len {
                    let ew = unsafe { *base.add(i + 1) };
                    items.push(Value::from_word(ew, elem));
                }
                Value::Array(items)
            }
            Ty::Struct(def) => {
                let base = w as *const u64;
                let mut fields = Vec::with_capacity(def.fields.len());
                for (i, (_, ft)) in def.fields.iter().enumerate() {
                    let fw = unsafe { *base.add(i + 1) };
                    fields.push(Value::from_word(fw, ft));
                }
                Value::Struct(fields)
            }
            // Only compileable types cross the membrane: a `Var` is resolved
            // before storage, and the checkable-only types (#162) never back a
            // native edition (`is_compileable` is false for them).
            Ty::Var(_)
            | Ty::List(_)
            | Ty::Pair(_, _)
            | Ty::Symbol
            | Ty::Str
            | Ty::Fn(_, _)
            | Ty::Record(_, _)
            | Ty::Any => {
                unreachable!("from_word on a non-compileable type {}", ty_name(ty))
            }
        }
    }
}

#[inline]
pub(super) fn as_i(w: u64) -> i64 {
    w as i64
}
#[inline]
pub(super) fn from_i(x: i64) -> u64 {
    x as u64
}
#[inline]
pub(super) fn as_f(w: u64) -> f64 {
    f64::from_bits(w)
}
#[inline]
pub(super) fn from_f(x: f64) -> u64 {
    x.to_bits()
}

#[derive(Clone, Copy, Debug)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
}

#[derive(Clone, Copy, Debug)]
pub enum CmpOp {
    Lt,
    Gt,
    Le,
    Ge,
    Eq,
    Ne,
}

/// The typed core IR. Variables are fixed slot indices; calls are function ids.
#[derive(Clone, Debug)]
pub enum Core {
    LitI(i64),
    LitF(f64),
    Var(usize),
    Bin(NumKind, BinOp, Box<Core>, Box<Core>),
    Cmp(NumKind, CmpOp, Box<Core>, Box<Core>),
    Not(Box<Core>),
    And(Box<Core>, Box<Core>),
    Or(Box<Core>, Box<Core>),
    If(Box<Core>, Box<Core>, Box<Core>),
    Let(usize, Box<Core>, Box<Core>),
    Call(usize, Vec<Core>),
    /// Narrow an `int64` word to a `char` by masking to a byte (`code-char`).
    /// The widening direction (`char-code`) needs no node: a `char` word already
    /// holds the byte value, so it is reused unchanged at type `int64`.
    ToChar(Box<Core>),
    /// `(array n)`: allocate a flat `n`-element buffer in the call arena,
    /// zero-initialized; evaluates to the buffer pointer word (#137/#138).
    ArrayNew(Box<Core>),
    /// `(fetch a i)`: bounds-checked element load (out-of-range yields `0`,
    /// matching the panic-free div-by-zero policy). The word is read per the
    /// statically-known element type by the surrounding nodes.
    ArrayGet(Box<Core>, Box<Core>),
    /// `(store a i v)`: bounds-checked element store (out-of-range is a no-op);
    /// evaluates to the stored value word.
    ArraySet(Box<Core>, Box<Core>, Box<Core>),
    /// `(array-length a)`: the element count (the buffer header), as `int64`.
    ArrayLen(Box<Core>),
    /// `(make-NAME f0 f1 …)`: allocate a struct buffer (one word per field) in
    /// the call arena and initialize each field in declaration order; evaluates
    /// to the buffer pointer word.
    StructNew(Vec<Core>),
    /// `(NAME-FIELD s)`: load field at the given fixed word offset.
    FieldGet(Box<Core>, usize),
    /// `(set-NAME-FIELD s v)`: store field at the given offset; evaluates to the
    /// stored value word.
    FieldSet(Box<Core>, usize, Box<Core>),
    /// A statement sequence: evaluate each in order (for side effects such as
    /// `store`), yielding the last. Non-empty by construction.
    Seq(Vec<Core>),
}

/// Public mirror of `NumTy` so [`Core`] can derive `Debug`/`Clone` cleanly.
#[derive(Clone, Copy, Debug)]
pub enum NumKind {
    I,
    F,
}
impl From<NumTy> for NumKind {
    fn from(n: NumTy) -> Self {
        match n {
            NumTy::I => NumKind::I,
            NumTy::F => NumKind::F,
        }
    }
}
