# Справочник конфигурации

Основной путь конфигурации wrapper:

```text
/opt/etc/trusttunnel/config.json
```

Если файл конфигурации отсутствует, wrapper завершается с ошибкой и не запускается
с неявными значениями по умолчанию. Файл конфигурации и TOML-файл клиента при
записи создаются с правами `0600`, поскольку содержат пароли.

Шаблон по умолчанию в пакете:

```text
package/etc/trusttunnel/config.json
```

## Полный пример

```json
{
  "tunnel": {
    "hostname": "",
    "custom_sni": "",
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
| `custom_sni` | string | `""` | Необязательная замена TLS SNI отдельно от hostname endpoint |
| `addresses` | string[] | `[]` | Адреса endpoint (`IP:port`) |
| `username` | string | `""` | Логин endpoint |
| `password` | string | `""` | Пароль endpoint |
| `upstream_protocol` | string | `"http2"` | Протокол (`http2`/`http3`) |
| `certificate` | string | `""` | PEM-сертификат endpoint (опционально) |
| `skip_verification` | bool | `false` | Пропуск TLS-проверки |
| `vpn_mode` | string | `"general"` | `general` или `selective` |
| `dns_upstreams` | string[] | `["tls://1.1.1.1"]` | DNS через VPN; в TOML клиента генерируется в секции `[endpoint]` |
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

## `geoip`

Подсистема GeoIP для сплит-туннелирования. По умолчанию выключена. При
включении wrapper скачивает страновые базы, конвертирует их в компактный
бинарный формат (`v4.bin` / `v6.bin` в `db_path`) и отвечает на запросы
IP→страна.

| Параметр | Тип | По умолчанию | Описание |
|---|---|---|---|
| `enabled` | bool | `false` | Включить подсистему GeoIP |
| `mode` | string | `"local"` | Режим поиска: `local` / `api` / `hybrid` |
| `db_providers` | object[] | `[]` | Источники скачиваемых баз (см. ниже) |
| `api_providers` | object[] | `[]` | Онлайн-API определения страны (см. ниже) |
| `trim_to_selected_countries` | bool | `true` | Хранить только диапазоны стран из `split_tunnel`; радикально уменьшает размер базы |
| `db_path` | string | `"/opt/etc/trusttunnel/geoip/"` | Каталог собранных баз. Размещение на USB снижает износ внутренней флеш-памяти |
| `cache_ttl_hours` | number | `24` | TTL записей кеша API-лукапов |
| `auto_update.enabled` | bool | `false` | Фоновое автообновление |
| `auto_update.interval_hours` | number | `168` | Интервал обновления (часы) |
| `auto_update.jitter_minutes` | number | `30` | Случайная начальная задержка (разгрузка зеркал) |
| `auto_update.on_startup_if_stale` | bool | `true` | Собрать базу при старте, если её нет |
| `auto_update.max_age_hours_hard` | number | `720` | Порог «совсем протухшей» базы |

**Поля db_providers[]:** `id` (уникальный), `url` (http/https), `format`
(один из `mmdb`, `geolite2-csv`, `ip2location-csv`, `zone`), `priority`
(меньше — выше приоритет при перекрытии данных), `enabled`. Скачивание
прозрачно поддерживает gzip/zip-контейнеры и условные запросы по
ETag/Last-Modified.

В пакет предустановлены три провайдера, не требующие ключей; активен ровно
один (одиночный выбор):

| id | Источник | URL | Формат |
|---|---|---|---|
| `geolite2-p3terx` | P3TERX/GeoLite.mmdb | `https://github.com/P3TERX/GeoLite.mmdb/raw/download/GeoLite2-Country.mmdb` | mmdb |
| `geolite2-wpstatistics` | wp-statistics через jsDelivr | `https://cdn.jsdelivr.net/npm/geolite2-country/GeoLite2-Country.mmdb.gz` | mmdb (gzip) |
| `ip2location-lite-db1` | lite.ip2location.com | `https://download.ip2location.com/lite/IP2LOCATION-LITE-DB1.CSV.ZIP` | ip2location-csv |

Все три скачиваются напрямую без регистрации и API-ключей. Выбор провайдера
в WebUI (`POST /api/geoip/provider`) немедленно скачивает и собирает его
базу; предыдущая база сохраняется до успешной валидации новой. Включение
сплит-туннелинга при отсутствии базы также запускает автоматическую первую
загрузку.

Примечание: wp-statistics ранее публиковал per-country `.zone` файлы; сейчас
это единый сжатый mmdb (см. выше). Формат `zone` остаётся поддерживаемым для
других источников — укажите двухбуквенный код страны в id провайдера
(например, `"ru-zone"`).

