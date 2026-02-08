use anyhow::{anyhow, Result};
use chrono::{DateTime, NaiveDate, Utc};
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::Read;
use std::path::Path;

pub const MAX_FOLDER_DEPTH: usize = 2;
pub const MAX_SCAN_DEPTH: usize = MAX_FOLDER_DEPTH + 1;

pub fn now_rfc3339() -> String {
    Utc::now().to_rfc3339()
}

pub fn modified_time_rfc3339(path: &Path) -> Result<String> {
    let metadata = std::fs::metadata(path)?;
    let modified = metadata.modified()?;
    let datetime: DateTime<Utc> = modified.into();
    Ok(datetime.to_rfc3339())
}

pub fn sha256_file(path: &Path) -> Result<String> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 8192];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hex::encode(hasher.finalize()))
}

pub fn format_decimal(value: f64) -> String {
    format!("{:.2}", value)
}

pub fn parse_decimal(value: &str) -> Result<f64> {
    value
        .replace(',', ".")
        .parse::<f64>()
        .map_err(|e| anyhow!("Parse decimal: {}", e))
}

pub fn normalize_date(value: Option<String>) -> Option<String> {
    let raw = value?.trim().to_string();
    if raw.is_empty() {
        return None;
    }

    let formats = ["%Y-%m-%d", "%d.%m.%Y", "%d/%m/%Y", "%Y/%m/%d", "%Y.%m.%d"];
    for fmt in formats.iter() {
        if let Ok(date) = NaiveDate::parse_from_str(&raw, fmt) {
            return Some(date.format("%Y-%m-%d").to_string());
        }
    }
    Some(raw)
}

pub fn within_allowed_depth(base: &Path, path: &Path) -> bool {
    let Ok(relative) = path.strip_prefix(base) else {
        return false;
    };
    let components = relative.components().count();
    components <= MAX_FOLDER_DEPTH + 1
}
