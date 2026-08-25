//! MaxMind `.mmdb` parser (GeoLite2-Country), built on the `maxminddb` crate.

use super::{ParsedRanges, WantFilter};
use ipnetwork::IpNetwork;
use std::io::Read;

pub fn parse(
    mut reader: Box<dyn std::io::Read + Send>,
    want: &WantFilter,
) -> Result<ParsedRanges, String> {
    let mut bytes = Vec::new();
    reader
        .read_to_end(&mut bytes)
        .map_err(|e| format!("mmdb read: {}", e))?;
    let reader = maxminddb::Reader::from_source(bytes).map_err(|e| format!("mmdb open: {}", e))?;

    let mut out = ParsedRanges::default();

    // IPv4 tree.
    let v4_all: IpNetwork = "0.0.0.0/0".parse().unwrap();
    collect_tree(&reader, v4_all, want, true, &mut out);
    // IPv6 tree (skip IPv4-mapped entries that some databases nest under ::/0).
    let v6_all: IpNetwork = "::/0".parse().unwrap();
    collect_tree(&reader, v6_all, want, false, &mut out);

    Ok(out)
}

fn collect_tree(
    reader: &maxminddb::Reader<Vec<u8>>,
    scope: IpNetwork,
    want: &WantFilter,
    expect_v4: bool,
    out: &mut ParsedRanges,
) {
    let iter = match reader.within::<maxminddb::geoip2::Country>(scope) {
        Ok(i) => i,
        Err(_) => return,
    };
    for item in iter {
        let item = match item {
            Ok(i) => i,
            Err(e) => {
                log::debug!("[geoip] mmdb entry error: {}", e);
                continue;
            }
        };
        let record = item.info;
        let cc_raw = record
            .country
            .and_then(|c| c.iso_code)
            .or_else(|| record.registered_country.and_then(|c| c.iso_code));
        let Some(cc_raw) = cc_raw else { continue };
        let b = cc_raw.as_bytes();
        if b.len() != 2 {
            continue;
        }
        let cc: [u8; 2] = [b[0].to_ascii_uppercase(), b[1].to_ascii_uppercase()];
        if !want.accepts(&cc) {
            continue;
        }
        let net = item.ip_net;
        let range: (u128, u128) = match crate::geoip::db::cidr_str_to_range(&format!(
            "{}/{}",
            net.network(),
            net.prefix()
        )) {
            Some((_, s, e)) => (s, e),
            None => continue,
        };
        if expect_v4 && net.is_ipv4() {
            out.v4.push((range.0 as u32, range.1 as u32, cc));
        } else if !expect_v4 && !net.is_ipv4() {
            out.v6.push((range.0, range.1, cc));
        }
    }
}
