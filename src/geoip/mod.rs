//! GeoIP subsystem: local compact databases, API providers, hybrid lookup
//! and the rebuild/update pipeline.

pub mod api;
pub mod db;
pub mod download;
pub mod parsers;

use crate::config::{GeoIpSettings, SplitTunnelSettings};
use db::{Cc, DbBuilder, DbMeta, GeoDb};
use std::collections::HashMap;
use std::net::IpAddr;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// Writes binary content atomically (tmp file + rename), mode 0600 on unix.
pub(crate) fn write_private_file_atomic(path: &Path, content: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("create_dir_all {}: {}", parent.display(), e))?;
    }
    let tmp = path.with_extension(format!("tmp.{}", std::process::id()));
    {
        #[cfg(unix)]
        use std::io::Write as _;
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            let mut f = std::fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .mode(0o600)
                .open(&tmp)
                .map_err(|e| format!("open {}: {}", tmp.display(), e))?;
            f.write_all(content)
                .and_then(|_| f.sync_all())
                .map_err(|e| format!("write {}: {}", tmp.display(), e))?;
        }
        #[cfg(not(unix))]
        std::fs::write(&tmp, content).map_err(|e| format!("write {}: {}", tmp.display(), e))?;
    }
    std::fs::rename(&tmp, path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        format!("rename {}: {}", tmp.display(), e)
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Local,
    Api,
    Hybrid,
}

impl Mode {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "local" => Some(Mode::Local),
            "api" => Some(Mode::Api),
            "hybrid" => Some(Mode::Hybrid),
            _ => None,
        }
    }
}

/// Result of a lookup with the source that produced it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LookupAnswer {
    pub cc: Cc,
    /// "local:<provider_id>" | "cache" | "api:<provider_id>"
    pub source: &'static str,
}

const CACHE_CAPACITY: usize = 4096;

struct TtlCache {
    entries: HashMap<IpAddr, (Cc, Instant)>,
    order: VecDeque<IpAddr>,
    ttl: Duration,
}

use std::collections::VecDeque;

impl TtlCache {
    fn new(ttl_hours: u64) -> Self {
        Self {
            entries: HashMap::new(),
            order: VecDeque::new(),
            ttl: Duration::from_secs(ttl_hours.max(1) * 3600),
        }
    }

    fn get(&mut self, ip: IpAddr) -> Option<Cc> {
        let now = Instant::now();
        let (cc, expires) = *self.entries.get(&ip)?;
        if now >= expires {
            self.entries.remove(&ip);
            return None;
        }
        Some(cc)
    }

    fn put(&mut self, ip: IpAddr, cc: Cc) {
        if !self.entries.contains_key(&ip) && self.entries.len() >= CACHE_CAPACITY {
            while let Some(oldest) = self.order.pop_front() {
                if self.entries.remove(&oldest).is_some() {
                    break;
                }
            }
        }
        self.order.push_back(ip);
        self.entries.insert(ip, (cc, Instant::now() + self.ttl));
    }
}

/// Runtime geo lookup service shared across the app.
pub struct GeoService {
    dir: PathBuf,
    mode: Mode,
    local_v4: Option<GeoDb>,
    local_v6: Option<GeoDb>,
    meta: Option<DbMeta>,
    apis: Vec<(usize, api::ApiClient)>, // sorted by priority asc
    cache: Mutex<TtlCache>,
}

impl GeoIpSettings {
    /// Whether the lookup pipeline should be constructed at all.
    pub fn enabled_for_lookup(&self) -> bool {
        self.enabled && Mode::parse(&self.mode).is_some()
    }

    /// Single-select semantics: enables exactly one db provider, disables the rest.
    pub fn select_single_db_provider(&mut self, id: &str) -> Result<(), String> {
        if !self.db_providers.iter().any(|p| p.id == id) {
            return Err(format!("unknown provider '{}'", id));
        }
        for p in self.db_providers.iter_mut() {
            p.enabled = p.id == id;
        }
        Ok(())
    }
}

impl GeoService {
    /// A service with lookups fully disabled (geoip off).
    pub fn disabled() -> Self {
        Self {
            dir: PathBuf::from("/tmp"),
            mode: Mode::Local,
            local_v4: None,
            local_v6: None,
            meta: None,
            apis: Vec::new(),
            cache: Mutex::new(TtlCache::new(24)),
        }
    }

