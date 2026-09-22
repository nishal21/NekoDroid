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

        let string_ids_size = read_u32(data, 56)? as usize;
        let string_ids_off = read_u32(data, 60)? as usize;
        let type_ids_size = read_u32(data, 64)? as usize;
        let type_ids_off = read_u32(data, 68)? as usize;
        let proto_ids_size = read_u32(data, 72)? as usize;
        let proto_ids_off = read_u32(data, 76)? as usize;
        let field_ids_size = read_u32(data, 80)? as usize;
        let field_ids_off = read_u32(data, 84)? as usize;
        let method_ids_size = read_u32(data, 88)? as usize;
        let method_ids_off = read_u32(data, 92)? as usize;
        let class_defs_size = read_u32(data, 96)? as usize;
        let class_defs_off = read_u32(data, 100)? as usize;

        let mut strings = Vec::with_capacity(string_ids_size);
        for i in 0..string_ids_size {
            let off = read_u32(data, string_ids_off + i * 4)? as usize;
            strings.push(read_mutf8_string(data, off)?);
        }

        let mut types = Vec::with_capacity(type_ids_size);
        for i in 0..type_ids_size {
            let idx = read_u32(data, type_ids_off + i * 4)? as usize;
            types.push(
                strings
                    .get(idx)
                    .cloned()
                    .ok_or_else(|| AndroidError::Dex("bad type string idx".into()))?,
            );
        }

        let mut protos = Vec::with_capacity(proto_ids_size);
        for i in 0..proto_ids_size {
            let base = proto_ids_off + i * 12;
            let shorty_idx = read_u32(data, base)? as usize;
            let return_idx = read_u32(data, base + 4)? as usize;
            let params_off = read_u32(data, base + 8)? as usize;
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
                let size = read_u32(data, params_off)? as usize;
                for p in 0..size {
                    let tidx = read_u16(data, params_off + 4 + p * 2)? as usize;
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

        let mut fields = Vec::with_capacity(field_ids_size);
        for i in 0..field_ids_size {
            let base = field_ids_off + i * 8;
            let class_idx = read_u16(data, base)? as usize;
            let type_idx = read_u16(data, base + 2)? as usize;
            let name_idx = read_u32(data, base + 4)? as usize;
            fields.push(FieldId {
                class: types.get(class_idx).cloned().unwrap_or_default(),
                type_desc: types.get(type_idx).cloned().unwrap_or_default(),
                name: strings.get(name_idx).cloned().unwrap_or_default(),
            });
        }

        let mut methods = Vec::with_capacity(method_ids_size);
        for i in 0..method_ids_size {
            let base = method_ids_off + i * 8;
            let class_idx = read_u16(data, base)? as usize;
            let proto_idx = read_u16(data, base + 2)? as usize;
            let name_idx = read_u32(data, base + 4)? as usize;
            methods.push(MethodId {
                class: types.get(class_idx).cloned().unwrap_or_default(),
                name: strings.get(name_idx).cloned().unwrap_or_default(),
                proto: protos.get(proto_idx).cloned().ok_or_else(|| {
                    AndroidError::Dex("bad method proto".into())
                })?,
            });
        }

        let mut classes = Vec::with_capacity(class_defs_size);
        for i in 0..class_defs_size {
            let base = class_defs_off + i * 32;
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

fn parse_class_data(data: &[u8], mut off: usize) -> Result<(Vec<EncodedMethod>, Vec<EncodedMethod>)> {
    let (static_fields_size, n1) = read_uleb128(data, off)?;
    off += n1;
    let (instance_fields_size, n2) = read_uleb128(data, off)?;
    off += n2;
    let (direct_methods_size, n3) = read_uleb128(data, off)?;
    off += n3;
    let (virtual_methods_size, n4) = read_uleb128(data, off)?;
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
    let registers_size = read_u16(data, off)?;
    let ins_size = read_u16(data, off + 2)?;
    let outs_size = read_u16(data, off + 4)?;
    let _tries_size = read_u16(data, off + 6)?;
    let _debug_info = read_u32(data, off + 8)?;
    let insns_size = read_u32(data, off + 12)? as usize;
    let mut insns = Vec::with_capacity(insns_size);
    for i in 0..insns_size {
        insns.push(read_u16(data, off + 16 + i * 2)?);
    }
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
        result |= (b & 0x7f) << shift;
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

fn read_mutf8_string(data: &[u8], off: usize) -> Result<String> {
    let (_utf16_len, n) = read_uleb128(data, off)?;
    let start = off + n;
    let mut end = start;
    while end < data.len() && data[end] != 0 {
        end += 1;
    }
    let bytes = data
        .get(start..end)
        .ok_or_else(|| AndroidError::Dex("string OOB".into()))?;
    // MUTF-8 is close enough to UTF-8 for ASCII fixtures.
    Ok(String::from_utf8_lossy(bytes).into_owned())
}
