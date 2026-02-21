use md5::{Digest as Md5Digest, Md5};
use sha2::Sha256;

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

    // Step 1: fetch challenge
    let (realm, challenge) = match fetch_challenge(&base_url) {
        Some(v) => v,
        None => {
            log::error!("NDM auth: failed to fetch challenge from {}", base_url);
            return false;
        }
    };

    // Step 2-3: compute response
    let response = compute_response(login, &realm, password, &challenge);

    // Step 4: submit
    submit_auth(&base_url, login, &response)
}

fn fetch_challenge(base_url: &str) -> Option<(String, String)> {
    let url = format!("{}/auth", base_url);

    // We expect a 401 response with the challenge headers
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

    if realm.is_empty() || challenge.is_empty() {
        log::error!("NDM auth: empty realm or challenge");
        return None;
    }

    Some((realm, challenge))
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

fn submit_auth(base_url: &str, login: &str, response: &str) -> bool {
    let url = format!("{}/auth", base_url);
    let body = serde_json::json!({
        "login": login,
        "password": response
    });

    match ureq::post(&url)
        .set("Content-Type", "application/json")
        .send_string(&body.to_string())
    {
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
