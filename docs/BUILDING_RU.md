# Сборка и разработка

English version: [`BUILDING.md`](BUILDING.md)

## Зависимости

- [Rust](https://www.rust-lang.org/tools/install) >= 1.75
- [cross](https://github.com/cross-rs/cross) для кросс-компиляции aarch64/armv7/x86_64
- Docker (нужен для `cross`)
- тулчейн [musl.cc](https://musl.cc) для локальной сборки `mipsel`
- `tar`, `gzip` для упаковки IPK
- `curl` или `wget` для загрузки pre-built клиента/тулчейна

## Быстрый старт (хост-машина)

```sh
git clone https://github.com/jounts/TrustTunnelWrap.git
cd trusttunnelwrap
cargo build
cargo run -- --foreground --config package/etc/trusttunnel/config.json
```

## Кросс-компиляция

### Поддерживаемые цели

| Архитектура роутера | Rust target triple | Метод |
|---------------------|--------------------|-------|
| aarch64 | `aarch64-unknown-linux-musl` | `cross` (Docker) |
| mipsel | `mipsel-unknown-linux-musl` | локально `cargo` + musl.cc |
| armv7 | `armv7-unknown-linux-musleabihf` | `cross` (Docker) |
| x86_64 | `x86_64-unknown-linux-musl` | `cross` (Docker) |

### Сборка через `cross`

```sh
cargo install cross --git https://github.com/cross-rs/cross

cross build --release --target aarch64-unknown-linux-musl
cross build --release --target armv7-unknown-linux-musleabihf
cross build --release --target x86_64-unknown-linux-musl
```

### Сборка `mipsel` локально

```sh
wget -qO- https://musl.cc/mipsel-linux-musl-cross.tgz | sudo tar xzf - -C /usr/local/ --strip-components=1
rustup target add mipsel-unknown-linux-musl
cargo build --release --target mipsel-unknown-linux-musl
```

Линкер `mipsel-linux-musl-gcc` настроен в `.cargo/config.toml`.

### Вспомогательный скрипт

```sh
./scripts/build-release.sh aarch64-unknown-linux-musl
```

## Упаковка IPK

### 1) Скачать pre-built TrustTunnelClient

```sh
./scripts/download-client.sh v0.99.105 linux-aarch64 client_bin
```

### 2) Собрать wrapper

```sh
cross build --release --target aarch64-unknown-linux-musl
```

### 3) Собрать `.ipk`

```sh
./scripts/package-ipk.sh aarch64 1.0.0 \
  target/aarch64-unknown-linux-musl/release/trusttunnel-keenetic \
  client_bin
```

Пример результата: `trusttunnel-keenetic_1.0.0_aarch64-3.10.ipk`

## CI/CD

Workflow: `.github/workflows/build.yml`

- запуск по тегам `v*` (релизная сборка)
- ручной запуск через GitHub Actions
- сборка всех поддерживаемых архитектур и публикация артефактов с checksum

## Разработка

### Тесты

```sh
cargo test
```

### Проверка конфигурации

```sh
cargo run -- --test --config package/etc/trusttunnel/config.json
```

### Локальный запуск

```sh
cp package/etc/trusttunnel/config.json /tmp/tt-test.json
cargo run -- --foreground --config /tmp/tt-test.json
```

Web UI будет доступен на `http://127.0.0.1:8080`.

### Линт и форматирование

```sh
cargo clippy --all-targets
cargo fmt --check
cargo fmt
```

## Скрипт установки/обновления на роутере

```sh
curl -fsSL https://raw.githubusercontent.com/jounts/TrustTunnelWrap/main/scripts/install.sh | sh
```

Альтернатива:

```sh
wget -O /tmp/install-trusttunnel.sh https://raw.githubusercontent.com/jounts/TrustTunnelWrap/main/scripts/install.sh
sh /tmp/install-trusttunnel.sh
```
