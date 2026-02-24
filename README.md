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

### Быстрая установка (рекомендуется)

Установщик сам:
- определит архитектуру роутера;
- скачает подходящий `.ipk` из последнего релиза;
- сделает backup текущего `/opt/etc/trusttunnel/config.json` (если пакет уже установлен);
- удалит старую версию и установит новую;
- восстановит конфиг из backup после установки.

```sh
curl -fsSL https://raw.githubusercontent.com/jounts/TrustTunnelWrap/main/scripts/install.sh | sh
```

Если `curl` недоступен:

```sh
wget -O /tmp/install-trusttunnel.sh https://raw.githubusercontent.com/jounts/TrustTunnelWrap/main/scripts/install.sh
sh /tmp/install-trusttunnel.sh
```

### Ручная установка

Скачайте `.ipk` для архитектуры вашего роутера со страницы [Releases](../../releases) и установите:

```sh
# Скопируйте пакет на роутер
scp -O trusttunnel-keenetic_1.0.0_aarch64-3.10.ipk root@192.168.1.1:/tmp/trusttunnel.ipk

# Установите
ssh root@192.168.1.1
opkg install /tmp/trusttunnel.ipk
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
    "certificate": "",
    "has_ipv6": true,
    "client_random": "",
    "vpn_mode": "general",
    "dns_upstreams": ["tls://1.1.1.1"],
    "killswitch_enabled": false,
    "killswitch_allow_ports": [],
    "post_quantum_group_enabled": true,
    "exclusions": [],
    "included_routes": ["0.0.0.0/0", "2000::/3"],
    "excluded_routes": ["10.0.0.0/8", "172.16.0.0/12", "192.168.0.0/16"],
    "mtu_size": 1280,
    "bound_if": "",
    "change_system_dns": false,
    "anti_dpi": false,
    "socks_address": "",
    "socks_username": "",
    "socks_password": "",
    "skip_verification": false,
    "reconnect_delay": 5,
    "loglevel": "info"
  },
  "webui": {
    "port": 8080,
    "bind": "0.0.0.0",
    "ndm_host": "",
    "ndm_port": 80
  },
  "logging": {
    "level": "info",
    "max_lines": 500,
    "file_enabled": true,
    "file_path": "/var/log/trusttunnel-keenetic/trusttunnel-keenetic.log",
    "rotate_size": "512KB",
    "rotate_keep": 1
  },
  "routing": {
    "enabled": true,
    "watchdog_enabled": true,
    "watchdog_interval": 30,
    "watchdog_failures": 3,
    "watchdog_check_url": "http://connectivitycheck.gstatic.com/generate_204",
    "watchdog_check_timeout": 5
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
# Через API (внутренний буфер + доступные системные источники)
curl -H "Authorization: <token>" http://127.0.0.1:8080/api/logs

# Через файл логов
tail -f /var/log/trusttunnel-keenetic/trusttunnel-keenetic.log

# Просмотр архивов ротации
ls -lh /var/log/trusttunnel-keenetic/
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
        ├── Watchdog (проверка tun/WAN/connectivity)
        ├── Настройка маршрутов через NDM (ndmc)
        └── Graceful shutdown (SIGTERM → SIGKILL)
```

Wrapper управляет бинарником `trusttunnel_client` как дочерним процессом. Настройки из JSON-конфига wrapper транслируются в TOML-файл, который понимает `trusttunnel_client`.

## Сборка и разработка

Подробные инструкции по сборке, кросс-компиляции, упаковке IPK, CI/CD и локальной разработке вынесены в:

- [`docs/BUILDING.md`](docs/BUILDING.md)

## Конфигурация

### Параметры туннеля (`tunnel`)

