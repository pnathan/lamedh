# 12. Codecs: JSON, URL, Base64, Hex, and MIME

Chapter 3 covered `TEXT`'s explicit String↔UTF-8 boundary and Chapter 11
covered `PORTS`' binary I/O. This chapter covers the pure-data codecs built
on top of both: `JSON`, `URL`, `BASE64`, `HEX`, and `MIME` — five optional
embedded libraries (`lib/32-base64.lisp` through `lib/36-mime.lisp`) for
ordinary application and HTTP programming. None of them touch the
filesystem, the network, or any other host resource: every operation is a
pure data transform, so **no capability is required** for any of these
modules, and they all work inside a sandbox with every capability denied.

Pull each in with `(require 'name)` on a `with_prelude()`-style
environment, or `(import name)` to bind its exports unqualified;
`with_stdlib()` environments (including the `lamedh` CLI) already have all
five loaded — `import` is all you need. They are independently requirable:
`(require 'json)` does not pull in `url`, `base64`, `hex`, or `mime`, and
vice versa.

```console
$ target/debug/lamedh -s "(progn (import json) (import url) (import base64) (import hex) (import mime) t)"
; => T
```

Naming follows the rest of the epic: `-p` predicates, `!` mutators (none
of these modules have any — everything here is a pure function), `->`
conversions, and encode/decode pairs named consistently across the five
modules (`base64:encode`/`decode`, `hex:encode`/`decode`,
`url:encode-*`/`decode`, `json:parse`/`stringify`).

`BASE64`, `HEX`, and `URL` all export `ENCODE`/`DECODE`-shaped names
(`URL` exports `DECODE`, not `ENCODE`, since it has two context-specific
encoders instead). `IMPORT` binds every export globally by value
(Chapter 10), so `(import base64) (import hex)` leaves the unqualified
`ENCODE`/`DECODE` bound to whichever module was imported *last* — the
same shadowing risk the `PORTS` chapter's `POSITION` name deliberately
avoided by staying qualified-only. Call the qualified name
(`base64:encode`, `hex:decode`, ...) when more than one codec module is
in scope at once.

## 12.1 JSON

`JSON` (`lib/35-json.lisp`) parses and serializes JSON text. The value
mapping is fixed and round-trippable in both directions:

| JSON | Lamedh |
|---|---|
| object | hash table, `String` keys (last-key-wins on duplicates) |
| array | `Array` — not a list; Lisp lists play no part in the JSON mapping |
| string | `String` |
| `true` | `T` |
| `false` | `NIL` |
| `null` | the keyword `:NULL` — **never** `NIL` |
| integer literal in `[-2^63, 2^63-1]` | `Number` (`i64`), exact |
| integer literal outside that range | `:ON-INTEGER-OVERFLOW` policy (below) |
| any other finite number (has `.` or an exponent) | `Float` |

`false`, `null`, and `[]` are three distinct, mutually distinguishable
values — the mapping deliberately avoids Lisp's `NIL`-is-both-false-and-
empty-list pun:

```console
$ target/debug/lamedh -s "(progn (import json) (list (parse \"false\") (parse \"null\") (null-p (parse \"null\"))))"
; => (() :NULL T)
```

```console
$ target/debug/lamedh -s "(progn (import json) (array->list (gethash (parse \"{\\\"a\\\":1,\\\"b\\\":[1,2.5,true,false,null]}\") \"b\")))"
; => (1 2.5 T () :NULL)
```

### 12.1.1 Numbers

Integers that fit exactly in an `i64` parse as an exact `Number`:

```console
$ target/debug/lamedh -s "(progn (import json) (parse \"9223372036854775807\"))"
; => 9223372036854775807
```

An integer literal outside `i64` range never silently loses precision.
`:ON-INTEGER-OVERFLOW` (default `:ERROR`) decides what happens instead:

