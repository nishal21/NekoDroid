//! Build minimal hello DEX + APK fixtures (no external smali required).

use std::collections::BTreeMap;
use std::io::Write;

use zip::write::SimpleFileOptions;
use zip::ZipWriter;

use crate::android::error::Result;

/// Build a minimal DEX with:
/// `Lcom/nekodroid/Hello;.main([Ljava/lang/String;)V` that const-strings and
/// invoke-statics `Lnekodroid/Hle;.println(Ljava/lang/String;)V`.
pub fn build_hello_dex() -> Result<Vec<u8>> {
    let mut b = DexBuilder::new();
    let s_hello = b.string("Hello from NekoDroid");
    let s_main = b.string("main");
    let s_init = b.string("<init>");
    let s_println = b.string("println");
    let s_v = b.string("V");
    let s_shorty_v = b.string("V");
    let s_shorty_l = b.string("VL");
    let s_shorty_va = b.string("V[L");

    let t_hello = b.type_of("Lcom/nekodroid/Hello;");
    let t_object = b.type_of("Ljava/lang/Object;");
    let t_hle = b.type_of("Lnekodroid/Hle;");
    let t_string = b.type_of("Ljava/lang/String;");
    let t_string_arr = b.type_of("[Ljava/lang/String;");
    let t_void = b.type_of("V");

    let p_void = b.proto(s_shorty_v, t_void, &[]);
    let p_println = b.proto(s_shorty_l, t_void, &[t_string]);
    let p_main = b.proto(s_shorty_va, t_void, &[t_string_arr]);

    let m_println = b.method(t_hle, p_println, s_println);
    let m_main = b.method(t_hello, p_main, s_main);
    let m_init = b.method(t_hello, p_void, s_init);

    // main code: const-string v0, hello; invoke-static {v0}, println; return-void
    // regs: 2 (v0 + unused), ins=1 (args array in v1)
    let mut insns = Vec::new();
    insns.push(0x001a); // const-string v0, ...
    insns.push(s_hello as u16);
    // invoke-static {v0}, meth@m_println  (A=1,G=0,C=0)
    insns.push(0x1071);
    insns.push(m_println as u16);
    insns.push(0x0000);
    insns.push(0x000e); // return-void

    let code_main = CodeBlob {
        registers_size: 2,
        ins_size: 1,
        outs_size: 1,
        insns,
    };

    // trivial <init>: return-void only, regs=1 ins=1
    let code_init = CodeBlob {
        registers_size: 1,
        ins_size: 1,
        outs_size: 0,
        insns: vec![0x000e],
    };

    b.add_class(
        t_hello,
        t_object,
        vec![
            EncodedMeth {
                method_idx: m_init,
                access_flags: 0x10001, // public constructor
                code: Some(code_init),
            },
            EncodedMeth {
                method_idx: m_main,
                access_flags: 0x0009, // public static
                code: Some(code_main),
            },
        ],
    );

    Ok(b.finish())
}

