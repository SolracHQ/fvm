use std::fs;

use fvm_core::{format::FvmFormat, section::Section};

use crate::error::{VmError, VmResult};

const PAGE_SIZE: u32 = 4096;
const STACK_BASE: u32 = 0xFFC0_0000;

#[derive(Clone, Copy)]
pub(crate) struct SectionPlacement {
    assumed_base: u32,
    pub(crate) actual_base: u32,
    pub(crate) phys_base: u32,
    pub(crate) len: u32,
    pub(crate) aligned_len: u32,
}

pub(crate) struct PatchedProgram {
    pub(crate) entry_point: u32,
    pub(crate) ro_data: Vec<u8>,
    pub(crate) code: Vec<u8>,
    pub(crate) rw_data: Vec<u8>,
    pub(crate) sections: [Option<SectionPlacement>; 3],
}

pub(crate) fn load_and_patch_program(
    rom_path: &str,
    reserved_bytes: u32,
    max_section_phys_end: u32,
) -> VmResult<PatchedProgram> {
    let rom_bytes = fs::read(rom_path).map_err(|error| {
        VmError::InvalidRomImage(format!("failed to read '{rom_path}': {error}"))
    })?;
    let mut rom = FvmFormat::from_bytes(&rom_bytes)?;
    let sections = compute_section_layouts(&rom, reserved_bytes, max_section_phys_end)?;

    patch_relocations(&mut rom, &sections)?;
    let entry_point = patch_entry_point(rom.entry_point, &sections)?;

    Ok(PatchedProgram {
        entry_point,
        ro_data: rom.ro_data,
        code: rom.code,
        rw_data: rom.rw_data,
        sections,
    })
}

fn compute_section_layouts(
    rom: &FvmFormat,
    reserved_bytes: u32,
    max_section_phys_end: u32,
) -> VmResult<[Option<SectionPlacement>; 3]> {
    let lengths = [
        rom.ro_data.len() as u32,
        rom.code.len() as u32,
        rom.rw_data.len() as u32,
    ];
    let assumed_bases = [
        0,
        lengths[0],
        lengths[0]
            .checked_add(lengths[1])
            .ok_or(VmError::AddressOverflow)?,
    ];

    let mut actual_cursor = reserved_bytes;
    let mut phys_cursor = reserved_bytes;
    let mut placements = [None, None, None];

    for (index, len) in lengths.into_iter().enumerate() {
        if len == 0 {
            continue;
        }

        actual_cursor = align_up(actual_cursor)?;
        phys_cursor = align_up(phys_cursor)?;
        let aligned_len = align_up(len)?;
        let phys_end = phys_cursor
            .checked_add(aligned_len)
            .ok_or(VmError::AddressOverflow)?;
        let actual_end = actual_cursor
            .checked_add(aligned_len)
            .ok_or(VmError::AddressOverflow)?;

        if actual_end > STACK_BASE {
            return Err(VmError::Layout(
                "loaded sections overlap the fixed stack virtual region".to_string(),
            ));
        }
        if phys_end > max_section_phys_end {
            return Err(VmError::Layout(
                "loaded sections do not fit below the reserved 4 MiB stack backing region"
                    .to_string(),
            ));
        }

        placements[index] = Some(SectionPlacement {
            assumed_base: assumed_bases[index],
            actual_base: actual_cursor,
            phys_base: phys_cursor,
            len,
            aligned_len,
        });

        actual_cursor = actual_end;
        phys_cursor = phys_end;
    }

    Ok(placements)
}

fn patch_relocations(rom: &mut FvmFormat, layouts: &[Option<SectionPlacement>; 3]) -> VmResult<()> {
    for (slot_section, offset) in rom.relocations.clone() {
        let target_bytes = section_bytes_mut(rom, slot_section);
        let slot_value = read_word_from_slice(target_bytes, offset as usize)?;
        let target_section = infer_target_section(slot_value, layouts)?;
        let patched = rebase_address(
            slot_value,
            layouts[section_index(target_section)].ok_or_else(|| {
                VmError::InvalidRomImage(format!(
                    "relocation references empty target section {:?}",
                    target_section
                ))
            })?,
        )?;
        write_word_to_slice(target_bytes, offset as usize, patched)?;
    }

    Ok(())
}

fn patch_entry_point(entry_point: u32, layouts: &[Option<SectionPlacement>; 3]) -> VmResult<u32> {
    let section = infer_target_section(entry_point, layouts)?;
    rebase_address(entry_point, layouts[section_index(section)].unwrap())
}

fn infer_target_section(
    address: u32,
    layouts: &[Option<SectionPlacement>; 3],
) -> VmResult<Section> {
    for section in [Section::RoData, Section::Code, Section::Data] {
        if let Some(layout) = layouts[section_index(section)] {
            let end = layout
                .assumed_base
                .checked_add(layout.len)
                .ok_or(VmError::AddressOverflow)?;
            if address >= layout.assumed_base && address < end {
                return Ok(section);
            }
        }
    }

    Err(VmError::InvalidRomImage(format!(
        "address 0x{address:08X} does not fall within any assumed section range"
    )))
}

fn rebase_address(address: u32, layout: SectionPlacement) -> VmResult<u32> {
    let relative = address.checked_sub(layout.assumed_base).ok_or_else(|| {
        VmError::InvalidRomImage(format!(
            "address 0x{address:08X} is below assumed section base 0x{:08X}",
            layout.assumed_base
        ))
    })?;
    layout
        .actual_base
        .checked_add(relative)
        .ok_or(VmError::AddressOverflow)
}

fn read_word_from_slice(bytes: &[u8], offset: usize) -> VmResult<u32> {
    let end = offset.checked_add(4).ok_or(VmError::AddressOverflow)?;
    let slice = bytes
        .get(offset..end)
        .ok_or_else(|| VmError::InvalidRomImage("relocation slot is out of bounds".to_string()))?;
    Ok(u32::from_be_bytes([slice[0], slice[1], slice[2], slice[3]]))
}

fn write_word_to_slice(bytes: &mut [u8], offset: usize, value: u32) -> VmResult<()> {
    let end = offset.checked_add(4).ok_or(VmError::AddressOverflow)?;
    let slice = bytes
        .get_mut(offset..end)
        .ok_or_else(|| VmError::InvalidRomImage("relocation slot is out of bounds".to_string()))?;
    slice.copy_from_slice(&value.to_be_bytes());
    Ok(())
}

fn section_index(section: Section) -> usize {
    match section {
        Section::RoData => 0,
        Section::Code => 1,
        Section::Data => 2,
    }
}

fn section_bytes_mut(rom: &mut FvmFormat, section: Section) -> &mut [u8] {
    match section {
        Section::RoData => rom.ro_data.as_mut_slice(),
        Section::Code => rom.code.as_mut_slice(),
        Section::Data => rom.rw_data.as_mut_slice(),
    }
}

fn align_up(value: u32) -> VmResult<u32> {
    if value == 0 {
        return Ok(0);
    }

    value
        .checked_add(PAGE_SIZE - 1)
        .ok_or(VmError::AddressOverflow)
        .map(|value| value / PAGE_SIZE * PAGE_SIZE)
}
