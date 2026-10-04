//! Minimal DEX 035 parser (in-memory).

use crate::android::error::{AndroidError, Result};

#[derive(Debug, Clone)]
pub struct DexFile {
    pub data: Vec<u8>,
    pub strings: Vec<String>,
    pub types: Vec<String>,
    pub protos: Vec<ProtoId>,
    pub fields: Vec<FieldId>,
    pub methods: Vec<MethodId>,
    pub classes: Vec<ClassDef>,
}

#[derive(Debug, Clone)]
pub struct ProtoId {
    pub shorty: String,
    pub return_type: String,
    pub parameters: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct FieldId {
    pub class: String,
    pub type_desc: String,
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct MethodId {
    pub class: String,
    pub name: String,
    pub proto: ProtoId,
}

#[derive(Debug, Clone)]
pub struct ClassDef {
    pub descriptor: String,
    pub access_flags: u32,
    pub superclass: Option<String>,
    pub class_data_off: u32,
    pub direct_methods: Vec<EncodedMethod>,
    pub virtual_methods: Vec<EncodedMethod>,
}

#[derive(Debug, Clone)]
pub struct EncodedMethod {
    pub method_idx: u32,
    pub access_flags: u32,
    pub code: Option<CodeItem>,
}

#[derive(Debug, Clone)]
pub struct CodeItem {
    pub registers_size: u16,
    pub ins_size: u16,
    pub outs_size: u16,
    pub insns: Vec<u16>,
}

impl DexFile {
    pub fn parse(data: &[u8]) -> Result<Self> {
        if data.len() < 0x70 {
            return Err(AndroidError::Dex("file too small".into()));
        }
        if &data[0..4] != b"dex\n" {
            return Err(AndroidError::Dex("bad magic".into()));
        }
        let endian = read_u32(data, 40)?;
        if endian != 0x1234_5678 {
            return Err(AndroidError::Dex("unsupported endian".into()));
        }

        let string_ids_size = read_u32(data, 56)?;
        let string_ids_off = read_u32(data, 60)?;
        let type_ids_size = read_u32(data, 64)?;
        let type_ids_off = read_u32(data, 68)?;
        let proto_ids_size = read_u32(data, 72)?;
        let proto_ids_off = read_u32(data, 76)?;
        let field_ids_size = read_u32(data, 80)?;
        let field_ids_off = read_u32(data, 84)?;
        let method_ids_size = read_u32(data, 88)?;
        let method_ids_off = read_u32(data, 92)?;
        let class_defs_size = read_u32(data, 96)?;
        let class_defs_off = read_u32(data, 100)?;

        require_table(data, string_ids_size, string_ids_off, 4, "string_ids")?;
        require_table(data, type_ids_size, type_ids_off, 4, "type_ids")?;
        require_table(data, proto_ids_size, proto_ids_off, 12, "proto_ids")?;
        require_table(data, field_ids_size, field_ids_off, 8, "field_ids")?;
        require_table(data, method_ids_size, method_ids_off, 8, "method_ids")?;
        require_table(data, class_defs_size, class_defs_off, 32, "class_defs")?;

        let mut strings = Vec::new();
        for i in 0..string_ids_size {
            let off = read_u32(data, table_index(string_ids_off, i, 4)?)? as usize;
            strings.push(read_mutf8_string(data, off)?);
        }

        let mut types = Vec::new();
        for i in 0..type_ids_size {
            let idx = read_u32(data, table_index(type_ids_off, i, 4)?)? as usize;
            types.push(
                strings
                    .get(idx)
                    .cloned()
                    .ok_or_else(|| AndroidError::Dex("bad type string idx".into()))?,
            );
        }

        let mut protos = Vec::new();
        for i in 0..proto_ids_size {
            let base = table_index(proto_ids_off, i, 12)?;
            let shorty_idx = read_u32(data, base)? as usize;
            let return_idx = read_u32(data, base + 4)? as usize;
            let params_off = read_u32(data, base + 8)?;
            let shorty = strings
                .get(shorty_idx)
                .cloned()
                .ok_or_else(|| AndroidError::Dex("bad proto shorty".into()))?;
            let return_type = types
                .get(return_idx)
                .cloned()
                .ok_or_else(|| AndroidError::Dex("bad proto return".into()))?;
            let mut parameters = Vec::new();
            if params_off != 0 {
                let size = read_u32(data, params_off as usize)?;
                let bytes = (size as usize)
                    .checked_mul(2)
                    .ok_or_else(|| AndroidError::Dex("proto params overflow".into()))?;
                let list_off = params_off
                    .checked_add(4)
                    .ok_or_else(|| AndroidError::Dex("proto params overflow".into()))?;
                let start = list_off as usize;
                let end = start
                    .checked_add(bytes)
                    .ok_or_else(|| AndroidError::Dex("proto params overflow".into()))?;
                if end > data.len() || u32_span(params_off, 4, size, 2).is_err() {
                    return Err(AndroidError::Dex("proto params out of range".into()));
                }
                for p in 0..size {
                    let tidx = read_u16(data, table_index(list_off, p, 2)?)? as usize;
                    parameters.push(
                        types
                            .get(tidx)
                            .cloned()
                            .ok_or_else(|| AndroidError::Dex("bad proto param".into()))?,
                    );
                }
            }
            protos.push(ProtoId {
                shorty,
                return_type,
                parameters,
            });
        }

        let mut fields = Vec::new();
        for i in 0..field_ids_size {
            let base = table_index(field_ids_off, i, 8)?;
            let class_idx = read_u16(data, base)? as usize;
            let type_idx = read_u16(data, base + 2)? as usize;
            let name_idx = read_u32(data, base + 4)? as usize;
            fields.push(FieldId {
                class: types.get(class_idx).cloned().unwrap_or_default(),
                type_desc: types.get(type_idx).cloned().unwrap_or_default(),
                name: strings.get(name_idx).cloned().unwrap_or_default(),
            });
        }

        let mut methods = Vec::new();
        for i in 0..method_ids_size {
            let base = table_index(method_ids_off, i, 8)?;
            let class_idx = read_u16(data, base)? as usize;
            let proto_idx = read_u16(data, base + 2)? as usize;
            let name_idx = read_u32(data, base + 4)? as usize;
            methods.push(MethodId {
                class: types.get(class_idx).cloned().unwrap_or_default(),
                name: strings.get(name_idx).cloned().unwrap_or_default(),
                proto: protos
                    .get(proto_idx)
                    .cloned()
                    .ok_or_else(|| AndroidError::Dex("bad method proto".into()))?,
            });
        }

        let mut classes = Vec::new();
        for i in 0..class_defs_size {
            let base = table_index(class_defs_off, i, 32)?;
            let class_idx = read_u32(data, base)? as usize;
            let access_flags = read_u32(data, base + 4)?;
            let superclass_idx = read_u32(data, base + 8)?;
            let class_data_off = read_u32(data, base + 24)?;
            let descriptor = types
                .get(class_idx)
                .cloned()
                .ok_or_else(|| AndroidError::Dex("bad class def".into()))?;
            let superclass = if superclass_idx == 0xffff_ffff {
                None
            } else {
                types.get(superclass_idx as usize).cloned()
            };

            let (direct_methods, virtual_methods) = if class_data_off == 0 {
                (Vec::new(), Vec::new())
            } else {
                parse_class_data(data, class_data_off as usize)?
            };

            classes.push(ClassDef {
                descriptor,
                access_flags,
                superclass,
                class_data_off,
                direct_methods,
                virtual_methods,
            });
        }

        Ok(DexFile {
            data: data.to_vec(),
            strings,
            types,
            protos,
            fields,
            methods,
            classes,
        })
    }

    pub fn find_method(&self, class: &str, name: &str) -> Option<&EncodedMethod> {
        let c = self.classes.iter().find(|c| c.descriptor == class)?;
        c.direct_methods
            .iter()
            .chain(c.virtual_methods.iter())
            .find(|m| {
                self.methods
                    .get(m.method_idx as usize)
                    .map(|id| id.name == name)
                    .unwrap_or(false)
            })
    }

    pub fn method_id(&self, idx: u32) -> Option<&MethodId> {
        self.methods.get(idx as usize)
    }

    pub fn string(&self, idx: u32) -> Option<&str> {
        self.strings.get(idx as usize).map(|s| s.as_str())
    }
}

fn parse_class_data(
    data: &[u8],
    mut off: usize,
) -> Result<(Vec<EncodedMethod>, Vec<EncodedMethod>)> {
    let (static_fields_size, n1) = read_count(data, off)?;
    off += n1;
    let (instance_fields_size, n2) = read_count(data, off)?;
    off += n2;
    let (direct_methods_size, n3) = read_count(data, off)?;
    off += n3;
    let (virtual_methods_size, n4) = read_count(data, off)?;
    off += n4;

    for _ in 0..(static_fields_size + instance_fields_size) {
        let (_, a) = read_uleb128(data, off)?;
        off += a;
        let (_, b) = read_uleb128(data, off)?;
        off += b;
    }

    let mut direct = Vec::new();
    let mut method_idx = 0u32;
    for _ in 0..direct_methods_size {
        let (diff, a) = read_uleb128(data, off)?;
        off += a;
        let (access, b) = read_uleb128(data, off)?;
        off += b;
        let (code_off, c) = read_uleb128(data, off)?;
        off += c;
        method_idx = method_idx.wrapping_add(diff);
        let code = if code_off == 0 {
            None
        } else {
            Some(parse_code_item(data, code_off as usize)?)
        };
        direct.push(EncodedMethod {
            method_idx,
            access_flags: access,
            code,
        });
    }

    let mut virtuals = Vec::new();
    method_idx = 0;
    for _ in 0..virtual_methods_size {
        let (diff, a) = read_uleb128(data, off)?;
        off += a;
        let (access, b) = read_uleb128(data, off)?;
        off += b;
        let (code_off, c) = read_uleb128(data, off)?;
        off += c;
        method_idx = method_idx.wrapping_add(diff);
        let code = if code_off == 0 {
            None
        } else {
            Some(parse_code_item(data, code_off as usize)?)
        };
        virtuals.push(EncodedMethod {
            method_idx,
            access_flags: access,
            code,
        });
    }

    Ok((direct, virtuals))
}

fn parse_code_item(data: &[u8], off: usize) -> Result<CodeItem> {
    if off.checked_add(16).map(|e| e > data.len()).unwrap_or(true) {
        return Err(AndroidError::Dex("code item OOB".into()));
    }
    let registers_size = read_u16(data, off)?;
    let ins_size = read_u16(data, off + 2)?;
    let outs_size = read_u16(data, off + 4)?;
    let _tries_size = read_u16(data, off + 6)?;
    let _debug_info = read_u32(data, off + 8)?;
    let insns_size = read_u32(data, off + 12)?;
    let off_u =
        u32::try_from(off).map_err(|_| AndroidError::Dex("code offset exceeds u32".into()))?;
    u32_span(off_u, 16, insns_size, 2)?;
    let byte_len = (insns_size as usize)
        .checked_mul(2)
        .ok_or_else(|| AndroidError::Dex("insns_size overflow".into()))?;
    let start = off + 16;
    let end = start
        .checked_add(byte_len)
        .ok_or_else(|| AndroidError::Dex("insns_size overflow".into()))?;
    if end > data.len() {
        return Err(AndroidError::Dex(format!(
            "insns_size {insns_size} exceeds file"
        )));
    }
    let insns: Vec<u16> = data[start..end]
        .chunks_exact(2)
        .map(|b| u16::from_le_bytes([b[0], b[1]]))
        .collect();
    Ok(CodeItem {
        registers_size,
        ins_size,
        outs_size,
        insns,
    })
}

fn read_u16(data: &[u8], off: usize) -> Result<u16> {
    data.get(off..off + 2)
        .map(|b| u16::from_le_bytes([b[0], b[1]]))
        .ok_or_else(|| AndroidError::Dex("u16 OOB".into()))
}

fn read_u32(data: &[u8], off: usize) -> Result<u32> {
    data.get(off..off + 4)
        .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        .ok_or_else(|| AndroidError::Dex("u32 OOB".into()))
}

fn read_uleb128(data: &[u8], off: usize) -> Result<(u32, usize)> {
    let mut result = 0u32;
    let mut shift = 0;
    let mut i = 0;
    loop {
        let b = *data
            .get(off + i)
            .ok_or_else(|| AndroidError::Dex("uleb OOB".into()))? as u32;
        let bits = b & 0x7f;
        if shift >= 32 || (shift > 0 && bits > (u32::MAX >> shift)) {
            return Err(AndroidError::Dex("uleb overflow".into()));
        }
        result |= bits << shift;
        i += 1;
        if b & 0x80 == 0 {
            break;
        }
        shift += 7;
        if shift > 28 {
            return Err(AndroidError::Dex("uleb overflow".into()));
        }
    }
    Ok((result, i))
}

fn read_count(data: &[u8], off: usize) -> Result<(u32, usize)> {
    let (n, len) = read_uleb128(data, off)?;
    if n as usize > data.len() {
        return Err(AndroidError::Dex("count exceeds file".into()));
    }
    Ok((n, len))
}

/// Byte offset of `index` within a table, rejecting `u32` wrap (wasm32).
fn table_index(base: u32, index: u32, stride: u32) -> Result<usize> {
    u32_span(base, 0, index, stride)?;
    let delta = index
        .checked_mul(stride)
        .ok_or_else(|| AndroidError::Dex("offset overflow".into()))?;
    Ok(base
        .checked_add(delta)
        .ok_or_else(|| AndroidError::Dex("offset overflow".into()))? as usize)
}

/// `base + extra + count * stride` must fit in `u32`.
fn u32_span(base: u32, extra: u32, count: u32, stride: u32) -> Result<u32> {
    let delta = count
        .checked_mul(stride)
        .ok_or_else(|| AndroidError::Dex("offset overflow".into()))?;
    base.checked_add(extra)
        .and_then(|b| b.checked_add(delta))
        .ok_or_else(|| AndroidError::Dex("offset overflow".into()))
}

fn require_table(data: &[u8], count: u32, off: u32, stride: u32, what: &str) -> Result<()> {
    if count == 0 {
        return Ok(());
    }
    let end = u32_span(off, 0, count, stride)?;
    if (off as usize) >= data.len() || (end as usize) > data.len() {
        return Err(AndroidError::Dex(format!("{what} table out of range")));
    }
    Ok(())
}

fn read_mutf8_string(data: &[u8], off: usize) -> Result<String> {
    let (_utf16_len, n) = read_uleb128(data, off)?;
    let start = off
        .checked_add(n)
        .ok_or_else(|| AndroidError::Dex("string offset overflow".into()))?;
    let mut end = start;
    while end < data.len() && data[end] != 0 {
        end += 1;
    }
    let bytes = data
        .get(start..end)
        .ok_or_else(|| AndroidError::Dex("string OOB".into()))?;
    decode_mutf8(bytes)
}

/// Decode a Modified UTF-8 byte sequence (DEX / JNI).
/// Overlong `C0 80` is U+0000. Surrogate pairs become one scalar.
pub(crate) fn decode_mutf8(bytes: &[u8]) -> Result<String> {
    let mut out = String::new();
    let mut i = 0;
    while i < bytes.len() {
        let b0 = bytes[i];
        if b0 == 0 {
            break;
        }
        if b0 < 0x80 {
            out.push(b0 as char);
            i += 1;
            continue;
        }
        if b0 & 0xe0 == 0xc0 {
            if i + 1 >= bytes.len() {
                return Err(AndroidError::Dex("truncated MUTF-8".into()));
            }
            let b1 = bytes[i + 1];
            if b1 & 0xc0 != 0x80 {
                return Err(AndroidError::Dex("bad MUTF-8".into()));
            }
            let cp = ((b0 as u32 & 0x1f) << 6) | (b1 as u32 & 0x3f);
            out.push(char::from_u32(cp).ok_or_else(|| AndroidError::Dex("bad MUTF-8".into()))?);
            i += 2;
            continue;
        }
        if b0 & 0xf0 == 0xe0 {
            if i + 2 >= bytes.len() {
                return Err(AndroidError::Dex("truncated MUTF-8".into()));
            }
            let b1 = bytes[i + 1];
            let b2 = bytes[i + 2];
            if b1 & 0xc0 != 0x80 || b2 & 0xc0 != 0x80 {
                return Err(AndroidError::Dex("bad MUTF-8".into()));
            }
            let cp = ((b0 as u32 & 0x0f) << 12) | ((b1 as u32 & 0x3f) << 6) | (b2 as u32 & 0x3f);
            if (0xD800..=0xDBFF).contains(&cp) {
                if i + 5 >= bytes.len() {
                    return Err(AndroidError::Dex("truncated MUTF-8 surrogate".into()));
                }
                let c0 = bytes[i + 3];
                let c1 = bytes[i + 4];
                let c2 = bytes[i + 5];
                if c0 & 0xf0 != 0xe0 || c1 & 0xc0 != 0x80 || c2 & 0xc0 != 0x80 {
                    return Err(AndroidError::Dex("bad MUTF-8 surrogate".into()));
                }
                let low =
                    ((c0 as u32 & 0x0f) << 12) | ((c1 as u32 & 0x3f) << 6) | (c2 as u32 & 0x3f);
                if !(0xDC00..=0xDFFF).contains(&low) {
                    return Err(AndroidError::Dex("bad MUTF-8 surrogate".into()));
                }
                let u = 0x10000 + (((cp - 0xD800) << 10) | (low - 0xDC00));
                out.push(char::from_u32(u).ok_or_else(|| AndroidError::Dex("bad MUTF-8".into()))?);
                i += 6;
            } else {
                out.push(char::from_u32(cp).ok_or_else(|| AndroidError::Dex("bad MUTF-8".into()))?);
                i += 3;
            }
            continue;
        }
        return Err(AndroidError::Dex("bad MUTF-8".into()));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mutf8_embedded_nul_is_not_replacement_chars() {
        let data = [0x03, 0x61, 0xC0, 0x80, 0x62, 0x00];
        let s = read_mutf8_string(&data, 0).expect("decode");
        assert_eq!(s, "a\0b");
    }

    #[test]
    fn table_span_rejects_u32_wrap() {
        assert!(u32_span(0xFFFF_FFFC, 0, 2, 4).is_err());
        assert!(table_index(0xFFFF_FFFC, 1, 4).is_err());
        assert_eq!(table_index(0x70, 1, 4).unwrap(), 0x74);
    }

    #[test]
    fn string_id_count_must_fit_in_the_file() {
        let mut dex = crate::android::fixture::build_hello_dex().expect("dex");
        dex[56..60].copy_from_slice(&0x00FF_FFFFu32.to_le_bytes());
        let err = DexFile::parse(&dex).unwrap_err();
        assert!(
            err.to_string().contains("string_ids"),
            "expected a bounded reject, got {err}"
        );
    }

    #[test]
    fn insns_size_must_fit_in_the_file() {
        let mut dex = crate::android::fixture::build_hello_dex().expect("dex");
        let pos = dex
            .windows(2)
            .position(|w| w == [0x1a, 0x00])
            .expect("const-string");
        dex[pos - 4..pos].copy_from_slice(&u32::MAX.to_le_bytes());
        let err = DexFile::parse(&dex).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("insns") || msg.contains("overflow") || msg.contains("code"),
            "expected insns cap, got {msg}"
        );
    }
}
