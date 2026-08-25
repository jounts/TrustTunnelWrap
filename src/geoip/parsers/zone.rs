//! Per-country CIDR zone lists (one network per line), e.g. the
//! wp-statistics/GeoLite2-Country `*.zone` exports.

use super::{ParsedRanges, WantFilter};
use crate::geoip::db::cidr_str_to_range;

pub fn parse(
    reader: Box<dyn std::io::Read + Send>,
    want: &WantFilter,
    country_code: Option<&str>,
) -> Result<ParsedRanges, String> {
    let cc_raw = country_code.ok_or("zone format requires a country code (set provider 'country_code' field or use per-country URL template)")?;
    let cc_b = cc_raw.as_bytes();
    if cc_b.len() != 2 {
        return Err(format!("invalid country code '{}'", cc_raw));
    }
    let cc: Cc = [cc_b[0].to_ascii_uppercase(), cc_b[1].to_ascii_uppercase()];
    let _ = &cc_raw;

    if !want.accepts(&cc) {
        return Ok(ParsedRanges::default());
    }

    use std::io::BufRead;
    let mut out = ParsedRanges::default();
    for line in std::io::BufReader::new(reader).lines() {
        let line = match line {
            Ok(l) => l,
            Err(e) => return Err(format!("zone read: {}", e)),
        };
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        match cidr_str_to_range(line) {
            Some((std::net::IpAddr::V4(_), s, e)) => out.v4.push((s as u32, e as u32, cc)),
            Some((std::net::IpAddr::V6(_), s, e)) => out.v6.push((s, e, cc)),
            None => log::debug!("[geoip] skipping bad zone line '{}'", line),
        }
    }
    Ok(out)
}

type Cc = [u8; 2];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_zone_lines() {
        let data = "# comment\n\n1.0.0.0/24\n77.88.8.8/32\n2001:db8::/32\nbad-line\n";
        let out = parse(
            Box::new(std::io::Cursor::new(data)),
            &WantFilter::all(),
            Some("RU"),
        )
        .unwrap();
        assert_eq!(out.v4.len(), 2);
        assert_eq!(out.v6.len(), 1);
        assert_eq!(out.v4[0].2, *b"RU");
    }

    #[test]
    fn respects_trim_filter() {
        let data = "1.0.0.0/24\n";
        let out = parse(
            Box::new(std::io::Cursor::new(data)),
            &WantFilter::only(&["US".into()]),
            Some("AU"),
        )
        .unwrap();
        assert!(out.total() == 0);
    }
}
