# LLM Reference (`llms.txt`)

Lamedh ships a single dense reference file for language models and coding
agents, following the [llms.txt convention](https://llmstxt.org/): syntax
gotchas, a Common-Lisp-divergence cheat sheet, the whole standard library
(one line per function: signature + description), and ten verified worked
examples covering records, protocols, pattern matching, sum types,
conditions/restarts, the typed JIT, modules, regex, testing, and the MCP
server.

- **Raw file, this Pages site**: <https://pnathan.github.io/lamedh/llms.txt>
- **Raw file, GitHub**: <https://raw.githubusercontent.com/pnathan/lamedh/main/llms.txt>
- **In the repository**: `llms.txt` at the repo root (also mirrored at
  `docs/llms.txt` for this site).

Point an agent's context/tool-use at the raw-file URL directly — it is
plain text, not this rendered page.

## Regenerating it

`llms.txt` is generated, not hand-written, so it never drifts from the
interpreter's own help database:

```sh
scripts/generate-llms-txt.sh   # also runs as part of scripts/generate-docs.sh
```

The hand-written framing (intro, gotchas, worked examples) lives in
`docs/llms-txt-template.md`; the function-index section is generated live
by evaluating `(render-llms-index)` (`lib/97-doc-renderer.lisp`) against a
release build, and the module-map table is extracted verbatim from
`src/lib.rs`'s doc comments. `tests/test_llms_txt.rs` guards the checked-in
file against silent bloat (100 KB budget) and template/docs-copy drift.
