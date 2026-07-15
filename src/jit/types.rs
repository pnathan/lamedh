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
    /// A declared sum type (#312 follow-up): the checker-level union of a
    /// closed set of constructor brands. Never compiled; checker-only.
    Variant(Rc<VariantDef>),
    /// An APPLIED parametric nominal (0.3 HM generics): `(option int64)`,
    /// `(pair a b)`. Nominal by definition name; arguments unify pairwise;
    /// a constructor application absorbs into its variant's application.
    /// Never compiled; checker-only.
    App(Rc<GenericDef>, Vec<Ty>),
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
        // A sum's representation varies by constructor: checker-only.
        Ty::Variant(_) => false,
        // Parametric nominals are checker-only (erased at runtime).
        Ty::App(_, _) => false,
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

/// A declared sum type: a closed set of constructor brands. Each constructor
/// is itself a registered record (a [`StructDef`]); the variant is the
/// checker-level union of those brands — a `CIRCLE` value unifies where a
/// `SHAPE` is demanded, two variants unify only by name, and nothing else
/// is a member.
#[derive(PartialEq, Eq, Debug)]
pub struct VariantDef {
    pub name: String,
    pub ctors: Vec<String>,
}

/// A PARAMETRIC nominal (0.3 HM generics): a record or variant taking type
/// parameters. `fields` (records and variant constructors) may reference
/// the parameters as `Ty::Var(0..arity)` — canonical ids, substituted with
/// fresh variables or concrete arguments at every use. `ctors` non-empty
/// marks a variant; `variant` on a constructor names its owning sum. Uses
/// appear in the type language as applications: `(pair int64 string)`,
/// `(option a)` — represented as [`Ty::App`].
#[derive(PartialEq, Eq, Debug)]
pub struct GenericDef {
    pub name: String,
    pub arity: usize,
    pub fields: Vec<(String, Ty)>,
    pub ctors: Vec<String>,
    pub variant: Option<String>,
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
        Ty::Variant(v) => v.name.clone(),
        Ty::App(d, args) => format!(
            "({} {})",
            d.name.to_lowercase(),
            args.iter().map(ty_name).collect::<Vec<_>>().join(" ")
        ),
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
/// (see `is_flat_scalar_array`), `None` otherwise. See
/// `Jit::call_with_array_writeback`.
/// Condition flags set by typed arithmetic during a JIT call (issue #228).
/// The membrane reads these after the call returns and propagates them to
/// the evaluator (OVERFLOW flag / division-by-zero error).
#[derive(Debug, Clone, Copy, Default)]
pub struct JitFlags {
    pub overflow: bool,
    pub div_by_zero: bool,
}

pub type WritebackResult = Result<(Value, Vec<Option<Value>>, JitFlags), String>;

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
            | Ty::Variant(_)
            | Ty::App(_, _)
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
    /// Bitwise AND/OR/XOR on int64 (`logand`/`logior`/`logxor`).
    BitAnd,
    BitOr,
    BitXor,
    /// Left shift `x << y` (`ash` with a positive constant); the right operand
    /// is always a compile-time constant in `1..=63`, so it never masks or
    /// overflows.
    Shl,
    /// Arithmetic right shift `x >> y` (`ash` with a negative constant); right
    /// operand a compile-time constant in `1..=63`.
    AShr,
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
    /// `(array-length* a)`: the element count (the buffer header), as `int64`.
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
    /// `(setq var val)` on a local slot: store the value word into the
    /// slot and evaluate to the stored word. Only elaborated for local
    /// variables (params and let-bindings); dynamic/global setq stays
    /// in the tree-walker.
    Assign(usize, Box<Core>),
    /// `(while test body)`: evaluate TEST; if truthy (nonzero), evaluate
    /// BODY for side effects, then loop. Evaluates to 0 (NIL). Statement
    /// node — legal only in discarded position (non-tail Seq element).
    While(Box<Core>, Box<Core>),
    /// `(for (var start end [step]) body...)`: evaluate START, END, STEP
    /// once; iterate VAR from START to END (inclusive) by STEP. Direction
    /// determined by sign of STEP. STEP=0 is an error. Overflow on the
    /// counter breaks the loop without setting the OVERFLOW flag (matching
    /// the tree-walker contract at special_forms.rs:106-126). Evaluates
    /// to 0 (NIL). Statement node.
    For {
        slot: usize,
        start: Box<Core>,
        end: Box<Core>,
        step: Box<Core>,
        body: Box<Core>,
    },
    /// A unary floating-point intrinsic over a `float64` argument. The op
    /// determines the math and the result representation (see [`FUnOp`]).
    FUnary(FUnOp, Box<Core>),
    /// `(float x)` on an `int64` argument: widen to `float64` (`fcvt_from_sint`
    /// natively / `i64 as f64` in the interpreter). `(float x)` on a value that
    /// is already `float64` needs no node — it elaborates to the argument
    /// unchanged.
    IntToFloat(Box<Core>),
    /// `(array-add!/-sub!/-mul! out a b)`: elementwise binary op over three
    /// `(array T)` values of the SAME element type `T` (`int64`/`float64`),
    /// out-param, iterating `min(len out, len a, len b)`. Mutates `out` in
    /// place and evaluates to `out`'s buffer pointer. Integer arithmetic is
    /// **wrapping** and never sets `OVERFLOW` — a vector `iadd`/`isub`/`imul`
    /// cannot set a per-lane flag, so the whole family is defined as wrapping
    /// (matching `wrapping_add`/`wrapping_sub`/`wrapping_mul`) rather than
    /// have the scalar tail disagree with the vectorized body. `op` is
    /// reused from [`BinOp`] (only `Add`/`Sub`/`Mul` are ever constructed
    /// here); `NumKind` selects int64 vs. float64 lowering. The native
    /// backend lowers this to a 2-lane SIMD loop (`I64X2`/`F64X2`) plus a
    /// scalar tail for an odd final element; the Core interpreter and the
    /// closure backend use a plain scalar loop — elementwise ops have no
    /// reduction/reassociation, so all three executors agree bit-for-bit.
    ArrayMap2(BinOp, NumKind, Box<Core>, Box<Core>, Box<Core>),
    /// `(array-sum a)`: **wrapping** sum of every `int64` element of `a`.
    /// int64-only (unlike [`Core::ArrayMap2`]) — float reduction reorders
    /// rounding and needs a reassociation policy this intrinsic does not
    /// attempt, so `array-sum` never elaborates over `(array float64)`.
    /// Wrapping int64 addition is **associative**
    /// (`(a+b)+c ≡ a+(b+c) mod 2^64`), so a multi-lane vector reduction (a
    /// 2-lane SIMD accumulator plus a horizontal add of its lanes) is
    /// **bit-identical** to a sequential left-fold — that is what lets the
    /// native backend use a vector accumulator and still match
    /// [`super::runtime`]'s scalar reference exactly. The native backend
    /// lowers this to a 2-lane `I64X2` accumulator loop (`splat(0)` seed,
    /// `iadd` per iteration) plus a horizontal `extractlane`/`iadd` reduction
    /// and a scalar tail for an odd final element.
    ArraySum(Box<Core>),
    /// `(array-dot a b)`: **wrapping** sum over `i in 0..min(len a, len b)`
    /// of `a[i] * b[i]` — each product wraps, and the running sum wraps.
    /// int64-only, same reason as [`Core::ArraySum`]. Both the per-lane
    /// `imul` and the `iadd` accumulation are wrapping two's-complement
    /// arithmetic, and wrapping addition is associative (see
    /// [`Core::ArraySum`]'s doc comment), so a vectorized (SIMD multiply +
    /// pairwise-add reduction) evaluation is bit-identical to the sequential
    /// scalar fold. The native backend lowers this like [`Core::ArraySum`]
    /// but with an `imul` of the two loaded vectors feeding the accumulator
    /// each iteration.
    ArrayDot(Box<Core>, Box<Core>),
}

