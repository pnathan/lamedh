# 10. Modules

A module in Lamedh is a naming discipline plus a little metadata, not a
new evaluator concept. Symbols defined inside `(with-module m ...)` are
stored under one flat global namespace as `M:SYMBOL` — there is no new
lookup machinery, no per-module environment, nothing an existing tool
(`describe`, `see-source`, the call graph, `spawn`) needs to know about
specially. `:` is an ordinary, non-initial symbol constituent, so
`GEOMETRY:AREA` is a ordinary symbol you can also just write directly,
with or without ever calling `import`.

This chapter covers `defmodule` and `with-module` (declaring a module and
populating it), `import` (bringing exports into the unqualified
namespace), introspection, module-provided custom capabilities, and the
one caveat worth knowing before you lean on this: qualification is
name-based, not scope-based.

## 10.1 `defmodule`: declaring a module

```console
$ target/debug/lamedh -s "(progn
      (defmodule geometry (:export area))
      (module-p 'geometry))"
; => T
```

The general shape is:

```lisp
(defmodule name
  (:export sym...)             ; names bound by IMPORT
  (:requires CAP...)           ; optional, introspection only (see §10.4)
  (:provides CAP...))          ; optional, registers new custom capabilities
```

`:requires` is documentation, not enforcement — it records which host
capabilities the module's operations are expected to need, readable back
via `module-requires`, but nothing checks that the module's functions
actually stay within that set; enforcement, as always, lives at the gated
builtins themselves (`read-file` and friends still check `READ-FS` no
matter what module called them). `:provides` is different — it registers
one or more *new* capability names into the same attenuable vocabulary the
five built-in capabilities live in; §10.4 covers the (deliberately
conservative) semantics.

`with-module` will auto-declare a module with no exports if you populate
it before ever calling `defmodule` explicitly:

```console
$ target/debug/lamedh -s "(progn
      (with-module widgets (defun make (n) (* n 2)))
      (list (module-p 'widgets) (module-exports 'widgets) (widgets:make 5)))"
; => (T () 10)
```

`widgets:make` is defined and callable either way; without a declared
`:export` there's just nothing for `import` to bind later.

## 10.2 `with-module`: qualified definitions

`(with-module name form...)` evaluates `form...` with two rewrites
applied to each top-level form first: a **definition head** — `defun`,
`defun*`, `defmacro`, `defexpr`, `defvau`, `def`, `defdynamic`, `defrecord`,
`defvariant` — has the name it defines qualified to `NAME:SYMBOL`, and
every **reference** to a name defined anywhere in this module (this call's
forms, plus every name collected by earlier `with-module` calls for the
same module) is qualified the same way. `quote` and `quasiquote` subtrees
are left untouched, so quoted data keeps its plain, unqualified symbols.

```console
$ target/debug/lamedh -s "(progn
      (defmodule geometry (:export area))
      (with-module geometry
        (defun area (r) (* 3 (* r r)))
        (defun helper (x) x))
      (list (geometry:area 2) (geometry:helper 5)))"
; => (12 5)
```

`area` calls nothing here, but the general rule — a module function
calling another module function by its bare, unqualified name — resolves
correctly because the caller's reference gets qualified right alongside
the callee's definition:

```console
$ target/debug/lamedh -s "(progn
      (defmodule geometry (:export area))
      (with-module geometry
        (defun helper (x) (* x 3))
        (defun area (r) (helper (* r r))))
      (geometry:area 4))"
; => 48
```

The unqualified name is *not* bound by definition alone — only the
qualified spelling is, until you `import` (§10.3):

```console
$ target/debug/lamedh -s "(progn
      (defmodule geometry (:export area))
      (with-module geometry (defun area (r) (* 3 (* r r))))
      (handler-case (area 2) (error (er) 'unbound)))"
; => UNBOUND
```

Quoted data inside the body is left alone — a module function that
returns a list of *symbols* (as opposed to calling them) gets the plain,
unqualified names back:

```console
$ target/debug/lamedh -s "(progn
      (defmodule geometry (:export tags))
      (with-module geometry (defun tags () '(area helper)))
      (geometry:tags))"
; => (AREA HELPER)
```

A later `with-module` call for the same module sees every name collected
by earlier calls, so a second block can reference the first block's
functions by their bare names too:

```console
$ target/debug/lamedh -s "(progn
      (defmodule geometry (:export area))
      (with-module geometry (defun area (r) (* 3 (* r r))))
      (with-module geometry (defun twice-area (r) (* 2 (area r))))
      (geometry:twice-area 2))"
; => 24
```

### Records and variants inside modules

`defrecord` and `defvariant` generate more than one symbol — constructors,
predicates, accessors, validators — and every generated name also gets
the uniform `MODULE:LOCAL` spelling, even where the *stored* name embeds
the qualified brand internally (`make-scene` is stored as
`MAKE-GEOMETRY:SCENE`, but callers only ever need to know one spelling:
`geometry:make-scene`):

```console
$ target/debug/lamedh -s "(progn
      (defmodule geometry (:export scene-area))
      (with-module geometry
        (defvariant shape (circle (r int64)) (rect (w int64) (h int64)))
        (defrecord scene (items (list shape)))
        (defun scene-area (sc)
          (reduce #'+
            (mapcar (lambda (s)
                      (variant-case s
                        (circle (r) (* 3 (* r r)))
                        (rect (w h) (* w h))))
                    (scene-items sc))
            0)))
      (list (geometry:circle-r (geometry:circle 3))
            (geometry:shape-p (geometry:rect 1 2))
            (geometry:scene-area
              (geometry:make-scene (list (geometry:circle 2) (geometry:rect 3 4))))))"
; => (3 T 24)
```

`geometry:circle`, `geometry:circle-r`, `geometry:shape-p`, `geometry:
make-scene`, `geometry:scene-area` — constructors, accessors, predicates,
and the hand-written function all qualify the same uniform way, so calling
into a module never requires remembering which generated-name convention a
particular symbol happens to use internally.

## 10.3 `import`: binding exports globally

`(import name)` binds each name in the module's `:export` list, globally
and unqualified, to that export's *current* value:

```console
$ target/debug/lamedh -s "(progn
      (defmodule geometry (:export area))
      (with-module geometry (defun area (r) (* 3 (* r r))))
      (import geometry)
      (area 3))"
; => 27
```

Like `defmodule`, `import` takes its argument unevaluated — write
`(import geometry)`, not `(import 'geometry)`.

Only exported names come across; `helper` above was never in the
`:export` list, so it stays unbound after import:

```console
$ target/debug/lamedh -s "(progn
      (defmodule geometry (:export area))
      (with-module geometry
        (defun helper (x) (* x 3))
        (defun area (r) (helper (* r r))))
      (import geometry)
      (handler-case (helper 3) (error (er) 'unbound)))"
; => UNBOUND
```

`import` is a **snapshot**, not a live alias — it copies the export's
value at the moment of the call, so redefining the module function later
does not retroactively change what an earlier `import` bound. Re-import to
pick up the new value:

```console
$ target/debug/lamedh -s "(progn
      (defmodule geometry (:export area))
      (with-module geometry (defun area (r) (* 3 (* r r))))
      (import geometry)
      (let ((first (area 2)))
        (with-module geometry (defun area (r) (* 4 (* r r))))
        (list first (area 2) (progn (import geometry) (area 2)))))"
; => (12 12 16)
```

`first` and the middle `(area 2)` both see the original, snapshotted
`area` (12 — the `(* 3 (* r r))` edition); only after the second
`(import geometry)` does the unqualified `area` see the redefinition (16 —
`(* 4 (* r r))`). `geometry:area` itself, the qualified name, was live the
whole time; it's only the unqualified copy that needs re-importing.

`import` errors on an unknown module, or on an exported name that was
declared but never actually defined:

```console
$ target/debug/lamedh -s "(import nosuchmodule)"
Error: import: unknown module NOSUCHMODULE

$ target/debug/lamedh -s "(progn
      (defmodule geometry (:export area perimeter))
      (with-module geometry (defun area (r) (* 3 (* r r))))
      (import geometry))"
Error: Unbound variable: GEOMETRY:PERIMETER
  in: MAPC ← MAPC
```

## 10.4 Introspection

| Function | Reports |
|---|---|
| `(module-p m)` | `T` when `m` has been declared (by `defmodule` or an auto-declaring `with-module`) |
| `(module-of fn-name)` | the module a qualified function name belongs to |
| `(module-functions m)` | every qualified name defined so far under `m`, in definition order |
| `(module-exports m)` | `m`'s `:export` list |
| `(module-requires m)` | `m`'s `:requires` list |
| `(module-provides m)` | `m`'s `:provides` list — the custom capabilities it registers |