| Параметр | Тип | По умолчанию | Описание |
|---|---|---|---|
| `hostname` | string | `""` | Hostname endpoint (SNI) |
| `addresses` | string[] | `[]` | Адреса endpoint (`IP:port`) |
| `username` | string | `""` | Имя пользователя |
| `password` | string | `""` | Пароль |
| `upstream_protocol` | string | `"http2"` | Протокол: `http2` или `http3` |
| `certificate` | string | `""` | PEM-сертификат endpoint (опционально) |
| `has_ipv6` | bool | `true` | Разрешить маршрутизацию IPv6 через endpoint |
| `client_random` | string | `""` | Префикс/маска TLS ClientHello random (`hex[/mask]`) |
| `vpn_mode` | string | `"general"` | Режим: `general` (весь трафик) или `selective` |
| `dns_upstreams` | string[] | `["tls://1.1.1.1"]` | DNS-серверы через VPN |
| `killswitch_enabled` | bool | `false` | Блокировка трафика вне VPN |
| `killswitch_allow_ports` | number[] | `[]` | Локальные порты, разрешённые при активном killswitch |
| `post_quantum_group_enabled` | bool | `true` | Включение post-quantum key exchange в TLS |
| `exclusions` | string[] | `[]` | Домены/IP/CIDR для special-routing по `vpn_mode` |
| `included_routes` | string[] | `["0.0.0.0/0", "2000::/3"]` | Маршруты через VPN |
| `excluded_routes` | string[] | `["10.0.0.0/8", ...]` | Маршруты в обход VPN |
| `mtu_size` | number | `1280` | MTU туннельного интерфейса |
| `bound_if` | string | `""` | Интерфейс исходящих подключений клиента (`""` = auto) |
| `change_system_dns` | bool | `false` | Разрешить клиенту менять системные DNS |
| `anti_dpi` | bool | `false` | Включение anti-DPI режима клиента |
| `socks_address` | string | `""` | Адрес SOCKS5 прокси (если нужен) |
| `socks_username` | string | `""` | Логин SOCKS5 (опционально) |
| `socks_password` | string | `""` | Пароль SOCKS5 (опционально) |
| `skip_verification` | bool | `false` | Пропуск проверки сертификата |
| `reconnect_delay` | number | `5` | Задержка переподключения (секунды) |
| `loglevel` | string | `"info"` | Уровень: `error`, `warn`, `info`, `debug`, `trace` |

### Параметры веб-интерфейса (`webui`)

| Параметр | Тип | По умолчанию | Описание |
|---|---|---|---|
| `port` | number | `8080` | Порт HTTP-сервера |
| `bind` | string | `"0.0.0.0"` | Адрес привязки |
| `ndm_host` | string | `""` | Хост NDM API (если пусто — автоопределение LAN IP) |
| `ndm_port` | number | `80` | Порт NDM API |

### Параметры логирования (`logging`)

| Параметр | Тип | По умолчанию | Описание |
|---|---|---|---|
| `level` | string | `"info"` | Уровень логирования wrapper |
| `max_lines` | number | `500` | Размер кольцевого буфера логов |
| `file_enabled` | bool | `true` | Включить запись логов wrapper в файл |
| `file_path` | string | `"/var/log/trusttunnel-keenetic/trusttunnel-keenetic.log"` | Путь к основному лог-файлу |
| `rotate_size` | string\|number | `"512KB"` | Порог ротации: байты (`1048576`) или с суффиксом (`512KB`, `10MB`, `1GB`) |
| `rotate_keep` | number | `1` | Количество архивов ротации (`.1`, `.2`, ... ) |

### Параметры маршрутизации (`routing`)

| Параметр | Тип | По умолчанию | Описание |
|---|---|---|---|
| `enabled` | bool | `true` | Включить настройку маршрутов через NDM при подключении |
| `watchdog_enabled` | bool | `true` | Включить watchdog проверки туннеля |
| `watchdog_interval` | number | `30` | Интервал watchdog-проверок (секунды) |
| `watchdog_failures` | number | `3` | Порог неудачных проверок до рестарта |
| `watchdog_check_url` | string | `"http://connectivitycheck.gstatic.com/generate_204"` | URL для HTTP health-check через `OpkgTun0` |
| `watchdog_check_timeout` | number | `5` | Таймаут проверки связности (секунды) |

Примечание по интерфейсам в Keenetic:
- Linux-интерфейс: `opkgtun0` (lowercase), его видно в `ip link`/`ip -s link`.
- NDM-интерфейс: `OpkgTun0` (CamelCase), его видно в `ndmc -c 'show interface'`.

## API

Полное описание эндпоинтов, форматов запросов/ответов и кодов ошибок:

- [`docs/API.md`](docs/API.md)

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

## Благодарности

- [TrustTunnel VPN](https://github.com/TrustTunnel/TrustTunnelClient) — VPN-клиент, для которого создан этот менеджер
- [TrustTunnel-Keenetic](https://github.com/artemevsevev/TrustTunnel-Keenetic) — проект, вдохновивший на создание этой обёртки

## Лицензия

MIT