/// Unary floating-point intrinsics that lower to native code. Each takes one
/// `float64` argument. The result is `float64` for the transcendentals and
/// `int64` for the rounding family (matching the evaluator, whose `floor`/
/// `ceiling`/`truncate` return an integer). Only ops whose native lowering is
/// bit-identical to the evaluator's Rust implementation appear here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u64)]
pub enum FUnOp {
    /// `sqrt`: `fsqrt` instruction; `float64 -> float64`.
    Sqrt,
    /// `floor`: `floor` instruction then saturating `f64 as i64`; `-> int64`.
    Floor,
    /// `ceiling`: `ceil` instruction then saturating `f64 as i64`; `-> int64`.
    Ceil,
    /// `truncate`: `trunc` instruction then saturating `f64 as i64`; `-> int64`.
    Trunc,
    /// `sin`: libm via the `jit_ftrans` trampoline; `float64 -> float64`.
    Sin,
    /// `cos`: libm trampoline; `float64 -> float64`.
    Cos,
    /// `tan`: libm trampoline; `float64 -> float64`.
    Tan,
    /// `exp`: libm trampoline; `float64 -> float64`.
    Exp,
    /// `round`: `f64::round` (half away from zero, unlike Cranelift's
    /// ties-to-even `nearest`) via the trampoline, then `as i64`; `-> int64`.
    Round,
}

