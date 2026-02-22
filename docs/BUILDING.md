# Сборка и разработка

## Зависимости

- [Rust](https://www.rust-lang.org/tools/install) ≥ 1.75
- [cross](https://github.com/cross-rs/cross) — для кросс-компиляции aarch64, armv7, x86_64
- Docker — требуется для `cross`
- Тулчейн [musl.cc](https://musl.cc) — для кросс-компиляции mipsel (без Docker)
- `tar`, `gzip` — для сборки IPK
- `curl` или `wget` — для скачивания pre-built клиента и тулчейна

## Быстрый старт (хост-система)

```sh
git clone https://github.com/jounts/TrustTunnelWrap.git
cd trusttunnelwrap
cargo build
cargo run -- --foreground --config package/etc/trusttunnel/config.json
```

## Кросс-компиляция

### Поддерживаемые платформы

| Архитектура роутера | Rust target triple | Метод сборки | Пример роутеров |
|---------------------|-------------------|--------------|-----------------|
| aarch64 | `aarch64-unknown-linux-musl` | `cross` (Docker) | Keenetic Ultra, Peak, Hopper |
| mipsel | `mipsel-unknown-linux-musl` | `cargo` + musl.cc | Keenetic Giga, Omni, City |
| armv7 | `armv7-unknown-linux-musleabihf` | `cross` (Docker) | Netcraze, старые Keenetic |
| x86_64 | `x86_64-unknown-linux-musl` | `cross` (Docker) | Тестирование на ПК/VM |

### Сборка через cross (aarch64, armv7, x86_64)

```sh
cargo install cross --git https://github.com/cross-rs/cross

cross build --release --target aarch64-unknown-linux-musl
cross build --release --target armv7-unknown-linux-musleabihf
cross build --release --target x86_64-unknown-linux-musl
```

### Сборка mipsel (локально, без Docker)

`cross` не предоставляет Docker-образ для `mipsel-unknown-linux-musl`, поэтому локально используется прямая компиляция с тулчейном [musl.cc](https://musl.cc):

```sh
# Установите тулчейн (Linux)
wget -qO- https://musl.cc/mipsel-linux-musl-cross.tgz | sudo tar xzf - -C /usr/local/ --strip-components=1

# На macOS (BSD tar)
wget -qO- https://musl.cc/mipsel-linux-musl-cross.tgz | sudo tar xzf - -C /usr/local/ --strip-components=1

# Добавьте Rust target
rustup target add mipsel-unknown-linux-musl

# Соберите
cargo build --release --target mipsel-unknown-linux-musl
```

Линкер `mipsel-linux-musl-gcc` уже настроен в `.cargo/config.toml`.

Бинарник будет в `target/<triple>/release/trusttunnel-keenetic`.

### Сборка mipsel в CI

В GitHub Actions для `mipsel` используется контейнер `ghcr.io/rust-cross/rust-musl-cross:mipsel-musl` с nightly и флагом `-Zbuild-std=std,panic_abort`.

### Скрипт build-release.sh

Обёртка над `cross` с проверкой размера:

```sh
./scripts/build-release.sh aarch64-unknown-linux-musl
```

## Оптимизация размера бинарника

Настройки в `Cargo.toml`:

```toml
[profile.release]
opt-level = "z"       # Оптимизация под размер
lto = true            # Link-time optimization
codegen-units = 1     # Один codegen unit для лучшего LTO
panic = "abort"       # Без unwind-таблиц
strip = true          # Убрать символы отладки
debug = false         # Без debug info
```

Дополнительно в `.cargo/config.toml` — GC неиспользуемых секций и статическая линковка CRT.

Ожидаемый размер wrapper бинарника: 1.5–3 MB (зависит от архитектуры).

## Сборка IPK-пакета

### 1. Скачайте pre-built TrustTunnelClient

```sh
./scripts/download-client.sh v0.99.105 linux-aarch64 client_bin
```

Параметры:
- Версия (тег релиза, например `v0.99.105`)
- Архитектура клиента (`linux-aarch64`, `linux-mipsel`, `linux-armv7`, `linux-x86_64`)
- Директория для скачивания

Скрипт скачает `trusttunnel_client` и `setup_wizard`.

### 2. Соберите wrapper

```sh
cross build --release --target aarch64-unknown-linux-musl
```

### 3. Соберите .ipk

```sh
./scripts/package-ipk.sh aarch64 1.0.0 \
  target/aarch64-unknown-linux-musl/release/trusttunnel-keenetic \
  client_bin
```

Параметры:
1. Архитектура (для имени пакета)
2. Версия
3. Путь к скомпилированному wrapper бинарнику
4. Директория с pre-built клиентом

Результат: `trusttunnel-keenetic_1.0.0_aarch64-3.10.ipk`

### Структура IPK

Entware использует формат tar.gz (в отличие от стандартного OpenWrt, который использует ar):

```
trusttunnel-keenetic_1.0.0_aarch64-3.10.ipk (tar.gz-архив)
├── ./debian-binary        # "2.0\n"
├── ./control.tar.gz       # метаданные пакета
│   ├── control            # имя, версия, архитектура, зависимости
│   ├── postinst           # скрипт после установки
│   ├── prerm              # скрипт перед удалением
│   ├── postrm             # финальная очистка
│   └── conffiles          # список конфигурационных файлов
└── ./data.tar.gz          # файлы для установки
    └── opt/
        ├── bin/
        │   ├── trusttunnel-keenetic    # wrapper
        │   ├── trusttunnel_client      # VPN клиент
        │   └── setup_wizard            # мастер настройки
        └── etc/trusttunnel/
            └── config.json             # конфигурация по умолчанию
```

## CI/CD (GitHub Actions)

Workflow `.github/workflows/build.yml` запускается при:

- **Push тега** `v*` — автоматическая сборка + релиз
- **Ручной запуск** — вкладка Actions, можно выбрать архитектуру и версию клиента

### Создание релиза

```sh
git tag v1.0.0
git push origin v1.0.0
```

GitHub Actions:
1. Скачает pre-built `trusttunnel_client` для aarch64, mipsel, armv7, x86_64
2. Соберёт wrapper: `cross` для aarch64/armv7/x86_64
3. Соберёт `mipsel` через `ghcr.io/rust-cross/rust-musl-cross:mipsel-musl` (nightly + `-Zbuild-std`)
4. Пакует `.ipk` (формат tar.gz для Entware)
5. Проверяет размер (предупреждение > 10 MB)
6. Публикует Release с файлами и SHA256SUMS

## Разработка

### Запуск тестов

```sh
cargo test
```

### Проверка конфигурации

```sh
cargo run -- --test --config package/etc/trusttunnel/config.json
```

### Локальный запуск (без роутера)

```sh
# Создайте тестовую конфигурацию
cp package/etc/trusttunnel/config.json /tmp/tt-test.json

# При необходимости укажите ndm_host вручную:
# "webui": { "ndm_host": "192.168.1.1", "ndm_port": 80 }

cargo run -- --foreground --config /tmp/tt-test.json
```

WebUI будет доступен на `http://127.0.0.1:8080`.

### Линтинг

```sh
cargo clippy --all-targets
```

### Форматирование

```sh
cargo fmt --check   # проверка
cargo fmt           # автоформатирование
```
