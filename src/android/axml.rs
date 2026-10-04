//! Android binary XML (AXML) helpers.
//!
//! Fixtures may use a text manifest. Real APKs use a binary XML chunk stream.
//! A loose ASCII scan is not a successful parse: `com.*` shows up in string
//! pools that are not the manifest package.

use crate::android::error::{AndroidError, Result};

/// Returns (package_name, launcher activity descriptor).
pub fn parse_package_and_launcher(data: &[u8]) -> Result<(String, Option<String>)> {
    if data.is_empty() {
        return Err(AndroidError::Manifest("empty".into()));
    }

    // Text fallback used by our own fixtures: "package=com.foo\nactivity=Lcom/foo/Hello;\n"
    if data.starts_with(b"package=") || data.starts_with(b"#nekodroid-manifest") {
        return parse_text_manifest(data);
    }

    // Binary AXML: type 0x0003, headerSize 0x0008.
    if data.len() >= 8 {
        let magic = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
        if magic == 0x0008_0003 {
            return parse_binary_manifest(data);
        }
    }

    Err(AndroidError::Manifest(
        "unrecognized manifest format".into(),
    ))
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

fn parse_binary_manifest(data: &[u8]) -> Result<(String, Option<String>)> {
    let header_size = read_u16(data, 2)? as usize;
    let file_size = read_u32(data, 4)? as usize;
    if header_size < 8 || file_size < header_size {
        return Err(AndroidError::Manifest("bad AXML header".into()));
    }
    let limit = file_size.min(data.len());
    let mut off = header_size;
    let mut strings: Option<Vec<String>> = None;
    let mut package: Option<String> = None;
    let mut stack: Vec<Frame> = Vec::new();
    let mut first_activity: Option<String> = None;
    let mut launcher: Option<String> = None;

    while off + 8 <= limit {
        let ty = read_u16(data, off)?;
        let chunk_header = read_u16(data, off + 2)? as usize;
        let size = read_u32(data, off + 4)? as usize;
        if size < 8
            || chunk_header > size
            || off.checked_add(size).map(|e| e > limit).unwrap_or(true)
        {
            return Err(AndroidError::Manifest("AXML chunk out of range".into()));
        }
        match ty {
            0x0001 => {
                strings = Some(parse_string_pool(data, off, size)?);
            }
            0x0102 => {
                let strings = strings.as_ref().ok_or_else(|| {
                    AndroidError::Manifest("start element before string pool".into())
                })?;
                let el = parse_start_element(data, off, size, chunk_header, strings)?;
                if el.local == "manifest" {
                    if let Some(pkg) = el.attr("package") {
                        package = Some(pkg);
                    }
                }
                if el.local == "action" {
                    if el.attr("name").as_deref() == Some("android.intent.action.MAIN") {
                        mark_activity(&mut stack, true, false);
                    }
                }
                if el.local == "category" {
                    if el.attr("name").as_deref() == Some("android.intent.category.LAUNCHER") {
                        mark_activity(&mut stack, false, true);
                    }
                }
                let kind = if el.local == "activity" || el.local == "activity-alias" {
                    Kind::Activity
                } else {
                    Kind::Other
                };
                stack.push(Frame {
                    kind,
                    name_attr: el.attr("name"),
                    main: false,
                    launcher: false,
                });
            }
            0x0103 => {
                if let Some(frame) = stack.pop() {
                    if frame.kind == Kind::Activity {
                        if let Some(name) = frame.name_attr {
                            if first_activity.is_none() {
                                first_activity = Some(name.clone());
                            }
                            if frame.main && frame.launcher {
                                launcher = Some(name);
                            }
                        }
                    }
                }
            }
            _ => {}
        }
        off += size;
    }

    let package = package.ok_or_else(|| {
        AndroidError::Manifest("binary AXML present but package attribute was not found".into())
    })?;
    let activity_name = launcher.or(first_activity);
    let activity = activity_name.map(|n| to_descriptor(&package, &n));
    Ok((package, activity))
}

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Activity,
    Other,
}

struct Frame {
    kind: Kind,
    name_attr: Option<String>,
    main: bool,
    launcher: bool,
}

fn mark_activity(stack: &mut [Frame], main: bool, launcher: bool) {
    if let Some(frame) = stack.iter_mut().rev().find(|f| f.kind == Kind::Activity) {
        if main {
            frame.main = true;
        }
        if launcher {
            frame.launcher = true;
        }
    }
}

