# Language Tour

> Source: `src/lexer.rs`, `src/parser.rs:77-410`, `src/value.rs`, `src/stdlib/mod.rs`.

## Lexical

- Line comments: `// ...`
- Statement terminators: `NewLine` or `;` (`parser.rs:44-51`).
- Keywords: `let`, `func`, `struct`, `impl`, `interface`, `if`/`else`, `while`, `for`/`in`, `return`, `break`, `continue`, `import`, `wo`, `defer`, `is`, `match`, `select`, `default`, `nil`, `true`, `false`.

## Variables and Types

Dynamically typed. Built-in type tags: `int`, `float`, `str`, `bool`, `list`, `map`, `func`, `chan` (as `Value::Type`, `vm.rs:329-335`).

```whalli
let x = 42
let pi = 3.14159
let name = "Whalli"
let ok = true
let nothing = nil

// formatted strings (f-strings) — `parser.rs:662-718`
let greeting = f"Language: {name}, answer: {x}"
```

Conversions via globals (`stdlib/mod.rs:39-93`): `int(v)`, `float(v)`, `str(v)`, `bool(v)`.

Tuples (immutable, literal syntax only):
```whalli
let (status, code) = (true, 200)
let empty = ()
let pair = (1, "a")
```

## Literals

- `int`: decimal, optional leading `-` (`parser.rs:495-507`)
- `float`: `3.14`
- `str`: `"..."`, `f"..."` with `{expr}` interpolation
- `bool`/`nil`
- `list`: `[1, 2, 3]`
- `map`: `{"k": 1, "a": 2}` and `{}` — keys are expressions; string keys most common
- `range`: not a literal; produced by `range()` global (`stdlib/mod.rs:166-198`)

## Operators

- Arithmetic: `+ - * / %` (int/int, float/float, mixed int/float; `+` also concatenates strings — `vm.rs:832-894`).
- Comparison: `== != < > <= >=`
- Logic: `and`, `or`, `!` (`not`)
- Type test: `is` (`parser.rs:441-446`, `vm.rs:188-276`) — e.g. `handler is map`, `x is int`.
- Assignment: `=`, `+= -= *= /= %=` (`parser.rs:328-357`).
- Channel: `ch <- value` (send, `Expr::ChanSend`), `<- ch` (recv, `Expr::ChanRecv`).

## Functions

```whalli
func calculate(a: int, b: int) -> int {
    return a + b
}
let result = calculate(10, 20)
```

- Header: `func name(params) -> retType? block` (`parser.rs:372-410`).
- Params: `name: Type` optional. Return type parsed but not enforced beyond decoration (`ast.rs`/`compiler.rs`).
- `return` without value → `nil` (`parser.rs:216-228`).
- Closures: captured variables via upvalues (`opcode.rs:OpCode::Closure`).

## Control Flow

```whalli
if x > 0 { println("pos") } else if x < 0 { println("neg") } else { println("zero") }

while true {
    let client = net.accept(server)
    if client != nil { break }
}

for i in range(0, 10, 2) { print(i, "") }

for k in store.keys() { println(k) }
```

- `if` with optional `else`/`else if` (`parser.rs:721-740`)
- `while {cond} block`
- `for item in iterable block` — iterable is `Range` or any object iterating via VM
- `break`/`continue`
- `match` expression (`parser.rs:807-847`): patterns `Literal`, `Variable`, `Wildcard _`, `Type(x: int)`, `Tuple`, `Range 1..10 / 1..=10`, `Or a | b`, optional `if guard`.

```whalli
let label = match n {
    0 => "zero",
    1 | 2 => "small",
    x: int if x > 10 => "big",
    _ => "other",
}
```

## Structs, Impl, Interfaces

```whalli
struct Vector2 { x: int, y: int }

impl Vector2 {
    func norm_sq() -> int { return this.x * this.x + this.y * this.y }
}

interface Printable { func str() }

let v = new("Vector2")  // illegal: structs require field count check via StructDef call
// Correct: let v = Vector2(3, 4) — calls StructDef as callable (vm.rs:958-978)
v.x = 3
```

- `struct Name { field: Type, ... }` (`parser.rs:86-121`)
- `impl Name { func ... }` — methods stored in `StructDef.methods` (`heap.rs:27-31`), dispatched via `call_method` (`value.rs`)
- `interface Name { func foo(), func bar() }` (`parser.rs:148-179`) — runtime `is` checks all required methods exist on struct (`vm.rs:251-268`)
- Instantiation: `Name(args)` — positional args must match `fields.len()` (`vm.rs:959`).

## Collections

```whalli
let arr = new(list)
arr.push(10)
arr.push(20)
println(arr.len(), arr[0])   // 2 10

let user = new(map)
user["username"] = "kepr"
println(user.keys())

let ch = new(chan, 3)        // capacity positional arg 2 (vm uses arg_cap)
```

- `new(type, len?, cap?)` — only `list|map|chan` accepted, identifier required not string (`stdlib/mod.rs:98-112`)
- `list` methods: `push`, `pop`, `len`, `sort`, `reverse`, `clear`, `contains`, `join` (`value.rs:183-274`)
- `map` methods: `len`, `remove`, `keys`, `get`, `json`, plus router sugar `get/post/put/delete/patch/handle` when `len>=2` (`value.rs:276-389`)
- `str` methods: `len`, `trim`, `to_lower`, `to_upper`, `contains`, `starts_with`, `ends_with`, `replace`, `split`, `json` (`value.rs:98-165`)
- Index assign: `a[idx] = v` (`Stmt::IndexAssign`)

## Imports

```whalli
import http         // stdlib module (stdlib/mod.rs:200-208)
import "./math_utils.wh"  // file import (parser.rs:271-275, vm.rs import handling)
```

File imports are deduplicated via `imported_files: HashSet<PathBuf>` (`vm.rs:81,109`).

## Other Statements

- `defer expr` — deferred call executed on frame exit (`parser.rs:303-312`, `vm.rs:Defer` handling).
- `wo expr` — spawn woroutine (`parser.rs:286-301`, `vm.rs:Spawn`). Expression or statement form; requires `Call` or `MethodCall`.
- `let (a, b) = tuple_expr` — destructuring (`parser.rs:182-203`).

## Error Handling

- `?` postfix (`parser.rs:654-655`, `Expr::Try`) — propagates `(nil, err)` tuples (see `05-stdlib.md` error conventions).
- Runtime errors produce `RuntimeError {message, line}` (`vm.rs:39`), fatal for `main` task, isolated for background tasks (`vm.rs:782-808`).
