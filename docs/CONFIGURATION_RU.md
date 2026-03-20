# Справочник конфигурации

Основной путь конфигурации wrapper:

```text
/opt/etc/trusttunnel/config.json
```

Шаблон по умолчанию в пакете:

```text
package/etc/trusttunnel/config.json
```

## Полный пример

```json
{
  "tunnel": {
    "hostname": "",
    "addresses": [],
    "username": "",
    "password": "",
    "upstream_protocol": "http2",
    "certificate": "",
    "skip_verification": false,
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
    "has_ipv6": true,
    "client_random": "",
    "socks_address": "",
    "socks_username": "",
    "socks_password": "",
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

## `tunnel`

| Параметр | Тип | По умолчанию | Описание |
|---|---|---|---|
| `hostname` | string | `""` | Hostname endpoint (SNI) |
| `addresses` | string[] | `[]` | Адреса endpoint (`IP:port`) |
| `username` | string | `""` | Логин endpoint |
| `password` | string | `""` | Пароль endpoint |
| `upstream_protocol` | string | `"http2"` | Протокол (`http2`/`http3`) |
| `certificate` | string | `""` | PEM-сертификат endpoint (опционально) |
| `skip_verification` | bool | `false` | Пропуск TLS-проверки |
| `vpn_mode` | string | `"general"` | `general` или `selective` |
| `dns_upstreams` | string[] | `["tls://1.1.1.1"]` | DNS через VPN |
| `killswitch_enabled` | bool | `false` | Блокировка трафика вне VPN |
| `killswitch_allow_ports` | number[] | `[]` | Разрешённые локальные порты при killswitch |
| `post_quantum_group_enabled` | bool | `true` | Включение post-quantum группы |
| `exclusions` | string[] | `[]` | Исключения доменов/IP/CIDR |
| `included_routes` | string[] | `["0.0.0.0/0","2000::/3"]` | Маршруты через VPN |
| `excluded_routes` | string[] | `["10.0.0.0/8","172.16.0.0/12","192.168.0.0/16"]` | Маршруты в обход VPN |
| `mtu_size` | number | `1280` | MTU туннеля |
| `bound_if` | string | `""` | Исходящий интерфейс (`""` = auto) |
| `change_system_dns` | bool | `false` | Разрешить менять системные DNS |
| `anti_dpi` | bool | `false` | Режим anti-DPI |
| `has_ipv6` | bool | `true` | IPv6 через endpoint |
| `client_random` | string | `""` | TLS ClientHello random (`hex[/mask]`) |
| `socks_address` | string | `""` | Адрес SOCKS5 (опционально) |
| `socks_username` | string | `""` | Логин SOCKS5 |
| `socks_password` | string | `""` | Пароль SOCKS5 |
| `reconnect_delay` | number | `5` | Задержка переподключения (сек) |
| `loglevel` | string | `"info"` | Уровень логов клиента |

## `webui`

| Параметр | Тип | По умолчанию | Описание |
|---|---|---|---|
| `port` | number | `8080` | HTTP-порт |
| `bind` | string | `"0.0.0.0"` | Адрес привязки |
| `ndm_host` | string | `""` | Хост NDM API; если пусто, автоопределение |
| `ndm_port` | number | `80` | Порт NDM API |

## `logging`

| Параметр | Тип | По умолчанию | Описание |
|---|---|---|---|
| `level` | string | `"info"` | Уровень логирования wrapper |
| `max_lines` | number | `500` | Размер буфера логов (строки) |
| `file_enabled` | bool | `true` | Запись логов в файл |
| `file_path` | string | `"/var/log/trusttunnel-keenetic/trusttunnel-keenetic.log"` | Путь к лог-файлу |
| `rotate_size` | string\|number | `"512KB"` | Порог ротации (`1048576`, `512KB`, `10MB`, `1GB`) |
| `rotate_keep` | number | `1` | Количество архивов ротации |

## `routing`

| Параметр | Тип | По умолчанию | Описание |
|---|---|---|---|
| `enabled` | bool | `true` | Обновлять маршруты через NDM при подключении |
| `watchdog_enabled` | bool | `true` | Включить watchdog туннеля |
| `watchdog_interval` | number | `30` | Интервал проверок (сек) |
| `watchdog_failures` | number | `3` | Порог ошибок до рестарта |
| `watchdog_check_url` | string | `"http://connectivitycheck.gstatic.com/generate_204"` | URL health-check |
| `watchdog_check_timeout` | number | `5` | Таймаут проверки (сек) |

## Имена интерфейсов (Keenetic)

- Linux: `opkgtun0` (lowercase), видно в `ip link`.
- NDM: `OpkgTun0` (CamelCase), видно в `ndmc -c 'show interface'`.

## Связанные документы

- Обзор: [`OVERVIEW_RU.md`](OVERVIEW_RU.md)
- API: [`API_RU.md`](API_RU.md)
- English version: [`CONFIGURATION.md`](CONFIGURATION.md)
