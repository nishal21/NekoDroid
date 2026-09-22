use thiserror::Error;

#[derive(Debug, Error)]
pub enum AndroidError {
    #[error("not a ZIP/APK: {0}")]
    NotZip(String),
    #[error("missing classes.dex in APK")]
    MissingDex,
    #[error("DEX parse error: {0}")]
    Dex(String),
    #[error("AXML/manifest error: {0}")]
    Manifest(String),
    #[error("runtime error: {0}")]
    Runtime(String),
    #[error("VM halted: unimplemented opcode 0x{op:02x} at pc={pc}")]
    UnimplementedOpcode { op: u8, pc: u32 },
    #[error("VM error: {0}")]
    Vm(String),
}

pub type Result<T> = std::result::Result<T, AndroidError>;
