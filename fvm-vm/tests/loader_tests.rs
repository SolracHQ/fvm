use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use fvm_core::{format::FvmFormat, opcode::Op, section::Section};
use fvm_vm::{
    error::VmError,
    vm::{
        VM,
        config::{DeviceConfig, GeneralDeviceConfig, VmConfig},
        interrupts::Interrupt,
    },
};

const PAGE_SIZE: u32 = 4096;
const STACK_BASE: u32 = 0xFFC0_0000;
const STACK_TOP: u32 = 0xFFFF_FFFF;
const STACK_FRAME_SIZE: u32 = 73;

static NEXT_ROM_ID: AtomicU64 = AtomicU64::new(0);

struct RomFixture {
    path: PathBuf,
}

impl RomFixture {
    fn new(bytes: &[u8]) -> Self {
        let id = NEXT_ROM_ID.fetch_add(1, Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("fvm-vm-test-{}-{}.fo", std::process::id(), id));
        fs::write(&path, bytes).expect("failed to write temporary ROM image");
        Self { path }
    }
}

impl Drop for RomFixture {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

fn align_up(value: u32) -> u32 {
    value.div_ceil(PAGE_SIZE) * PAGE_SIZE
}

fn main_ram_size() -> u32 {
    6 * 1024 * 1024 + 123
}

fn minimal_rom() -> RomFixture {
    let rom = FvmFormat::new(0, Vec::new(), vec![Op::Halt as u8], Vec::new(), Vec::new());
    RomFixture::new(&rom.to_bytes().expect("failed to serialize ROM"))
}

fn sectioned_rom() -> RomFixture {
    let rom = FvmFormat::new(
        4,
        vec![0, 0, 0, 4],
        vec![Op::Halt as u8],
        vec![0xAA, 0xBB, 0xCC, 0xDD],
        vec![(Section::RoData, 0)],
    );
    RomFixture::new(&rom.to_bytes().expect("failed to serialize ROM"))
}

fn build_vm(rom_path: &Path, devices: Vec<DeviceConfig>) -> VM {
    VM::new(VmConfig {
        mem_size: main_ram_size(),
        rom: rom_path.to_string_lossy().into_owned(),
        devices,
    })
    .expect("VM should initialize")
}

#[test]
fn initialization_writes_discovery_table_and_rounds_main_ram() {
    let rom = minimal_rom();
    let rounded_main_ram = align_up(main_ram_size());
    let extra_ram_id = *b"EXTRA001";

    let vm = build_vm(
        &rom.path,
        vec![DeviceConfig::Ram {
            size: PAGE_SIZE,
            general: GeneralDeviceConfig {
                id: Some(extra_ram_id),
                fail_fast: true,
            },
        }],
    );

    assert_eq!(vm.bus.read_u32(0).unwrap(), 2);

    assert_eq!(vm.bus.read_u32(4).unwrap(), 0);
    assert_eq!(vm.bus.read_u32(8).unwrap(), rounded_main_ram);

    let main_id: Vec<u8> = (12..20)
        .map(|address| vm.bus.read_byte(address).unwrap())
        .collect();
    assert_eq!(main_id.as_slice(), b"RAM\0\0\0\0\0");
    assert_eq!(vm.bus.read_byte(20).unwrap(), 0b011);

    assert_eq!(vm.bus.read_u32(28).unwrap(), rounded_main_ram);
    assert_eq!(vm.bus.read_u32(32).unwrap(), PAGE_SIZE);
    let extra_id: Vec<u8> = (36..44)
        .map(|address| vm.bus.read_byte(address).unwrap())
        .collect();
    assert_eq!(extra_id.as_slice(), &extra_ram_id);
}

#[test]
fn initialization_maps_sections_patches_relocations_and_sets_kernel_state() {
    let rom = sectioned_rom();
    let vm = build_vm(&rom.path, Vec::new());

    assert_eq!(vm.active, 0);
    assert_eq!(vm.cr, 0);
    assert_eq!(vm.files[0].ip, 0x3000);
    assert_eq!(vm.files[0].sp, STACK_TOP);
    assert_eq!(vm.files[1].ip, 0);
    assert_eq!(vm.files[1].sp, 0);

    assert_eq!(vm.bus.read_u32(0x2000).unwrap(), 0x3000);
    assert_eq!(vm.bus.fetch_byte(0x3000).unwrap(), Op::Halt as u8);
    assert_eq!(vm.bus.read_u32(0x4000).unwrap(), 0xAABB_CCDD);

    assert!(matches!(
        vm.bus.write_byte(0x2000, 0xFF),
        Err(VmError::PermissionDenied { .. })
    ));
    assert!(matches!(
        vm.bus.write_byte(0x3000, 0xFF),
        Err(VmError::PermissionDenied { .. })
    ));
    assert!(matches!(
        vm.bus.fetch_byte(0x4000),
        Err(VmError::PermissionDenied { .. })
    ));

    vm.bus.write_byte(0x4000, 0x11).unwrap();
    assert_eq!(vm.bus.read_byte(0x4000).unwrap(), 0x11);
}

#[test]
fn initialization_maps_stack_window() {
    let rom = minimal_rom();
    let vm = build_vm(&rom.path, Vec::new());

    vm.bus.write_byte(STACK_BASE, 0xAB).unwrap();
    vm.bus.write_byte(STACK_TOP, 0xCD).unwrap();

    assert_eq!(vm.bus.read_byte(STACK_BASE).unwrap(), 0xAB);
    assert_eq!(vm.bus.read_byte(STACK_TOP).unwrap(), 0xCD);
}

#[test]
fn step_delivers_fetch_bus_fault_through_interrupt_vector() {
    let rom = minimal_rom();
    let mut vm = build_vm(&rom.path, Vec::new());
    let handler = vm.files[0].ip;

    vm.ivt[Interrupt::BusFault.index() as usize] = handler;
    vm.files[0].ip = 0x1234_5000;

    vm.step().unwrap();

    assert!(!vm.halted);
    assert_eq!(vm.active, 0);
    assert_eq!(vm.files[0].ip, handler);
    assert_eq!(vm.files[0].sp, STACK_TOP - STACK_FRAME_SIZE);
    assert_eq!(vm.bus.read_byte(vm.files[0].sp).unwrap(), 0);
    assert_eq!(vm.bus.read_u32(vm.files[0].sp + 5).unwrap(), 0x1234_5001);
}

#[test]
fn step_halts_on_unhandled_fetch_bus_fault() {
    let rom = minimal_rom();
    let mut vm = build_vm(&rom.path, Vec::new());

    vm.files[0].ip = 0xDEAD_0000;
    vm.step().unwrap();

    assert!(vm.halted);
    assert_eq!(vm.files[0].sp, STACK_TOP - STACK_FRAME_SIZE);
}
