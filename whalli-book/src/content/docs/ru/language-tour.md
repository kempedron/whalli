---
title: Обзор языка
description: Обзор синтаксиса Whalli, типов данных, управляющих конструкций, структур и обработки ошибок.
---

## Синтаксис

- **Комментарии**: `// однострочный комментарий`
- **Разделители инструкций**: Перевод строки либо точка с запятой `;`
- **Ключевые слова**: `let`, `func`, `struct`, `impl`, `interface`, `if`/`else`, `while`, `for`/`in`, `return`, `break`, `continue`, `import`, `wo`, `defer`, `is`, `match`, `select`, `default`, `nil`, `true`, `false`.

## Переменные и типы данных

Whalli — динамически типизированный язык со строгими встроенными тегами типов (`int`, `float`, `str`, `bool`, `list`, `map`, `func`, `chan`).

```whalli
let x = 42
let pi = 3.14159
let name = "Whalli"
let ok = true
let nothing = nil

// Форматированные строки (f-strings)
let greeting = f"Язык: {name}, ответ: {x}"

// Неизменяемые кортежи и деструктуризация
let (status, code) = (true, 200)
```

Функции явного приведения типов: `int(v)`, `float(v)`, `str(v)`, `bool(v)`.

## Операторы

- **Арифметика**: `+ - * / %` (поддерживает числа и конкатенацию строк через `+`).
- **Сравнение**: `== != < > <= >=`
- **Логика**: `and`, `or`, `!` (`not`)
- **Проверка типа**: `is` (например: `handler is map`, `x is int`)
- **Присваивание**: `=`, `+= -= *= /= %=`
- **Каналы**: `ch <- val` (отправка), `<- ch` (получение)

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

// Каналы (Channel)
let ch = new(chan, 10) // буфер на 10 элементов
```

## Обработка ошибок

Whalli использует Go-стиль возврата кортежа `(значение, ошибка)`:

```whalli
let (data, err) = fs.read("file.txt")
if err != nil {
    println("Ошибка:", err)
    return
}

// Постфиксный оператор '?' автоматически пробрасывает ошибку наверх:
let content = fs.read("config.json")?
```
