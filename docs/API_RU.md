# API Reference (RU)

HTTP API для `trusttunnel-keenetic`. По умолчанию сервер слушает `http://0.0.0.0:8080`.

> WebUI работает только по HTTP без TLS. Session token передаётся в открытом виде,
> поэтому доступ к порту 8080 следует ограничить firewall или использовать только
> в доверенной LAN-сети.

English version: [`API.md`](API.md)

## Авторизация

- `POST /api/login` и `GET /` доступны без токена.
- Для остальных `/api/*` нужен заголовок:

```text
Authorization: <session-token>
```

Токен выдаётся через `POST /api/login`. TTL сессии: 1 час, продлевается при активности.

### Быстрый шаблон для curl

```sh
BASE_URL="http://192.168.1.1:8080"
TOKEN="<session-token>"
```

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

| Код | Значение |
|-----|----------|
| 200 | Успешная авторизация |
| 400 | Некорректный JSON или нет `login/password` |
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

### Успешный ответ (200)

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

Возвращает текущий объект `tunnel` из конфигурации wrapper.

---

## POST /api/config

Полностью заменяет блок `tunnel` и сохраняет конфиг на диск.

Важно: это не merge-обновление. Передавайте полный объект (как в `GET /api/config`).

### Ответы

| Код | Значение |
|-----|----------|
| 200 | Конфигурация обновлена |
| 400 | Некорректный JSON конфигурации |
| 500 | Ошибка сохранения |

### Успешный ответ (200)

```json
{
  "status": "updated"
}
```

---

## POST /api/control

Управление состоянием туннеля.

### Тело запроса

```json
{
  "action": "connect"
}
```

### Допустимые значения `action`

| Значение | Описание |
|----------|----------|
| `connect` | Запуск туннеля |
| `disconnect` | Остановка туннеля |
| `restart` | Перезапуск туннеля |

### Ответы

| Код | Значение |
|-----|----------|
| 200 | Действие принято |
| 400 | Некорректный JSON, неизвестное действие или ошибка запуска/рестарта |

---

## GET /api/logs

Возвращает последние строки логов из объединённых источников рантайма.

### Query-параметры

| Параметр | Тип | По умолчанию | Ограничение |
|----------|-----|--------------|-------------|
| `limit` | number | `100` | максимум `500` |

### Успешный ответ (200)

```json
{
  "lines": [
    "[tunnel] started PID 12345",
    "[routing] setup complete (WAN=eth0)"
  ],
  "total": 237
}
```

`total` — текущее количество строк во внутреннем буфере wrapper.

---

## GET /api/geoip/status

Возвращает статус подсистемы GeoIP: доступность, активный режим и метаданные
базы (провайдер, дата сборки, число записей, размер на диске, обрезанные
страны).

### Успешный ответ (200)

```json
{
  "enabled": true,
  "policy": "tunnel_all_except",
  "active": true,
  "last_error": "",
  "geoip": {
    "available": true,
    "mode": "local",
    "meta": {
      "provider_id": "geolite2-country-mmdb",
      "format": "mmdb",
      "built_unix": 1724500000,
      "records_v4": 18342,
      "records_v6": 4210,
      "bytes_on_disk": 262144,
      "trimmed_countries": ["RU", "KZ"],
      "sanity_ok": true
    }
  },
  "config_enabled": true,
  "auto_update": { "enabled": true, "interval_hours": 168, "max_age_hours_hard": 720 }
}
```

## GET /api/geoip/providers

Возвращает настроенные массивы `db_providers` и `api_providers` секции
`geoip`.

## POST /api/geoip/provider

Выбирает активного db-провайдера (одиночный выбор: выбранный включается,
остальные выключаются) и немедленно скачивает/собирает его базу. Конфиг
сохраняется только после успешной сборки; при ошибке остаётся предыдущая
база.

### Тело запроса

```json
{ "id": "ip2location-lite-db1" }
```

### Успешный ответ (200)

```json
{
  "status": "updated",
  "provider": "ip2location-lite-db1",
  "reports": [ { "provider_id": "ip2location-lite-db1", "...": "..." } ],
  "meta": { "provider_id": "ip2location-lite-db1", "...": "..." }
}
```

Ошибки: `400` (неизвестный id) или `500` (сбой скачивания/валидации).

## POST /api/geoip/update

Запускает немедленную пересборку базы GeoIP (все включённые провайдеры).
Предыдущая база сохраняется до успешной валидации новой; при успехе сервис
лукапов перезагружается, а активная политика сплит-туннелинга применяется
заново.

### Тело запроса (опционально)

```json
{ "provider_id": "geolite2-country-mmdb" }
```

`provider_id` зарезервирован для точечных обновлений; сейчас обновляются все
включённые провайдеры.

### Успешный ответ (200)

```json
{
  "status": "updated",
  "reports": [
    { "provider_id": "geolite2-country-mmdb", "records_v4": 18342, "records_v6": 4210, "bytes_written": 262144, "elapsed_secs": 12.4 }
  ]
}
```

Ошибки возвращают `500` с `{"error": "..."}`.

## GET /api/splittunnel/policy

Возвращает текущую политику сплит-туннелинга и живой статус.

```json
{
  "policy": "tunnel_all_except",
  "countries_bypass": ["RU", "KZ"],
  "countries_tunnel": [],
  "manual_bypass": ["example.local"],
  "manual_tunnel": [],
  "detection_mode": "local",
  "enabled": true,
  "status": { "active": true, "last_error": "", "...": "..." }
}
```

## POST /api/splittunnel/policy

Проверяет и сохраняет новую политику, затем применяет её «на горячую», не
разрывая туннель.

### Тело запроса

```json
{
  "enabled": true,
  "policy": "tunnel_all_except",
  "countries_bypass": ["RU", "KZ"],
  "countries_tunnel": [],
  "manual_bypass": ["example.local", "192.168.50.0/24"],
  "manual_tunnel": ["some-blocked-service.com"],
  "detection_mode": "local"
}
```

### Ответы

- `200 {"status":"applied"}` — сохранено и применено.
- `202 {"status":"saved","apply_error":"..."}` — сохранено; применение
  не удалось (например, нет `ipset`). Будет применено при следующем
  подключении туннеля.
- `400` — ошибка валидации.

## POST /api/splittunnel/test

Резолвит цель (IP или домен) и объясняет, каким маршрутом она пойдёт.

### Тело запроса

```json
{ "target": "ya.ru" }
```

### Успешный ответ (200)

```json
{
  "target": "ya.ru",
  "results": [
    { "ip": "77.88.8.8", "country": "RU", "matched_rule": "country_list", "route": "direct" }
  ]
}
```

`route` — `"direct"` или `"tunnel"`; `matched_rule` — одно из значений:
`manual_bypass`, `manual_tunnel`, `country_list`, `default`.

---

## GET /

Возвращает встроенный HTML интерфейса Web UI.

В разделе «Настройка туннеля» доступны два режима:

- **Ручной** — полная форма настройки туннеля;
- **DeepLink** — вставка ссылки TrustTunnel вида `tt://?...` и импорт её параметров в ручную форму.

Импорт не сохраняет конфигурацию автоматически: после проверки полей нужно нажать
«Сохранить». DeepLink содержит логин и пароль endpoint в кодированном, но не
зашифрованном виде, поэтому его нельзя передавать через недоверенные каналы.

---

## Формат ошибок

```json
{
  "error": "описание ошибки"
}
```
