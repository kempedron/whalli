---
title: Стандартная библиотека
description: Справочник по встроенным модулям стандартной библиотеки языка Whalli.
---

## Обзор

Стандартные модули Whalli встроены непосредственно в рантайм языка. Они импортируются стандартной конструкцией `import <модуль>` без указания путей к локальным файлам.

---

## Модуль `requests` (HTTP-клиент)

Удобный HTTP/HTTPS клиент:

- `requests.get(url: str, options: map = nil) -> map`
- `requests.post(url: str, options: map = nil) -> map`
- `requests.put(url: str, options: map = nil) -> map`
- `requests.delete(url: str, options: map = nil) -> map`

Объект ответа содержит: `status_code`, `status_text`, `ok` (bool), `text`, `url`, `error`, метод `.json()` и метод `.header(name)`.

```whalli
import requests

let res = requests.get("https://httpbin.org/get", {"timeout": 5})
if res.ok {
    println("HTTP Status:", res.status_code)
    let body = res.json()
}
```

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
let i = 0
while i < rows.len() {
    let u = rows[i]
    println(u["id"], u["name"])
    i += 1
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

---

## Модуль `crypto`

Криптографические алгоритмы, хеширование, генерация случайных токенов и Base64/Hex кодирование:

- `crypto.sha256(data: str | bytes) -> str`: Вычисление хеша SHA-256 (в формате hex).
- `crypto.sha1(data: str | bytes) -> str`: Вычисление хеша SHA-1.
- `crypto.md5(data: str | bytes) -> str`: Вычисление хеша MD5.
- `crypto.hmac_sha256(key: str | bytes, message: str | bytes) -> str`: Вычисление подписи HMAC-SHA256.
- `crypto.base64_encode(data: str | bytes) -> str`: Кодирование данных в строку Base64.
- `crypto.base64_decode(encoded_str: str) -> (data: bytes | nil, err: str | nil)`: Декодирование Base64 в байты.
- `crypto.hex_encode(data: bytes | str) -> str`: Преобразование байтов в шестнадцатеричную строку.
- `crypto.hex_decode(hex_str: str) -> (data: bytes | nil, err: str | nil)`: Парсинг hex-строки в байты.
- `crypto.random_bytes(length: int = 16) -> bytes`: Генерация криптографически стойких случайных байтов.
- `crypto.random_hex(length: int = 16) -> str`: Генерация случайного hex-токена (длина строки = 2 * length).
- `crypto.uuid4() -> str`: Генерация случайного UUID версии 4.

```whalli
import crypto

let password_hash = crypto.sha256("my_secret_pass")
let session_token = crypto.random_hex(32)
let request_uuid = crypto.uuid4()

let signature = crypto.hmac_sha256("api_secret", "action=pay&amount=100")
```