```console
$ target/debug/lamedh -s "(progn (import json) (parse \"99999999999999999999\"))"
Error: JSON: integer 99999999999999999999 is out of i64 range (line 1, column 1)
  in: JSON:$JSON-ERROR ← PARSE

$ target/debug/lamedh -s "(progn (import json) (parse \"99999999999999999999\" :on-integer-overflow ':float))"
; => 100000000000000000000.0
```

Every other JSON number (anything with a `.` or an exponent) is a `Float`.
`STRINGIFY` is the exact inverse in both directions: a `Float` always
serializes with a `.` — even a whole-valued one like `2.0`, which Rust's
default float formatter would otherwise print as a bare `2` — so it always
round-trips back through `PARSE` as a `Float`, never silently becoming an
integer `Number`:

```console
$ target/debug/lamedh -s "(progn (import json) (floatp (parse (stringify 2.0))))"
; => T
```

A `NaN` or infinite `Float` cannot be represented in JSON at all; `STRINGIFY`
signals an error rather than emitting invalid JSON or a silent
approximation.

### 12.1.2 Strings

JSON strings decode through the standard escapes (`\" \\ \/ \b \f \n \r
\t`) plus `\uXXXX`, including UTF-16 surrogate pairs for astral-plane
code points — `🎉` decodes to 🎉. A lone/unpaired surrogate
escape is an error: a Lamedh `String` is valid Unicode text and cannot
hold a lone surrogate code point. An unescaped control character
(code point < `0x20`) inside a string is also an error — this is a strict
data codec, not a lenient reader.

### 12.1.3 Parsing is strict

`PARSE` rejects trailing garbage after the top-level value, leading zeros
in a number literal (`"01"`), and any structurally malformed input,
naming the line and column:

```console
$ target/debug/lamedh -s "(progn (import json) (parse \"{\n  \\\"a\\\": ,\n}\"))"
Error: JSON: unexpected character "," (line 2, column 8)
  in: JSON:$JSON-ERROR ← JSON:$JSON-PARSE-OBJECT-MEMBERS ← JSON:$JSON-PARSE-OBJECT ← PARSE
```

### 12.1.4 Depth limit

`:MAX-DEPTH` (default 512) bounds array/object nesting. Input nested
deeper than the limit is a clean JSON error, never a native stack
overflow:

```console
$ target/debug/lamedh -s "(progn (import json) (parse \"[[[[[1]]]]]\" :max-depth 3))"
Error: JSON: nesting depth exceeds limit of 3 (line 1, column 4)
  in: JSON:$JSON-ERROR ← JSON:$JSON-PARSE-ARRAY ← JSON:$JSON-PARSE-ARRAY-ITEMS-ACC ← JSON:$JSON-PARSE-ARRAY ← JSON:$JSON-PARSE-ARRAY-ITEMS-ACC ← JSON:$JSON-PARSE-ARRAY ← JSON:$JSON-PARSE-ARRAY-ITEMS-ACC ← JSON:$JSON-PARSE-ARRAY ← PARSE
```

### 12.1.5 Serializing

`STRINGIFY` produces compact output by default (no insignificant
whitespace); `:PRETTY T` produces indented multi-line output (`:INDENT`,
default 2, spaces per level):

```console
$ target/debug/lamedh -s "(progn (import json) (stringify (parse \"[1,2]\") :pretty t))"
; => "[\n  1,\n  2\n]"
```

## 12.2 Base64 and Hex

`BASE64` (`lib/32-base64.lisp`) and `HEX` (`lib/33-hex.lisp`) both encode
`Array<Char>` bytes (a byte is a `Char` **or** an integer 0–255, the
Chapter 3 byte-array convention) to an ASCII `String`, and decode back to
a fresh `Array<Char>` of the exact original bytes — every one of the 256
byte values, in every position, round-trips exactly.

