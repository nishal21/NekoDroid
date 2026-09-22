//! Android APK / DEX HLE runtime for nekodroid (browser Wasm).

pub mod apk;
pub mod axml;
pub mod decode;
pub mod dex;
pub mod error;
pub mod fixture;
pub mod hle;
pub mod interp;
pub mod opcodes;
pub mod runtime;

pub use error::AndroidError;
pub use runtime::AppRuntime;
