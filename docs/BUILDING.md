# Build and Development

Russian version: [`BUILDING_RU.md`](BUILDING_RU.md)

## Dependencies

- [Rust](https://www.rust-lang.org/tools/install) >= 1.75
- [cross](https://github.com/cross-rs/cross) for aarch64/armv7/x86_64 cross-builds
- Docker (required by `cross`)
- [musl.cc](https://musl.cc) toolchain for local `mipsel` builds
- `tar`, `gzip` for IPK packaging
- `curl` or `wget` for downloading pre-built client/toolchain

## Quick Start (host machine)

```sh
git clone https://github.com/jounts/TrustTunnelWrap.git
cd trusttunnelwrap
cargo build
cargo run -- --foreground --config package/etc/trusttunnel/config.json
```

## Cross Compilation

### Supported targets

| Router arch | Rust target triple | Build method |
|-------------|--------------------|--------------|
| aarch64 | `aarch64-unknown-linux-musl` | `cross` (Docker) |
| mipsel | `mipsel-unknown-linux-musl` | local `cargo` + musl.cc |
| armv7 | `armv7-unknown-linux-musleabihf` | `cross` (Docker) |
| x86_64 | `x86_64-unknown-linux-musl` | `cross` (Docker) |

### Build with `cross` (aarch64, armv7, x86_64)

```sh
cargo install cross --git https://github.com/cross-rs/cross

cross build --release --target aarch64-unknown-linux-musl
cross build --release --target armv7-unknown-linux-musleabihf
cross build --release --target x86_64-unknown-linux-musl
```

### Build `mipsel` locally

```sh
wget -qO- https://musl.cc/mipsel-linux-musl-cross.tgz | sudo tar xzf - -C /usr/local/ --strip-components=1
rustup target add mipsel-unknown-linux-musl
cargo build --release --target mipsel-unknown-linux-musl
```

The linker `mipsel-linux-musl-gcc` is configured in `.cargo/config.toml`.

### Helper script

```sh
./scripts/build-release.sh aarch64-unknown-linux-musl
```

## Binary Size Optimization

Release profile in `Cargo.toml` enables size-oriented optimization (`opt-level = "z"`, LTO, strip, `panic = "abort"`).

## IPK Packaging

### 1) Download pre-built TrustTunnelClient

```sh
./scripts/download-client.sh v1.0.23 linux-aarch64 client_bin
```

### 2) Build wrapper binary

```sh
cross build --release --target aarch64-unknown-linux-musl
```

### 3) Build `.ipk`

```sh
./scripts/package-ipk.sh aarch64 1.0.0 \
  target/aarch64-unknown-linux-musl/release/trusttunnel-keenetic \
  client_bin
```

Result example: `trusttunnel-keenetic_1.0.0_aarch64-3.10.ipk`

## CI/CD

Workflow: `.github/workflows/build.yml`

- Runs on `v*` tags for automatic release builds
- Can be started manually from GitHub Actions
- Builds all supported architectures and publishes artifacts with checksums

## Development

### Tests

```sh
cargo test
```

### Config validation

```sh
cargo run -- --test --config package/etc/trusttunnel/config.json
```

### Local run

```sh
cp package/etc/trusttunnel/config.json /tmp/tt-test.json
cargo run -- --foreground --config /tmp/tt-test.json
```

Web UI is available at `http://127.0.0.1:8080`.

### Lint and format

```sh
cargo clippy --all-targets
cargo fmt --check
cargo fmt
```

## Router Install/Update Script

Use `scripts/install.sh` on the router:

```sh
curl -fsSL https://raw.githubusercontent.com/jounts/TrustTunnelWrap/main/scripts/install.sh | sh
```

Fallback with `wget`:

```sh
wget -O /tmp/install-trusttunnel.sh https://raw.githubusercontent.com/jounts/TrustTunnelWrap/main/scripts/install.sh
sh /tmp/install-trusttunnel.sh
```
