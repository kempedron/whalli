---
title: Language Tour
description: Tour of Whalli syntax, types, control flow, structs, and error handling.
---

## Lexical & Syntax

- **Line comments**: `// this is a comment`
- **Statement terminators**: Newline or semicolon `;`
- **Keywords**: `let`, `func`, `struct`, `impl`, `interface`, `if`/`else`, `while`, `for`/`in`, `return`, `break`, `continue`, `import`, `wo`, `defer`, `is`, `match`, `select`, `default`, `nil`, `true`, `false`.

## Variables and Types

Whalli is dynamically typed with strong built-in type tags (`int`, `float`, `str`, `bytes`, `bool`, `list`, `map`, `func`, `chan`, `tuple`).

```whalli
let x = 42
let pi = 3.14159
let name = "Whalli"
let ok = true
let nothing = nil

// Formatted strings (f-strings)
let greeting = f"Language: {name}, answer: {x}"

// Multiline strings ("""...""") and Raw strings (r"...")
let query = """
SELECT id, name
FROM users
WHERE active = true
"""
let path = r"C:\Windows\System32\drivers"

// Byte literals (bytes type) and Raw bytes (br"...")
let b = b"hello\x20world"
let b_hex = b.hex()            // "68656c6c6f20776f726c64"
let (decoded, _) = b.decode()  // "hello world"
let raw_b = br"raw\x00bytes"

// Destructuring for Tuples, Lists, and Objects:
let (status, code) = (true, 200)
let [first, second] = [10, 20]
let { name, age: user_age } = {"name": "Alice", "age": 25}
```

Type conversion globals: `int(v)`, `float(v)`, `str(v)`, `bytes(v)`, `bool(v)`.

## Operators

- **Arithmetic**: `+ - * / %` (numbers, string concatenation `str + str`, byte concatenation `bytes + bytes`).
- **Comparison**: `== != < > <= >=`
- **Logic**: `and`, `or`, `!` (`not`)
- **Type test**: `is` (e.g. `b is bytes`, `handler is map`, `x is int`)
- **Assignment**: `=`, `+= -= *= /= %=`
- **Channel**: `ch <- val` (send), `<- ch` (receive)
- **Error Propagation**: `expr?` (unwraps success value or early returns error tuple)

## Functions

```whalli
func calculate(a: int, b: int) -> int {
    return a + b
}

let result = calculate(10, 20)
```

- Anonymous closures and higher-order functions are fully supported.
- Functions capture lexical scope via VM upvalues.

## Control Flow

### If / Else
```whalli
if x > 0 {
    println("positive")
} else if x < 0 {
    println("negative")
} else {
    println("zero")
}
```

### While & For Loops
```whalli
while running {
    // ...
    break
}

for i in range(0, 10, 2) {
    print(i, " ") // 0 2 4 6 8
}

for key in user.keys() {
    println(key, user[key])
}
```

### Pattern Matching (`match`)
```whalli
let label = match n {
    0 => "zero",
    1 | 2 => "small",
    x: int if x > 10 => "big",
    _ => "other",
}
```

## Structs, Methods & Interfaces

```whalli
struct Vector2 {
    x: int,
    y: int
}

impl Vector2 {
    func norm_sq() -> int {
        return this.x * this.x + this.y * this.y
    }
}

interface Printable {
    func str()
}

let v = Vector2(3, 4)
println(v.norm_sq()) // 25
```

## Collections

```whalli
// Lists
let arr = new(list)
arr.push(10)
arr.push(20)
println(arr.len(), arr[0]) // 2 10

// Maps
let user = new(map)
user["username"] = "kepr"
println(user.keys())

// Bytes
let raw = b"PING"
let sub = raw.slice(0, 2) // b"PI"

// Channels
let ch = new(chan, 10) // buffered capacity 10
```

## Deferred Cleanup (`defer`)

The `defer` keyword registers a function or method call to be executed when the surrounding function returns (in LIFO order), ensuring reliable cleanup even during early returns or errors:

```whalli
import sync

let mu = sync.Mutex()

func critical_task() {
    mu.lock()
    defer mu.unlock() // guaranteed unlock upon function exit

    // do work...
}
```

## Error Handling

Whalli adopts the Go-style `(value, err)` error convention where `err` is `nil` on success:

```whalli
let (data, err) = fs.read("file.txt")
if err != nil {
    println("Failed:", err)
    return
}

// '?' postfix operator unwraps the success value or automatically propagates (nil, err):
func load_config() {
    let content = fs.read("config.json")?
    let config = json.decode(content)?
    return (config, nil)
}
```

## Modules: Export and Import (`pub`, `from ... import`)

Files and libraries are organized with public exports and selective imports:

```whalli
// In math_lib.wh:
pub let pi = 3.14159
let private_seed = 42 // not exported

pub func add(a: int, b: int) -> int {
    return a + b
}

// In main script:
// 1. Whole module import
import "./math_lib.wh" as math_lib
println(math_lib.add(2, 3))

// 2. Selective symbol import via 'from'
from "./math_lib.wh" import pi, add as sum_fn
println(pi, sum_fn(5, 10))

// 3. Selective standard library import
from math import sin, pi
```