struct Element {
    local: String,
    attrs: Vec<(String, String)>,
}

impl Element {
    fn attr(&self, name: &str) -> Option<String> {
        self.attrs
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.clone())
    }
}

fn parse_start_element(
    data: &[u8],
    off: usize,
    size: usize,
    header_size: usize,
    strings: &[String],
) -> Result<Element> {
    if header_size < 16 || off + header_size + 20 > off + size {
        return Err(AndroidError::Manifest("bad start element".into()));
    }
    let ext = off + header_size;
    let name_idx = read_u32(data, ext + 4)? as usize;
    let attr_start = read_u16(data, ext + 8)? as usize;
    let attr_size = read_u16(data, ext + 10)? as usize;
    let attr_count = read_u16(data, ext + 12)? as usize;
    let local = strings
        .get(name_idx)
        .cloned()
        .ok_or_else(|| AndroidError::Manifest("bad element name".into()))?;
    let local = local.rsplit('.').next().unwrap_or(&local).to_string();
    if attr_size < 20 {
        return Err(AndroidError::Manifest("bad attribute size".into()));
    }
    let bytes = attr_count
        .checked_mul(attr_size)
        .ok_or_else(|| AndroidError::Manifest("attribute count overflow".into()))?;
    let attrs_at = ext
        .checked_add(attr_start)
        .ok_or_else(|| AndroidError::Manifest("attribute offset overflow".into()))?;
    let attrs_end = attrs_at
        .checked_add(bytes)
        .ok_or_else(|| AndroidError::Manifest("attribute offset overflow".into()))?;
    if attrs_end > off + size {
        return Err(AndroidError::Manifest("attributes out of range".into()));
    }
    let mut attrs = Vec::with_capacity(attr_count);
    for i in 0..attr_count {
        let a = attrs_at + i * attr_size;
        let name = read_u32(data, a + 4)? as usize;
        let raw = read_u32(data, a + 8)?;
        let dtype = *data
            .get(a + 15)
            .ok_or_else(|| AndroidError::Manifest("attribute OOB".into()))?;
        let data_idx = read_u32(data, a + 16)?;
        let name = strings.get(name).map(|s| s.as_str()).unwrap_or("");
        let name = name.rsplit('.').next().unwrap_or(name).to_string();
        let value_idx = if dtype == 0x03 {
            Some(data_idx as usize)
        } else if raw != 0xffff_ffff {
            Some(raw as usize)
        } else {
            None
        };
        if let Some(idx) = value_idx {
            if let Some(v) = strings.get(idx) {
                attrs.push((name, v.clone()));
            }
        }
    }
    Ok(Element { local, attrs })
}

fn parse_string_pool(data: &[u8], off: usize, size: usize) -> Result<Vec<String>> {
    if size < 28 || off + size > data.len() {
        return Err(AndroidError::Manifest("bad string pool".into()));
    }
    let header_size = read_u16(data, off + 2)? as usize;
    let string_count = read_u32(data, off + 8)?;
    let flags = read_u32(data, off + 16)?;
    let strings_start = read_u32(data, off + 20)? as usize;
    if header_size < 28 || header_size > size {
        return Err(AndroidError::Manifest("bad string pool header".into()));
    }
    let utf8 = flags & (1 << 8) != 0;
    let offsets_end = header_size
        .checked_add(
            (string_count as usize)
                .checked_mul(4)
                .ok_or_else(|| AndroidError::Manifest("string count overflow".into()))?,
        )
        .ok_or_else(|| AndroidError::Manifest("string count overflow".into()))?;
    if offsets_end > size {
        return Err(AndroidError::Manifest("string offsets out of range".into()));
    }
    let mut strings = Vec::with_capacity(string_count as usize);
    for i in 0..string_count {
        let rel = read_u32(data, off + header_size + i as usize * 4)? as usize;
        let abs = off
            .checked_add(strings_start)
            .and_then(|b| b.checked_add(rel))
            .ok_or_else(|| AndroidError::Manifest("string offset overflow".into()))?;
        if abs >= data.len() {
            return Err(AndroidError::Manifest("string OOB".into()));
        }
        let s = if utf8 {
            read_pool_utf8(data, abs)?
        } else {
            read_pool_utf16(data, abs)?
        };
        strings.push(s);
    }
    Ok(strings)
}

