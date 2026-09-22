//! Minimal Android binary XML helpers.
//! Full AXML is complex; for fixtures we also accept a tiny text manifest fallback
//! and otherwise return an error so callers can use DEX heuristics.

use crate::android::error::{AndroidError, Result};

/// Returns (package_name, launcher activity descriptor-ish name).
pub fn parse_package_and_launcher(data: &[u8]) -> Result<(String, Option<String>)> {
    if data.is_empty() {
        return Err(AndroidError::Manifest("empty".into()));
    }

    // Text fallback used by our own fixtures: "package=com.foo\nactivity=Lcom/foo/Hello;\n"
    if data.starts_with(b"package=") || data.starts_with(b"#nekodroid-manifest") {
        return parse_text_manifest(data);
    }

    // Binary AXML magic 0x00080003
    if data.len() >= 8 {
        let magic = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
        if magic == 0x0008_0003 {
            // Best-effort: scan UTF-16LE / ASCII-ish strings for package-looking tokens.
            if let Some(pkg) = scan_package_ascii(data) {
                return Ok((pkg, None));
            }
            return Err(AndroidError::Manifest(
                "binary AXML present but package not extracted; using DEX heuristics".into(),
            ));
        }
    }

    Err(AndroidError::Manifest("unrecognized manifest format".into()))
}

fn parse_text_manifest(data: &[u8]) -> Result<(String, Option<String>)> {
    let text = String::from_utf8_lossy(data);
    let mut package = None;
    let mut activity = None;
    for line in text.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("package=") {
            package = Some(rest.trim().to_string());
        } else if let Some(rest) = line.strip_prefix("activity=") {
            activity = Some(rest.trim().to_string());
        }
    }
    Ok((
        package.ok_or_else(|| AndroidError::Manifest("no package=".into()))?,
        activity,
    ))
}

fn scan_package_ascii(data: &[u8]) -> Option<String> {
    // Look for patterns like com.xxx.yyy in ASCII bytes.
    let s = String::from_utf8_lossy(data);
    for token in s.split(|c: char| !c.is_ascii_alphanumeric() && c != '.' && c != '_') {
        if token.starts_with("com.") && token.matches('.').count() >= 1 && token.len() > 5 {
            return Some(token.to_string());
        }
    }
    None
}
