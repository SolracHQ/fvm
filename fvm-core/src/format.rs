use crate::error::FvmError;
use crate::section::Section;
use crate::types::{Byte, Word};

#[derive(Debug, Clone)]
pub struct FvmFormat {
    pub version: Byte,
    pub entry_point: Word,
    pub ro_data: Vec<Byte>,
    pub code: Vec<Byte>,
    pub rw_data: Vec<Byte>,
    pub relocations: Vec<(Section, Word)>,
}

const FVM_MAGIC: [Byte; 4] = [0x46, 0x56, 0x4D, 0x21];
const FVM_VERSION: Byte = 3;

impl FvmFormat {
    pub fn new(
        entry_point: Word,
        ro_data: Vec<Byte>,
        code: Vec<Byte>,
        rw_data: Vec<Byte>,
        relocations: Vec<(Section, Word)>,
    ) -> Self {
        Self {
            version: FVM_VERSION,
            entry_point,
            ro_data,
            code,
            rw_data,
            relocations,
        }
    }

    pub fn to_bytes(&self) -> Result<Vec<Byte>, FvmError> {
        let mut bytes = Vec::new();
        let ro_len: Word = self.ro_data.len() as Word;
        let code_len: Word = self.code.len() as Word;
        let rw_len: Word = self.rw_data.len() as Word;
        let reloc_count: Word = self.relocations.len() as Word;

        // Magic number
        bytes.extend_from_slice(&FVM_MAGIC);
        // Version
        bytes.push(self.version);
        // Entry point
        bytes.extend_from_slice(&self.entry_point.to_be_bytes());
        // Section lengths and relocation count
        bytes.extend_from_slice(&ro_len.to_be_bytes());
        bytes.extend_from_slice(&code_len.to_be_bytes());
        bytes.extend_from_slice(&rw_len.to_be_bytes());
        bytes.extend_from_slice(&reloc_count.to_be_bytes());

        // Section payloads
        bytes.extend_from_slice(&self.ro_data);
        bytes.extend_from_slice(&self.code);
        bytes.extend_from_slice(&self.rw_data);

        // Relocations
        for reloc in &self.relocations {
            bytes.push(reloc.0 as u8);
            bytes.extend_from_slice(&reloc.1.to_be_bytes());
        }
        Ok(bytes)
    }

    pub fn from_bytes(bytes: &[Byte]) -> Result<Self, FvmError> {
        const HEADER_SIZE: usize = 25;

        if bytes.len() < HEADER_SIZE {
            return Err(FvmError::InvalidFormat(
                "binary is shorter than the 25-byte header".to_string(),
            ));
        }
        if bytes[..4] != FVM_MAGIC {
            return Err(FvmError::InvalidFormat(
                "binary has an invalid magic header".to_string(),
            ));
        }

        let version = bytes[4];
        if version != FVM_VERSION {
            return Err(FvmError::UnsupportedVersion(version));
        }

        let entry_point = read_be_u32(bytes, 5)?;
        let ro_len = read_be_u32(bytes, 9)? as usize;
        let code_len = read_be_u32(bytes, 13)? as usize;
        let rw_len = read_be_u32(bytes, 17)? as usize;
        let reloc_count = read_be_u32(bytes, 21)? as usize;

        let payload_end = HEADER_SIZE
            .checked_add(ro_len)
            .and_then(|end| end.checked_add(code_len))
            .and_then(|end| end.checked_add(rw_len))
            .ok_or_else(|| FvmError::InvalidFormat("binary section sizes overflow".to_string()))?;
        let reloc_end = payload_end
            .checked_add(reloc_count.checked_mul(5).ok_or_else(|| {
                FvmError::InvalidFormat("relocation table size overflow".to_string())
            })?)
            .ok_or_else(|| FvmError::InvalidFormat("binary size overflow".to_string()))?;

        if bytes.len() < reloc_end {
            return Err(FvmError::InvalidFormat("binary is truncated".to_string()));
        }
        if bytes.len() != reloc_end {
            return Err(FvmError::InvalidFormat(
                "binary has trailing bytes after the relocation table".to_string(),
            ));
        }

        let mut cursor = HEADER_SIZE;
        let ro_data = bytes[cursor..cursor + ro_len].to_vec();
        cursor += ro_len;
        let code = bytes[cursor..cursor + code_len].to_vec();
        cursor += code_len;
        let rw_data = bytes[cursor..cursor + rw_len].to_vec();
        cursor += rw_len;

        let mut relocations = Vec::with_capacity(reloc_count);
        for _ in 0..reloc_count {
            let section = Section::try_from(bytes[cursor]).map_err(|_| {
                FvmError::InvalidFormat(format!(
                    "invalid relocation section byte 0x{:02X}",
                    bytes[cursor]
                ))
            })?;
            let offset = read_be_u32(bytes, cursor + 1)?;
            relocations.push((section, offset));
            cursor += 5;
        }

        Ok(Self {
            version,
            entry_point,
            ro_data,
            code,
            rw_data,
            relocations,
        })
    }
}

fn read_be_u32(bytes: &[Byte], offset: usize) -> Result<Word, FvmError> {
    let end = offset
        .checked_add(4)
        .ok_or_else(|| FvmError::InvalidFormat("binary offset overflow".to_string()))?;
    let slice = bytes.get(offset..end).ok_or_else(|| {
        FvmError::InvalidFormat("unexpected end of binary while reading u32".to_string())
    })?;
    Ok(Word::from_be_bytes([
        slice[0], slice[1], slice[2], slice[3],
    ]))
}
