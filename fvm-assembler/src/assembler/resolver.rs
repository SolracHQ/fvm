use super::parser::ParsedFile;
use crate::error::{AssemblerError, Result};
use fvm_core::argument::Argument;
use fvm_core::instruction::Instruction;
use fvm_core::section::Section;

pub struct ResolvedFile {
    pub rodata: Vec<Argument>,
    pub code: Vec<Instruction>,
    pub data: Vec<Argument>,
    // Absolute addresses of every label, in the same address space the VM sees.
    pub labels: std::collections::HashMap<String, u32>,
}

fn rodata_byte_len(args: &[Argument]) -> u32 {
    args.iter().map(|a| a.size() as u32).sum()
}

fn code_byte_len(instructions: &[Instruction]) -> u32 {
    instructions.iter().map(|i| i.size as u32).sum()
}

fn resolve_arg(
    arg: &Argument,
    resolved_labels: &std::collections::HashMap<String, (Section, u32)>,
) -> Result<Argument> {
    match arg {
        Argument::UnresolvedLabel(name, line, col) => {
            let (section, addr) = resolved_labels.get(name).copied().ok_or_else(|| {
                AssemblerError::resolver(*line, *col, format!("Undefined label: {name}"))
            })?;
            Ok(Argument::Label {
                address: addr,
                section,
            })
        }
        other => Ok(other.clone()),
    }
}

pub fn resolve(parsed: ParsedFile) -> Result<ResolvedFile> {
    let rodata_base: u32 = 0;
    let code_base: u32 = rodata_base + rodata_byte_len(&parsed.rodata);
    let data_base: u32 = code_base + code_byte_len(&parsed.code);

    // Build absolute address for every label.
    let resolved_labels: std::collections::HashMap<String, (Section, u32)> = parsed
        .labels
        .into_iter()
        .map(|(name, (section, offset))| {
            let base = match section {
                Section::RoData => rodata_base,
                Section::Code => code_base,
                Section::Data => data_base,
            };
            (name, (section, base + offset))
        })
        .collect();

    // Patch instructions.
    let code = parsed
        .code
        .into_iter()
        .map(|inst| {
            let mut args = inst.arguments;
            for arg in args.iter_mut().take(inst.argument_count) {
                *arg = resolve_arg(arg, &resolved_labels)?;
            }
            Ok(Instruction::new(inst.opcode, args, inst.argument_count))
        })
        .collect::<Result<Vec<_>>>()?;

    // Patch data-section label references (e.g. `dw some_label`).
    let rodata = parsed
        .rodata
        .into_iter()
        .map(|arg| resolve_arg(&arg, &resolved_labels))
        .collect::<Result<Vec<_>>>()?;

    let data = parsed
        .data
        .into_iter()
        .map(|arg| resolve_arg(&arg, &resolved_labels))
        .collect::<Result<Vec<_>>>()?;

    Ok(ResolvedFile {
        rodata,
        code,
        data,
        labels: resolved_labels
            .into_iter()
            .map(|(name, (_, addr))| (name, addr))
            .collect(),
    })
}