/// Build a DEX that computes 2+3 via add-int/2addr and prints `5` via HLE.
pub fn build_arith_dex() -> Result<Vec<u8>> {
    let mut b = DexBuilder::new();
    let s_main = b.string("main");
    let s_init = b.string("<init>");
    let s_print_int = b.string("printInt");
    let s_shorty_v = b.string("V");
    let s_shorty_i = b.string("VI");
    let s_shorty_va = b.string("V[L");

    let t_arith = b.type_of("Lcom/nekodroid/Arith;");
    let t_object = b.type_of("Ljava/lang/Object;");
    let t_hle = b.type_of("Lnekodroid/Hle;");
    let t_string_arr = b.type_of("[Ljava/lang/String;");
    let t_void = b.type_of("V");
    let t_int = b.type_of("I");

    let p_void = b.proto(s_shorty_v, t_void, &[]);
    let p_print_int = b.proto(s_shorty_i, t_void, &[t_int]);
    let p_main = b.proto(s_shorty_va, t_void, &[t_string_arr]);

    let m_print_int = b.method(t_hle, p_print_int, s_print_int);
    let m_main = b.method(t_arith, p_main, s_main);
    let m_init = b.method(t_arith, p_void, s_init);

    // const/4 v0,#2; const/4 v1,#3; add-int/2addr v0,v1; invoke-static {v0}, printInt; return-void
    let insns = vec![
        0x2012, // const/4 v0, #2
        0x3112, // const/4 v1, #3
        0x10b0, // add-int/2addr v0, v1
        0x1071, // invoke-static {v0}, printInt
        m_print_int as u16,
        0x0000,
        0x000e, // return-void
    ];

    let code_main = CodeBlob {
        registers_size: 2,
        ins_size: 1,
        outs_size: 1,
        insns,
    };
    let code_init = CodeBlob {
        registers_size: 1,
        ins_size: 1,
        outs_size: 0,
        insns: vec![0x000e],
    };

    b.add_class(
        t_arith,
        t_object,
        vec![
            EncodedMeth {
                method_idx: m_init,
                access_flags: 0x10001,
                code: Some(code_init),
            },
            EncodedMeth {
                method_idx: m_main,
                access_flags: 0x0009,
                code: Some(code_main),
            },
        ],
    );

    Ok(b.finish())
}

pub fn build_hello_apk() -> Result<Vec<u8>> {
    let dex = build_hello_dex()?;
    let mut cursor = std::io::Cursor::new(Vec::new());
    {
        let mut zip = ZipWriter::new(&mut cursor);
        let opts = SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        zip.start_file("classes.dex", opts)
            .map_err(|e| crate::android::error::AndroidError::Runtime(e.to_string()))?;
        zip.write_all(&dex)
            .map_err(|e| crate::android::error::AndroidError::Runtime(e.to_string()))?;
        zip.start_file("AndroidManifest.xml", opts)
            .map_err(|e| crate::android::error::AndroidError::Runtime(e.to_string()))?;
        zip.write_all(
            b"#nekodroid-manifest\npackage=com.nekodroid\nactivity=Lcom/nekodroid/Hello;\n",
        )
        .map_err(|e| crate::android::error::AndroidError::Runtime(e.to_string()))?;
        zip.finish()
            .map_err(|e| crate::android::error::AndroidError::Runtime(e.to_string()))?;
    }
    Ok(cursor.into_inner())
}

struct EncodedMeth {
    method_idx: u32,
    access_flags: u32,
    code: Option<CodeBlob>,
}

struct CodeBlob {
    registers_size: u16,
    ins_size: u16,
    outs_size: u16,
    insns: Vec<u16>,
}

struct DexBuilder {
    strings: Vec<String>,
    string_index: BTreeMap<String, u32>,
    types: Vec<u32>, // string idx
    type_index: BTreeMap<String, u32>,
    protos: Vec<(u32, u32, Vec<u16>)>, // shorty, return, params type idxs
    methods: Vec<(u16, u16, u32)>,     // class, proto, name
    classes: Vec<ClassBuild>,
}

struct ClassBuild {
    class_idx: u32,
    superclass_idx: u32,
    methods: Vec<EncodedMeth>,
}

impl DexBuilder {
    fn new() -> Self {
        Self {
            strings: Vec::new(),
            string_index: BTreeMap::new(),
            types: Vec::new(),
            type_index: BTreeMap::new(),
            protos: Vec::new(),
            methods: Vec::new(),
            classes: Vec::new(),
        }
    }

    fn string(&mut self, s: &str) -> u32 {
        if let Some(&i) = self.string_index.get(s) {
            return i;
        }
        let i = self.strings.len() as u32;
        self.strings.push(s.to_string());
        self.string_index.insert(s.to_string(), i);
        i
    }

    fn type_of(&mut self, desc: &str) -> u32 {
        if let Some(&i) = self.type_index.get(desc) {
            return i;
        }
        let s = self.string(desc);
        let i = self.types.len() as u32;
        self.types.push(s);
        self.type_index.insert(desc.to_string(), i);
        i
    }

