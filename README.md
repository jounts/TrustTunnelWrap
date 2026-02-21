# TrustTunnel Keenetic Wrapper

IPK-пакет для запуска [TrustTunnel VPN](https://github.com/TrustTunnel/TrustTunnelClient) на роутерах Keenetic и Netcraze с установленным [Entware](https://github.com/Entware/Entware).

## Возможности

- Веб-интерфейс управления (по умолчанию порт 8080)
- Авторизация через NDM API роутера (учётные записи Keenetic)
- Автоматическое переподключение при обрыве
- Просмотр логов в реальном времени
- Поддержка архитектур: aarch64, mipsel, armv7, x86_64
- Статическая линковка (musl) — минимум зависимостей на роутере

## Требования

- Роутер Keenetic или Netcraze с установленным Entware
- USB-накопитель или достаточно места на NAND
- Настроенный TrustTunnel endpoint ([документация](https://trusttunnel.org/))

## Установка

Скачайте `.ipk` для архитектуры вашего роутера со страницы [Releases](../../releases) и установите:

```sh
# Скопируйте пакет на роутер
scp trusttunnel-keenetic_1.0.0_aarch64.ipk root@192.168.1.1:/tmp/

# Установите
ssh root@192.168.1.1
opkg install /tmp/trusttunnel-keenetic_1.0.0_aarch64.ipk
```

После установки пакет выведет адрес веб-интерфейса и инструкции по запуску.

## Настройка

### Через веб-интерфейс

1. Откройте `http://<ip-роутера>:8080`
2. Войдите с учётными данными роутера (те же, что для web-интерфейса Keenetic)
3. Заполните параметры endpoint: hostname, адреса, логин, пароль
4. Нажмите **Сохранить**, затем **Подключить**

### Через конфигурационный файл

Отредактируйте `/opt/etc/trusttunnel/config.json`:

```json
{
  "tunnel": {
    "hostname": "vpn.example.com",
    "addresses": ["1.2.3.4:443"],
    "username": "myuser",
    "password": "mypassword",
    "upstream_protocol": "http2",
    "vpn_mode": "general",
    "dns_upstreams": ["tls://1.1.1.1"],
    "excluded_routes": ["10.0.0.0/8", "172.16.0.0/12", "192.168.0.0/16"],
    "reconnect_delay": 5,
    "loglevel": "info"
  },
  "webui": {
    "port": 8080,
    "bind": "0.0.0.0",
    "auth": true
  },
  "logging": {
    "level": "info",
    "max_lines": 500
  }
}
```

### С помощью setup_wizard

Если у вас есть файл конфигурации, экспортированный с endpoint:

```sh
/opt/bin/setup_wizard \
  --mode non-interactive \
  --endpoint_config /tmp/endpoint_config.toml \
  --settings /opt/etc/trusttunnel/trusttunnel_client.toml
```

## Управление сервисом

```sh
# Запуск
/opt/etc/init.d/S50trusttunnel start

# Остановка
/opt/etc/init.d/S50trusttunnel stop

# Перезапуск
/opt/etc/init.d/S50trusttunnel restart
```

### CLI

```sh
# Запуск в foreground (для отладки)
/opt/bin/trusttunnel-keenetic --foreground --config /opt/etc/trusttunnel/config.json

# Запуск как демон
/opt/bin/trusttunnel-keenetic --daemon

# Тест конфигурации
/opt/bin/trusttunnel-keenetic --test

# Версия
/opt/bin/trusttunnel-keenetic --version
```

### Просмотр логов

```sh
# Через syslog
logread | grep trusttunnel

# Через API
curl -H "Authorization: <token>" http://127.0.0.1:8080/api/logs
```

## Архитектура

```
trusttunnel-keenetic (wrapper, Rust)
  │
  ├── WebUI HTTP-сервер (tiny_http, порт 8080)
  │     ├── /api/login   — авторизация через NDM API
  │     ├── /api/status  — статус туннеля
  │     ├── /api/config  — чтение/запись конфигурации
  │     ├── /api/control — connect/disconnect/restart
  │     └── /api/logs    — последние строки логов
  │
  └── Tunnel Manager
        ├── Генерация TOML-конфига для trusttunnel_client
        ├── Запуск процесса trusttunnel_client --config <path>
        ├── Мониторинг и авто-переподключение
        └── Graceful shutdown (SIGTERM → SIGKILL)
```

Wrapper управляет бинарником `trusttunnel_client` как дочерним процессом. Настройки из JSON-конфига wrapper транслируются в TOML-файл, который понимает `trusttunnel_client`.

## Сборка из исходников

### Для хост-системы (тестирование)

```sh
cargo build --release
```

### Кросс-компиляция для роутера

```sh
# Установите cross
cargo install cross --git https://github.com/cross-rs/cross

# Соберите для aarch64
cross build --release --target aarch64-unknown-linux-musl

# Или используйте скрипт
./scripts/build-release.sh aarch64-unknown-linux-musl
```

### Сборка IPK-пакета

```sh
# 1. Скачайте pre-built trusttunnel_client
./scripts/download-client.sh v0.99.105 linux-aarch64 client_bin

# 2. Соберите wrapper
cross build --release --target aarch64-unknown-linux-musl

# 3. Соберите .ipk
./scripts/package-ipk.sh aarch64 1.0.0 \
  target/aarch64-unknown-linux-musl/release/trusttunnel-keenetic \
  client_bin
```

## CI/CD

При пуше тега `v*` GitHub Actions автоматически:

1. Скачивает pre-built `trusttunnel_client` для каждой архитектуры
2. Собирает wrapper через `cross` (статическая линковка musl)
3. Пакует `.ipk` для aarch64, mipsel, armv7, x86_64
4. Проверяет размер (предупреждение при > 10 MB)
5. Публикует Release с пакетами и SHA256SUMS

Ручной запуск: вкладка Actions → Build TrustTunnel IPK → Run workflow.

## Структура проекта

```
├── Cargo.toml                       # Зависимости и профили оптимизации
├── Cargo.lock                       # Фиксированные версии
├── .cargo/config.toml               # Настройки кросс-компиляции
├── .github/workflows/build.yml      # CI/CD
├── src/
│   ├── main.rs                      # CLI, daemon, сигналы
│   ├── config.rs                    # JSON-конфиг wrapper + генерация TOML
│   ├── tunnel.rs                    # Управление процессом trusttunnel_client
│   ├── webui.rs                     # HTTP-сервер и API
│   ├── auth.rs                      # NDM API авторизация
│   └── logs.rs                      # Кольцевой буфер логов
├── package/
│   ├── CONTROL/
│   │   ├── postinst                 # Скрипт после установки
│   │   ├── prerm                    # Скрипт перед удалением
│   │   └── conffiles                # Список файлов конфигурации
│   ├── etc/trusttunnel/config.json  # Шаблон конфигурации
│   └── www/index.html               # Веб-интерфейс (встраивается в бинарник)
└── scripts/
    ├── build-release.sh             # Сборка release-бинарника
    ├── download-client.sh           # Скачивание pre-built клиента
    └── package-ipk.sh              # Сборка .ipk пакета
```

## Конфигурация

### Параметры туннеля (`tunnel`)

| Параметр | Тип | По умолчанию | Описание |
|---|---|---|---|
| `hostname` | string | `""` | Hostname endpoint (SNI) |
| `addresses` | string[] | `[]` | Адреса endpoint (`IP:port`) |
| `username` | string | `""` | Имя пользователя |
| `password` | string | `""` | Пароль |
| `upstream_protocol` | string | `"http2"` | Протокол: `http2` или `http3` |
| `vpn_mode` | string | `"general"` | Режим: `general` (весь трафик) или `selective` |
| `dns_upstreams` | string[] | `["tls://1.1.1.1"]` | DNS-серверы через VPN |
| `killswitch_enabled` | bool | `false` | Блокировка трафика вне VPN |
| `included_routes` | string[] | `["0.0.0.0/0"]` | Маршруты через VPN |
| `excluded_routes` | string[] | `["10.0.0.0/8", ...]` | Маршруты в обход VPN |
| `mtu_size` | number | `1280` | MTU туннельного интерфейса |
| `socks_address` | string | `""` | Адрес SOCKS5 прокси (если нужен) |
| `skip_verification` | bool | `false` | Пропуск проверки сертификата |
| `reconnect_delay` | number | `5` | Задержка переподключения (секунды) |
| `loglevel` | string | `"info"` | Уровень: `error`, `warn`, `info`, `debug`, `trace` |

### Параметры веб-интерфейса (`webui`)

| Параметр | Тип | По умолчанию | Описание |
|---|---|---|---|
| `port` | number | `8080` | Порт HTTP-сервера |
| `bind` | string | `"0.0.0.0"` | Адрес привязки |
| `auth` | bool | `true` | Требовать авторизацию |

### Параметры логирования (`logging`)

| Параметр | Тип | По умолчанию | Описание |
|---|---|---|---|
| `level` | string | `"info"` | Уровень логирования wrapper |
| `max_lines` | number | `500` | Размер кольцевого буфера логов |

## API

Все эндпоинты кроме `/api/login` требуют заголовок `Authorization` с токеном сессии.

| Метод | Путь | Описание |
|---|---|---|
| POST | `/api/login` | Авторизация (`{"login":"...","password":"..."}`) |
| GET | `/api/status` | Статус туннеля |
| GET | `/api/config` | Текущие настройки туннеля |
| POST | `/api/config` | Обновление настроек |
| POST | `/api/control` | Управление (`{"action":"connect\|disconnect\|restart"}`) |
| GET | `/api/logs` | Логи (`?limit=100`) |
| GET | `/` | Веб-интерфейс (HTML) |

## Безопасность

- Авторизация WebUI проходит через NDM API роутера — используются те же учётные записи
- Сессии хранятся в памяти с TTL 1 час
- Пароли endpoint хранятся в `/opt/etc/trusttunnel/config.json` — убедитесь, что права доступа ограничены (`chmod 600`)
- По умолчанию WebUI слушает на `0.0.0.0` — для ограничения доступа измените `bind` на `127.0.0.1`

## Удаление

```sh
opkg remove trusttunnel-keenetic
```

Конфигурация в `/opt/etc/trusttunnel/` сохраняется при удалении пакета.

## Лицензия

MIT