impl FUnOp {
    /// Apply the op to an `f64`, returning the result as a raw 64-bit word
    /// (float bits for `float64` results, the integer value for `int64`
    /// results). This is the single source of truth: the Core interpreter
    /// calls it directly, and the native backend either emits the identical
    /// instruction sequence (the direct ops) or calls [`super::jit_ftrans`],
    /// which is itself a thin wrapper over this method.
    pub fn apply_word(self, x: f64) -> u64 {
        match self {
            FUnOp::Sqrt => x.sqrt().to_bits(),
            // `as i64` is saturating in Rust (matches Cranelift `fcvt_to_sint_sat`).
            FUnOp::Floor => x.floor() as i64 as u64,
            FUnOp::Ceil => x.ceil() as i64 as u64,
            FUnOp::Trunc => x.trunc() as i64 as u64,
            FUnOp::Sin => x.sin().to_bits(),
            FUnOp::Cos => x.cos().to_bits(),
            FUnOp::Tan => x.tan().to_bits(),
            FUnOp::Exp => x.exp().to_bits(),
            FUnOp::Round => x.round() as i64 as u64,
        }
    }

    /// True when this op lowers to a libm call (via `jit_ftrans`) rather than a
    /// direct Cranelift instruction.
    pub fn is_libm(self) -> bool {
        matches!(
            self,
            FUnOp::Sin | FUnOp::Cos | FUnOp::Tan | FUnOp::Exp | FUnOp::Round
        )
    }

    /// The `jit_ftrans` opcode (the `#[repr(u64)]` discriminant).
    pub fn opcode(self) -> u64 {
        self as u64
    }

    /// Inverse of [`Self::opcode`], for the trampoline. Panics on an unknown
    /// code — the native backend only ever passes an op's own discriminant.
    pub fn from_opcode(op: u64) -> FUnOp {
        match op {
            0 => FUnOp::Sqrt,
            1 => FUnOp::Floor,
            2 => FUnOp::Ceil,
            3 => FUnOp::Trunc,
            4 => FUnOp::Sin,
            5 => FUnOp::Cos,
            6 => FUnOp::Tan,
            7 => FUnOp::Exp,
            8 => FUnOp::Round,
            other => panic!("jit_ftrans: unknown FUnOp opcode {other}"),
        }
    }
}

/// True when `c` is exactly the variable reference for `slot` (the fast
/// structural check `core_may_mutate_slot` uses to spot a direct write
/// through a parameter, as opposed to a write reached only through some
/// other expression that merely *reads* the slot).
fn is_var_slot(c: &Core, slot: usize) -> bool {
    matches!(c, Core::Var(s) if *s == slot)
}

