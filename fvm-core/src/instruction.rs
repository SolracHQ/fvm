use super::{error::FvmError, types::Byte};

/// Represents a single instruction in the FVM.
#[derive(Debug, Clone)]
pub struct Instruction {
    pub opcode: super::opcode::Op,
    pub arguments: [super::argument::Argument; 3],
    pub argument_count: usize,
    pub size: usize,
}

impl Instruction {
    pub fn new(
        opcode: super::opcode::Op,
        arguments: [super::argument::Argument; 3],
        argument_count: usize,
    ) -> Self {
        let size = 1 + arguments
            .iter()
            .take(argument_count)
            .map(|arg| arg.size())
            .sum::<usize>(); // 1 byte for opcode + size of each argument
        Self {
            opcode,
            arguments,
            argument_count,
            size,
        }
    }

    pub fn as_bytes(&self) -> Result<Vec<Byte>, FvmError> {
        let mut bytes = Vec::new();
        bytes.push(self.opcode as u8);
        for arg in self.arguments.iter().take(self.argument_count) {
            bytes.extend(arg.as_bytes()?);
        }
        Ok(bytes)
    }
}
