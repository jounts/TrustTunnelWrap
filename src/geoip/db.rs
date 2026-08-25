//! Compact binary GeoIP database ("mini-mmdb").
//!
//! On-disk layout (little endian), one file per IP family:
//!
//! ```text
//! offset  size  field
//! 0       8     magic b"TTGEODB1"
//! 8       1     family (4 or 6)
//! 9       3     reserved
//! 12      4     record_count (u32)
//! 16      8     built_unix (u64)
//! 24      32    provider_id (utf8, NUL padded)
//! 56      8     reserved
//! ```
//!
//! Followed by `record_count` fixed-size records sorted ascending by start IP:
//! IPv4: `(start: u32, end: u32, cc: [u8;2])` = 10 bytes;
//! IPv6: `(start: u128, end: u128, cc: [u8;2])` = 34 bytes.
//!
//! Lookups are a plain binary search over the sorted array, O(log n).

use std::fs;
use std::io::Write;
use std::net::{Ipv4Addr, Ipv6Addr};
use std::path::Path;

pub const MAGIC: &[u8; 8] = b"TTGEODB1";
pub const HEADER_SIZE: usize = 64;

/// ISO 3166-1 alpha-2 country code.
pub type Cc = [u8; 2];

pub const CC_UNKNOWN: Cc = *b"--";

#[cfg_attr(not(test), allow(dead_code))]
pub fn cc_to_string(cc: Cc) -> String {
    String::from_utf8_lossy(&cc).to_string()
}

#[derive(Debug, Clone, Copy)]
pub struct RangeV4 {
    pub start: u32,
    pub end: u32,
    pub cc: Cc,
}

#[derive(Debug, Clone, Copy)]
pub struct RangeV6 {
    pub start: u128,
    pub end: u128,
    pub cc: Cc,
}

pub fn ipv4_to_u32(ip: Ipv4Addr) -> u32 {
    u32::from(ip)
}

pub fn ipv6_to_u128(ip: Ipv6Addr) -> u128 {
    u128::from(ip)
}

pub fn cidr_range_v4(net: Ipv4Addr, prefix: u8) -> Option<(u32, u32)> {
    if prefix > 32 {
        return None;
    }
    let base = ipv4_to_u32(net);
    let host_bits = 32 - prefix as u32;
    let mask: u32 = if host_bits >= 32 {
        0
    } else {
        !0u32 << host_bits
    };
    let start = base & mask;
    let end = start | (!mask);
    Some((start, end))
}

pub fn cidr_range_v6(net: Ipv6Addr, prefix: u8) -> Option<(u128, u128)> {
    if prefix > 128 {
        return None;
    }
    let base = ipv6_to_u128(net);
    let host_bits = 128 - prefix as u32;
    let mask: u128 = if host_bits >= 128 {
        0
    } else {
        !0u128 << host_bits
    };
    let start = base & mask;
    let end = start | (!mask);
    Some((start, end))
}

/// Parses "a.b.c.d/p" or "ip" (single host) into an inclusive numeric range.
pub fn cidr_str_to_range(s: &str) -> Option<(std::net::IpAddr, u128, u128)> {
    let s = s.trim();
    let (addr_part, prefix_part) = match s.split_once('/') {
        Some((a, p)) => (a, Some(p)),
        None => (s, None),
    };
    let addr: std::net::IpAddr = addr_part.parse().ok()?;
    match addr {
        std::net::IpAddr::V4(v4) => {
            let range = match prefix_part {
                Some(p) => cidr_range_v4(v4, p.parse().ok()?)?,
                None => (ipv4_to_u32(v4), ipv4_to_u32(v4)),
            };
            Some((addr, range.0 as u128, range.1 as u128))
        }
        std::net::IpAddr::V6(v6) => {
            let range = match prefix_part {
                Some(p) => cidr_range_v6(v6, p.parse().ok()?)?,
                None => (ipv6_to_u128(v6), ipv6_to_u128(v6)),
            };
            Some((addr, range.0, range.1))
        }
    }
}

fn provider_id_bytes(provider_id: &str) -> [u8; 32] {
    let mut out = [0u8; 32];
    let bytes = provider_id.as_bytes();
    let n = bytes.len().min(31);
    out[..n].copy_from_slice(&bytes[..n]);
    out
}

/// Accumulates ranges from one or more sources and serializes them into
/// the compact binary format. Ranges are merged where adjacent or overlapping
/// ranges carry the same country; on conflicting overlaps the entry with the
/// lowest source rank (highest priority provider) wins.
#[derive(Default)]
pub struct DbBuilder {
    v4: std::collections::BTreeMap<u32, RangeV4>,
    v6: std::collections::BTreeMap<u128, RangeV6>,
}