    fn proto(&mut self, shorty: u32, ret: u32, params: &[u32]) -> u32 {
        let params_u16: Vec<u16> = params.iter().map(|p| *p as u16).collect();
        let i = self.protos.len() as u32;
        self.protos.push((shorty, ret, params_u16));
        i
    }

    fn method(&mut self, class: u32, proto: u32, name: u32) -> u32 {
        let i = self.methods.len() as u32;
        self.methods.push((class as u16, proto as u16, name));
        i
    }

    fn add_class(&mut self, class_idx: u32, superclass_idx: u32, methods: Vec<EncodedMeth>) {
        self.classes.push(ClassBuild {
            class_idx,
            superclass_idx,
            methods,
        });
    }

    fn finish(self) -> Vec<u8> {
        // Layout plan (after header 0x70):
        // string_ids, type_ids, proto_ids, field_ids(0), method_ids, class_defs,
        // then data: string_data, proto params, code items, class_data
        let mut data_section = Vec::new();
        let mut string_data_offs = Vec::new();

        for s in &self.strings {
            string_data_offs.push(data_section.len() as u32);
            write_uleb128(&mut data_section, s.chars().count() as u32); // utf16 length approx
            data_section.extend_from_slice(s.as_bytes());
            data_section.push(0);
        }

        let mut proto_param_offs = Vec::new();
        for (_shorty, _ret, params) in &self.protos {
            if params.is_empty() {
                proto_param_offs.push(0u32);
            } else {
                align4(&mut data_section);
                proto_param_offs.push(data_section.len() as u32);
                data_section.extend_from_slice(&(params.len() as u32).to_le_bytes());
                for p in params {
                    data_section.extend_from_slice(&p.to_le_bytes());
                }
            }
        }

        // code items for each method that has code, map method_idx -> code_off (relative to file later)
        // We'll place code items in data_section and remember offsets relative to data_section start;
        // final file offset = data_base + off.
        let mut code_offs: BTreeMap<u32, u32> = BTreeMap::new();
        for c in &self.classes {
            for m in &c.methods {
                if let Some(code) = &m.code {
                    align4(&mut data_section);
                    let off = data_section.len() as u32;
                    code_offs.insert(m.method_idx, off);
                    data_section.extend_from_slice(&code.registers_size.to_le_bytes());
                    data_section.extend_from_slice(&code.ins_size.to_le_bytes());
                    data_section.extend_from_slice(&code.outs_size.to_le_bytes());
                    data_section.extend_from_slice(&0u16.to_le_bytes()); // tries
                    data_section.extend_from_slice(&0u32.to_le_bytes()); // debug
                    data_section.extend_from_slice(&(code.insns.len() as u32).to_le_bytes());
                    for w in &code.insns {
                        data_section.extend_from_slice(&w.to_le_bytes());
                    }
                    align4(&mut data_section);
                }
            }
        }

        // class_data
        let mut class_data_offs = Vec::new();
        for c in &self.classes {
            align4(&mut data_section);
            class_data_offs.push(data_section.len() as u32);
            write_uleb128(&mut data_section, 0); // static fields
            write_uleb128(&mut data_section, 0); // instance fields
            write_uleb128(&mut data_section, c.methods.len() as u32);
            write_uleb128(&mut data_section, 0); // virtual
            let mut last = 0u32;
            // sort by method_idx for diff encoding
            let mut methods = c.methods.clone();
            methods.sort_by_key(|m| m.method_idx);
            for m in methods {
                let diff = m.method_idx - last;
                last = m.method_idx;
                write_uleb128(&mut data_section, diff);
                write_uleb128(&mut data_section, m.access_flags);
                let code_off = m
                    .code
                    .as_ref()
                    .and_then(|_| code_offs.get(&m.method_idx).copied())
                    .unwrap_or(0);
                // placeholder; patch after data_base known — store relative for now
                write_uleb128(&mut data_section, code_off); // will add data_base below when writing ids... 
                // Problem: uleb of absolute offset needs data_base. We'll rebuild class_data after knowing data_base.
                let _ = code_off;
            }
        }

        // Recompute sizes for id tables
        let string_ids_size = self.strings.len() as u32;
        let type_ids_size = self.types.len() as u32;
        let proto_ids_size = self.protos.len() as u32;
        let field_ids_size = 0u32;
        let method_ids_size = self.methods.len() as u32;
        let class_defs_size = self.classes.len() as u32;

        let header_size = 0x70u32;
        let string_ids_off = header_size;
        let type_ids_off = string_ids_off + string_ids_size * 4;
        let proto_ids_off = type_ids_off + type_ids_size * 4;
        let field_ids_off = proto_ids_off + proto_ids_size * 12;
        let method_ids_off = field_ids_off + field_ids_size * 8;
        let class_defs_off = method_ids_off + method_ids_size * 8;
        let data_off = class_defs_off + class_defs_size * 32;
        // align data_off to 4
        let data_off = (data_off + 3) & !3;

        // Rebuild data section with absolute code offsets in class_data
        let mut data_section = Vec::new();
        let mut string_data_offs = Vec::new();
        for s in &self.strings {
            string_data_offs.push(data_off + data_section.len() as u32);
            write_uleb128(&mut data_section, s.chars().count() as u32);
            data_section.extend_from_slice(s.as_bytes());
            data_section.push(0);
        }
        let mut proto_param_offs = Vec::new();
        for (_shorty, _ret, params) in &self.protos {
            if params.is_empty() {
                proto_param_offs.push(0u32);
            } else {
                align4(&mut data_section);
                proto_param_offs.push(data_off + data_section.len() as u32);
                data_section.extend_from_slice(&(params.len() as u32).to_le_bytes());
                for p in params {
                    data_section.extend_from_slice(&p.to_le_bytes());
                }
            }
        }
        let mut code_offs: BTreeMap<u32, u32> = BTreeMap::new();
        for c in &self.classes {
            for m in &c.methods {
                if let Some(code) = &m.code {
                    align4(&mut data_section);
                    let off = data_off + data_section.len() as u32;
                    code_offs.insert(m.method_idx, off);
                    data_section.extend_from_slice(&code.registers_size.to_le_bytes());
                    data_section.extend_from_slice(&code.ins_size.to_le_bytes());
                    data_section.extend_from_slice(&code.outs_size.to_le_bytes());
                    data_section.extend_from_slice(&0u16.to_le_bytes());
                    data_section.extend_from_slice(&0u32.to_le_bytes());
                    data_section.extend_from_slice(&(code.insns.len() as u32).to_le_bytes());
                    for w in &code.insns {
                        data_section.extend_from_slice(&w.to_le_bytes());
                    }
                    align4(&mut data_section);
                }
            }
        }
        let mut class_data_offs = Vec::new();
        for c in &self.classes {
            align4(&mut data_section);
            class_data_offs.push(data_off + data_section.len() as u32);
            write_uleb128(&mut data_section, 0);
            write_uleb128(&mut data_section, 0);
            write_uleb128(&mut data_section, c.methods.len() as u32);
            write_uleb128(&mut data_section, 0);
            let mut methods = c.methods.clone();
            methods.sort_by_key(|m| m.method_idx);
            let mut last = 0u32;
            for m in methods {
                let diff = m.method_idx - last;
                last = m.method_idx;
                write_uleb128(&mut data_section, diff);
                write_uleb128(&mut data_section, m.access_flags);
                let code_off = code_offs.get(&m.method_idx).copied().unwrap_or(0);
                write_uleb128(&mut data_section, code_off);
            }
        }

        let file_size = data_off + data_section.len() as u32;
        let mut out = vec![0u8; file_size as usize];

        // magic
        out[0..8].copy_from_slice(b"dex\n035\0");
        // checksum/signature left 0 for fixtures (parsers typically don't verify in our path)
        write_u32(&mut out, 36, header_size);
        write_u32(&mut out, 40, 0x1234_5678);
        write_u32(&mut out, 44, 0); // link_size
        write_u32(&mut out, 48, 0); // link_off
        write_u32(&mut out, 52, 0); // map_off
        write_u32(&mut out, 56, string_ids_size);
        write_u32(&mut out, 60, string_ids_off);
        write_u32(&mut out, 64, type_ids_size);
        write_u32(&mut out, 68, type_ids_off);
        write_u32(&mut out, 72, proto_ids_size);
        write_u32(&mut out, 76, proto_ids_off);
        write_u32(&mut out, 80, field_ids_size);
        write_u32(&mut out, 84, field_ids_off);
        write_u32(&mut out, 88, method_ids_size);
        write_u32(&mut out, 92, method_ids_off);
        write_u32(&mut out, 96, class_defs_size);
        write_u32(&mut out, 100, class_defs_off);
        write_u32(&mut out, 104, data_section.len() as u32);
        write_u32(&mut out, 108, data_off);
        write_u32(&mut out, 32, file_size);

        for (i, off) in string_data_offs.iter().enumerate() {
            write_u32(&mut out, (string_ids_off as usize) + i * 4, *off);
        }
        for (i, sidx) in self.types.iter().enumerate() {
            write_u32(&mut out, (type_ids_off as usize) + i * 4, *sidx);
        }
        for (i, (shorty, ret, _)) in self.protos.iter().enumerate() {
            let base = proto_ids_off as usize + i * 12;
            write_u32(&mut out, base, *shorty);
            write_u32(&mut out, base + 4, *ret);
            write_u32(&mut out, base + 8, proto_param_offs[i]);
        }
        for (i, (class, proto, name)) in self.methods.iter().enumerate() {
            let base = method_ids_off as usize + i * 8;
            out[base..base + 2].copy_from_slice(&class.to_le_bytes());
            out[base + 2..base + 4].copy_from_slice(&proto.to_le_bytes());
            write_u32(&mut out, base + 4, *name);
        }
        for (i, c) in self.classes.iter().enumerate() {
            let base = class_defs_off as usize + i * 32;
            write_u32(&mut out, base, c.class_idx);
            write_u32(&mut out, base + 4, 0x0001); // public
            write_u32(&mut out, base + 8, c.superclass_idx);
            write_u32(&mut out, base + 12, 0); // interfaces
            write_u32(&mut out, base + 16, 0xffff_ffff); // source file
            write_u32(&mut out, base + 20, 0); // annotations
            write_u32(&mut out, base + 24, class_data_offs[i]);
            write_u32(&mut out, base + 28, 0); // static values
        }

        out[data_off as usize..].copy_from_slice(&data_section);

        // Adler32 checksum of file except magic+checksum field
        let checksum = adler32(&out[12..]);
        write_u32(&mut out, 8, checksum);

        out
    }
}

