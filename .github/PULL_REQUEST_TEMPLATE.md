## What and why

<!-- What changed, and the issue it closes (Fixes #N). -->

## Verified

<!-- Commands run and results. `scripts/gauntlet.sh` is the merge gate. -->

## Checklist

- [ ] Title is a Conventional Commit: `type(scope): description`
- [ ] Behavior change has a `CHANGELOG.md` entry (breaking changes flagged **Breaking**), or this PR is refactor/test/CI only
- [ ] Tests added or updated
- [ ] `cargo fmt --all` and `cargo clippy --workspace --all-targets --features fuzz -- -D warnings` clean
