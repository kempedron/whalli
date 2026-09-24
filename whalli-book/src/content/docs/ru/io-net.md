---
title: Ввод-вывод и HTTP бэкенд
description: Неблокирующие TCP сокеты, современный HTTP бэкенд фреймворк, Keep-Alive и клиент запросов.
---

## Обзор

Сетевой стек Whalli полностью неблокирующий и интегрирован с планировщиком задач через библиотеку [`mio`](https://github.com/tokio-rs/mio). Когда операция ввода-вывода возвращает статус `WouldBlock`, виртуальная машина усыпляет текущую ворутину и автоматически возвращает её в очередь исполнения при наступлении события готовности сокета от ОС.

---

## Модуль `http`

Модуль `http` — это высокопроизводительный серверный бэкенд-фреймворк в стиле пакета `net/http` языка Go.

### Запуск сервера

```whalli
import http

let router = http.router()

router.get("/hello", func(req) {
    return http.json_response(200, {"message": "Привет от Whalli!"})
})

// Запускает неблокирующий сервер на порту 8080 с включенным Keep-Alive
http.listen_and_serve(":8080", router)
```

### HTTP/1.1 Keep-Alive

Все ответы сервера по умолчанию отправляют заголовки постоянного соединения (`Connection: keep-alive` и `Keep-Alive: timeout=5, max=100`). Если клиент запрашивает `Connection: close`, сервер корректно закрывает соединение после отправки ответа.

### Объект запроса (`req`)

Объект `req` предоставляет удобные методы для обработчиков:

| Метод / Свойство | Описание |
|---|---|
| `req["method"]` | HTTP метод (`"GET"`, `"POST"` и т.д.) |
| `req["path"]` | Путь URL (`"/api/tasks"`) |
| `req["remote_addr"]` | IP-адрес и порт клиента (`"127.0.0.1:54321"`) |
| `req.header(key, default = nil)` | Регистронезависимый поиск заголовка |
| `req.cookie(name, default = nil)` | Извлечение куки из заголовка `Cookie` |
| `req.query(key, default = nil)` | Параметр query-строки URL |
| `req.param(key, default = nil)` | Параметр маршрута (`:id` или `*filepath`) |
| `req.json()` | Автоматический парсинг тела через `serde_json` |
| `req.form()` | Словарь разобранных полей `application/x-www-form-urlencoded` |
| `req.form_value(key, default = nil)` | Получение отдельного поля формы |

### Маршрутизация и группы

```whalli
let router = http.router()

// Параметризованные маршруты
router.get("/users/:id", func(req) {
    let id = req.param("id")
    return http.json_response(200, {"user_id": id})
})

// Wildcard-пути (жадный захват остатка)
router.get("/static/*filepath", func(req) {
    let path = req.param("filepath")
    return http.text_response(200, f"Файл: {path}")
})

// Группировка эндпоинтов
let api = router.group("/api")
let v1 = api.group("/v1")
v1.get("/status", func(req) { return http.json_response(200, {"ok": true}) })
```

### Встроенные Middleware

Middleware регистрируются как глобально (`router.use(...)`), так и на уровне групп (`group.use(...)`):

```whalli
import http

let router = http.router()

// 1. CORS middleware (обработка preflight OPTIONS и Access-Control-* заголовков)
router.use(http.cors({
    "origin": "https://myapp.com",
    "methods": "GET, POST, PUT, DELETE",
    "credentials": true
}))

// 2. Логгер входящих запросов
router.use(http.logger())

// 3. Заголовки безопасности браузера (X-Frame-Options, nosniff, CSP)
router.use(http.secure_headers())

// 4. Трассировка с заголовком X-Request-ID
router.use(http.request_id())

// 5. Rate Limiter (ограничение частоты по IP клиента)
router.use(http.rate_limiter(100, 60)) // 100 запросов за 60 секунд
```

### Раздача статических файлов

```whalli
// Безопасная раздача файлов из каталога с определением MIME-типов и защитой от path traversal (..)
router.static("/public", "./static_files")
```

---

## Модуль `net` (Сырой TCP)

Низкоуровневые неблокирующие сокеты:

```whalli
import net

let (server_id, err) = net.listen(9000)
while true {
    let client_id = net.accept(server_id)
    if client_id != nil {
        let (data, _) = net.read(client_id)
        net.write_all(client_id, "Эхо: " + data)
        net.close(client_id)
    }
}
```

---

## Модуль `requests` (Клиент)

Удобный синхронный HTTP/HTTPS клиент:

```whalli
import requests

let r = requests.get("https://httpbin.org/get", {"timeout": 5})
if r.ok {
    println("Статус:", r.status_code)
    let json_data = r.json()
}

let r2 = requests.post("https://httpbin.org/post", {
    "json": {"name": "Alice", "role": "admin"}
})
```
