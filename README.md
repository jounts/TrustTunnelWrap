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

In the tunnel configuration section, choose either **Manual** to enter the full
configuration form or **DeepLink** to import a TrustTunnel `tt://?...` link.
DeepLink import only fills the manual form; review the values and click **Save**
to persist the configuration. Links contain endpoint credentials in encoded but
unencrypted form and must be treated as sensitive data.

The optional **Split Tunneling** tab routes traffic by country (GeoIP) or by
manual domain/IP/CIDR rules either directly or through the VPN. The GeoIP
database is downloaded automatically: three keyless providers are preconfigured
and selecting one fetches its database immediately. See `geoip` and
`split_tunnel` in [`docs/CONFIGURATION.md`](docs/CONFIGURATION.md).

The Web UI intentionally uses plain HTTP and binds to `0.0.0.0` by default. Do not
expose port 8080 outside a trusted LAN: session tokens can be intercepted. Restrict
access with firewall rules when needed.

## Remove

```sh
opkg remove trusttunnel-keenetic
```

The package keeps configuration files in `/opt/etc/trusttunnel/` so they can be reused after reinstallation.
