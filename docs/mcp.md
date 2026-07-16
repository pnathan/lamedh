# MCP Server (`lamedh --mcp`)

`lamedh --mcp` runs Lamedh as a [Model Context
Protocol](https://modelcontextprotocol.io/) server over stdio: it reads
newline-delimited JSON-RPC 2.0 requests on stdin and writes responses on
stdout, driving **one persistent interpreter environment**. An agent connects
once and then converses with a live Lisp image — definitions and variables set
by one `eval` call are visible to the next.

It is the agent-facing sibling of `--check`, `--fmt`, and `--test`: where those
are one-shot batch tools, `--mcp` keeps a warm interpreter that an LLM can
poke at interactively, with the language's teaching errors delivered straight
into the tool results.

## Safety model

This is the important part, and it is **deliberately different from every other
CLI mode.**

The interactive REPL and the batch modes enable *all* capabilities by default —
they are developer tools run on code you wrote. `--mcp` does the opposite:

* **Sandboxed by default.** The server starts with **all capabilities OFF**,
  because its whole purpose is to evaluate untrusted, agent-generated code. A
  gated builtin (filesystem, shell, network, OS) fails with a capability error
  unless you granted it.
* **`--capability X` grants specific ones back**, exactly as elsewhere
  (repeatable, case-insensitive): `lamedh --mcp --capability READ-FS`.
* **`--sandbox` is a no-op** in MCP mode — all-off is already the default. It is
  accepted so scripts can pass it harmlessly.
* **Every `eval` / `run-tests` call is metered** by a per-call fuel budget (see
  below) so a runaway loop terminates instead of hanging the server.

The known capability names are `READ-FS`, `CREATE-FS`, `TEMP-FS`, `SHELL`,
`IO`, `NET-DNS`, `NET-CONNECT`, `NET-LISTEN`, `OS-ENV`, `OS-ENV-WRITE`,
`OS-PROCESS`, `OS-SIGNAL`. See Chapter 7 of the manual for the full capability
model.

Because untrusted code prints straight to the process's stdout — which in MCP
mode *is* the JSON-RPC wire — the server redirects the interpreter's stdout
aside (on unix) so `princ`/`terpri`/`format` output can never corrupt the
protocol. Captured output is returned *inside* the relevant tool result. Only
protocol JSON is ever written to stdout; diagnostics go to stderr.

## Fuel semantics

Each `eval` and `run-tests` call runs inside a `WITH-FUEL` fence (the same
kernel step budget documented in Chapter 7 and behind `--fuel`; see
[Getting Started §1.8](manual/01-getting-started.md)):

* `--fuel N` sets the per-call budget. **Without the flag it defaults to a
  generous `100000000` steps**, so the server can never hang on ordinary
  runaway code even if the operator forgets to set it.
* The budget is **re-armed fresh for every call** — it is not shared across
  calls.
* `WITH-FUEL` is a **narrow-only** fence: untrusted code inside it cannot widen
  or disarm the budget (`(kernel-fuel-set! nil)` errors), so a program cannot
  simply turn the guard off and loop forever.
* On exhaustion the call returns a `fuel exhausted (kernel step budget)` error
  with `isError: true`, and the server stays responsive for the next request.
* Arming fuel disables the native JIT for the metered call (a documented
  no-compile consequence of the fuel machinery). This affects speed only.
* The `doc`, `apropos`, `check`, and `introspect` tools are bounded, trusted
  operations and are **not** fenced — in particular `introspect` must run
  unmetered so `why-not-typed`'s compile probe reports accurately.

**Known limitation.** Code that explicitly *catches* the `fuel exhausted`
condition and re-enters a loop can evade the budget (the exhaustion signal
disarms the counter so handler and cleanup forms can themselves run). This is
the same class of documented hole noted for pre-compiled natives in
`lib/22-guard.lisp`. Ordinary non-terminating programs — unbounded recursion,
`(while t ...)`, the omega combinator — always terminate. Lamedh's interpreter
is single-threaded (`Rc`-based values), so there is no wall-clock watchdog; the
fuel fence is the termination mechanism.

## Protocol surface

Requests and responses are one JSON object per line.

| Method                     | Response                                                                 |
|----------------------------|--------------------------------------------------------------------------|
| `initialize`               | `{protocolVersion, capabilities:{tools:{}}, serverInfo:{name,version}}`   |
| `notifications/initialized`| *(none — notifications carry no `id`)*                                    |
| `ping`                     | `{}`                                                                      |
| `tools/list`               | `{tools:[…]}` — the six tools with JSON-Schema `inputSchema`             |
| `tools/call`               | `{content:[{type:"text",text}], isError}`                                |
| *unknown method*           | JSON-RPC error `-32601`                                                   |
| *unparseable line*         | JSON-RPC error `-32700`                                                   |

`initialize` advertises protocol version `2025-06-18`, echoing the client's
requested `protocolVersion` when it sent one (so an older-but-known client keeps
its version). `serverInfo.version` is the `lamedh-cli` crate version.

Any message without an `id` is a notification and receives no response.
Tool-level failures (a Lisp error, an unknown tool name) are reported *inside*
the `tools/call` result with `isError: true`, not as JSON-RPC errors —
JSON-RPC errors are reserved for protocol-level problems.

## Tools

All six tools take a single string argument. Schemas are `{"type":"object",
"properties":{<arg>:{"type":"string"}}, "required":[<arg>]}`.

| Tool         | Argument  | Behaviour |
|--------------|-----------|-----------|
| `eval`       | `source`  | Evaluate `source` in the **persistent** env (under a fuel fence). Returns the printed value of the last form, prefixed with anything the code printed. On error, the Lisp error text — which carries did-you-mean / teaching hints — with `isError: true`. |
| `check`      | `source`  | Statically check `source` **without executing it** (see [`--check`](check.md)): parse errors, unbound-function calls with did-you-mean, and provable arity mismatches, as one s-expression diagnostic per line. `(no findings)` when clean. `isError` is true only for a hard parse failure. |
| `doc`        | `symbol`  | The built-in help text for `symbol` — the same output the REPL's `help` prints (syntax, description, arguments, examples, see-also). |
| `apropos`    | `pattern` | Bound symbol names containing `pattern` (case-insensitive), one per line. |
| `run-tests`  | `source`  | Evaluate `source` in a **fresh scratch environment** (isolated from the persistent one and from testing bookkeeping), run every `deftest` it registered, and return the `test result: N passed; M failed` summary plus one `FAIL <name>: <message>` line per failure. Fenced by the per-call fuel budget. |
| `introspect` | `symbol`  | Combined report: the symbol's inferred `signature`, whether it compiled to native code (`compiled-p`), and why it did not type-check (`why-not-typed`). Runs unmetered. |

`run-tests` deliberately uses a throwaway world so a test file's definitions do
not leak into the persistent `eval` session, and so its `deftest` registrations
do not accumulate across calls. If you want tests to run against your live
image, define and run them via `eval` instead.

## Examples

A minimal session (requests you send, responses the server returns):

```json
→ {"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18"}}
← {"jsonrpc":"2.0","id":1,"result":{"protocolVersion":"2025-06-18","capabilities":{"tools":{}},"serverInfo":{"name":"lamedh","version":"0.3.1"}}}
→ {"jsonrpc":"2.0","method":"notifications/initialized"}
→ {"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"eval","arguments":{"source":"(setq x 5)"}}}
← {"jsonrpc":"2.0","id":2,"result":{"content":[{"type":"text","text":"5"}],"isError":false}}
→ {"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"eval","arguments":{"source":"(* x x)"}}}
← {"jsonrpc":"2.0","id":3,"result":{"content":[{"type":"text","text":"25"}],"isError":false}}
→ {"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"eval","arguments":{"source":"(lenght nil)"}}}
← {"jsonrpc":"2.0","id":4,"result":{"content":[{"type":"text","text":"Error: Unbound variable: LENGHT — did you mean LENGTH?"}],"isError":true}}
```

## Example agent configuration

Most MCP-aware agents take a command plus arguments. To register a read-only
Lamedh server (filesystem reads allowed, nothing else):

```json
{
  "mcpServers": {
    "lamedh": {
      "command": "lamedh",
      "args": ["--mcp", "--capability", "READ-FS"]
    }
  }
}
```

Drop the `--capability` arguments for a fully sandboxed server (pure
computation, no host access), or add more (`--capability SHELL`,
`--capability NET-CONNECT`, …) to widen it. To cap per-call work more tightly
than the 100M default, add `"--fuel", "5000000"`.
