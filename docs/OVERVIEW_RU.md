# Обзор

`trusttunnel-keenetic` — это обёртка для роутеров вокруг `trusttunnel_client`, предназначенная для Keenetic/Netcraze с Entware.

## Что предоставляет

- Встроенный Web UI (по умолчанию `:8080`)
- Авторизацию через NDM API роутера (те же локальные учётные записи)
- Управление туннелем (`connect`, `disconnect`, `restart`) из UI/API
- Сессии в памяти (TTL токена: 1 час, продление при активности)
- Мониторинг процесса, переподключение и watchdog-проверки
- Управление интерфейсами и маршрутами через NDM (`OpkgTun0`) и Linux (`opkgtun0`/`tun0`)
- Кольцевой буфер логов и запись в файл с ротацией

## Архитектура (кратко)

```text
trusttunnel-keenetic (Rust wrapper)
  |
  +- WebUI HTTP server (tiny_http)
  |    +- /api/login
  |    +- /api/status
  |    +- /api/config
  |    +- /api/control
  |    +- /api/logs
  |    \- /
  |
  \- Tunnel Manager
       +- Генерация TOML для trusttunnel_client
       +- Запуск/остановка дочернего процесса trusttunnel_client
       +- Мониторинг и переподключение при сбоях
       +- Watchdog и роутинг-проверки
       \- Корректное завершение
```

## Безопасность

- Авторизация Web UI использует креденшелы роутера через NDM API.
- Токены сессий хранятся только в памяти.
- Данные endpoint хранятся в `/opt/etc/trusttunnel/config.json`; ограничьте права (`chmod 600`).
- Если UI не должен быть доступен извне, установите `webui.bind = "127.0.0.1"`.

## Связанные документы

- Конфигурация: [`CONFIGURATION_RU.md`](CONFIGURATION_RU.md)
- API: [`API_RU.md`](API_RU.md)
- Сборка и разработка: [`BUILDING_RU.md`](BUILDING_RU.md)
- English version: [`OVERVIEW.md`](OVERVIEW.md)