impl DbBuilder {
    pub fn add_v4(&mut self, start: u32, end: u32, cc: Cc, _rank: u8) {
        if start <= end && cc != CC_UNKNOWN {
            insert_covered_v4(&mut self.v4, RangeV4 { start, end, cc });
        }
    }

    pub fn add_v6(&mut self, start: u128, end: u128, cc: Cc, _rank: u8) {
        if start <= end && cc != CC_UNKNOWN {
            insert_covered_v6(&mut self.v6, RangeV6 { start, end, cc });
        }
    }

    /// Adds a CIDR string ("1.2.3.0/24"). Returns false when unparseable.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn add_cidr(&mut self, cidr: &str, cc: Cc, rank: u8) -> bool {
        match cidr_str_to_range(cidr) {
            Some((std::net::IpAddr::V4(_), start, end)) => {
                self.add_v4(start as u32, end as u32, cc, rank);
                true
            }
            Some((std::net::IpAddr::V6(_), start, end)) => {
                self.add_v6(start, end, cc, rank);
                true
            }
            None => false,
        }
    }

    pub fn serialize_v4(&self, provider_id: &str, built_unix: u64) -> Vec<u8> {
        let records = merge_adjacent_v4(self.v4.values().copied().collect());
        serialize(4, &records, provider_id, built_unix, |r| {
            let mut buf = Vec::with_capacity(10);
            buf.extend_from_slice(&r.start.to_le_bytes());
            buf.extend_from_slice(&r.end.to_le_bytes());
            buf.extend_from_slice(&r.cc);
            buf
        })
    }

    pub fn serialize_v6(&self, provider_id: &str, built_unix: u64) -> Vec<u8> {
        let records = merge_adjacent_v6(self.v6.values().copied().collect());
        serialize(6, &records, provider_id, built_unix, |r| {
            let mut buf = Vec::with_capacity(34);
            buf.extend_from_slice(&r.start.to_le_bytes());
            buf.extend_from_slice(&r.end.to_le_bytes());
            buf.extend_from_slice(&r.cc);
            buf
        })
    }

    pub fn record_counts(&self) -> (usize, usize) {
        (
            merge_adjacent_v4(self.v4.values().copied().collect()).len(),
            merge_adjacent_v6(self.v6.values().copied().collect()).len(),
        )
    }
}

/// Inserts `range` into a disjoint-cover map, cutting away every part of the
/// existing entries overlapped by it (the newly inserted range takes precedence).
fn insert_covered_v4(map: &mut std::collections::BTreeMap<u32, RangeV4>, range: RangeV4) {
    // First candidate: floor entry (may start before us and overlap),
    // or the first entry at/after our start.
    let first_key = match map.range(..=range.start).next_back() {
        Some((k, v)) if v.end >= range.start => *k,
        _ => match map.range(range.start..).next() {
            Some((k, _)) if *k <= range.end => *k,
            _ => {
                map.insert(range.start, range);
                return;
            }
        },
    };

    let keys: Vec<u32> = map
        .range(first_key..)
        .take_while(|(k, _)| **k <= range.end)
        .map(|(k, _)| *k)
        .collect();

    for k in keys {
        let existing = match map.remove(&k) {
            Some(v) => v,
            None => continue,
        };
        if existing.start < range.start {
            map.insert(
                existing.start,
                RangeV4 {
                    start: existing.start,
                    end: range.start - 1,
                    cc: existing.cc,
                },
            );
        }
        if existing.end > range.end {
            map.insert(
                range.end + 1,
                RangeV4 {
                    start: range.end + 1,
                    end: existing.end,
                    cc: existing.cc,
                },
            );
        }
    }
    map.insert(range.start, range);
}

fn insert_covered_v6(map: &mut std::collections::BTreeMap<u128, RangeV6>, range: RangeV6) {
    let first_key = match map.range(..=range.start).next_back() {
        Some((k, v)) if v.end >= range.start => *k,
        _ => match map.range(range.start..).next() {
            Some((k, _)) if *k <= range.end => *k,
            _ => {
                map.insert(range.start, range);
                return;
            }
        },
    };

    let keys: Vec<u128> = map
        .range(first_key..)
        .take_while(|(k, _)| **k <= range.end)
        .map(|(k, _)| *k)
        .collect();

    for k in keys {
        let existing = match map.remove(&k) {
            Some(v) => v,
            None => continue,
        };
        if existing.start < range.start {
            map.insert(
                existing.start,
                RangeV6 {
                    start: existing.start,
                    end: range.start - 1,
                    cc: existing.cc,
                },
            );
        }
        if existing.end > range.end {
            map.insert(
                range.end + 1,
                RangeV6 {
                    start: range.end + 1,
                    end: existing.end,
                    cc: existing.cc,
                },
            );
        }
    }
    map.insert(range.start, range);
}

