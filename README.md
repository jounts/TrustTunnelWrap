# TrustTunnel Keenetic Wrapper

`trusttunnel-keenetic` is an Entware package and Rust wrapper for running [TrustTunnel VPN](https://github.com/TrustTunnel/TrustTunnelClient) on Keenetic and Netcraze routers.

It provides a local Web UI, configuration persistence, process supervision, and routing/watchdog integration around `trusttunnel_client`.

English docs index: [`docs/README.md`](docs/README.md)  
Russian docs index: [`docs/README_RU.md`](docs/README_RU.md)

## Install

### Automatic install/update (recommended)

```sh
curl -fsSL https://raw.githubusercontent.com/jounts/TrustTunnelWrap/main/scripts/install.sh | sh
```

If `curl` is unavailable:

```sh
wget -O /tmp/install-trusttunnel.sh https://raw.githubusercontent.com/jounts/TrustTunnelWrap/main/scripts/install.sh
sh /tmp/install-trusttunnel.sh
```

### Manual install

1. Download the `.ipk` for your architecture from GitHub Releases.
2. Copy it to the router and install:

```sh
scp -O trusttunnel-keenetic_<version>_<arch>.ipk root@192.168.1.1:/tmp/trusttunnel.ipk
ssh root@192.168.1.1
opkg install /tmp/trusttunnel.ipk
```

## Run

Start and stop via init script:

```sh
/opt/etc/init.d/S50trusttunnel start
/opt/etc/init.d/S50trusttunnel stop
/opt/etc/init.d/S50trusttunnel restart
```

Web UI default URL:

```text
http://<router-ip>:8080
```

## Remove

```sh
opkg remove trusttunnel-keenetic
```

The package keeps configuration files in `/opt/etc/trusttunnel/` so they can be reused after reinstallation.