fn read_pool_utf8(data: &[u8], mut off: usize) -> Result<String> {
    let (_, off1) = read_utf8_len(data, off)?;
    let (nbytes, off2) = read_utf8_len(data, off1)?;
    off = off2;
    let end = off
        .checked_add(nbytes as usize)
        .ok_or_else(|| AndroidError::Manifest("utf8 string overflow".into()))?;
    if end > data.len() {
        return Err(AndroidError::Manifest("utf8 string OOB".into()));
    }
    crate::android::dex::decode_mutf8(&data[off..end])
        .map_err(|e| AndroidError::Manifest(e.to_string()))
}

fn read_utf8_len(data: &[u8], off: usize) -> Result<(u32, usize)> {
    let b0 = *data
        .get(off)
        .ok_or_else(|| AndroidError::Manifest("utf8 len OOB".into()))?;
    if b0 & 0x80 != 0 {
        let b1 = *data
            .get(off + 1)
            .ok_or_else(|| AndroidError::Manifest("utf8 len OOB".into()))?;
        Ok(((((b0 & 0x7f) as u32) << 8) | b1 as u32, off + 2))
    } else {
        Ok((b0 as u32, off + 1))
    }
}

fn read_pool_utf16(data: &[u8], off: usize) -> Result<String> {
    let (nchars, mut cur) = read_utf16_len(data, off)?;
    let byte_len = (nchars as usize)
        .checked_mul(2)
        .ok_or_else(|| AndroidError::Manifest("utf16 string overflow".into()))?;
    let end = cur
        .checked_add(byte_len)
        .ok_or_else(|| AndroidError::Manifest("utf16 string overflow".into()))?;
    if end > data.len() {
        return Err(AndroidError::Manifest("utf16 string OOB".into()));
    }
    let mut units = Vec::with_capacity(nchars as usize);
    while cur + 1 < end {
        units.push(u16::from_le_bytes([data[cur], data[cur + 1]]));
        cur += 2;
    }
    Ok(String::from_utf16_lossy(&units))
}

fn read_utf16_len(data: &[u8], off: usize) -> Result<(u32, usize)> {
    let first = read_u16(data, off)?;
    if first & 0x8000 != 0 {
        let second = read_u16(data, off + 2)?;
        let len = (((first & 0x7fff) as u32) << 16) | second as u32;
        Ok((len, off + 4))
    } else {
        Ok((first as u32, off + 2))
    }
}

fn to_descriptor(package: &str, name: &str) -> String {
    if name.starts_with('L') && name.ends_with(';') {
        return name.to_string();
    }
    let full = if let Some(rest) = name.strip_prefix('.') {
        format!("{package}.{rest}")
    } else if name.contains('.') {
        name.to_string()
    } else {
        format!("{package}.{name}")
    };
    format!("L{};", full.replace('.', "/"))
}

fn read_u16(data: &[u8], off: usize) -> Result<u16> {
    data.get(off..off + 2)
        .map(|b| u16::from_le_bytes([b[0], b[1]]))
        .ok_or_else(|| AndroidError::Manifest("u16 OOB".into()))
}

fn read_u32(data: &[u8], off: usize) -> Result<u32> {
    data.get(off..off + 4)
        .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        .ok_or_else(|| AndroidError::Manifest("u32 OOB".into()))
}

/// Binary manifest used by tests. String 0 is a decoy `com.*` token.
/// `decoy_activity` is listed first and is not MAIN/LAUNCHER.
/// `launcher_activity` is the activity that should actually start.
pub fn encode_binary_manifest(
    package: &str,
    decoy_activity: &str,
    launcher_activity: &str,
) -> Vec<u8> {
    let strings = [
        "com.decoy.other",
        "manifest",
        "package",
        package,
        "application",
        "activity",
        "name",
        decoy_activity,
        launcher_activity,
        "intent-filter",
        "action",
        "android.intent.action.MAIN",
        "category",
        "android.intent.category.LAUNCHER",
    ];
    let pool = encode_utf8_pool(&strings);
    let mut body = pool;
    body.extend(start_element(1, &[(2, 3)]));
    body.extend(start_element(4, &[]));
    body.extend(start_element(5, &[(6, 7)]));
    body.extend(end_element(5));
    body.extend(start_element(5, &[(6, 8)]));
    body.extend(start_element(9, &[]));
    body.extend(start_element(10, &[(6, 11)]));
    body.extend(end_element(10));
    body.extend(start_element(12, &[(6, 13)]));
    body.extend(end_element(12));
    body.extend(end_element(9));
    body.extend(end_element(5));
    body.extend(end_element(4));
    body.extend(end_element(1));

    let mut file = Vec::with_capacity(8 + body.len());
    file.extend_from_slice(&0x0003u16.to_le_bytes());
    file.extend_from_slice(&8u16.to_le_bytes());
    file.extend_from_slice(&((8 + body.len()) as u32).to_le_bytes());
    file.extend(body);
    file
}

