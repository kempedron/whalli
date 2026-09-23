---
title: Language Tour
description: Tour of Whalli syntax, types, control flow, structs, and error handling.
---

## Lexical & Syntax

- **Line comments**: `// this is a comment`
- **Statement terminators**: Newline or semicolon `;`
- **Keywords**: `let`, `func`, `struct`, `impl`, `interface`, `if`/`else`, `while`, `for`/`in`, `return`, `break`, `continue`, `import`, `wo`, `defer`, `is`, `match`, `select`, `default`, `nil`, `true`, `false`.

## Variables and Types

Whalli is dynamically typed with strong built-in type tags (`int`, `float`, `str`, `bool`, `list`, `map`, `func`, `chan`).

```whalli
let x = 42
let pi = 3.14159
let name = "Whalli"
let ok = true
let nothing = nil

// Formatted strings (f-strings)
let greeting = f"Language: {name}, answer: {x}"

// Immutable Tuples & Destructuring
let (status, code) = (true, 200)
```

Type conversion globals: `int(v)`, `float(v)`, `str(v)`, `bool(v)`.

## Operators

- **Arithmetic**: `+ - * / %` (int/int, float/float, mixed, and `+` concatenates strings).
- **Comparison**: `== != < > <= >=`
- **Logic**: `and`, `or`, `!` (`not`)
- **Type test**: `is` (e.g. `handler is map`, `x is int`)
- **Assignment**: `=`, `+= -= *= /= %=`
- **Channel**: `ch <- val` (send), `<- ch` (receive)

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

// Channels
let ch = new(chan, 10) // buffered capacity 10
```

## Error Handling

Whalli adopts the Go-style `(value, err)` error convention where `err` is `nil` on success:

```whalli
let (data, err) = fs.read("file.txt")
if err != nil {
    println("Failed:", err)
    return
}

// '?' postfix operator propagates (nil, err) tuples automatically:
let content = fs.read("config.json")?
```
