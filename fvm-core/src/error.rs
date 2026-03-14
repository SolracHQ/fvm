use thiserror::Error;

#[derive(Error, Debug, Clone)]
pub enum FvmError {
    #[error("Invalid register encoding: 0x{0:02X}")]
    InvalidRegisterEncoding(u8),
    #[error("Invalid format: {0}")]
    InvalidFormat(String),
    #[error("Unsupported format version: {0}")]
    UnsupportedVersion(u8),
    #[error("Value out of range for Word: {0}")]
    ValueOutOfRangeForWord(usize),
    #[error("Unresolved label: {0} at line {1}, column {2}")]
    UnresolvedLabel(String, u32, u32),
}

pub type Result<T> = std::result::Result<T, FvmError>;