**Поля api_providers[]:** `id`, `kind` (`ip2c`, `ipapi` или `generic-json`
с плейсхолдером `{ip}`), `url`, необязательный `api_key`, `priority`,
`rate_limit_per_min`. Лукапы через API выполняются только на границе новых
соединений / диагностических запросов — никогда на каждый пакет — и
кешируются на `cache_ttl_hours`. Провайдеры, требующие учётных данных
(обязательный непустой `api_key`), не опрашиваются автоматически сверх
обработки лимитов/ошибок — цепочка просто переходит к следующему провайдеру.

## `split_tunnel`

Политико-ориентированный сплит-туннелинг на уровне ОС. Клиент остаётся в
режиме `general` (или автоматически переводится в пустой `selective`);
маршрутизация выполняется через `ipset` + iptables-метки + policy routing,
поэтому NDM никогда не видит тысячи статических маршрутов. Требует
`geoip.enabled`.

| Параметр | Тип | По умолчанию | Описание |
|---|---|---|---|
| `enabled` | bool | `false` | Включить сплит-туннелинг |
| `policy` | string | `"tunnel_all_except"` | `tunnel_all_except`: весь трафик через VPN, кроме перечисленных стран; `tunnel_only_listed`: только перечисленный трафик идёт в туннель |
| `countries_bypass` | string[] | `[]` | ISO 3166-1 alpha-2 коды стран напрямую (`tunnel_all_except`) |
| `countries_tunnel` | string[] | `[]` | ISO коды стран через VPN (`tunnel_only_listed`) |
| `manual_bypass` | string[] | `[]` | Домен/IP/CIDR всегда напрямую (наивысший приоритет) |
| `manual_tunnel` | string[] | `[]` | Домен/IP/CIDR всегда через VPN (второй приоритет) |
| `detection_mode` | string | `"local"` | Источник определения страны: `local` / `api` / `hybrid` |

Приоритет правил (первое совпадение побеждает): manual bypass → manual
tunnel → страновой список → направление по умолчанию базовой политики.
Домены в ручных правилах резолвятся в IP в момент применения политики.

Требования к Entware: бинарник `ipset` + модули ядра (`ip_set`,
`ip_set_hash_net`, `xt_set`), таблица mangle в `iptables`. Wrapper проверяет
их перед применением политики (`check_capabilities()`) и сообщает об отсутствии
в WebUI; пока чего-то не хватает, политики не применяются. На части прошивок
Keenetic `ipset`/модули ядра недоступны из коробки — предварительно установите
их из Entware: `opkg install ipset iptables`. Осторожно с
`killswitch_enabled`: killswitch блокирует весь трафик вне tun-устройства,
включая «прямые» потоки сплит-туннелинга.

Cron-задача может создавать файл `touch /opt/etc/trusttunnel/geoip/.update-request`
для внепланового обновления базы.

## Имена интерфейсов (Keenetic)

- Linux: `opkgtun0` (lowercase), видно в `ip link`.
- NDM: `OpkgTun0` (CamelCase), видно в `ndmc -c 'show interface'`.

## NDMS 5: «Маршруты DNS» и переустановка пакета

В веб-интерфейсе Keenetic/Netcraze (NDMS 5) **«Маршрутизация → Маршруты DNS»** создаются `object-group fqdn` и правила в `dns-proxy` вида `route object-group … OpkgTun0 …`.

Скрипты пакета **не редактируют** `dns-proxy`. Но при `opkg remove trusttunnel-keenetic` выполняется снятие интерфейса NDM (`no interface OpkgTun0`). В такой ситуации прошивка может **удалить правила `dns-proxy`, ссылающиеся на несуществующий `OpkgTun0`**, при этом **сами списки доменов (`object-group`) часто остаются**.

**Восстановить маршруты можно** — после установки новой версии и запуска сервиса интерфейс `OpkgTun0` снова появится, но правила в `dns-proxy` нужно **заново привязать** (через веб-интерфейс или CLI), либо **восстановить фрагмент конфига** из заранее сохранённого `show running-config` / резервной копии роутера.

Практика: перед обновлением пакета сохраните копию `dns-proxy` с маршрутами на `OpkgTun0` (или полный `running-config`), чтобы после обновления быстро вернуть те же строки.

## Связанные документы

- Обзор: [`OVERVIEW_RU.md`](OVERVIEW_RU.md)
- API: [`API_RU.md`](API_RU.md)
- English version: [`CONFIGURATION.md`](CONFIGURATION.md)
