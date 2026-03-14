use crate::vm::flags::Flags;

/// Register file: 16 general-purpose registers, plus IP, SP, and flags.
#[derive(Debug, Clone)]
pub struct RegisterFile {
    pub regs: [u32; 16],
    pub ip: u32,
    pub sp: u32,
    pub flags: Flags,
}

impl RegisterFile {
    pub fn new() -> Self {
        Self {
            regs: [0u32; 16],
            ip: 0,
            sp: 0,
            flags: Flags::new(),
        }
    }
}