/// Static may-mutate analysis over the elaborated [`Core`] IR (issue #216
/// follow-up): does any reachable subterm of `core` write through the local
/// `slot`? A "write" is a direct [`Core::ArraySet`]/[`Core::ArrayMap2`]
/// out-param/[`Core::FieldSet`] whose target is *exactly* `Core::Var(slot)`
/// — not merely an expression that reads it. This is deliberately
/// conservative: it does not attempt alias tracking (e.g. `(let ((b a)) (store
/// b i v))` is invisible to this pass, since `b`, not `a`, is the direct
/// target), so a `false` result is a sound guarantee of no write, while a
/// `true` result may over-approximate. Used by the registry to skip the
/// post-call write-back copy for array parameters a function provably never
/// mutates — the worst case for a wrong `true` is a harmless redundant copy,
/// never a correctness bug.
pub fn core_may_mutate_slot(core: &Core, slot: usize) -> bool {
    match core {
        Core::LitI(_) | Core::LitF(_) | Core::Var(_) => false,
        Core::Bin(_, _, a, b) | Core::Cmp(_, _, a, b) => {
            core_may_mutate_slot(a, slot) || core_may_mutate_slot(b, slot)
        }
        Core::Not(a) => core_may_mutate_slot(a, slot),
        Core::And(a, b) | Core::Or(a, b) => {
            core_may_mutate_slot(a, slot) || core_may_mutate_slot(b, slot)
        }
        Core::If(c, t, e) => {
            core_may_mutate_slot(c, slot)
                || core_may_mutate_slot(t, slot)
                || core_may_mutate_slot(e, slot)
        }
        Core::Let(_, v, body) => core_may_mutate_slot(v, slot) || core_may_mutate_slot(body, slot),
        Core::Call(_, args) => args.iter().any(|a| core_may_mutate_slot(a, slot)),
        Core::ToChar(a) => core_may_mutate_slot(a, slot),
        Core::ArrayNew(n) => core_may_mutate_slot(n, slot),
        Core::ArrayGet(a, i) => core_may_mutate_slot(a, slot) || core_may_mutate_slot(i, slot),
        Core::ArraySet(a, i, v) => {
            is_var_slot(a, slot)
                || core_may_mutate_slot(a, slot)
                || core_may_mutate_slot(i, slot)
                || core_may_mutate_slot(v, slot)
        }
        Core::ArrayLen(a) => core_may_mutate_slot(a, slot),
        Core::StructNew(fields) => fields.iter().any(|f| core_may_mutate_slot(f, slot)),
        Core::FieldGet(s, _) => core_may_mutate_slot(s, slot),
        Core::FieldSet(s, _, v) => {
            is_var_slot(s, slot) || core_may_mutate_slot(s, slot) || core_may_mutate_slot(v, slot)
        }
        Core::Seq(items) => items.iter().any(|i| core_may_mutate_slot(i, slot)),
        Core::Assign(_, v) => core_may_mutate_slot(v, slot),
        Core::While(c, b) => core_may_mutate_slot(c, slot) || core_may_mutate_slot(b, slot),
        Core::For {
            start,
            end,
            step,
            body,
            ..
        } => {
            core_may_mutate_slot(start, slot)
                || core_may_mutate_slot(end, slot)
                || core_may_mutate_slot(step, slot)
                || core_may_mutate_slot(body, slot)
        }
        Core::FUnary(_, a) => core_may_mutate_slot(a, slot),
        Core::IntToFloat(a) => core_may_mutate_slot(a, slot),
        Core::ArrayMap2(_, _, out, a, b) => {
            is_var_slot(out, slot)
                || core_may_mutate_slot(out, slot)
                || core_may_mutate_slot(a, slot)
                || core_may_mutate_slot(b, slot)
        }
        Core::ArraySum(a) => core_may_mutate_slot(a, slot),
        Core::ArrayDot(a, b) => core_may_mutate_slot(a, slot) || core_may_mutate_slot(b, slot),
    }
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

// ---------------------------------------------------------------------------
// Core-level inlining (jit/core-loops): splice a small, non-recursive,
// already-defined callee's body directly into a caller's Core tree at
// `compile_now` time, so the callee's `call_indirect`/`Ctx::call` overhead
// disappears at that call site entirely. The eligibility decision (budget,
// recursion) lives in `registry.rs`; this is the pure tree transform.
// ---------------------------------------------------------------------------

/// Splice every callee in `registry` into `core` wherever it is referenced by
/// a `Core::Call(id, args)` node. `registry` entries are `(callee_id,
/// callee_core, callee_n_slots)`, where `callee_n_slots` is the callee's own
/// total frame size (`TypedFn::n_slots`) — every slot its body can reference
/// lies in `0..callee_n_slots` (parameters *and* any of its own `let`/`for`
/// locals), so that many fresh slots are reserved per inlined call site (not
/// just its parameter count) to keep every reference collision-free against
/// the caller's existing slots and against other inlined call sites.
///
/// Each inlined call site becomes a chain of `Core::Let` bindings — one per
/// argument, evaluated in order exactly as a real call would — around the
/// callee's own body with every slot reference shifted into the fresh range.
/// Everything else is walked and copied structurally unchanged.
///
/// Only ONE level of inlining is performed: a `Call` node that appears
/// *inside* a spliced-in callee body is left as an ordinary call — its own
/// callee is never looked up in `registry` again during this pass (avoids
/// inlining into an already-inlined body; deeper inlining, if ever wanted,
/// falls out of the callee's *own* `compile_now` inlining its own callees
/// before this function's next (re)compile).
///
/// Returns the transformed tree together with the first slot number *not*
/// used anywhere in it — i.e. the caller's new required `n_slots` (never
/// smaller than `slot_base`, since a caller with nothing to inline gets its
/// tree back unchanged and `slot_base` echoed).
pub fn inline_calls(
    core: &Core,
    registry: &[(usize, &Core, usize)],
    slot_base: usize,
) -> (Core, usize) {
    let mut next = slot_base;
    let out = inline_xform(core, 0, registry, true, &mut next);
    (out, next)
}

/// The recursive worker behind [`inline_calls`]. `shift` is added to every
/// slot index encountered (nonzero only while walking a just-spliced callee
/// body, to relocate its local slot numbering into the fresh range reserved
/// for it). `allow_inline` gates whether a `Call` node may itself be spliced
/// — `false` while walking inside an already-spliced callee body, enforcing
/// the one-level-only rule.
fn inline_xform(
    core: &Core,
    shift: usize,
    registry: &[(usize, &Core, usize)],
    allow_inline: bool,
    next: &mut usize,
) -> Core {
    match core {
        Core::LitI(n) => Core::LitI(*n),
        Core::LitF(f) => Core::LitF(*f),
        Core::Var(i) => Core::Var(i + shift),
        Core::Not(a) => Core::Not(Box::new(inline_xform(
            a,
            shift,
            registry,
            allow_inline,
            next,
        ))),
        Core::ToChar(a) => Core::ToChar(Box::new(inline_xform(
            a,
            shift,
            registry,
            allow_inline,
            next,
        ))),
        Core::FUnary(op, a) => Core::FUnary(
            *op,
            Box::new(inline_xform(a, shift, registry, allow_inline, next)),
        ),
        Core::IntToFloat(a) => Core::IntToFloat(Box::new(inline_xform(
            a,
            shift,
            registry,
            allow_inline,
            next,
        ))),
        Core::Bin(k, op, a, b) => Core::Bin(
            *k,
            *op,
            Box::new(inline_xform(a, shift, registry, allow_inline, next)),
            Box::new(inline_xform(b, shift, registry, allow_inline, next)),
        ),
        Core::Cmp(k, op, a, b) => Core::Cmp(
            *k,
            *op,
            Box::new(inline_xform(a, shift, registry, allow_inline, next)),
            Box::new(inline_xform(b, shift, registry, allow_inline, next)),
        ),
        Core::And(a, b) => Core::And(
            Box::new(inline_xform(a, shift, registry, allow_inline, next)),
            Box::new(inline_xform(b, shift, registry, allow_inline, next)),
        ),
        Core::Or(a, b) => Core::Or(
            Box::new(inline_xform(a, shift, registry, allow_inline, next)),
            Box::new(inline_xform(b, shift, registry, allow_inline, next)),
        ),
        Core::If(c, t, e) => Core::If(
            Box::new(inline_xform(c, shift, registry, allow_inline, next)),
            Box::new(inline_xform(t, shift, registry, allow_inline, next)),
            Box::new(inline_xform(e, shift, registry, allow_inline, next)),
        ),
        Core::Let(slot, v, body) => Core::Let(
            slot + shift,
            Box::new(inline_xform(v, shift, registry, allow_inline, next)),
            Box::new(inline_xform(body, shift, registry, allow_inline, next)),
        ),
        Core::Call(id, args) => {
            let new_args: Vec<Core> = args
                .iter()
                .map(|a| inline_xform(a, shift, registry, allow_inline, next))
                .collect();
            if allow_inline {
                if let Some(entry) = registry.iter().find(|e| e.0 == *id) {
                    let (_, callee_core, callee_slots) = *entry;
                    let base = *next;
                    *next += callee_slots;
                    // The spliced body's own slots (and any deeper calls it
                    // makes) are relocated by `base`; `allow_inline: false`
                    // enforces the one-level-only rule.
                    let inlined_body = inline_xform(callee_core, base, registry, false, next);
                    let mut wrapped = inlined_body;
                    for (i, a) in new_args.into_iter().enumerate().rev() {
                        wrapped = Core::Let(base + i, Box::new(a), Box::new(wrapped));
                    }
                    return wrapped;
                }
            }
            Core::Call(*id, new_args)
        }
        Core::ArrayNew(a) => Core::ArrayNew(Box::new(inline_xform(
            a,
            shift,
            registry,
            allow_inline,
            next,
        ))),
        Core::ArrayGet(a, b) => Core::ArrayGet(
            Box::new(inline_xform(a, shift, registry, allow_inline, next)),
            Box::new(inline_xform(b, shift, registry, allow_inline, next)),
        ),
        Core::ArraySet(a, b, c) => Core::ArraySet(
            Box::new(inline_xform(a, shift, registry, allow_inline, next)),
            Box::new(inline_xform(b, shift, registry, allow_inline, next)),
            Box::new(inline_xform(c, shift, registry, allow_inline, next)),
        ),
        Core::ArrayLen(a) => Core::ArrayLen(Box::new(inline_xform(
            a,
            shift,
            registry,
            allow_inline,
            next,
        ))),
        Core::StructNew(items) => Core::StructNew(
            items
                .iter()
                .map(|c| inline_xform(c, shift, registry, allow_inline, next))
                .collect(),
        ),
        Core::FieldGet(s, idx) => Core::FieldGet(
            Box::new(inline_xform(s, shift, registry, allow_inline, next)),
            *idx,
        ),
        Core::FieldSet(s, idx, v) => Core::FieldSet(
            Box::new(inline_xform(s, shift, registry, allow_inline, next)),
            *idx,
            Box::new(inline_xform(v, shift, registry, allow_inline, next)),
        ),
        Core::Seq(items) => Core::Seq(
            items
                .iter()
                .map(|c| inline_xform(c, shift, registry, allow_inline, next))
                .collect(),
        ),
        Core::Assign(slot, v) => Core::Assign(
            slot + shift,
            Box::new(inline_xform(v, shift, registry, allow_inline, next)),
        ),
        Core::While(t, b) => Core::While(
            Box::new(inline_xform(t, shift, registry, allow_inline, next)),
            Box::new(inline_xform(b, shift, registry, allow_inline, next)),
        ),
        Core::For {
            slot,
            start,
            end,
            step,
            body,
        } => Core::For {
            slot: slot + shift,
            start: Box::new(inline_xform(start, shift, registry, allow_inline, next)),
            end: Box::new(inline_xform(end, shift, registry, allow_inline, next)),
            step: Box::new(inline_xform(step, shift, registry, allow_inline, next)),
            body: Box::new(inline_xform(body, shift, registry, allow_inline, next)),
        },
        Core::ArrayMap2(op, k, o, a, b) => Core::ArrayMap2(
            *op,
            *k,
            Box::new(inline_xform(o, shift, registry, allow_inline, next)),
            Box::new(inline_xform(a, shift, registry, allow_inline, next)),
            Box::new(inline_xform(b, shift, registry, allow_inline, next)),
        ),
        Core::ArraySum(a) => Core::ArraySum(Box::new(inline_xform(
            a,
            shift,
            registry,
            allow_inline,
            next,
        ))),
        Core::ArrayDot(a, b) => Core::ArrayDot(
            Box::new(inline_xform(a, shift, registry, allow_inline, next)),
            Box::new(inline_xform(b, shift, registry, allow_inline, next)),
        ),
    }
}
