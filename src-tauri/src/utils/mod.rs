use anyhow::{anyhow, Result};
use chrono::{DateTime, NaiveDate, Utc};
use regex::{Captures, Regex, RegexBuilder};
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

pub fn normalize_api_key(value: &str) -> String {
    value
        .chars()
        .filter(|c| c.is_ascii_graphic() && !c.is_ascii_whitespace())
        .collect()
}

pub fn normalize_entity_name(value: &str) -> String {
    value.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_lowercase()
}

pub fn redact_profile_data(
    text: &str,
    company_name: Option<&str>,
    company_email: Option<&str>,
    company_address: Option<&str>,
) -> String {
    let mut cleaned = text.replace("\r\n", "\n");
    let candidates = redaction_candidates(company_name, company_email, company_address);

    for candidate in candidates {
        let pattern = build_redaction_pattern(&candidate);
        if pattern.is_empty() {
            continue;
        }
        if let Ok(regex) = RegexBuilder::new(&pattern).case_insensitive(true).build() {
            cleaned = regex
                .replace_all(&cleaned, |_: &Captures| "")
                .into_owned();
        }
    }

    cleanup_redacted_text(&cleaned)
}

pub fn within_allowed_depth(base: &Path, path: &Path) -> bool {
    let Ok(relative) = path.strip_prefix(base) else {
        return false;
    };
    let components = relative.components().count();
    components <= MAX_FOLDER_DEPTH + 1
}

fn redaction_candidates(
    company_name: Option<&str>,
    company_email: Option<&str>,
    company_address: Option<&str>,
) -> Vec<String> {
    let mut values = Vec::new();

    if let Some(value) = company_name.map(str::trim).filter(|value| !value.is_empty()) {
        values.push(value.to_string());
    }

    if let Some(value) = company_email.map(str::trim).filter(|value| !value.is_empty()) {
        values.push(value.to_string());
    }

    if let Some(address) = company_address.map(str::trim).filter(|value| !value.is_empty()) {
        values.push(address.to_string());
        let fragments = address
            .split(|ch: char| ch == '\n' || ch == '\r' || ch == ',' || ch == ';')
            .map(str::trim)
            .filter(|fragment| !fragment.is_empty())
            .map(ToString::to_string)
            .collect::<Vec<_>>();
        for fragment in fragments.iter() {
            values.push(fragment.clone());
        }
        if !fragments.is_empty() {
            values.push(fragments.join(" "));
            values.push(fragments.join(", "));
        }
    }

    values.sort_by_key(|value| std::cmp::Reverse(value.len()));
    values.dedup();
    values
}

fn build_redaction_pattern(candidate: &str) -> String {
    let trimmed = candidate.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    if trimmed.contains('@') {
        return regex::escape(trimmed);
    }

    let tokens = trimmed
        .split(|ch: char| ch.is_whitespace() || ",;:|/\\-()".contains(ch))
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .map(regex::escape)
        .collect::<Vec<_>>();
    if tokens.is_empty() {
        String::new()
    } else {
        tokens.join(r"[\s,;:|/\\\-()]*")
    }
}

fn cleanup_redacted_text(value: &str) -> String {
    let blank_line_re = Regex::new(r"\n{3,}").expect("invalid blank line regex");
    let multi_space_re = Regex::new(r"[ \t]{2,}").expect("invalid spacing regex");

    let lines = value
        .lines()
        .map(|line| multi_space_re.replace_all(line, " ").into_owned())
        .map(|line| line.trim_matches(|ch: char| ch.is_whitespace()).to_string())
        .filter(|line| {
            let reduced = line.trim_matches(|ch: char| ch.is_whitespace() || ",;:|-".contains(ch));
            !reduced.is_empty()
        })
        .collect::<Vec<_>>();

    let joined = lines.join("\n");
    blank_line_re.replace_all(joined.trim(), "\n\n").into_owned()
}
