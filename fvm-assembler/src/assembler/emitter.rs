use super::resolver::ResolvedFile;
use crate::error::{AssemblerError, Result};
use fvm_core::argument::Argument;
use fvm_core::format::FvmFormat;
use fvm_core::section::Section;
use fvm_core::types::{Byte, Word};

// Byte offset within a section where this argument's 4-byte address slot begins.
struct RelocationSite {
    section: Section,
    offset: u32,
}

struct SectionEmitter {
    section: Section,
    bytes: Vec<Byte>,
    relocations: Vec<RelocationSite>,
}

impl SectionEmitter {
    fn new(section: Section) -> Self {
        Self {
            section,
            bytes: Vec::new(),
            relocations: Vec::new(),
        }
    }

    fn emit_arg(&mut self, arg: &Argument) -> Result<()> {
        match arg {
            Argument::None => {}

            Argument::Register(reg) => self.bytes.push(reg.0),

            Argument::Inmm8(v) => self.bytes.push(*v),

            Argument::Inmm16(v) => {
                let [hi, lo] = v.to_be_bytes();
                self.bytes.push(hi);
                self.bytes.push(lo);
            }

            Argument::Inmm32(v) => {
                for b in v.to_be_bytes() {
                    self.bytes.push(b);
                }
            }

            Argument::Label { address, .. } => {
                self.relocations.push(RelocationSite {
                    section: self.section,
                    offset: self.bytes.len() as u32,
                });
                for b in address.to_be_bytes() {
                    self.bytes.push(b);
                }
            }

            Argument::UnresolvedLabel(name, line, col) => {
                return Err(AssemblerError::emit(
                    *line,
                    *col,
                    format!("Unresolved label: {name}"),
                ));
            }
        }
        Ok(())
    }
}

fn emit_data_section(
    args: &[Argument],
    section: Section,
) -> Result<(Vec<Byte>, Vec<RelocationSite>)> {
    let mut emitter = SectionEmitter::new(section);
    for arg in args {
        emitter.emit_arg(arg)?;
    }
    Ok((emitter.bytes, emitter.relocations))
}

fn emit_code_section(
    instructions: &[fvm_core::instruction::Instruction],
) -> Result<(Vec<Byte>, Vec<RelocationSite>)> {
    let mut emitter = SectionEmitter::new(Section::Code);
    for inst in instructions {
        emitter.bytes.push(inst.opcode as u8);
        for arg in inst.arguments.iter().take(inst.argument_count) {
            emitter.emit_arg(arg)?;
        }
    }
    Ok((emitter.bytes, emitter.relocations))
}

pub fn emit(resolved: ResolvedFile, entry_label: &str) -> Result<FvmFormat> {
    let entry_addr = resolved.labels.get(entry_label).copied().ok_or_else(|| {
        AssemblerError::emit(0, 0, format!("Entry point label not found: {entry_label}"))
    })?;

    let (ro_bytes, ro_relocs) = emit_data_section(&resolved.rodata, Section::RoData)?;
    let (code_bytes, code_relocs) = emit_code_section(&resolved.code)?;
    let (rw_bytes, rw_relocs) = emit_data_section(&resolved.data, Section::Data)?;

    let relocations: Vec<(Section, Word)> = ro_relocs
        .into_iter()
        .chain(code_relocs)
        .chain(rw_relocs)
        .map(|site| (site.section, site.offset))
        .collect();

    Ok(FvmFormat::new(
        entry_addr,
        ro_bytes,
        code_bytes,
        rw_bytes,
        relocations,
    ))
}
