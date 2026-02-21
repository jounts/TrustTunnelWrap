# API Reference

HTTP API wrapper-а TrustTunnel Keenetic. Сервер по умолчанию слушает на `http://0.0.0.0:8080`.

## Авторизация

Если в конфигурации включён `webui.auth: true`, все запросы (кроме `POST /api/login` и `GET /`) должны содержать заголовок:

```
Authorization: <session-token>
```

Токен сессии получается через `POST /api/login`. Сессия действует 1 час.

---

## POST /api/login

Авторизация через NDM API роутера (Keenetic). Использует challenge-response с MD5 + SHA256.

### Тело запроса

```json
{
  "login": "admin",
  "password": "secret"
}
```

### Ответы

| Код | Описание |
|-----|----------|
| 200 | Успешная авторизация |
| 401 | Неверные учётные данные |
| 400 | Некорректный запрос |

### Успешный ответ (200)

```json
{
  "token": "xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx"
}
```

### Пример

```sh
curl -X POST http://192.168.1.1:8080/api/login \
  -H "Content-Type: application/json" \
  -d '{"login":"admin","password":"mypassword"}'
```

---

## GET /api/status

Текущий статус туннеля.

### Ответ (200)

```json
{
  "status": "connected",
  "pid": 12345,
  "uptime_secs": 3600,
  "hostname": "vpn.example.com"
}
```

Возможные значения `status`:
- `stopped` — туннель не запущен
- `connecting` — идёт подключение
- `connected` — подключён
- `reconnecting` — переподключение после обрыва
- `error` — ошибка запуска

### Пример

```sh
curl -H "Authorization: <token>" http://192.168.1.1:8080/api/status
```

---

## GET /api/config

Возвращает текущие настройки туннеля из JSON-конфигурации.

### Ответ (200)

```json
{
  "hostname": "vpn.example.com",
  "addresses": ["1.2.3.4:443"],
  "username": "myuser",
  "password": "mypassword",
  "upstream_protocol": "http2",
  "vpn_mode": "general",
  "dns_upstreams": ["tls://1.1.1.1"],
  "killswitch_enabled": false,
  "included_routes": ["0.0.0.0/0", "2000::/3"],
  "excluded_routes": ["10.0.0.0/8", "172.16.0.0/12", "192.168.0.0/16"],
  "mtu_size": 1280,
  "socks_address": "",
  "skip_verification": false,
  "reconnect_delay": 5,
  "loglevel": "info"
}
```

### Пример

```sh
curl -H "Authorization: <token>" http://192.168.1.1:8080/api/config
```

---

## POST /api/config

Обновление настроек туннеля. Принимает частичное или полное обновление. Сохраняет конфигурацию на диск.

### Тело запроса

```json
{
  "hostname": "new-vpn.example.com",
  "addresses": ["5.6.7.8:443"],
  "username": "newuser",
  "password": "newpassword",
  "upstream_protocol": "http3",
  "vpn_mode": "selective",
  "dns_upstreams": ["tls://8.8.8.8"],
  "excluded_routes": ["10.0.0.0/8"],
  "skip_verification": false,
  "reconnect_delay": 10,
  "loglevel": "debug"
}
```

### Ответы

| Код | Описание |
|-----|----------|
| 200 | Конфигурация сохранена |
| 400 | Некорректный JSON |
| 500 | Ошибка записи файла |

### Успешный ответ (200)

```json
{
  "ok": true
}
```

### Пример

```sh
curl -X POST http://192.168.1.1:8080/api/config \
  -H "Authorization: <token>" \
  -H "Content-Type: application/json" \
  -d '{"hostname":"vpn.example.com","addresses":["1.2.3.4:443"]}'
```

---

## POST /api/control

Управление туннелем: подключение, отключение, перезапуск.

### Тело запроса

```json
{
  "action": "connect"
}
```

Допустимые значения `action`:

| Action | Описание |
|--------|----------|
| `connect` | Запуск туннеля |
| `disconnect` | Остановка туннеля |
| `restart` | Перезапуск (disconnect + connect) |

### Ответы

| Код | Описание |
|-----|----------|
| 200 | Действие выполнено |
| 400 | Неизвестное действие |
| 500 | Ошибка выполнения |

### Успешный ответ (200)

```json
{
  "ok": true
}
```

### Ошибка (500)

```json
{
  "error": "Failed to spawn trusttunnel_client: No such file or directory"
}
```

### Пример

```sh
# Подключить
curl -X POST http://192.168.1.1:8080/api/control \
  -H "Authorization: <token>" \
  -H "Content-Type: application/json" \
  -d '{"action":"connect"}'

# Отключить
curl -X POST http://192.168.1.1:8080/api/control \
  -H "Authorization: <token>" \
  -H "Content-Type: application/json" \
  -d '{"action":"disconnect"}'
```

---

## GET /api/logs

Последние строки логов из кольцевого буфера.

### Параметры запроса

| Параметр | Тип | По умолчанию | Описание |
|----------|-----|--------------|----------|
| `limit` | number | `100` | Количество строк |

### Ответ (200)

```json
{
  "lines": [
    "[2025-01-15 10:30:00] INFO trusttunnel_client started (pid 12345)",
    "[2025-01-15 10:30:01] INFO Connected to vpn.example.com:443",
    "[2025-01-15 10:30:02] INFO TUN interface configured"
  ],
  "count": 3
}
```

### Пример

```sh
# Последние 50 строк
curl -H "Authorization: <token>" "http://192.168.1.1:8080/api/logs?limit=50"
```

---

## GET /

Возвращает встроенный HTML веб-интерфейс. Не требует авторизации (авторизация происходит внутри интерфейса через `/api/login`).

---

## Коды ошибок

| Код | Описание |
|-----|----------|
| 200 | Успех |
| 400 | Некорректный запрос |
| 401 | Не авторизован или невалидный токен |
| 404 | Маршрут не найден |
| 405 | Метод не поддерживается |
| 500 | Внутренняя ошибка сервера |

Все ответы с ошибкой возвращают JSON:

```json
{
  "error": "описание ошибки"
}
```