fn write_u32(buf: &mut [u8], off: usize, v: u32) {
    buf[off..off + 4].copy_from_slice(&v.to_le_bytes());
}

fn write_uleb128(buf: &mut Vec<u8>, mut v: u32) {
    loop {
        let mut b = (v & 0x7f) as u8;
        v >>= 7;
        if v != 0 {
            b |= 0x80;
        }
        buf.push(b);
        if v == 0 {
            break;
        }
    }
}

fn align4(buf: &mut Vec<u8>) {
    while buf.len() % 4 != 0 {
        buf.push(0);
    }
}

fn adler32(data: &[u8]) -> u32 {
    const MOD: u32 = 65521;
    let mut a = 1u32;
    let mut b = 0u32;
    for &byte in data {
        a = (a + byte as u32) % MOD;
        b = (b + a) % MOD;
    }
    (b << 16) | a
}

// Allow EncodedMeth clone for sort
impl Clone for EncodedMeth {
    fn clone(&self) -> Self {
        Self {
            method_idx: self.method_idx,
            access_flags: self.access_flags,
            code: self.code.as_ref().map(|c| CodeBlob {
                registers_size: c.registers_size,
                ins_size: c.ins_size,
                outs_size: c.outs_size,
                insns: c.insns.clone(),
            }),
        }
    }
}
