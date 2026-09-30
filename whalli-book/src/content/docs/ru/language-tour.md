---
title: Обзор языка
description: Обзор синтаксиса Whalli, типов данных, управляющих конструкций, структур и обработки ошибок.
---

## Синтаксис

- **Комментарии**: `// однострочный комментарий`
- **Разделители инструкций**: Перевод строки либо точка с запятой `;`
- **Ключевые слова**: `let`, `func`, `struct`, `impl`, `interface`, `if`/`else`, `while`, `for`/`in`, `return`, `break`, `continue`, `import`, `wo`, `defer`, `is`, `match`, `select`, `default`, `nil`, `true`, `false`.

## Переменные и типы данных

Whalli — динамически типизированный язык со строгими встроенными тегами типов (`int`, `float`, `str`, `bytes`, `bool`, `list`, `map`, `func`, `chan`, `tuple`).

```whalli
let x = 42
let pi = 3.14159
let name = "Whalli"
let ok = true
let nothing = nil

// Форматированные строки (f-strings)
let greeting = f"Язык: {name}, ответ: {x}"

// Многострочные строки ("""...""") и сырые строки (r"...")
let query = """
SELECT id, name
FROM users
WHERE active = true
"""
let path = r"C:\Windows\System32\drivers"

// Литералы байтов (тип bytes) и сырые байты (br"...")
let b = b"hello\x20world"
let b_hex = b.hex()            // "68656c6c6f20776f726c64"
let (decoded, _) = b.decode()  // "hello world"
let raw_b = br"raw\x00bytes"

// Деструктуризация кортежей, списков и объектов:
let (status, code) = (true, 200)
let [first, second] = [10, 20]
let { name, age: user_age } = {"name": "Alice", "age": 25}
```

Функции явного приведения типов: `int(v)`, `float(v)`, `str(v)`, `bytes(v)`, `bool(v)`.

## Операторы

- **Арифметика**: `+ - * / %` (числа, конкатенация строк `str + str` и байтов `bytes + bytes`).
- **Сравнение**: `== != < > <= >=`
- **Логика**: `and`, `or`, `!` (`not`)
- **Проверка типа**: `is` (например: `b is bytes`, `handler is map`, `x is int`)
- **Присваивание**: `=`, `+= -= *= /= %=`
- **Каналы**: `ch <- val` (отправка), `<- ch` (получение)
- **Обработка ошибок**: `expr?` (автоматический unwrap значения или ранний возврат ошибки)

## Функции

```whalli
func calculate(a: int, b: int) -> int {
    return a + b
}

let result = calculate(10, 20)
```

- Поддерживаются анонимные функции (замыкания) и функции высшего порядка.
- Замыкания захватывают лексическое окружение через upvalues в VM.

## Управляющие конструкции

### Условия If / Else
```whalli
if x > 0 {
    println("положительное")
} else if x < 0 {
    println("отрицательное")
} else {
    println("ноль")
}
```

### Циклы While и For
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

### Сопоставление с образцом (`match`)
```whalli
let label = match n {
    0 => "ноль",
    1 | 2 => "мало",
    x: int if x > 10 => "много",
    _ => "другое",
}
```

## Структуры, методы и интерфейсы

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

## Коллекции

```whalli
// Списки (List)
let arr = new(list)
arr.push(10)
arr.push(20)
println(arr.len(), arr[0]) // 2 10

// Словари (Map)
let user = new(map)
user["username"] = "kepr"
println(user.keys())

// Байты (Bytes)
let raw = b"PING"
let sub = raw.slice(0, 2) // b"PI"

// Каналы (Channel)
let ch = new(chan, 10) // буфер на 10 элементов
```

## Отложенные вызовы (`defer`)

Ключевое слово `defer` регистрирует вызов функции, который гарантированно выполняется при выходе из текущей функции по принципу LIFO (последним пришёл — первым ушёл), даже при панике или возврате по оператору `?`:

```whalli
import sync

let mu = sync.Mutex()

func critical_task() {
    mu.lock()
    defer mu.unlock() // замок гарантированно освободится при выходе

    // полезная работа...
}
```

## Обработка ошибок

Whalli использует Go-стиль возврата кортежа `(значение, ошибка)`:

```whalli
let (data, err) = fs.read("file.txt")
if err != nil {
    println("Ошибка:", err)
    return
}

// Постфиксный оператор '?' автоматически извлекает результат
// или пробрасывает кортеж ошибки (nil, err) наверх из текущей функции:
func load_config() {
    let content = fs.read("config.json")?
    let config = json.decode(content)?
    return (config, nil)
}
```

## Модули: экспорт и импорт (`pub`, `from ... import`)

Файлы и библиотеки структурируются с помощью публичного экспорта и гибкого импорта:

```whalli
// В файле math_lib.wh:
pub let pi = 3.14159
let private_seed = 42 // не экспортируется

pub func add(a: int, b: int) -> int {
    return a + b
}

// В основном файле:
// 1. Импорт всего модуля целиком
import "./math_lib.wh" as math_lib
println(math_lib.add(2, 3))

// 2. Выборочный импорт через 'from'
from "./math_lib.wh" import pi, add as sum_fn
println(pi, sum_fn(5, 10))

// 3. Выборочный импорт из стандартной библиотеки
from math import sin, pi
```