fn encode_utf8_pool(strings: &[&str]) -> Vec<u8> {
    let mut data = Vec::new();
    let mut offsets = Vec::new();
    for s in strings {
        offsets.push(data.len() as u32);
        let bytes = s.as_bytes();
        data.push(bytes.len() as u8);
        data.push(bytes.len() as u8);
        data.extend_from_slice(bytes);
        data.push(0);
    }
    let header_size = 28u32;
    let strings_start = header_size + (strings.len() as u32) * 4;
    let mut out = Vec::new();
    out.extend_from_slice(&0x0001u16.to_le_bytes());
    out.extend_from_slice(&28u16.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes()); // size patched below
    out.extend_from_slice(&(strings.len() as u32).to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&(1u32 << 8).to_le_bytes());
    out.extend_from_slice(&strings_start.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    for o in offsets {
        out.extend_from_slice(&o.to_le_bytes());
    }
    out.extend(data);
    while out.len() % 4 != 0 {
        out.push(0);
    }
    let size = out.len() as u32;
    out[4..8].copy_from_slice(&size.to_le_bytes());
    out
}

fn start_element(name_idx: u32, attrs: &[(u32, u32)]) -> Vec<u8> {
    let size = 16 + 20 + attrs.len() * 20;
    let mut out = Vec::with_capacity(size);
    out.extend_from_slice(&0x0102u16.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(&(size as u32).to_le_bytes());
    out.extend_from_slice(&1u32.to_le_bytes());
    out.extend_from_slice(&0xffff_ffffu32.to_le_bytes());
    out.extend_from_slice(&0xffff_ffffu32.to_le_bytes());
    out.extend_from_slice(&name_idx.to_le_bytes());
    out.extend_from_slice(&20u16.to_le_bytes());
    out.extend_from_slice(&20u16.to_le_bytes());
    out.extend_from_slice(&(attrs.len() as u16).to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    for (name, value) in attrs {
        out.extend_from_slice(&0xffff_ffffu32.to_le_bytes());
        out.extend_from_slice(&name.to_le_bytes());
        out.extend_from_slice(&value.to_le_bytes());
        out.extend_from_slice(&8u16.to_le_bytes());
        out.push(0);
        out.push(0x03);
        out.extend_from_slice(&value.to_le_bytes());
    }
    out
}

fn end_element(name_idx: u32) -> Vec<u8> {
    let mut out = Vec::with_capacity(24);
    out.extend_from_slice(&0x0103u16.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(&24u32.to_le_bytes());
    out.extend_from_slice(&1u32.to_le_bytes());
    out.extend_from_slice(&0xffff_ffffu32.to_le_bytes());
    out.extend_from_slice(&0xffff_ffffu32.to_le_bytes());
    out.extend_from_slice(&name_idx.to_le_bytes());
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii_com_token_is_not_a_manifest() {
        let mut data = vec![0x03, 0x00, 0x08, 0x00];
        data.extend_from_slice(&32u32.to_le_bytes());
        data.extend_from_slice(b"com.decoy.other\0");
        let err = parse_package_and_launcher(&data).unwrap_err();
        let msg = err.to_string();
        assert!(
            !msg.contains("com.decoy.other"),
            "scanner treated a decoy token as the package: {msg}"
        );
    }

    #[test]
    fn binary_manifest_ignores_decoy_and_picks_launcher() {
        let xml = encode_binary_manifest(
            "com.nekodroid",
            "com.nekodroid.MainActivity",
            ".HelloScreen",
        );
        assert!(
            xml.windows(15).any(|w| w == b"com.decoy.other"),
            "fixture must contain the decoy token"
        );
        let (pkg, act) = parse_package_and_launcher(&xml).expect("parse");
        assert_eq!(pkg, "com.nekodroid");
        assert_eq!(act.as_deref(), Some("Lcom/nekodroid/HelloScreen;"));
    }
}
