//! CSV parsers for GeoLite2-Country style and IP2Location LITE DB1 exports.
//!
//! Both dialects are auto-detected from the header row; when no header is
//! present, positional columns are assumed:
//! - geolite2-csv: `network,country_iso_code`
//! - ip2location-csv: `ip_from,ip_to,country_code`

use super::{ParsedRanges, WantFilter};
use crate::geoip::db::{cidr_str_to_range, Cc};

fn parse_cc(value: &str) -> Option<Cc> {
    let v = value.trim().trim_matches('"');
    let b = v.as_bytes();
    if b.len() == 2 && b.iter().all(|c| c.is_ascii_alphabetic()) {
        Some([b[0].to_ascii_uppercase(), b[1].to_ascii_uppercase()])
    } else {
        None
    }
}

pub fn parse(
    mut reader: Box<dyn std::io::Read + Send>,
    format: &str,
    want: &WantFilter,
) -> Result<ParsedRanges, String> {
    use std::io::Read as _;
    let mut raw = Vec::new();
    reader
        .read_to_end(&mut raw)
        .map_err(|e| format!("csv read: {}", e))?;

    // Sniff whether the first line is a header: its first cell must be
    // neither a number nor a CIDR/IP.
    let first_line_end = raw.iter().position(|&b| b == b'\n').unwrap_or(raw.len());
    let first_cell = String::from_utf8_lossy(&raw[..first_line_end])
        .split(',')
        .next()
        .unwrap_or("")
        .trim()
        .trim_matches('"')
        .to_string();
    let has_headers = first_cell.parse::<u64>().is_err()
        && crate::geoip::db::cidr_str_to_range(&first_cell).is_none();

    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(has_headers)
        .flexible(true)
        .from_reader(std::io::Cursor::new(raw));

    let empty: [String; 0] = [];
    let headers = if has_headers {
        rdr.headers()
            .map_err(|e| format!("csv header: {}", e))?
            .clone()
    } else {
        csv::StringRecord::from(empty.iter().map(|s| s.as_str()).collect::<Vec<&str>>())
    };
    let names: Vec<String> = headers
        .iter()
        .map(|s| s.trim().to_ascii_lowercase())
        .collect();

    let is_ip2l = names
        .iter()
        .any(|n| n == "ip_from" || n.starts_with("ip_from"));
    let _ = format; // declared format is advisory; detection wins

    let net_col: Option<usize>;
    let from_col: usize;
    let to_col: usize;
    let cc_col: usize;
    if is_ip2l {
        net_col = None;
        from_col = col_of(&names, &["ip_from"]).ok_or("ip2location csv: missing ip_from column")?;
        to_col = col_of(&names, &["ip_to"]).ok_or("ip2location csv: missing ip_to column")?;
        cc_col = col_of(&names, &["country_code", "country"])
            .ok_or("ip2location csv: missing country_code column")?;
    } else {
        net_col = if names.is_empty() {
            // Headerless positional layout: network,country
            Some(0)
        } else {
            Some(
                col_of(&names, &["network", "network_ip", "cidr", "range"])
                    .or(if !names.is_empty() { Some(0) } else { None })
                    .ok_or("geolite2 csv: missing network column")?,
            )
        };
        from_col = 0;
        to_col = 0;
        cc_col = if names.is_empty() {
            1
        } else {
            col_of(
                &names,
                &[
                    "country_iso_code",
                    "registered_country_iso_code",
                    "represented_country_iso_code",
                    "country_code",
                    "country",
                ],
            )
            .unwrap_or(1)
        };
    }

    let mut out = ParsedRanges::default();
    let mut skipped_cc = 0usize;
    for rec in rdr.records() {
        let row = match rec {
            Ok(r) => r,
            Err(e) => {
                log::debug!("[geoip] skipping malformed csv row: {}", e);
                continue;
            }
        };
        let cc_raw = row.get(cc_col).unwrap_or("");
        let cc = match parse_cc(cc_raw) {
            Some(c) => c,
            None => {
                skipped_cc += 1;
                continue;
            }
        };
        if !want.accepts(&cc) {
            continue;
        }
        if is_ip2l {
            let from = parse_u64(row.get(from_col).unwrap_or(""));
            let to = parse_u64(row.get(to_col).unwrap_or(""));
            let (from, to) = match (from, to) {
                (Some(a), Some(b)) => (a, b),
                _ => continue,
            };
            if to <= 0xFFFF_FFFF {
                out.v4.push((from as u32, to as u32, cc));
            } else {
                out.v6.push((from as u128, to as u128, cc));
            }
        } else {
            let nc = net_col.unwrap();
            let field = row.get(nc).unwrap_or("");
            match cidr_str_to_range(field) {
                Some((std::net::IpAddr::V4(_), s, e)) => {
                    out.v4.push((s as u32, e as u32, cc));
                }
                Some((std::net::IpAddr::V6(_), s, e)) => out.v6.push((s, e, cc)),
                None => {
                    log::debug!("[geoip] unparseable network field '{}'", field);
                }
            }
        }
    }
    if skipped_cc > 0 && out.total() == 0 {
        return Err(format!(
            "csv parsed but no rows carried a usable country code ({} skipped)",
            skipped_cc
        ));
    }
    Ok(out)
}