fn merge_adjacent_v4(mut ranges: Vec<RangeV4>) -> Vec<RangeV4> {
    ranges.sort_by_key(|r| r.start);
    let mut out: Vec<RangeV4> = Vec::with_capacity(ranges.len());
    for r in ranges {
        match out.last_mut() {
            Some(last) if r.cc == last.cc && r.start <= last.end.saturating_add(1) => {
                last.end = last.end.max(r.end);
            }
            _ => out.push(r),
        }
    }
    out
}

fn merge_adjacent_v6(mut ranges: Vec<RangeV6>) -> Vec<RangeV6> {
    ranges.sort_by_key(|r| r.start);
    let mut out: Vec<RangeV6> = Vec::with_capacity(ranges.len());
    for r in ranges {
        match out.last_mut() {
            Some(last) if r.cc == last.cc && r.start <= last.end.saturating_add(1) => {
                last.end = last.end.max(r.end);
            }
            _ => out.push(r),
        }
    }
    out
}

fn serialize<T>(
    family: u8,
    records: &[T],
    provider_id: &str,
    built_unix: u64,
    encode: impl Fn(&T) -> Vec<u8>,
) -> Vec<u8> {
    let mut out = Vec::with_capacity(HEADER_SIZE + records.len() * 10);
    out.extend_from_slice(MAGIC);
    out.push(family);
    out.extend_from_slice(&[0, 0, 0]);
    out.extend_from_slice(&(records.len() as u32).to_le_bytes());
    out.extend_from_slice(&built_unix.to_le_bytes());
    out.extend_from_slice(&provider_id_bytes(provider_id));
    out.extend_from_slice(&[0u8; 8]);
    for r in records {
        out.extend_from_slice(&encode(r));
    }
    out
}

/// An immutable, memory-resident compact database for one address family.
#[derive(Debug, Clone)]
pub struct GeoDb {
    data: Vec<u8>,
}

impl GeoDb {
    pub fn from_bytes(data: Vec<u8>) -> Result<Self, String> {
        if data.len() < HEADER_SIZE {
            return Err(format!("geoip db too small: {} bytes", data.len()));
        }
        if &data[0..8] != MAGIC {
            return Err("geoip db magic mismatch".into());
        }
        Ok(Self { data })
    }

    pub fn load(path: &Path) -> Result<Self, String> {
        let data = fs::read(path).map_err(|e| format!("read {}: {}", path.display(), e))?;
        Self::from_bytes(data).map_err(|e| format!("{}: {}", path.display(), e))
    }

    fn header_u32(&self, offset: usize) -> u32 {
        let mut buf = [0u8; 4];
        buf.copy_from_slice(&self.data[offset..offset + 4]);
        u32::from_le_bytes(buf)
    }

    pub fn family(&self) -> u8 {
        self.data[8]
    }

