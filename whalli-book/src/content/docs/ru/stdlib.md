---
title: Стандартная библиотека
description: Справочник по встроенным модулям стандартной библиотеки языка Whalli.
---

## Обзор

Стандартные модули Whalli встроены непосредственно в рантайм языка. Они импортируются стандартной конструкцией `import <модуль>` без указания путей к локальным файлам.

---

## Модуль `sql`

Универсальный SQL-клиент с поддержкой **SQLite**, **PostgreSQL** и **MySQL**.

### Поддерживаемые драйверы

- `"sqlite"` / `"sqlite3"`: Встроенная база SQLite без внешних зависимостей (`":memory:"` либо путь к файлу `"app.db"`).
- `"postgres"` / `"postgresql"` / `"pg"`: Подключение к PostgreSQL с автоматической трансляцией плейсхолдеров `?` в `$1, $2`.
- `"mysql"` / `"mariadb"`: Подключение к MySQL / MariaDB.

### Интерфейс

- `sql.open(driver: str, conn_str: str) -> (db: map | nil, err: str | nil)`
- `db.exec(query: str, params: list = []) -> (result: map | nil, err: str | nil)`: DDL и изменения данных. Возвращает `{"rows_affected": int, "last_insert_id": int}`.
- `db.query(query: str, params: list = []) -> (rows: list[map] | nil, err: str | nil)`: выборка `SELECT`. Возвращает список строк-словарей.
- `db.query_row(query: str, params: list = []) -> (row: map | nil, err: str | nil)`: возвращает первую найденную запись либо `nil`.
- `db.close() -> bool`: закрытие соединения.

```whalli
import sql

let (db, err) = sql.open("sqlite", "app.db")
db.exec("CREATE TABLE IF NOT EXISTS users (id INTEGER PRIMARY KEY, name TEXT)")
db.exec("INSERT INTO users (name) VALUES (?)", ["Alice"])

let (rows, _) = db.query("SELECT * FROM users")
for u in rows {
    println(u["id"], u["name"])
}
db.close()
```

---

## Модуль `http`

Серверный HTTP-фреймворк, роутер, генераторы ответов и готовые middleware.

- `http.router()`: Создание маршрутизатора.
- `http.listen_and_serve(addr, router)`: Запуск неблокирующего сервера с Keep-Alive.
- `http.response(code, headers, body)`: Общий конструктор HTTP-ответа.
- `http.json_response(code, data, headers)`: Ответ в формате JSON.
- `http.text_response(code, text, headers)`: Ответ обычным текстом.
- `http.html_response(code, html, headers)`: Ответ HTML.
- `http.file_response(base_dir, rel_path, headers)`: Раздача файла с определением MIME-типа.
- `http.redirect(url, code = 302)`: HTTP-редирект.
- `http.error(code, msg)`: Текстовый ответ с кодом ошибки.
- `http.json_error(code, msg)`: JSON-ответ с кодом ошибки.
- `http.set_cookie(name, value, opts)`: Генератор заголовка `Set-Cookie`.
- `http.cors(opts)`: Middleware CORS.
- `http.logger()`: Middleware логирования запросов.
- `http.secure_headers()`: Middleware заголовков безопасности браузера.
- `http.request_id()`: Middleware сквозной трассировки `X-Request-ID`.
- `http.rate_limiter(max_req, window_sec)`: Middleware ограничения частоты запросов.

---

## Модуль `json`

- `json.encode(value: any) -> str`: Сериализация любых структур Whalli в строку JSON.
- `json.decode(json_str: str) -> (value: any, err: str | nil)`: Парсинг JSON-строки в структуры языка.

---

## Модуль `fs`

Файловые операции:

- `fs.read(path: str) -> (content: str, err: str | nil)`: Чтение файла целиком.
- `fs.write(path: str, data: str) -> (ok: bool, err: str | nil)`: Запись строки в файл.

---

## Модуль `os`

Системные утилиты и переменные окружения:

- `os.args() -> list`: Аргументы командной строки процесса.
- `os.env(key: str) -> str | nil`: Чтение переменной окружения.
- `os.set_env(key: str, val: str)`: Установка переменной окружения.
- `os.exit(code: int)`: Завершение процесса с кодом возврата.

---

## Модуль `time`

- `time.now() -> float`: Unix-время в секундах (с дробной частью).
- `time.sleep(seconds: float | int)`: Кооперативное усыпление текущей ворутины.
- `time.after(seconds: float | int) -> chan`: Канал, передающий сигнал по истечении заданного времени.

---

## Модуль `math`

Математические функции: `math.sqrt`, `math.pow`, `math.sin`, `math.cos`, `math.tan`, `math.floor`, `math.ceil`, `math.round`, `math.abs`, и константы `math.pi`, `math.e`.
