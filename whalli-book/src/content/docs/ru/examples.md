---
title: Примеры программ
description: Готовые программы на языке Whalli — REST API, конкурентность, сетевые сокеты и базы данных.
---

## REST API задач с базой SQLite (`main.wh`)

Полноценный рабочий сервис задач. Исходный код находится в файле `main.wh`:

```bash
cargo run --bin whalli -- main.wh
# Whalli Tasks REST API listening on http://127.0.0.1:8888
```

**Фрагмент настройки роутера и базы данных:**

```whalli
import http
import sql
import sync

let (db, _) = sql.open("sqlite", ":memory:")
db.exec("CREATE TABLE IF NOT EXISTS tasks (id INTEGER PRIMARY KEY AUTOINCREMENT, title TEXT NOT NULL, done BOOLEAN NOT NULL)")

let router = http.router()
router.use(http.logger())
router.not_found(func(req) { return http.json_error(404, f"Ресурс {req[\"path\"]} не найден") })

let api = router.group("/api")
api.get("/health", health)
api.get("/tasks", list_tasks)
api.get("/tasks/:id", get_task)
api.post("/tasks", create_task)
api.put("/tasks/:id", update_task)
api.delete("/tasks/:id", delete_task)

router.static("/static", ".")
http.listen_and_serve(":8888", router)
```

**Примеры запросов через curl:**

```bash
curl http://127.0.0.1:8888/api/health
curl http://127.0.0.1:8888/api/tasks
curl -X POST http://127.0.0.1:8888/api/tasks -H "Content-Type: application/json" -d '{"title":"Купить хлеб"}'
curl http://127.0.0.1:8888/api/tasks/1
curl -X PUT http://127.0.0.1:8888/api/tasks/1 -H "Content-Type: application/json" -d '{"done":true}'
curl -X DELETE http://127.0.0.1:8888/api/tasks/1
curl http://127.0.0.1:8888/static/Cargo.toml
```

---

## Работа с базами данных — SQLite, PostgreSQL, MySQL

```whalli
import sql

// SQLite (встроенная, без внешних зависимостей)
let (db, err) = sql.open("sqlite", "app.db")

// PostgreSQL — параметры '?' автоматически заменяются на '$1, $2'
let (pg, err) = sql.open("postgres", "postgresql://user:pass@localhost:5432/mydb")

// MySQL
let (my, err) = sql.open("mysql", "mysql://user:pass@localhost:3306/mydb")

db.exec("CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT, age INTEGER)")
db.exec("INSERT INTO users (name, age) VALUES (?, ?)", ["Alice", 30])

let (rows, _) = db.query("SELECT * FROM users WHERE age >= ?", [18])
let (user, _) = db.query_row("SELECT * FROM users WHERE id = ?", [1])
db.close()
```

---

## Использование Middleware

```whalli
import http

let router = http.router()
router.use(http.cors({"origin": "https://myapp.com", "credentials": true}))
router.use(http.logger())
router.use(http.secure_headers())
router.use(http.request_id())
router.use(http.rate_limiter(100, 60))

router.get("/hello", func(req) {
    return http.json_response(200, {"message": "Привет!"})
})
```

---

## Ворутины и каналы

```whalli
import time

func producer(ch) {
    for i in range(1, 4) {
        time.sleep(0.5)
        ch <- f"задача #{i}"
    }
}
let ch = new(chan, 3)
wo producer(ch)
println(<- ch)
println(<- ch)
println(<- ch)
```

---

## Сырой TCP-сервер

```whalli
import net
let (server_id, err) = net.listen(8080)
println("Слушаем http://127.0.0.1:8080 ...")
while true {
    let client_id = net.accept(server_id)
    if client_id != nil {
        let (request, _) = net.read(client_id)
        println("Запрос:", request)
        net.write(client_id, "HTTP/1.1 200 OK\r\nContent-Length: 12\r\n\r\nПривет!")
        net.close(client_id)
    }
}
```

---

## Структуры и интерфейсы

```whalli
struct Vector2 { x: int, y: int }
impl Vector2 {
    func norm_sq() -> int { return this.x * this.x + this.y * this.y }
}
interface Printable { func str() }
let v = Vector2(3, 4)
println(v.norm_sq())
```
