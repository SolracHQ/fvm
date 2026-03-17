use crate::error::*;
use crate::section::Section;
use crate::utils::OkAble;

/// This module defines the `Argument` enum, which represents the different types of arguments that can be used in instructions.
#[derive(Debug, Clone)]
pub enum Argument {
    None,
    Register(super::register::RegisterEncoding),
    Label { address: u32, section: Section },
    Inmm8(u8),
    Inmm16(u16),
    Inmm32(u32),
}

impl Argument {
    pub fn size(&self) -> usize {
        match self {
            Argument::None => 0,
            Argument::Register(_) => 1,
            Argument::Inmm8(_) => 1,
            Argument::Inmm16(_) => 2,
            Argument::Inmm32(_) => 4,
            Argument::Label { .. } => 4, // Labels are resolved to 32-bit addresses
        }
    }

    pub fn as_bytes(&self) -> Result<Vec<super::types::Byte>> {
        match self {
            Argument::None => vec![],
            Argument::Register(reg) => vec![reg.0],
            Argument::Inmm8(val) => vec![*val],
            Argument::Inmm16(val) => {
                let bytes = val.to_be_bytes();
                vec![bytes[0], bytes[1]]
            }
            Argument::Inmm32(val) => {
                let bytes = val.to_be_bytes();
                vec![bytes[0], bytes[1], bytes[2], bytes[3]]
            }
            Argument::Label { address: val, .. } => {
                let bytes = val.to_be_bytes();
                vec![bytes[0], bytes[1], bytes[2], bytes[3]]
            }
        }
        .ok()
    }
}