```console
$ target/debug/lamedh -s "(progn
      (defmodule geometry (:export area) (:requires READ-FS) (:provides FAST-MATH))
      (with-module geometry
        (defun helper (x) (* x 3))
        (defun area (r) (helper (* r r))))
      (list (module-of 'geometry:area)
            (module-functions 'geometry)
            (module-exports 'geometry)
            (module-requires 'geometry)
            (module-provides 'geometry)))"
; => (GEOMETRY (GEOMETRY:HELPER GEOMETRY:AREA) (AREA) (READ-FS) (FAST-MATH))
```

## 10.5 Capability provision

`(:provides CAP...)` lets a module register brand-new capability names
into the same attenuable vocabulary the five built-in capabilities
(`READ-FS`, `CREATE-FS`, `TEMP-FS`, `SHELL`, `IO`, Chapter 7) live in. This
is deliberately the most conservative extension point in the module
system, because it touches the sandbox story:

- Registering `(:provides CAP)` **holds** `CAP` at the outermost level —
  it shows up in `(capabilities-effective)` as soon as the module is
  declared, with no host grant needed.
- It **gates only explicit `(require-capability 'CAP)` checks** that Lisp
  code chooses to place — there is no way to make an existing gated
  builtin (`read-file`, `write-file`, ...) respect a custom capability;
  those remain gated by the five built-in names only.
- It **attenuates exactly like a built-in** — `with-capabilities`,
  `sandboxed`, and `spawn`'s capability intersection all narrow a custom
  capability away the same way they narrow `READ-FS` away.
- It **can never grant a kernel ability** — `:provides` only ever extends
  the vocabulary of names `require-capability` can check; it cannot loosen
  what the kernel itself enforces for filesystem, shell, or stdin access.

Held by registration, and narrowed by a fence — the deny/allow pair:

```console
$ target/debug/lamedh -s "(progn
      (defmodule geometry (:export area) (:provides FAST-MATH))
      (member 'FAST-MATH (capabilities-effective)))"
; => (FAST-MATH)

$ target/debug/lamedh -s "(progn
      (defmodule geometry (:export area) (:provides FAST-MATH))
      (with-capabilities '(FAST-MATH) (require-capability 'FAST-MATH)))"
; => T

$ target/debug/lamedh -s "(progn
      (defmodule geometry (:export area) (:provides FAST-MATH))
      (handler-case (with-capabilities '() (require-capability 'FAST-MATH))
        (error (er) (error-message er))))"
; => "capability denied: FAST-MATH (effective: ())"
```

And the ceiling: a module capability never reaches into kernel-enforced
territory, fence or no fence —

```console
$ target/debug/lamedh -s "(progn
      (defmodule geometry (:export area) (:provides FAST-MATH))
      (handler-case (read-file \"/etc/hostname\") (error (er) 'denied)))"
; => DENIED
```

`FAST-MATH` being effective never touched `READ-FS`; `read-file` is denied
here for the ordinary §7.1 reason — the host never granted `READ-FS` at
all — completely independent of anything the module registered.

## 10.6 Caveat: name-based qualification

`with-module`'s rewrite is **name-based**: it walks the body forms and
substitutes any bare symbol matching a known module name, wherever it
appears — parameter lists, `let` bindings, ordinary references, all alike
— with no notion of lexical scope. Most of the time this is invisible,
because a rename applied uniformly to both a binding and its uses stays
self-consistent (a parameter that happens to share a module function's
name still shadows correctly, since binding and reference are rewritten
together). The case that actually breaks is a local binding whose name
you also want to reference **as the outer function** inside the same body:

```console
$ target/debug/lamedh -s "(progn
      (defmodule geometry (:export area))
      (with-module geometry
        (defun area (r) (* 3 (* r r)))
        (defun mix (w h)
          (let ((area (* w h)))
            (+ area (area 5)))))
      (geometry:mix 2 3))"
Error: Not a function: Number(6)
  in: GEOMETRY:MIX
```

`mix` meant `(area 5)` as a call to the real `geometry:area` function, but
because the local variable is *also* named `area`, the rewrite qualifies
every occurrence to `geometry:area` uniformly — binding, reference, and
the intended function call alike — so by the time `(area 5)` runs,
`geometry:area` has been shadowed by the `let`-bound number `6`, and
calling it errors. The fix is the usual one: don't reuse a module
function's name as an inner binding inside that module's own body.
