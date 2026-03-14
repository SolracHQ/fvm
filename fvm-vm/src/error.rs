use thiserror::Error;

use crate::vm::interrupts::Interrupt;

#[derive(Error, Debug, Clone)]
pub enum VmError {
    #[error("Interrupt {0:?}")]
    Interrupt(Interrupt),

    #[error("Invalid ROM image: {0}")]
    InvalidRomImage(String),

    #[error("Unsupported ROM format version {0}")]
    UnsupportedRomVersion(u8),

    #[error("VM layout error: {0}")]
    Layout(String),

    #[error("Address arithmetic overflow")]
    AddressOverflow,

    #[error("Unmapped address: 0x{address:08X} in context 0x{context:08X}")]
    UnmappedAddress { context: u32, address: u32 },

    #[error(
        "Permission denied at 0x{address:08X} in context 0x{context:08X}: required {required}, got {got}"
    )]
    PermissionDenied {
        context: u32,
        address: u32,
        required: &'static str,
        got: u8,
    },

    #[error("Device error on '{device:?}' at offset 0x{offset:08X}: {message}")]
    DeviceError {
        device: [u8; 8],
        offset: u32,
        message: String,
    },

    #[error("MMAP failed: physical address 0x{address:08X} is not mapped to any device")]
    UnmappedPhysicalAddress { address: u32 },

    #[error("Value {0} is out of range for a 32-bit word")]
    ValueOutOfRangeForWord(usize),

    #[error("Unresolved label: {0}")]
    UnresolvedLabel(String),

    #[error("Invalid opcode 0x{opcode:02X} at address 0x{address:08X}")]
    InvalidOpcode { opcode: u8, address: u32 },

    #[error("Core error: {0}")]
    CoreErrors(#[from] fvm_core::error::FvmError),
}

pub type VmResult<T> = std::result::Result<T, VmError>;
