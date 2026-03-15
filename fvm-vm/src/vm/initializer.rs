use std::{collections::HashMap, rc::Rc};

use fvm_core::types::Word;

use crate::error::VmResult;

use super::{
    bus::{PhysicalRegionInfo, PAGE_SIZE},
    constants::{STACK_SIZE, STACK_BASE, STACK_TOP, KERNEL_CONTEXT},
    device::PortMappedDevice,
    registers::RegisterFile,
};

/// Kernel mapping kinds for tracking loaded sections
#[repr(u8)]
#[derive(Clone, Copy, Debug)]
pub enum KernelMappingKind {
    LoaderInfo = 0,
    Rodata = 1,
    Code = 2,
    Data = 3,
    Stack = 4,
}

impl KernelMappingKind {
    pub fn as_byte(self) -> u8 {
        self as u8
    }
}

/// A kernel-managed virtual memory mapping
#[derive(Clone, Copy, Debug)]
pub struct KernelMapping {
    /// The type of region being mapped (loader info, code, data, stack, etc)
    pub kind: KernelMappingKind,
    /// Physical base address of the region (in bytes)
    pub phys_base: u32,
    /// Virtual base address of the region (in bytes)
    pub virt_base: u32,
    /// Number of pages in this mapping
    pub page_count: u32,
}

/// Calculate loader info size in bytes given device counts
pub fn calculate_loader_info_size(num_regions: u32, num_ports: u32) -> u32 {
    // Memory regions: 4 (count) + (24 * num_regions)
    let memory_size = 4 + (num_regions * 24);
    // Port devices: 4 (count) + (12 * num_ports)
    let ports_size = 4 + (num_ports * 12);
    // Kernel mappings: 4 (count) + (13 * 5) - always 5 mappings
    let mappings_size = 4 + (5 * 13);
    
    memory_size + ports_size + mappings_size
}

/// Calculate loader info size in pages
pub fn calculate_loader_info_pages(num_regions: u32, num_ports: u32) -> u32 {
    calculate_loader_info_size(num_regions, num_ports).div_ceil(PAGE_SIZE)
}

/// Serialize and write loader info to RAM
pub fn write_loader_info(
    bus: &crate::vm::bus::Bus,
    regions: &[PhysicalRegionInfo],
    port_devices: &HashMap<Word, Rc<dyn PortMappedDevice>>,
    mappings: &[KernelMapping],
) -> VmResult<()> {
    let mut bytes = Vec::new();

    // Write memory regions
    bytes.extend_from_slice(&(regions.len() as u32).to_be_bytes());
    for region in regions {
        bytes.extend_from_slice(&region.phys_base.to_be_bytes());
        bytes.extend_from_slice(&region.size.to_be_bytes());
        bytes.extend_from_slice(&region.id);
        bytes.push(default_permissions_for_device(region.id));
        bytes.extend_from_slice(&[0; 7]); // padding
    }

    // Write port devices
    bytes.extend_from_slice(&(port_devices.len() as u32).to_be_bytes());
    let mut ports: Vec<_> = port_devices.iter().collect();
    ports.sort_by_key(|(port, _)| *port);
    for (port, device) in ports {
        bytes.extend_from_slice(&device.id());
        bytes.extend_from_slice(&port.to_be_bytes());
    }

    // Write kernel mappings (always 5)
    bytes.extend_from_slice(&(mappings.len() as u32).to_be_bytes());
    for mapping in mappings {
        bytes.push(mapping.kind.as_byte());
        bytes.extend_from_slice(&mapping.phys_base.to_be_bytes());
        bytes.extend_from_slice(&mapping.virt_base.to_be_bytes());
        bytes.extend_from_slice(&mapping.page_count.to_be_bytes());
    }

    bus.write_physical_bytes(0, &bytes)?;
    Ok(())
}

fn default_permissions_for_device(_id: [u8; 8]) -> u8 {
    super::bus::perm::READ | super::bus::perm::WRITE
}