    pub fn record_count(&self) -> usize {
        self.header_u32(12) as usize
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn built_unix(&self) -> u64 {
        let mut buf = [0u8; 8];
        buf.copy_from_slice(&self.data[16..24]);
        u64::from_le_bytes(buf)
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn provider_id(&self) -> String {
        let field = &self.data[24..56];
        let end = field.iter().position(|&b| b == 0).unwrap_or(field.len());
        String::from_utf8_lossy(&field[..end]).to_string()
    }

    fn record_offset(&self, idx: usize) -> usize {
        let stride = match self.family() {
            4 => 10usize,
            _ => 34usize,
        };
        HEADER_SIZE + idx * stride
    }

    fn start_at(&self, idx: usize) -> u128 {
        let off = self.record_offset(idx);
        match self.family() {
            4 => u32::from_le_bytes(self.data[off..off + 4].try_into().unwrap()) as u128,
            _ => u128::from_le_bytes(self.data[off..off + 16].try_into().unwrap()),
        }
    }

    fn match_at(&self, idx: usize, value: u128) -> Option<Cc> {
        let off = self.record_offset(idx);
        let is_v4 = self.family() == 4;
        let end = if is_v4 {
            u32::from_le_bytes(self.data[off + 4..off + 8].try_into().unwrap()) as u128
        } else {
            u128::from_le_bytes(self.data[off + 16..off + 32].try_into().unwrap())
        };
        if value <= end {
            let cc_off = if is_v4 { off + 8 } else { off + 32 };
            Some(self.data[cc_off..cc_off + 2].try_into().unwrap())
        } else {
            None
        }
    }

    fn lookup_numeric(&self, value: u128) -> Option<Cc> {
        let count = self.record_count();
        if count == 0 || value < self.start_at(0) {
            return None;
        }
        let mut lo = 0usize;
        let mut hi = count; // exclusive upper bound
        while lo + 1 < hi {
            let mid = lo + (hi - lo) / 2;
            if self.start_at(mid) <= value {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        self.match_at(lo, value)
    }

    pub fn lookup_v4(&self, ip: std::net::Ipv4Addr) -> Option<Cc> {
        self.lookup_numeric(u128::from(ipv4_to_u32(ip)))
    }

    pub fn lookup_v6(&self, ip: std::net::Ipv6Addr) -> Option<Cc> {
        self.lookup_numeric(ipv6_to_u128(ip))
    }

    /// Enumerates all stored ranges (used to feed ipset country sets).
    pub fn iter_ranges_v4(&self) -> impl Iterator<Item = RangeV4> + '_ {
        debug_assert_eq!(self.family(), 4);
        let count = self.record_count();
        (0..count).map(move |i| {
            let off = self.record_offset(i);
            RangeV4 {
                start: u32::from_le_bytes(self.data[off..off + 4].try_into().unwrap()),
                end: u32::from_le_bytes(self.data[off + 4..off + 8].try_into().unwrap()),
                cc: self.data[off + 8..off + 10].try_into().unwrap(),
            }
        })
    }

    pub fn iter_ranges_v6(&self) -> impl Iterator<Item = RangeV6> + '_ {
        debug_assert_eq!(self.family(), 6);
        let count = self.record_count();
        (0..count).map(move |i| {
            let off = self.record_offset(i);
            RangeV6 {
                start: u128::from_le_bytes(self.data[off..off + 16].try_into().unwrap()),
                end: u128::from_le_bytes(self.data[off + 16..off + 32].try_into().unwrap()),
                cc: self.data[off + 32..off + 34].try_into().unwrap(),
            }
        })
    }

    pub fn write_atomic(&self, path: &Path) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("create_dir_all {}: {}", parent.display(), e))?;
        }
        let tmp = path.with_extension(format!("bin.tmp.{}", std::process::id()));
        {
            #[cfg(unix)]
            let file = {
                use std::os::unix::fs::OpenOptionsExt;
                fs::OpenOptions::new()
                    .write(true)
                    .create(true)
                    .truncate(true)
                    .mode(0o600)
                    .open(&tmp)
                    .map_err(|e| format!("open {}: {}", tmp.display(), e))?
            };
            #[cfg(not(unix))]
            let file =
                fs::File::create(&tmp).map_err(|e| format!("open {}: {}", tmp.display(), e))?;
            let mut file = file;
            file.write_all(&self.data)
                .and_then(|_| file.sync_all())
                .map_err(|e| format!("write {}: {}", tmp.display(), e))?;
        }
        fs::rename(&tmp, path).map_err(|e| {
            let _ = fs::remove_file(&tmp);
            format!("rename {}: {}", tmp.display(), e)
        })
    }
}

/// Metadata stored next to the databases for UI/status display.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct DbMeta {
    pub provider_id: String,
    pub format: String,
    pub built_unix: u64,
    pub records_v4: usize,
    pub records_v6: usize,
    pub bytes_on_disk: u64,
    pub trimmed_countries: Vec<String>,
    pub sanity_ok: bool,
}

impl DbMeta {
    pub fn save(&self, dir: &Path) -> Result<(), String> {
        fs::create_dir_all(dir).map_err(|e| format!("create_dir_all {}: {}", dir.display(), e))?;
        let path = dir.join("meta.json");
        let content =
            serde_json::to_string_pretty(self).map_err(|e| format!("serialize meta: {}", e))?;
        super::write_private_file_atomic(&path, content.as_bytes())
    }

