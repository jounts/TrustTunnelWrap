# API Reference

HTTP API wrapper-а TrustTunnel Keenetic. По умолчанию сервер слушает на `http://0.0.0.0:8080`.

## Авторизация

- `POST /api/login` и `GET /` доступны без токена.
- Для остальных API требуется заголовок:

```
Authorization: <session-token>
```

Токен выдаётся через `POST /api/login`, TTL сессии — 1 час (продлевается при активности).

---

## POST /api/login

Авторизация через NDM API роутера (challenge-response).

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
| 400 | Некорректный JSON или отсутствуют `login/password` |
| 401 | Неверные учётные данные |

### Успешный ответ (200)

```json
{
  "token": "xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx",
  "status": "ok"
}
```

---

## GET /api/status

Текущий статус процесса `trusttunnel_client`.

### Ответ (200)

```json
{
  "connected": true,
  "uptime_seconds": 3600,
  "last_error": "",
  "pid": 12345
}
```

---

## GET /api/config

Возвращает текущий блок `tunnel` из wrapper-конфига.

### Ответ (200)

```json
{
  "hostname": "vpn.example.com",
  "addresses": ["1.2.3.4:443"],
  "username": "myuser",
  "password": "mypassword",
  "upstream_protocol": "http2",
  "certificate": "",
  "skip_verification": false,
  "vpn_mode": "general",
  "dns_upstreams": ["tls://1.1.1.1"],
  "killswitch_enabled": false,
  "included_routes": ["0.0.0.0/0", "2000::/3"],
  "excluded_routes": ["10.0.0.0/8", "172.16.0.0/12", "192.168.0.0/16"],
  "mtu_size": 1280,
  "anti_dpi": false,
  "socks_address": "",
  "reconnect_delay": 5,
  "loglevel": "info"
}
```

---

## POST /api/config

Полностью заменяет `tunnel`-конфиг и сохраняет файл на диск.

Важно: это не merge-обновление. Поля, не переданные в JSON, получат значения по умолчанию.

### Тело запроса

Передавайте полный объект `TunnelSettings` (как в ответе `GET /api/config`).

### Ответы

| Код | Описание |
|-----|----------|
| 200 | Конфигурация обновлена |
| 400 | Некорректная структура JSON |

### Успешный ответ (200)

```json
{
  "status": "updated"
}
```

---

## POST /api/control

Управление туннелем.

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
| `restart` | Перезапуск |

### Ответы

| Код | Описание |
|-----|----------|
| 200 | Действие принято |
| 400 | Некорректный JSON, неизвестное действие или ошибка запуска |

### Успешные ответы (200)

```json
{ "status": "connecting" }
```

```json
{ "status": "disconnected" }
```

```json
{ "status": "restarting" }
```

### Пример ошибки (400)

```json
{
  "error": "Endpoint hostname and addresses are required"
}
```

---

## GET /api/logs

Возвращает последние строки логов.

### Параметры запроса

| Параметр | Тип | По умолчанию | Ограничение |
|----------|-----|--------------|-------------|
| `limit` | number | `100` | максимум `500` |

### Ответ (200)

```json
{
  "lines": [
    "[tunnel] started PID 12345",
    "[routing] setup complete (WAN=eth0)"
  ],
  "total": 237
}
```

`total` — текущее количество строк во внутреннем буфере wrapper-а.

---

## GET /

Возвращает встроенный HTML WebUI.

---

## Формат ошибок

Типовой JSON ошибки:

```json
{
  "error": "описание ошибки"
}
```
