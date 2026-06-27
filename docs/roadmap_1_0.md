# Roadmap To 1.0

Lamedh remains on the `0.2.x` version line while the 1.0 release gates are
closed. The goal of this ramp is not more surface area; it is making the
existing language predictable, documented, and supportable.

## Release Gates

- **Semantic hardening:** malformed evaluator forms must reject improper lists;
  recursive `LABEL` must remain a function naming form, not a general delayed
  expression mechanism; hash-table keys must obey Rust `Eq`/`Hash` contracts.
- **Typed boundary:** typed definitions, inferred untyped functions, arrays,
  strings, structs, and native membranes need stable behavior in both default
  and `--no-default-features` builds.
- **Capability model:** filesystem, shell, stdin, and temp-file operations must
  stay opt-in and documented at the host and CLI levels.
- **Documentation:** README, manual, docs pages, REPL help, benchmark notes, and
  rustdoc should agree with current behavior.
- **Verification:** `cargo test --workspace`, `cargo test --workspace
  --no-default-features`, clippy, rustdoc, and benchmark smoke checks should be
  clean before a release candidate.

## Deferred Past 1.0 Unless Needed

- Common Lisp packages and reader package syntax
- Common Lisp streams and `WITH-OPEN-FILE`
- A full condition/restart system
- Hygienic macro facilities beyond `GENSYM`
- General local mutual recursion via a separate `LABELS` form

## Version Policy

Keep package versions at `0.2.x` while these gates are being closed. Cut 1.0
only after the release checklist is passing and the remaining limitations are
documented as intentional non-goals or explicit post-1.0 work.
