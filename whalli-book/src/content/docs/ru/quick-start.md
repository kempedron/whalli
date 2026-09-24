---
title: Быстрый старт
description: Как установить, собрать и запустить программы на Whalli.
---

## Предварительные требования

- **Rust и Cargo** (`edition 2024` или новее). Установите через официальный сайт [rustup.rs](https://rustup.rs).
- Linux или macOS (протестировано на Linux с циклом событий MIO).

## Сборка из исходников

```bash
git clone https://github.com/kempedron/whalli.git
cd whalli

# Сборка интерпретатора и языкового сервера в release-режиме
cargo build --release
```

Полученные бинарники:
- `target/release/whalli` — Виртуальная машина и CLI-интерпретатор.
- `target/release/whalli-lsp` — Сервер протокола Language Server.

## Запуск скрипта

Запустите демонстрационный REST API с базой SQLite:

```bash
./target/release/whalli main.wh
# либо напрямую через Cargo:
cargo run --bin whalli -- main.wh
```

## Hello World

Создайте файл `hello.wh`:

```whalli
println("Hello, Whalli!")
let answer = 42
println(f"Ответ на главный вопрос: {answer}")
```

Запуск:

```bash
cargo run --bin whalli -- hello.wh
```

## Переменные окружения

- `WHALLI_NUM_THREADS`: Задаёт количество рабочих потоков планировщика задач (`vm.rs`). По умолчанию равно `available_parallelism()` либо `4`.

## Следующий шаг

Переходите к [Обзору языка](/whalli/ru/language-tour/) для знакомства с синтаксисом и типами.