/// Initialize VM: set up loader info, load program, and prepare execution
pub fn initialize(
    vm: &mut crate::vm::VM,
    rom_path: String,
) -> VmResult<()> {
    let physical_regions = vm.bus.physical_regions();
    let main_ram = physical_regions
        .first()
        .ok_or_else(|| crate::error::VmError::Layout("VM requires a main RAM device".to_string()))?;

    if main_ram.size < STACK_SIZE {
        return Err(crate::error::VmError::Layout(format!(
            "main RAM must be at least {} bytes to back the fixed 4 MiB stack",
            STACK_SIZE
        )));
    }

    vm.bus.set_context(KERNEL_CONTEXT);
    vm.files[0].cr = KERNEL_CONTEXT;

    // Calculate loader info size (always 5 mappings for the 5 KernelMappingKind values)
    let loader_pages = calculate_loader_info_pages(physical_regions.len() as u32, vm.port_devices.len() as u32);
    let program_start_addr = loader_pages * PAGE_SIZE;

    // Load and patch program
    let patched_program = super::program_patcher::load_and_patch_program(
        &rom_path,
        program_start_addr,
        main_ram.size - STACK_SIZE,
    )?;

    // Load sections and collect mappings
    let mut mappings = vec![
        KernelMapping {
            kind: KernelMappingKind::LoaderInfo,
            phys_base: 0,
            virt_base: 0,
            page_count: loader_pages,
        },
    ];

    // Load rodata
    let mut rodata_mapping = KernelMapping {
        kind: KernelMappingKind::Rodata,
        phys_base: 0,
        virt_base: 0,
        page_count: 0,
    };
    if let Some(placement) = patched_program.sections[0] {
        vm.bus.write_physical_bytes(placement.phys_base, &patched_program.ro_data)?;
        vm.bus.mmap(
            KERNEL_CONTEXT,
            placement.actual_base / PAGE_SIZE,
            placement.phys_base / PAGE_SIZE,
            placement.aligned_len / PAGE_SIZE,
            super::bus::perm::READ,
        )?;
        rodata_mapping.phys_base = placement.phys_base;
        rodata_mapping.virt_base = placement.actual_base;
        rodata_mapping.page_count = placement.aligned_len / PAGE_SIZE;
    }
    mappings.push(rodata_mapping);

    // Load code
    let mut code_mapping = KernelMapping {
        kind: KernelMappingKind::Code,
        phys_base: 0,
        virt_base: 0,
        page_count: 0,
    };
    if let Some(placement) = patched_program.sections[1] {
        vm.bus.write_physical_bytes(placement.phys_base, &patched_program.code)?;
        vm.bus.mmap(
            KERNEL_CONTEXT,
            placement.actual_base / PAGE_SIZE,
            placement.phys_base / PAGE_SIZE,
            placement.aligned_len / PAGE_SIZE,
            super::bus::perm::READ | super::bus::perm::EXECUTE,
        )?;
        code_mapping.phys_base = placement.phys_base;
        code_mapping.virt_base = placement.actual_base;
        code_mapping.page_count = placement.aligned_len / PAGE_SIZE;
    }
    mappings.push(code_mapping);

    // Load data
    let mut data_mapping = KernelMapping {
        kind: KernelMappingKind::Data,
        phys_base: 0,
        virt_base: 0,
        page_count: 0,
    };
    if let Some(placement) = patched_program.sections[2] {
        vm.bus.write_physical_bytes(placement.phys_base, &patched_program.rw_data)?;
        vm.bus.mmap(
            KERNEL_CONTEXT,
            placement.actual_base / PAGE_SIZE,
            placement.phys_base / PAGE_SIZE,
            placement.aligned_len / PAGE_SIZE,
            super::bus::perm::READ | super::bus::perm::WRITE,
        )?;
        data_mapping.phys_base = placement.phys_base;
        data_mapping.virt_base = placement.actual_base;
        data_mapping.page_count = placement.aligned_len / PAGE_SIZE;
    }
    mappings.push(data_mapping);

    // Map stack
    let stack_phys_base = main_ram.size - STACK_SIZE;
    vm.bus.mmap(
        KERNEL_CONTEXT,
        STACK_BASE / PAGE_SIZE,
        stack_phys_base / PAGE_SIZE,
        STACK_SIZE / PAGE_SIZE,
        super::bus::perm::READ | super::bus::perm::WRITE,
    )?;
    mappings.push(KernelMapping {
        kind: KernelMappingKind::Stack,
        phys_base: stack_phys_base,
        virt_base: STACK_BASE,
        page_count: STACK_SIZE / PAGE_SIZE,
    });

    // All 5 mappings are now populated (LoaderInfo, Rodata, Code, Data, Stack)

    // Write loader info
    write_loader_info(&vm.bus, &physical_regions, &vm.port_devices, &mappings[..5])?;

    // Map loader info as read-only
    vm.bus.mmap(
        KERNEL_CONTEXT,
        0,
        0,
        loader_pages,
        super::bus::perm::READ,
    )?;

    // Set up execution
    vm.files[0].ip = patched_program.entry_point;
    vm.files[0].sp = STACK_TOP;
    vm.files[1] = RegisterFile::new();
    vm.active = KERNEL_CONTEXT as usize;

    Ok(())
}
