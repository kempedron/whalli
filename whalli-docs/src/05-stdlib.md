# Standard Library

> Source `src/stdlib/mod.rs:11-211`, `src/stdlib/*.rs`, `src/value.rs:92-503`.

## Globals

Injected into `VM.globals` (`stdlib/mod.rs:20-164`):

| Name | Signature | Notes |
|---|---|---|
| `print` | `print(...any)` | `join(" ")` without newline |
| `println` | `println(...any)` | appends newline |
| `int` | `int(val) -> int` | int passthrough, float→int, str→parse, bool→1/0 else 0 |
| `float` | `float(val) -> float` | similar |
| `str` | `str(val) -> str` | `stringify` |
| `bool` | `bool(val) -> bool` | empty str/nil/0 → false |
| `new` | `new(type, len?, cap?) -> obj` | Only `list\|map\|chan` as `Value::Type`; strings rejected (`mod.rs:98-112`). `chan` capacity `cap==0` → 1 |
| `range` | `range(end)->Range`, `range(start,end)`, `range(start,end,step)` | `step==0` → `nil` |

Builtin types available as `Value::Type`: `int float str bool list map func chan` (`vm.rs:329-335`).

## Module `math`

`import math` (`stdlib/math.rs`): wrappers around `std` — `pi`, `e`, `abs`, `ceil`, `floor`, `round`, `sqrt`, `pow`, `sin`, `cos`, `tan`, etc. Exact set mirrors `math.rs:register`.

## Module `fs`

`import fs` (`stdlib/fs.rs`): file I/O (blocking). Methods: `read(path) -> (content, err)`, `write(path, data) -> (ok, err)`, `exists`, `remove`, `mkdir`, `list_dir`, etc. All blocking; for small files.

## Module `os`

`import os` (`stdlib/os.rs`): `args() -> list`, `env(key) -> str|nil`, `set_env`, `exit(code)`.

## Module `time`

`import time` (`stdlib/time.rs`): `now()->float`, `sleep(secs)`, `after(secs)->chan`.

## Module `json`

`import json` (`stdlib/json.rs`):

- `json.encode(any) -> str` — uses `value_to_json` (`json.rs:7-54`) + `serde_json::to_string`.
- `json.decode(str) -> (value, err)` — `serde_json::from_str` → `json_to_value` (`json.rs:56-82`); numbers fit `i64` → `Int` else `Float`.

`value_to_json` mapping: `Nil→null`, `Bool→bool`, `Int/Float→number`, `Str→string`, `Tuple/List→array`, `Range→{start,end,step}`, `Map/Instance→object`.

## Module `net`

See [I/O and Networking](./04-io-net.md) for full table. Re-exported host is `net`.

## Module `http`

See [I/O and Networking](./04-io-net.md). Includes `router`, `match_route`, `listen_and_serve`, `response` family.

## Module `requests`

See [I/O and Networking](./04-io-net.md). Blocking client.

## Module `sync`

`import sync` (`stdlib/sync.rs`): `sync.WaitGroup()`, `sync.Mutex()` — see Concurrency chapter.

## Value Methods

Dispatched via `Value::call_method` (`value.rs:92-398`):

- **String**: `len()`, `trim()`, `to_lower()`, `to_upper()`, `contains(sub)`, `starts_with(pfx)`, `ends_with(sfx)`, `replace(from,to)`, `split(delim)->list`, `json()->any`
- **List**: `push(v)`, `pop()->v`, `len()`, `sort()`, `reverse()`, `clear()`, `contains(v)->bool`, `join(sep="")->str`
- **Map**: `len()`, `remove(key)->old`, `keys()->list`, `get(key)->val` (1-arg), `json()->any`, router sugar `get(path,handler)`, `post`, `put`, `delete`, `patch`, `head`, `options`, `handle(method,path,handler)`
- **Tuple**: `len()`, `sort()->tuple`
- **Response/Request maps**: `.json()` parses `"text"` or `"body"` field.

Index operations: `map["key"]`, `list[i]`, `str[i]` goes through VM `Index` opcodes; `map.get` is separate method (not index).

## Error Conventions

No exceptions. Functions that can fail return `(value, err)` tuple where `err` is `str|nil` (`nil` on success) — e.g. `http.listen`, `net.read`, `json.decode`. Client `requests` returns a response map with `error` field instead. Use `?` postfix (`Expr::Try`) to propagate errors.
