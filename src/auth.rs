use md5::{Digest as Md5Digest, Md5};
#[cfg(unix)]
use nix::ifaddrs::getifaddrs;
use sha2::Sha256;
#[cfg(unix)]
use std::net::Ipv4Addr;

const BRIDGE_INTERFACES: &[&str] = &["br0", "br-lan"];
const FALLBACK_HOST: &str = "192.168.1.1";

/// Auto-detect the router's LAN IP from the bridge interface.
pub fn detect_ndm_host() -> String {
    #[cfg(unix)]
    {
        if let Ok(addrs) = getifaddrs() {
            for ifaddr in addrs {
                if !BRIDGE_INTERFACES.contains(&ifaddr.interface_name.as_str()) {
                    continue;
                }
                if let Some(addr) = ifaddr.address {
                    if let Some(sin) = addr.as_sockaddr_in() {
                        let ip = Ipv4Addr::from(sin.ip());
                        if !ip.is_loopback() && !ip.is_unspecified() {
                            log::info!(
                                "NDM auth: detected LAN IP {} on {}",
                                ip,
                                ifaddr.interface_name
                            );
                            return ip.to_string();
                        }
                    }
                }
            }
        }
    }

    #[cfg(not(unix))]
    {
        let _ = BRIDGE_INTERFACES;
    }
    log::warn!("NDM auth: could not detect LAN IP, using {}", FALLBACK_HOST);
    FALLBACK_HOST.to_string()
}

/// Keenetic NDM API challenge-response authentication.
///
/// Flow:
///   1. GET /auth  -> 401 with X-NDM-Realm + X-NDM-Challenge headers
///   2. Compute: md5_hex = hex(MD5(login:realm:password))
///   3. Compute: response = hex(SHA256(challenge + md5_hex))
///   4. POST /auth with JSON {"login": "<login>", "password": "<response>"}
///   5. Success -> 200 with Set-Cookie: ndm_session=...
pub fn authenticate(router_host: &str, router_port: u16, login: &str, password: &str) -> bool {
    let base_url = format!("http://{}:{}", router_host, router_port);

    let (realm, challenge, cookie) = match fetch_challenge(&base_url) {
        Some(v) => v,
        None => {
            log::error!("NDM auth: failed to fetch challenge from {}", base_url);
            return false;
        }
    };

    let response = compute_response(login, &realm, password, &challenge);
    submit_auth(&base_url, login, &response, &cookie)
}

fn fetch_challenge(base_url: &str) -> Option<(String, String, String)> {
    let url = format!("{}/auth", base_url);

    let resp = match ureq::get(&url).call() {
        Ok(r) => r,
        Err(ureq::Error::Status(401, r)) => r,
        Err(e) => {
            log::error!("NDM auth GET /auth failed: {}", e);
            return None;
        }
    };

    let realm = resp.header("X-NDM-Realm")?.to_string();
    let challenge = resp.header("X-NDM-Challenge")?.to_string();

    let cookie = resp
        .header("Set-Cookie")
        .unwrap_or("")
        .split(';')
        .next()
        .unwrap_or("")
        .to_string();

    if realm.is_empty() || challenge.is_empty() {
        log::error!("NDM auth: empty realm or challenge");
        return None;
    }

    Some((realm, challenge, cookie))
}

fn compute_response(login: &str, realm: &str, password: &str, challenge: &str) -> String {
    // MD5(login:realm:password)
    let md5_input = format!("{}:{}:{}", login, realm, password);
    let md5_hash = Md5::digest(md5_input.as_bytes());
    let md5_hex = hex::encode(md5_hash);

    // SHA256(challenge + md5_hex)
    let sha_input = format!("{}{}", challenge, md5_hex);
    let sha_hash = Sha256::digest(sha_input.as_bytes());
    hex::encode(sha_hash)
}

fn submit_auth(base_url: &str, login: &str, response: &str, cookie: &str) -> bool {
    let url = format!("{}/auth", base_url);
    let body = serde_json::json!({
        "login": login,
        "password": response
    });

    let mut req = ureq::post(&url).set("Content-Type", "application/json");
    if !cookie.is_empty() {
        req = req.set("Cookie", cookie);
    }

    match req.send_string(&body.to_string()) {
        Ok(resp) => {
            let code = resp.status();
            if code == 200 || code == 302 {
                log::info!("NDM auth: login successful for '{}'", login);
                true
            } else {
                log::warn!("NDM auth: unexpected status {}", code);
                false
            }
        }
        Err(ureq::Error::Status(code, _)) => {
            log::warn!("NDM auth: login failed with status {}", code);
            false
        }
        Err(e) => {
            log::error!("NDM auth: POST /auth failed: {}", e);
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_response() {
        let resp = compute_response("admin", "Keenetic", "12345", "abc123");
        // MD5("admin:Keenetic:12345") then SHA256(challenge + md5_hex)
        assert!(!resp.is_empty());
        assert_eq!(resp.len(), 64); // SHA256 hex = 64 chars
    }
}