fn col_of(names: &[String], candidates: &[&str]) -> Option<usize> {
    for cand in candidates {
        if let Some(pos) = names.iter().position(|n| n == cand) {
            return Some(pos);
        }
    }
    None
}

fn parse_u64(value: &str) -> Option<u64> {
    value.trim().trim_matches('"').parse::<u64>().ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geoip::db::ipv4_to_u32;

    fn filter(countries: &[&str]) -> WantFilter<'static> {
        WantFilter::only(&countries.iter().map(|s| s.to_string()).collect::<Vec<_>>())
    }

    #[test]
    fn parses_geolite2_style_csv() {
        let data = "network,country_iso_code\n\
                    1.0.0.0/24,AU\n\
                    8.8.8.0/24,US\n\
                    77.88.8.8/32,RU\n\
                    2001:db8::/32,DE\n";
        let out = parse(
            Box::new(std::io::Cursor::new(data)),
            "geolite2-csv",
            &WantFilter::all(),
        )
        .unwrap();
        assert_eq!(out.v4.len(), 3);
        assert_eq!(out.v6.len(), 1);
        assert_eq!(
            out.v4[2],
            (
                ipv4_to_u32("77.88.8.8".parse().unwrap()),
                ipv4_to_u32("77.88.8.8".parse().unwrap()),
                *b"RU"
            )
        );
    }

    #[test]
    fn trims_geolite2_by_country() {
        let data = "network,country_iso_code\n1.0.0.0/24,AU\n8.8.8.0/24,US\n";
        let trimmed = parse(
            Box::new(std::io::Cursor::new(data)),
            "geolite2-csv",
            &filter(&["AU"]),
        )
        .unwrap();
        assert_eq!(trimmed.v4.len(), 1);
        assert_eq!(trimmed.v4[0].2, *b"AU");
    }

    #[test]
    fn parses_ip2location_csv_with_trim() {
        let data = "\"ip_from\",\"ip_to\",\"country_code\",\"country_name\"\n\
                    \"16777216\",\"16777471\",\"AU\",\"AUSTRALIA\"\n\
                    \"16777472\",\"16778239\",\"CN\",\"CHINA\"\n";
        let all = parse(
            Box::new(std::io::Cursor::new(data)),
            "ip2location-csv",
            &WantFilter::all(),
        )
        .unwrap();
        assert_eq!(all.v4.len(), 2);

        let trimmed = parse(
            Box::new(std::io::Cursor::new(data)),
            "ip2location-csv",
            &filter(&["AU"]),
        )
        .unwrap();
        assert_eq!(trimmed.v4.len(), 1);
        assert_eq!(trimmed.v4[0].2, *b"AU");
    }

    #[test]
    fn headerless_positional_columns() {
        let data = "1.0.0.0/24,AU\n8.8.8.0/24,US\n";
        let out = parse(
            Box::new(std::io::Cursor::new(data)),
            "geolite2-csv",
            &WantFilter::all(),
        )
        .unwrap();
        assert_eq!(out.v4.len(), 2);
    }
}
