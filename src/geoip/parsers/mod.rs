//! Source-format parsers: each turns a raw download into `(range, country)` pairs.

pub mod csv;
pub mod mmdb;
pub mod zone;

use super::db::Cc;

/// Ranges extracted from one source, keyed by numeric IP bounds.
#[derive(Debug, Default)]
pub struct ParsedRanges {
    pub v4: Vec<(u32, u32, Cc)>,
    pub v6: Vec<(u128, u128, Cc)>,
}

impl ParsedRanges {
    pub fn total(&self) -> usize {
        self.v4.len() + self.v6.len()
    }
}

/// Country filter used for trim-to-selected-countries.
pub struct WantFilter<'a> {
    all: bool,
    set: std::collections::HashSet<Cc>,
    _marker: std::marker::PhantomData<&'a ()>,
}

impl<'a> WantFilter<'a> {
    /// Accepts everything.
    pub fn all() -> Self {
        Self {
            all: true,
            set: Default::default(),
            _marker: Default::default(),
        }
    }
    /// Accepts only the given ISO codes (upper case).
    pub fn only(countries: &[String]) -> Self {
        Self {
            all: false,
            set: countries
                .iter()
                .filter_map(|c| {
                    let b = c.as_bytes();
                    if b.len() == 2 {
                        Some([b[0], b[1]])
                    } else {
                        None
                    }
                })
                .collect(),
            _marker: Default::default(),
        }
    }

    pub fn accepts(&self, cc: &Cc) -> bool {
        self.all || self.set.contains(cc)
    }
}

/// Dispatches to the concrete parser based on the provider's declared `format`.
pub fn parse_source(
    format: &str,
    reader: Box<dyn std::io::Read + Send>,
    want: &WantFilter,
    zone_country: Option<&str>,
) -> Result<ParsedRanges, String> {
    match format {
        "geolite2-csv" | "ip2location-csv" => csv::parse(reader, format, want),
        "zone" => zone::parse(reader, want, zone_country),
        "mmdb" => mmdb::parse(reader, want),
        other => Err(format!("unsupported geoip db format '{}'", other)),
    }
}