```console
$ target/debug/lamedh -s "(progn (import text) (import base64) (encode (text:string->utf8 \"hello\")))"
; => "aGVsbG8="

$ target/debug/lamedh -s "(progn (import text) (import base64) (text:utf8->string (decode \"aGVsbG8=\")))"
; => "hello"
```

`BASE64:ENCODE`/`DECODE` take `:ALPHABET` (`:STANDARD`, RFC 4648 §4's
`+/`, or `:URL`, RFC 4648 §5's `-_`) and `:PAD` (default `T`; `NIL` omits
trailing `=` padding on encode and rejects it on decode) — independently:

```console
$ target/debug/lamedh -s "(progn (import text) (import base64) (list (encode (text:string->utf8 \"f\") :pad nil) (encode (text:string->utf8 \"f\") :alphabet ':url)))"
; => ("Zg" "Zg==")
```

`HEX:ENCODE` takes `:CASE` (`:LOWER`, the default, or `:UPPER`) —
predictable output case; `HEX:DECODE` is case-insensitive on input, the
usual hex-codec leniency:

```console
$ target/debug/lamedh -s "(progn (import hex) (list (encode (list->array (list 171 205 239))) (encode (list->array (list 171 205 239)) :case ':upper)))"
; => ("abcdef" "ABCDEF")
```

Both `DECODE`s are strict: an invalid character, wrong padding count/
position, or a length inconsistent with the padding policy is a named
error, not a silently truncated or ignored result.

## 12.3 URL

`URL` (`lib/34-url.lisp`) covers full URL parse/build, percent-encoding,
and query-string parse/build.

### 12.3.1 Percent-encoding: two encoders, one decoder

Path segments and query components have *different* safe-character sets —
conflating them is a real bug class (an unencoded `&` inside a path
segment is harmless; the same byte inside a query value truncates it), so
there are two encoders:

- `ENCODE-PATH-SEGMENT` — unreserved characters plus sub-delimiters and
  `:`/`@` stay literal; `/` is always encoded (it is the segment
  separator).
- `ENCODE-QUERY-COMPONENT` — only unreserved characters stay literal;
  everything else, including `&`, `=`, `+`, stays percent-encoded.

```console
$ target/debug/lamedh -s "(progn (import url) (encode-path-segment \"a b&c\"))"
; => "a%20b&c"

$ target/debug/lamedh -s "(progn (import url) (encode-query-component \"a b&c\"))"
; => "a%20b%26c"
```

Percent-*decoding* is context-free — `%XX` always means the same byte no
matter which encoder produced it — so there is one `DECODE` (`:LOSSY`,
default `NIL`, mirrors `TEXT:UTF8->STRING`/`-LOSSY`: a malformed escape is
always an error, but invalid UTF-8 *after* decoding is either a strict
error or `U+FFFD` replacement). `DECODE-PATH-SEGMENT`/
`DECODE-QUERY-COMPONENT` are plain aliases of `DECODE`, provided purely so
every `ENCODE-*` has a same-named inverse.

### 12.3.2 Full URL parse/build

`PARSE` splits a URL string into an alist — `SCHEME`, `USERINFO`, `HOST`,
`PORT`, `PATH`, `QUERY`, `FRAGMENT` — with matching accessors of the same
names; `BUILD` is the inverse:

```console
$ target/debug/lamedh -s "(progn (import url) (let ((u (parse \"https://user:pw@example.com:8080/a/b?x=1&y=2#frag\"))) (list (scheme u) (userinfo u) (host u) (port u) (path u) (query u) (fragment u))))"
; => ("https" "user:pw" "example.com" 8080 "/a/b" "x=1&y=2" "frag")

$ target/debug/lamedh -s "(progn (import url) (build (parse \"https://example.com/a/b?x=1#f\")))"
; => "https://example.com/a/b?x=1#f"
```

