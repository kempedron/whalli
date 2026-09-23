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

## Module `sql`

`import sql` (`stdlib/sql.rs`): Unified SQL database interface supporting `sqlite`, `postgres`, and `mysql`.

Supported drivers:
- `"sqlite"` / `"sqlite3"`: Bundled zero-config SQLite (`":memory:"` or `"file.db"`).
- `"postgres"` / `"postgresql"` / `"pg"`: Connection string e.g. `"postgresql://user:pass@localhost:5432/mydb"`. (Automatic `?` to `$1` parameter translation).
- `"mysql"` / `"mariadb"`: Connection string e.g. `"mysql://user:pass@localhost:3306/mydb"`.

Functions & Methods:
- `sql.open(driver: str, conn_str: str) -> (db: map | nil, err: str | nil)`: connects to database.
- `db.exec(query: str, params: list = []) -> (result: map | nil, err: str | nil)`: runs DDL, INSERT, UPDATE, DELETE queries. Returns `{"rows_affected": int, "last_insert_id": int}`.
- `db.query(query: str, params: list = []) -> (rows: list[map] | nil, err: str | nil)`: runs SELECT queries. Returns a list of maps keyed by column name.
- `db.query_row(query: str, params: list = []) -> (row: map | nil, err: str | nil)`: returns first matching row as a map, or `nil` if not found.
- `db.close() -> bool`: closes the database connection.

Example:
```whalli
import sql

// Connect to SQLite, PostgreSQL, or MySQL
let (db, err) = sql.open("sqlite", "app.db")
db.exec("CREATE TABLE IF NOT EXISTS users (id INTEGER PRIMARY KEY, name TEXT, age INTEGER)")
db.exec("INSERT INTO users (name, age) VALUES (?, ?)", ["Alice", 30])

let (rows, _) = db.query("SELECT * FROM users WHERE age >= ?", [18])
let i = 0
while i < rows.len() {
    let u = rows[i]
    println(u["id"], u["name"], u["age"])
    i += 1
}
db.close()
```

## Module `sync`

`import sync` (`stdlib/sync.rs`): `sync.WaitGroup()`, `sync.Mutex()` — see Concurrency chapter.

## Value Methods

Dispatched via `Value::call_method` (`value.rs:92-398`):

- **String**: `len()`, `trim()`, `to_lower()`, `to_upper()`, `contains(sub)`, `starts_with(pfx)`, `ends_with(sfx)`, `replace(from,to)`, `split(delim)->list`, `json()->any`
- **List**: `push(v)`, `pop()->v`, `len()`, `sort()`, `reverse()`, `clear()`, `contains(v)->bool`, `join(sep="")->str`
- **Map**: `len()`, `remove(key)->old`, `keys()->list`, `get(key, default?)->val`, `json()->any`; router methods: `get(path,handler)`, `post`, `put`, `delete`, `patch`, `head`, `options`, `handle(method,path,handler)`, `use(mw)`, `group(prefix)`, `static(prefix,dir)`, `not_found(fn)`, `method_not_allowed(fn)`; request methods: `header(k,def?)`, `cookie(k,def?)`, `query(k,def?)`, `param(k,def?)`, `form()`, `form_value(k,def?)`
- **Tuple**: `len()`, `sort()->tuple`
- **Response/Request maps**: `.json()` parses `"text"` or `"body"` field.

Index operations: `map["key"]`, `list[i]`, `str[i]` goes through VM `Index` opcodes; `map.get` is separate method (not index).

## Error Conventions

No exceptions. Functions that can fail return `(value, err)` tuple where `err` is `str|nil` (`nil` on success) — e.g. `http.listen`, `net.read`, `json.decode`. Client `requests` returns a response map with `error` field instead. Use `?` postfix (`Expr::Try`) to propagate errors.
