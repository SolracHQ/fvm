use super::parser::ParsedFile;
use super::syntax::{ParsedArgument, ParsedInstruction};
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

fn rodata_byte_len(args: &[ParsedArgument]) -> u32 {
    args.iter().map(|a| a.size() as u32).sum()
}

fn code_byte_len(instructions: &[ParsedInstruction]) -> u32 {
    instructions.iter().map(|i| i.size as u32).sum()
}

fn resolve_arg(
    arg: &ParsedArgument,
    resolved_labels: &std::collections::HashMap<String, (Section, u32)>,
) -> Result<Argument> {
    match arg {
        ParsedArgument::LabelRef(label_ref) => {
            let (section, addr) = resolved_labels
                .get(&label_ref.name)
                .copied()
                .ok_or_else(|| {
                    AssemblerError::resolver(
                        label_ref.file,
                        label_ref.span.clone(),
                        format!("Undefined label: {}", label_ref.name),
                    )
                })?;
            Ok(Argument::Label {
                address: addr,
                section,
            })
        }
        ParsedArgument::Value(argument) => Ok(argument.clone()),
    }
}

fn resolve_instruction(
    inst: ParsedInstruction,
    resolved_labels: &std::collections::HashMap<String, (Section, u32)>,
) -> Result<Instruction> {
    let mut args = [Argument::None, Argument::None, Argument::None];

    for (index, arg) in inst.arguments.iter().take(inst.argument_count).enumerate() {
        args[index] = resolve_arg(arg, resolved_labels)?;
    }

    Ok(Instruction::new(inst.opcode, args, inst.argument_count))
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
        .map(|inst| resolve_instruction(inst, &resolved_labels))
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