`PATH`/`QUERY`/`FRAGMENT`/`USERINFO` come back **raw** — still
percent-encoded exactly as they appeared in the URL, never auto-decoded —
so there is no double-decoding ambiguity; call `DECODE`/`PARSE-QUERY`
yourself on the pieces you want decoded. `HOST` handles a bracketed IPv6
literal (`[::1]`) as a unit. No regular expressions are used anywhere in
this module — parsing is a small explicit state machine over `#`, `?`,
the first valid `scheme:`, `//`, `@`, and the last `:` in the host:port
piece.

### 12.3.3 Query strings

`PARSE-QUERY`/`BUILD-QUERY` preserve repeated keys and ordering — a query
string is a list of `(key . value)` conses, never collapsed into a hash
table (which would silently drop repeats):

```console
$ target/debug/lamedh -s "(progn (import url) (parse-query \"a=1&b=2&a=3\"))"
; => (("a" . "1") ("b" . "2") ("a" . "3"))

$ target/debug/lamedh -s "(progn (import url) (build-query (list (cons \"q\" \"hello world\") (cons \"lang\" \"en\"))))"
; => "q=hello%20world&lang=en"
```

## 12.4 MIME: headers and Content-Type

`MIME` (`lib/36-mime.lisp`) covers case-insensitive HTTP-style headers and
Content-Type parse/build.

### 12.4.1 Headers

A header list is `(name . value)` conses, in original order, **original
case preserved** — deliberately not a hash table, since a hash keyed by a
case-folded name could hold only one value per key and would silently
collapse a repeated header like `Set-Cookie`:

```console
$ target/debug/lamedh -s "(progn (import mime) (headers-get-all (headers-add (headers-add (list) \"Set-Cookie\" \"a=1\") \"Set-Cookie\" \"b=2\") \"set-cookie\"))"
; => ("a=1" "b=2")
```

`HEADER-NAME=` is the case-insensitive comparison primitive; `HEADERS-GET`
returns only the first match (convenience), `HEADERS-GET-ALL` returns
every match (the multi-value accessor — use this for anything that might
repeat), `HEADERS-ADD` appends without touching existing entries,
`HEADERS-SET` replaces every existing match with one new entry (use only
for headers that must be singular, e.g. `Content-Type`), `HEADERS-REMOVE`
drops every match, and `HEADERS-NAMES` lists the distinct names in the
casing each was *first* given.

### 12.4.2 Content-Type

`PARSE-CONTENT-TYPE` returns an alist — `TYPE`, `SUBTYPE`, `PARAMETERS` (a
list of `(name . value)` conses, quoted-string values already unescaped)
— and `CONTENT-TYPE-PARAMETER` looks a parameter up case-insensitively:

```console
$ target/debug/lamedh -s "(progn (import mime) (parse-content-type \"text/html; charset=UTF-8\"))"
; => ((TYPE . "text") (SUBTYPE . "html") (PARAMETERS ("charset" . "UTF-8")))
```

`BUILD-CONTENT-TYPE` is the inverse; a parameter value is written as a
bare token when possible, else a quoted-string with `\` and `"` escaped.

## 12.5 Summary

- All five modules are pure data transforms — no capability is required,
  and they work fully inside a sandbox with every capability denied.
- JSON: object↔hash table, array↔`Array` (never a list),
  true/false/null↔`T`/`NIL`/`:NULL` (three distinct values), integers
  exact in `i64` range with an explicit overflow policy, `STRINGIFY`
  round-trips `Float` as `Float`. Strict, line/column-located errors;
  `:MAX-DEPTH` bounds nesting.
- Base64/Hex: `Array<Char>` bytes↔ASCII `String`, every byte value
  round-trips exactly; explicit alphabet/padding/case policies; strict
  decode errors.
- URL: two percent-encoders (path segment vs. query component), one
  context-free decoder; full parse/build with raw (not auto-decoded)
  path/query/fragment; query-string parse/build preserves repeats and
  order.
- MIME: case-insensitive header comparison with original-case
  preservation and a genuine multi-value accessor; Content-Type
  parse/build with quoted-string parameters.