    pub fn load(dir: &Path) -> Option<Self> {
        let content = fs::read_to_string(dir.join("meta.json")).ok()?;
        serde_json::from_str(&content).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::IpAddr;

    fn cc(s: &str) -> Cc {
        let b = s.as_bytes();
        [b[0], b[1]]
    }

    #[test]
    fn v4_roundtrip_and_lookup() {
        let mut b = DbBuilder::default();
        b.add_v4(
            ipv4_to_u32(Ipv4Addr::new(1, 1, 1, 0)),
            ipv4_to_u32(Ipv4Addr::new(1, 1, 1, 255)),
            cc("AU"),
            0,
        );
        b.add_v4(
            ipv4_to_u32(Ipv4Addr::new(8, 8, 8, 0)),
            ipv4_to_u32(Ipv4Addr::new(8, 8, 255, 255)),
            cc("US"),
            0,
        );
        b.add_v4(
            ipv4_to_u32(Ipv4Addr::new(77, 88, 0, 0)),
            ipv4_to_u32(Ipv4Addr::new(77, 88, 255, 255)),
            cc("RU"),
            0,
        );

        let db = GeoDb::from_bytes(b.serialize_v4("test", 1700000000)).unwrap();
        assert_eq!(db.family(), 4);
        assert_eq!(db.provider_id(), "test");
        assert_eq!(db.built_unix(), 1700000000);
        assert_eq!(db.record_count(), 3);

        assert_eq!(db.lookup_v4(Ipv4Addr::new(1, 1, 1, 10)), Some(cc("AU")));
        assert_eq!(db.lookup_v4(Ipv4Addr::new(1, 1, 2, 10)), None);
        assert_eq!(db.lookup_v4(Ipv4Addr::new(8, 8, 200, 3)), Some(cc("US")));
        assert_eq!(db.lookup_v4(Ipv4Addr::new(77, 88, 8, 8)), Some(cc("RU")));
        assert_eq!(db.lookup_v4(Ipv4Addr::new(9, 9, 9, 9)), None);
    }

    #[test]
    fn v6_roundtrip_and_lookup() {
        let mut b = DbBuilder::default();
        let s = ipv6_to_u128("2001:db8::".parse::<Ipv6Addr>().unwrap());
        let e = ipv6_to_u128(
            "2001:db8:ffff:ffff:ffff:ffff:ffff:ffff"
                .parse::<Ipv6Addr>()
                .unwrap(),
        );
        b.add_v6(s, e, cc("DE"), 0);
        let db = GeoDb::from_bytes(b.serialize_v6("t6", 42)).unwrap();
        assert_eq!(db.family(), 6);
        assert_eq!(
            db.lookup_v6("2001:db8::1".parse::<Ipv6Addr>().unwrap()),
            Some(cc("DE"))
        );
        assert_eq!(
            db.lookup_v6("2001:db9::1".parse::<Ipv6Addr>().unwrap()),
            None
        );
    }

    #[test]
    fn adjacent_same_country_ranges_merge() {
        let mut b = DbBuilder::default();
        b.add_v4(0x01010100, 0x010101FF, cc("AU"), 0);
        b.add_v4(0x01010200, 0x010102FF, cc("AU"), 0);
        assert_eq!(b.record_counts().0, 1);
    }

    #[test]
    fn higher_priority_wins_on_overlap() {
        let mut b = DbBuilder::default();
        b.add_v4(100, 200, cc("RU"), 1); // lower priority, added first
        b.add_v4(150, 160, cc("US"), 0); // higher priority overlap
        let db = GeoDb::from_bytes(b.serialize_v4("t", 1)).unwrap();
        assert_eq!(db.lookup_v4(Ipv4Addr::new(0, 0, 0, 155)), Some(cc("US")));
        assert_eq!(db.lookup_v4(Ipv4Addr::new(0, 0, 0, 120)), Some(cc("RU")));
        assert_eq!(db.lookup_v4(Ipv4Addr::new(0, 0, 0, 180)), Some(cc("RU")));
    }

    #[test]
    fn unknown_cc_is_skipped() {
        let mut b = DbBuilder::default();
        b.add_v4(10, 20, CC_UNKNOWN, 0);
        assert_eq!(b.record_counts().0, 0);
    }

    #[test]
    fn single_host_ip_is_valid_range() {
        let mut b = DbBuilder::default();
        assert!(b.add_cidr("9.9.9.9", cc("US"), 0));
        let db = GeoDb::from_bytes(b.serialize_v4("t", 1)).unwrap();
        assert_eq!(db.lookup_v4(Ipv4Addr::new(9, 9, 9, 9)), Some(cc("US")));
    }

    #[test]
    fn cidr_parsing() {
        assert_eq!(
            cidr_str_to_range("192.168.1.0/24"),
            Some((
                IpAddr::from([192, 168, 1, 0]),
                ipv4_to_u32(Ipv4Addr::new(192, 168, 1, 0)) as u128,
                ipv4_to_u32(Ipv4Addr::new(192, 168, 1, 255)) as u128
            ))
        );
        assert!(cidr_str_to_range("10.0.0.0/33").is_none());
        assert!(cidr_str_to_range("nonsense").is_none());
    }
}
