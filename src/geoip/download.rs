//! HTTP fetching of GeoIP sources: ETag-based conditional requests and
//! streaming-friendly in-memory decompression (gzip / zip / raw).

use std::io::{Cursor, Read};
use std::time::Duration;

const MAX_DOWNLOAD_BYTES: u64 = 128 * 1024 * 1024;
const DOWNLOAD_TIMEOUT_SECS: u64 = 300;

pub struct Fetch {
    pub bytes: Option<Vec<u8>>,
    pub etag: Option<String>,
}

/// Conditional GET. Returns `bytes: None` when the server answers 304.
pub fn fetch_if_modified(url: &str, etag: Option<&str>) -> Result<Fetch, String> {
    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(DOWNLOAD_TIMEOUT_SECS))
        .user_agent(concat!("trusttunnel-keenetic/", env!("CARGO_PKG_VERSION")))
        .build();
    let mut req = agent.get(url);
    if let Some(tag) = etag {
        req = req.set("If-None-Match", tag);
    }
    let resp = req.call().map_err(|e| format!("GET {}: {}", url, e))?;

    match resp.status() {
        304 => Ok(Fetch {
            bytes: None,
            etag: etag.map(|s| s.to_string()),
        }),
        _ => {
            let mut etag_out = resp
                .header("ETag")
                .map(|s| s.to_string())
                .or_else(|| resp.header("Last-Modified").map(|s| s.to_string()));
            let mut reader = resp.into_reader();
            let body = super::api::read_limited(&mut reader, MAX_DOWNLOAD_BYTES)
                .map_err(|e| format!("download {}: {}", url, e))?;
            if body.is_empty() {
                return Err(format!("download {}: empty response", url));
            }
            if etag_out.as_deref().map(str::is_empty).unwrap_or(true) {
                etag_out = None;
            }
            Ok(Fetch {
                bytes: Some(body),
                etag: etag_out,
            })
        }
    }
}

/// Detects gzip/zip containers by magic bytes and returns a reader over the
/// decompressed payload; raw data passes through unchanged.
pub fn decompress_sniff(bytes: Vec<u8>) -> Result<Box<dyn Read + Send>, String> {
    if bytes.len() >= 2 && bytes[0] == 0x1f && bytes[1] == 0x8b {
        let inner = flate2::read::GzDecoder::new(Cursor::new(bytes));
        return Ok(Box::new(inner));
    }
    if bytes.len() >= 4 && &bytes[0..4] == b"PK\x03\x04" {
        let cursor = Cursor::new(bytes);
        let mut archive =
            zip::ZipArchive::new(cursor).map_err(|e| format!("open zip archive: {}", e))?;
        // Prefer a plausible data file by extension, otherwise the largest entry.
        let mut entries: Vec<(usize, String, u64)> = Vec::new();
        for i in 0..archive.len() {
            let file = archive
                .by_index(i)
                .map_err(|e| format!("zip entry {}: {}", i, e))?;
            entries.push((i, file.name().to_string(), file.size()));
        }
        let idx = entries
            .iter()
            .find(|(_, name, _)| {
                ["csv", "mmdb", "zone", "txt"]
                    .iter()
                    .any(|ext| name.to_ascii_lowercase().ends_with(ext))
            })
            .or_else(|| entries.iter().max_by_key(|(_, _, size)| *size))
            .map(|(i, _, _)| *i)
            .ok_or("zip archive is empty")?;
        let mut file = archive
            .by_index(idx)
            .map_err(|e| format!("zip entry {}: {}", idx, e))?;
        log::info!("[geoip] extracting '{}' from zip archive", file.name());
        let mut out = Vec::with_capacity(file.size() as usize);
        file.read_to_end(&mut out)
            .map_err(|e| format!("unzip {}: {}", file.name(), e))?;
        return Ok(Box::new(Cursor::new(out)));
    }
    Ok(Box::new(Cursor::new(bytes)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    #[test]
    fn raw_passes_through() {
        let mut r = decompress_sniff(b"hello world".to_vec()).unwrap();
        let mut s = String::new();
        r.read_to_string(&mut s).unwrap();
        assert_eq!(s, "hello world");
    }

    #[test]
    fn gzip_is_detected_and_decoded() {
        use std::io::Write;
        let mut enc = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        enc.write_all(b"payload-data").unwrap();
        let compressed = enc.finish().unwrap();
        let mut r = decompress_sniff(compressed).unwrap();
        let mut s = String::new();
        r.read_to_string(&mut s).unwrap();
        assert_eq!(s, "payload-data");
    }

    #[test]
    fn zip_is_detected_and_decoded() {
        let mut w = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        w.start_file(
            "data/IP2LOCATION-LITE-DB1.CSV",
            zip::write::FileOptions::default(),
        )
        .unwrap();
        std::io::Write::write_all(&mut w, b"1,2,AU,AUSTRALIA").unwrap();
        let zipped = w.finish().unwrap().into_inner();
        let mut r = decompress_sniff(zipped).unwrap();
        let mut s = String::new();
        r.read_to_string(&mut s).unwrap();
        assert_eq!(s, "1,2,AU,AUSTRALIA");
    }
}
