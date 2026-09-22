//! APK (ZIP) loader.

use std::io::{Cursor, Read};

use zip::ZipArchive;

use crate::android::axml;
use crate::android::dex::DexFile;
use crate::android::error::{AndroidError, Result};

#[derive(Debug, Clone)]
pub struct LoadedApk {
    pub package_name: String,
    pub launcher_activity: Option<String>,
    pub dex: DexFile,
    pub apk_size: usize,
}

pub fn load_apk_bytes(apk_bytes: &[u8]) -> Result<LoadedApk> {
    if apk_bytes.len() < 4 || &apk_bytes[0..2] != b"PK" {
        return Err(AndroidError::NotZip("missing PK header".into()));
    }
    let cursor = Cursor::new(apk_bytes);
    let mut archive = ZipArchive::new(cursor)
        .map_err(|e| AndroidError::NotZip(e.to_string()))?;

    let mut dex_bytes: Option<Vec<u8>> = None;
    let mut manifest_bytes: Option<Vec<u8>> = None;

    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .map_err(|e| AndroidError::NotZip(e.to_string()))?;
        let name = file.name().to_string();
        if name == "classes.dex" {
            let mut buf = Vec::new();
            file.read_to_end(&mut buf)
                .map_err(|e| AndroidError::NotZip(e.to_string()))?;
            dex_bytes = Some(buf);
        } else if name == "AndroidManifest.xml" {
            let mut buf = Vec::new();
            file.read_to_end(&mut buf)
                .map_err(|e| AndroidError::NotZip(e.to_string()))?;
            manifest_bytes = Some(buf);
        }
    }

    let dex_bytes = dex_bytes.ok_or(AndroidError::MissingDex)?;
    let dex = DexFile::parse(&dex_bytes)?;

    let (package_name, launcher_activity) = if let Some(m) = manifest_bytes.as_deref() {
        axml::parse_package_and_launcher(m).unwrap_or_else(|_| {
            (
                guess_package_from_dex(&dex),
                guess_activity_from_dex(&dex),
            )
        })
    } else {
        (
            guess_package_from_dex(&dex),
            guess_activity_from_dex(&dex),
        )
    };

    Ok(LoadedApk {
        package_name,
        launcher_activity,
        dex,
        apk_size: apk_bytes.len(),
    })
}

fn guess_package_from_dex(dex: &DexFile) -> String {
    for c in &dex.classes {
        if let Some(pkg) = descriptor_to_package(&c.descriptor) {
            return pkg;
        }
    }
    "unknown".into()
}

fn guess_activity_from_dex(dex: &DexFile) -> Option<String> {
    for c in &dex.classes {
        if c.descriptor.contains("Hello") || c.descriptor.contains("Main") || c.descriptor.contains("Activity") {
            return Some(c.descriptor.clone());
        }
    }
    dex.classes.first().map(|c| c.descriptor.clone())
}

fn descriptor_to_package(desc: &str) -> Option<String> {
    // Lcom/foo/Bar; -> com.foo
    let s = desc.strip_prefix('L')?.strip_suffix(';')?;
    let parts: Vec<&str> = s.split('/').collect();
    if parts.len() < 2 {
        return Some(s.replace('/', "."));
    }
    Some(parts[..parts.len() - 1].join("."))
}
