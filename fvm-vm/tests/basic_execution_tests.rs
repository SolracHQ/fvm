use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use fvm_assembler::assemble_source;
use fvm_vm::{
    vm::{
        VM,
        config::{DeviceConfig, GeneralDeviceConfig, VmConfig},
    },
};

const MAIN_RAM_SIZE: u32 = 8 * 1024 * 1024;
static NEXT_FIXTURE_ID: AtomicU64 = AtomicU64::new(0);

struct TempFile {
    path: PathBuf,
}

impl TempFile {
    fn new(extension: &str) -> Self {
        let id = NEXT_FIXTURE_ID.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "fvm-vm-basic-test-{}-{}.{}",
            std::process::id(),
            id,
            extension
        ));
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

fn write_rom(source: &str) -> TempFile {
    let rom = TempFile::new("fo");
    let bytes = assemble_source(source)
        .expect("source should assemble")
        .to_bytes()
        .expect("ROM should serialize");
    fs::write(rom.path(), bytes).expect("failed to write temporary ROM");
    rom
}

fn load_example(name: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("examples")
        .join(name);
    fs::read_to_string(path).expect("failed to read example source")
}

fn run_vm(source: &str, devices: Vec<DeviceConfig>) -> VM {
    let rom = write_rom(source);
    let mut vm = VM::new(VmConfig {
        mem_size: MAIN_RAM_SIZE,
        rom: rom.path().to_string_lossy().into_owned(),
        devices,
    })
    .expect("VM should initialize");
    vm.run().expect("VM should run without errors");
    vm
}

#[test]
fn arithmetic_example_runs_to_expected_register_state() {
    let vm = run_vm(&load_example("arithmetic.fa"), Vec::new());

    assert!(vm.halted);
    assert_eq!(vm.files[0].regs[0], 13);
    assert_eq!(vm.files[0].regs[2], 63);
    assert_eq!(vm.files[0].regs[4], 0);
    assert_eq!(vm.files[0].regs[6], 0xFFFF_FF00);
    assert_eq!(vm.files[0].regs[7] & 0xFF, 8);
    assert!(vm.files[0].flags.is_set(fvm_vm::vm::flags::Flag::Zero));
}

#[test]
fn jumps_example_runs_and_returns_sum() {
    let vm = run_vm(&load_example("jumps.fa"), Vec::new());

    assert!(vm.halted);
    assert_eq!(vm.files[0].regs[0], 6);
    assert_eq!(vm.files[0].regs[15], 0xFFFF_FFFF);
}

#[test]
fn zext_and_sext_example_extends_values_correctly() {
    let vm = run_vm(&load_example("zext_and_sext.fa"), Vec::new());

    assert!(vm.halted);
    assert_eq!(vm.files[0].regs[1] & 0xFFFF, 0x00FF);
    assert_eq!(vm.files[0].regs[3], 0x0000_0080);
    assert_eq!(vm.files[0].regs[5], 0x0000_8001);
    assert_eq!(vm.files[0].regs[7] & 0xFFFF, 0xFF80);
    assert_eq!(vm.files[0].regs[9], 0xFFFF_FFFF);
    assert_eq!(vm.files[0].regs[11], 0xFFFF_8001);
}

#[test]
fn hello_world_example_emits_ascii_codes_through_decimal_device() {
    let output = TempFile::new("txt");
    let devices = vec![DeviceConfig::DecimalIo {
        port: 0,
        input: "stdin".to_string(),
        output: output.path().to_string_lossy().into_owned(),
        general: GeneralDeviceConfig::default(),
    }];

    let vm = run_vm(&load_example("hello_world.fa"), devices);
    drop(vm);

    let rendered = fs::read_to_string(output.path()).expect("failed to read decimal output");
    let expected = [72, 101, 108, 108, 111, 44, 32, 119, 111, 114, 108, 100, 33, 10]
        .into_iter()
        .map(|value| value.to_string())
        .collect::<Vec<_>>()
        .join("\n");

    assert_eq!(rendered, format!("{expected}\n"));
}