    pub fn new(cfg: &GeoIpSettings) -> Self {
        let dir = PathBuf::from(cfg.db_path.trim_end_matches('/'));
        let mut service = Self {
            dir,
            mode: Mode::parse(&cfg.mode).unwrap_or(Mode::Local),
            local_v4: None,
            local_v6: None,
            meta: None,
            apis: Vec::new(),
            cache: Mutex::new(TtlCache::new(cfg.cache_ttl_hours)),
        };
        service.reload_local();
        let mut apis: Vec<(usize, api::ApiClient)> = cfg
            .api_providers
            .iter()
            .filter(|p| p.enabled)
            .map(|p| {
                (
                    p.priority as usize,
                    api::ApiClient::new(&p.id, &p.kind, &p.url, &p.api_key, p.rate_limit_per_min),
                )
            })
            .collect();
        apis.sort_by_key(|(prio, _)| *prio);
        service.apis = apis;
        service
    }

    /// (Re)loads the compact databases from disk.
    pub fn reload_local(&mut self) {
        self.local_v4 = GeoDb::load(&self.dir.join("v4.bin")).ok();
        self.local_v6 = GeoDb::load(&self.dir.join("v6.bin")).ok();
        self.meta = DbMeta::load(&self.dir);
    }

    pub fn is_local_available(&self) -> bool {
        self.local_v4.is_some() || self.local_v6.is_some()
    }

    pub fn status_json(&self) -> serde_json::Value {
        serde_json::json!({
            "available": self.is_local_available(),
            "mode": format!("{:?}", self.mode).to_lowercase(),
            "meta": self.meta,
        })
    }

    pub fn lookup(&self, ip: IpAddr) -> Option<LookupAnswer> {
        // 1. Local database (authoritative when present).
        if matches!(self.mode, Mode::Local | Mode::Hybrid) {
            if let Some(cc) = self.lookup_local(ip) {
                return Some(LookupAnswer {
                    cc,
                    source: "local",
                });
            }
        }
        if matches!(self.mode, Mode::Api | Mode::Hybrid) {
            // 2. TTL cache.
            if let Some(cc) = self.cache.lock().unwrap().get(ip) {
                return Some(LookupAnswer {
                    cc,
                    source: "cache",
                });
            }
            // 3. API chain in priority order.
            for (_, client) in &self.apis {
                match client.lookup(ip) {
                    Ok(cc) => {
                        self.cache.lock().unwrap().put(ip, cc);
                        return Some(LookupAnswer { cc, source: "api" });
                    }
                    Err(e) => log::debug!("[geoip] api '{}' lookup failed: {}", client.id, e),
                }
            }
        }
        None
    }

