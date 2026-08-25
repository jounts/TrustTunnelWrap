//! Online IP-to-country API providers (ipapi.com, ip2c.org, generic JSON)
//! with a token-bucket rate limiter per provider.

use crate::geoip::db::Cc;
use std::time::{Duration, Instant};

pub struct ApiClient {
    pub id: String,
    pub kind: String,
    url: String,
    api_key: String,
    rate_limit_per_min: u32,
    rate: std::sync::Mutex<RateBucket>,
}

struct RateBucket {
    tokens: f64,
    last_refill: Instant,
}

impl RateBucket {
    fn new(per_min: u32) -> Self {
        Self {
            tokens: per_min as f64,
            last_refill: Instant::now(),
        }
    }

    /// Returns true when a request slot was consumed.
    fn acquire(&mut self, per_min: u32) -> bool {
        let now = Instant::now();
        let rate = per_min as f64 / 60.0;
        let elapsed = now.duration_since(self.last_refill).as_secs_f64();
        self.tokens = (self.tokens + elapsed * rate).min(per_min as f64);
        self.last_refill = now;
        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            true
        } else {
            false
        }
    }
}

impl ApiClient {
    pub fn new(id: &str, kind: &str, url: &str, api_key: &str, rate_limit_per_min: u32) -> Self {
        Self {
            id: id.to_string(),
            kind: kind.to_string(),
            url: url.trim_end_matches('/').to_string(),
            api_key: api_key.to_string(),
            rate_limit_per_min: rate_limit_per_min.max(1),
            rate: std::sync::Mutex::new(RateBucket::new(rate_limit_per_min.max(1))),
        }
    }

    pub fn lookup(&self, ip: std::net::IpAddr) -> Result<Cc, String> {
        if !self.rate.lock().unwrap().acquire(self.rate_limit_per_min) {
            return Err(format!("api provider '{}' rate limited", self.id));
        }
        match self.kind.as_str() {
            "ip2c" => self.lookup_ip2c(ip),
            "ipapi" => self.lookup_ipapi(ip),
            _ => self.lookup_generic_json(ip),
        }
    }

    fn http_get_text(&self, url: &str) -> Result<String, String> {
        let agent = ureq::AgentBuilder::new()
            .timeout(Duration::from_secs(10))
            .user_agent(concat!("trusttunnel-keenetic/", env!("CARGO_PKG_VERSION")))
            .build();
        let resp = agent
            .get(url)
            .call()
            .map_err(|e| format!("GET {}: {}", url, e))?;
        let mut reader = resp.into_reader();
        let text_bytes =
            read_limited(&mut reader, 256 * 1024).map_err(|e| format!("read {}: {}", url, e))?;
        Ok(String::from_utf8_lossy(&text_bytes).to_string())
    }

    /// ip2c.org plain-text protocol: `<status>;<iso2>;<iso3>;<name>` (status 1 = found).
    fn lookup_ip2c(&self, ip: std::net::IpAddr) -> Result<Cc, String> {
        let text = self.http_get_text(&format!("{}/{}", self.url, ip))?;
        let mut parts = text.trim().split(';');
        let status = parts.next().unwrap_or("0");
        if status != "1" {
            return Err(format!("ip2c: not found ({})", status));
        }
        cc_from_str(parts.next().unwrap_or(""))
    }

    /// ipapi.com JSON: `{"country_code": "RU", ...}`.
    fn lookup_ipapi(&self, ip: std::net::IpAddr) -> Result<Cc, String> {
        let sep = if self.url.contains('?') { '&' } else { '?' };
        let key_param = if self.api_key.is_empty() {
            String::new()
        } else {
            format!("{}access_key={}", sep, self.api_key)
        };
        let body = self.http_get_text(&format!("{}/{}{}", self.url, ip, key_param))?;
        let json: serde_json::Value =
            serde_json::from_str(&body).map_err(|e| format!("ipapi: bad json: {}", e))?;
        if let Some(err) = json.get("error") {
            return Err(format!("ipapi error: {}", err));
        }
        let code = json
            .get("country_code")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        cc_from_str(code)
    }

    /// Configurable endpoint; `{ip}` placeholder is substituted and the first
    /// recognized country field of the JSON response wins.
    fn lookup_generic_json(&self, ip: std::net::IpAddr) -> Result<Cc, String> {
        let url = self.url.replace("{ip}", &ip.to_string());
        let body = self.http_get_text(&url)?;
        let json: serde_json::Value =
            serde_json::from_str(&body).map_err(|e| format!("generic-json: bad json: {}", e))?;
        for key in ["country_code", "countryCode", "country_iso_code", "country"] {
            if let Some(v) = json.get(key).and_then(|v| v.as_str()) {
                return cc_from_str(v);
            }
        }
        Err("generic-json: no country field in response".into())
    }
}

fn cc_from_str(s: &str) -> Result<Cc, String> {
    let v = s.trim();
    let b = v.as_bytes();
    if b.len() == 2 && b.iter().all(|c| c.is_ascii_alphabetic()) && v != "--" {
        Ok([b[0].to_ascii_uppercase(), b[1].to_ascii_uppercase()])
    } else {
        Err(format!("invalid country code '{}'", v))
    }
}

/// Reads at most `max` bytes from a reader; errors when the payload is larger.
pub(crate) fn read_limited(reader: &mut dyn std::io::Read, max: u64) -> std::io::Result<Vec<u8>> {
    let mut out = Vec::new();
    let mut buf = [0u8; 8192];
    loop {
        let n = reader.read(&mut buf)?;
        if n == 0 {
            return Ok(out);
        }
        if out.len() as u64 + n as u64 > max {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("response exceeds {} bytes", max),
            ));
        }
        out.extend_from_slice(&buf[..n]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rate_bucket_refills_over_time() {
        let mut bucket = RateBucket::new(60); // 1 token/sec refill
        assert!(bucket.acquire(60));
        assert!(bucket.acquire(60)); // starts full with 60 tokens
                                     // Drain everything.
        for _ in 0..70 {
            bucket.acquire(60);
        }
        assert!(!bucket.acquire(60));
    }

    #[test]
    fn cc_parsing() {
        assert_eq!(cc_from_str("ru"), Ok(*b"RU"));
        assert!(cc_from_str("").is_err());
        assert!(cc_from_str("--").is_err());
        assert!(cc_from_str("RUS").is_err());
    }
}