    fn lookup_local(&self, ip: IpAddr) -> Option<Cc> {
        match ip {
            IpAddr::V4(v4) => self.local_v4.as_ref()?.lookup_v4(v4),
            IpAddr::V6(v6) => self.local_v6.as_ref()?.lookup_v6(v6),
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct UpdateReport {
    pub provider_id: String,
    pub records_v4: usize,
    pub records_v6: usize,
    pub bytes_written: u64,
    pub elapsed_secs: f64,
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Rebuilds `v4.bin` / `v6.bin` / `meta.json` from the enabled DB providers.
///
/// `countries` drives trim-to-selected-countries; an empty list keeps all.
/// The previous database files are left untouched until the new ones are
/// fully validated and atomically renamed into place.
pub fn rebuild_databases(
    cfg: &GeoIpSettings,
    countries: &[String],
) -> Result<Vec<UpdateReport>, String> {
    let started = Instant::now();
    let mut providers: Vec<&crate::config::GeoIpDbProvider> =
        cfg.db_providers.iter().filter(|p| p.enabled).collect();
    providers.sort_by_key(|p| p.priority);

    if providers.is_empty() {
        return Err("no enabled geoip db providers".into());
    }

    let want = if cfg.trim_to_selected_countries && !countries.is_empty() {
        parsers::WantFilter::only(countries)
    } else {
        parsers::WantFilter::all()
    };

    let mut builder = DbBuilder::default();
    let mut reports = Vec::new();
    let mut last_error: Option<String> = None;

    for provider in providers {
        let t0 = Instant::now();
        let etag_path = PathBuf::from(cfg.db_path.trim_end_matches('/')).join("etag.txt");
        let cached_etag = std::fs::read_to_string(&etag_path)
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());

        log::info!("[geoip] fetching provider '{}'", provider.id);
        match download::fetch_if_modified(&provider.url, cached_etag.as_deref()) {
            Ok(fetch) => {
                if fetch.bytes.is_none() {
                    log::info!("[geoip] provider '{}' unchanged (304)", provider.id);
                    continue;
                }
                match process_source(provider, fetch.bytes.unwrap(), &want, cfg) {
                    Ok(parsed) => {
                        for (s, e, cc) in &parsed.v4 {
                            builder.add_v4(*s, *e, *cc, 0);
                        }
                        for (s, e, cc) in &parsed.v6 {
                            builder.add_v6(*s, *e, *cc, 0);
                        }
                        if let Some(tag) = fetch.etag {
                            write_private_file_atomic(&etag_path, tag.as_bytes())?;
                        }
                        reports.push((provider.clone(), parsed, t0.elapsed().as_secs_f64()));
                    }
                    Err(e) => {
                        log::warn!("[geoip] provider '{}' failed: {}", provider.id, e);
                        last_error = Some(e);
                    }
                }
            }
            Err(e) => {
                log::warn!("[geoip] provider '{}' download failed: {}", provider.id, e);
                last_error = Some(e);
            }
        }
    }

    if reports.is_empty() {
        return Err(last_error.unwrap_or_else(|| "all geoip providers failed".into()));
    }

    let built = unix_now();
    let primary = reports[0].0.clone();
    let dir = PathBuf::from(cfg.db_path.trim_end_matches('/'));

    let v4_bytes = builder.serialize_v4(primary.id.as_str(), built);
    let v6_bytes = builder.serialize_v6(primary.id.as_str(), built);

    // Sanity check before swapping anything in.
    let sanity_ok = run_sanity_checks(&builder, countries);
    if !sanity_ok {
        log::warn!("[geoip] sanity checks did not pass; keeping previous database");
        return Err("sanity checks failed on newly built geoip database".into());
    }

    let (records_v4, records_v6) = builder.record_counts();
    if records_v4 + records_v6 == 0 {
        return Err("newly built geoip database contains no records".into());
    }

    let v4_db = GeoDb::from_bytes(v4_bytes)?;
    let v6_db = GeoDb::from_bytes(v6_bytes)?;

    // Atomic swap only after everything validated.
    v4_db.write_atomic(&dir.join("v4.bin"))?;
    v6_db.write_atomic(&dir.join("v6.bin"))?;

    let bytes_on_disk = std::fs::metadata(dir.join("v4.bin"))
        .map(|m| m.len())
        .unwrap_or(0)
        + std::fs::metadata(dir.join("v6.bin"))
            .map(|m| m.len())
            .unwrap_or(0);

    let meta = DbMeta {
        provider_id: primary.id.clone(),
        format: primary.format.clone(),
        built_unix: built,
        records_v4,
        records_v6,
        bytes_on_disk,
        trimmed_countries: if cfg.trim_to_selected_countries {
            countries.to_vec()
        } else {
            Vec::new()
        },
        sanity_ok,
    };
    meta.save(&dir)?;

    log::info!(
        "[geoip] database rebuilt: {} v4 + {} v6 records, {} bytes, from '{}' in {:.1}s",
        records_v4,
        records_v6,
        bytes_on_disk,
        primary.id,
        started.elapsed().as_secs_f64()
    );

    Ok(reports
        .into_iter()
        .map(|(provider, parsed, elapsed)| UpdateReport {
            provider_id: provider.id,
            records_v4: parsed.v4.len(),
            records_v6: parsed.v6.len(),
            bytes_written: bytes_on_disk,
            elapsed_secs: elapsed,
        })
        .collect())
}

fn process_source(
    provider: &crate::config::GeoIpDbProvider,
    raw: Vec<u8>,
    want: &parsers::WantFilter,
    cfg: &GeoIpSettings,
) -> Result<parsers::ParsedRanges, String> {
    let reader = download::decompress_sniff(raw)?;
    // Zone files describe a single country; the two-letter code is taken from
    // the provider id ("ru-zone") or a pre-substituted "{cc}" URL template.
    let zone_country = if provider.format == "zone" {
        Some(extract_country_hint(provider))
    } else {
        None
    };
    let _ = cfg;
    parsers::parse_source(&provider.format, reader, want, zone_country.as_deref())
}

/// Two-letter country code for zone-format providers, taken from a token in
/// the provider id ("ru-zone", "de_list").
fn extract_country_hint(provider: &crate::config::GeoIpDbProvider) -> String {
    provider
        .id
        .split(['-', '_', ' '])
        .find(|tok| tok.len() == 2 && tok.chars().all(|c| c.is_ascii_alphabetic()))
        .unwrap_or("")
        .to_string()
}

/// Known-answer checks against well-known public ranges. Skipped gracefully
/// when the relevant countries were trimmed away.
fn run_sanity_checks(builder: &DbBuilder, countries: &[String]) -> bool {
    let probes: [(&str, &str); 3] = [
        ("8.8.8.8", "US"),
        ("1.1.1.1", "AU"), // APNIC range; may resolve to AU or US depending on source
        ("77.88.8.8", "RU"),
    ];
    let trimmed = !countries.is_empty();
    let mut checked = 0;
    let mut passed = 0;
    let probe_db = GeoDb::from_bytes(builder.serialize_v4("probe", 0));
    if let Ok(db) = probe_db {
        for (ip, expected) in probes {
            if trimmed && !countries.iter().any(|c| c.eq_ignore_ascii_case(expected)) {
                continue;
            }
            let addr = match ip.parse::<std::net::Ipv4Addr>() {
                Ok(a) => a,
                Err(_) => continue,
            };
            checked += 1;
            if let Some(cc) = db.lookup_v4(addr) {
                let got = String::from_utf8_lossy(&cc).to_string();
                let acceptable = got.eq_ignore_ascii_case(expected)
                    || (ip == "1.1.1.1" && got.eq_ignore_ascii_case("US"));
                if acceptable {
                    passed += 1;
                } else {
                    log::warn!(
                        "[geoip] sanity: {} resolved to {}, expected {}",
                        ip,
                        got,
                        expected
                    );
                }
            } else {
                log::warn!("[geoip] sanity: {} not found in new database", ip);
            }
        }
        if checked > 0 && passed == 0 {
            return false;
        }
    }
    true
}

/// Convenience wrapper: rebuild using split-tunnel selected countries.
pub fn rebuild_for_split_tunnel(
    geoip_cfg: &GeoIpSettings,
    st_cfg: &SplitTunnelSettings,
) -> Result<Vec<UpdateReport>, String> {
    let countries = if geoip_cfg.trim_to_selected_countries {
        st_cfg.selected_countries()
    } else {
        Vec::new()
    };
    rebuild_databases(geoip_cfg, &countries)
}

/// True when a built compact database exists on disk.
pub fn is_db_present(geoip_cfg: &GeoIpSettings) -> bool {
    let dir = Path::new(geoip_cfg.db_path.trim_end_matches('/'));
    dir.join("v4.bin").is_file() || dir.join("v6.bin").is_file()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::GeoIpDbProvider;

    fn provider(id: &str, enabled: bool) -> GeoIpDbProvider {
        GeoIpDbProvider {
            id: id.to_string(),
            url: format!("https://example.com/{}.mmdb", id),
            format: "mmdb".into(),
            priority: 1,
            enabled,
        }
    }

    #[test]
    fn single_select_enables_exactly_one_provider() {
        let mut cfg = GeoIpSettings {
            db_providers: vec![
                provider("a", true),
                provider("b", false),
                provider("c", false),
            ],
            ..Default::default()
        };
        cfg.select_single_db_provider("c").unwrap();
        let flags: Vec<bool> = cfg.db_providers.iter().map(|p| p.enabled).collect();
        assert_eq!(flags, vec![false, false, true]);

        // Re-selecting moves the flag.
        cfg.select_single_db_provider("a").unwrap();
        let flags: Vec<bool> = cfg.db_providers.iter().map(|p| p.enabled).collect();
        assert_eq!(flags, vec![true, false, false]);
    }

    #[test]
    fn single_select_rejects_unknown_id() {
        let mut cfg = GeoIpSettings {
            db_providers: vec![provider("a", true)],
            ..Default::default()
        };
        assert!(cfg
            .select_single_db_provider("nope")
            .err()
            .unwrap()
            .contains("unknown provider"));
        // State unchanged on failure.
        assert!(cfg.db_providers[0].enabled);
    }

    #[test]
    fn is_db_present_checks_files_on_disk() {
        let mut cfg = GeoIpSettings::default();
        assert!(!cfg.enabled);
        let base = std::env::temp_dir().join(format!("tt_geoip_test_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        cfg.db_path = base.to_string_lossy().to_string();
        assert!(!is_db_present(&cfg));
        std::fs::create_dir_all(&base).unwrap();
        std::fs::write(base.join("v4.bin"), b"not-a-real-db").unwrap();
        assert!(is_db_present(&cfg));
        let _ = std::fs::remove_dir_all(&base);
    }
}
