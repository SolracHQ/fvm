//! Tests for instruction set coverage:
//!
//! - Shift and rotate operations: SHL, SHR, SAR, ROL, ROR at 32/16/8-bit widths
//! - Privilege levels and kernel-only operations: SIE, DPL, MMAP, MUNMAP, MPROTECT, TUR, TKR, MOV cr
//! - Memory mapping and context register: MMAP, MUNMAP, MPROTECT, cr reads/writes
//! - Remaining instructions: NOT, PUSH/POP, CALL/RET, IN/OUT, LOAD/STORE, TUR/TKR

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
        flags::Flag,
    },
};

const MAIN_RAM_SIZE: u32 = 8 * 1024 * 1024;
static NEXT_ID: AtomicU64 = AtomicU64::new(0);

// Privilege violation interrupt index
const PRIV_FAULT: u8 = 6;

struct TempFile {
    path: PathBuf,
}

impl TempFile {
    fn new(ext: &str) -> Self {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        Self {
            path: std::env::temp_dir()
                .join(format!("fvm-insn-test-{}-{}.{}", std::process::id(), id, ext)),
        }
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
    fs::write(rom.path(), bytes).expect("failed to write ROM");
    rom
}

fn run_vm(source: &str, devices: Vec<DeviceConfig>) -> VM {
    let rom = write_rom(source);
    let mut vm = VM::new(VmConfig {
        mem_size: MAIN_RAM_SIZE,
        rom: rom.path().to_string_lossy().into_owned(),
        devices,
    })
    .expect("VM should initialize");
    vm.run().expect("VM should run");
    vm
}

fn build_vm(source: &str) -> (VM, TempFile) {
    let rom = write_rom(source);
    let vm = VM::new(VmConfig {
        mem_size: MAIN_RAM_SIZE,
        rom: rom.path().to_string_lossy().into_owned(),
        devices: Vec::new(),
    })
    .expect("VM should initialize");
    (vm, rom)
}

// ============================================================
// Shift and Rotate Operations
// ============================================================

#[test]
fn shl_32bit_by_immediate_shifts_left() {
    // 0x80000001 << 1 = 0x00000002; carry = bit 31 of original = 1
    let vm = run_vm(
        "main:\n    MOV rw0, 0x80000001\n    SHL rw0, 1\n    HALT\n",
        vec![],
    );
    assert_eq!(vm.files[0].regs[0], 0x00000002);
    assert!(vm.files[0].flags.is_set(Flag::Carry));
    assert!(!vm.files[0].flags.is_set(Flag::Zero));
    assert!(!vm.files[0].flags.is_set(Flag::Negative));
}

#[test]
fn shl_32bit_overflow_produces_zero_and_sets_zero_carry_flags() {
    // 0x80000000 << 1 = 0; carry = bit 31 = 1, zero = 1
    let vm = run_vm(
        "main:\n    MOV rw0, 0x80000000\n    SHL rw0, 1\n    HALT\n",
        vec![],
    );
    assert_eq!(vm.files[0].regs[0], 0x00000000);
    assert!(vm.files[0].flags.is_set(Flag::Zero));
    assert!(vm.files[0].flags.is_set(Flag::Carry));
    assert!(!vm.files[0].flags.is_set(Flag::Negative));
}

#[test]
fn shl_32bit_by_zero_is_nop() {
    let vm = run_vm(
        "main:\n    MOV rw0, 0xDEADBEEF\n    SHL rw0, 0\n    HALT\n",
        vec![],
    );
    assert_eq!(vm.files[0].regs[0], 0xDEAD_BEEF);
}

#[test]
fn shl_32bit_by_width_or_more_gives_zero() {
    // Shifting 32-bit register by 32 should always yield 0
    let vm = run_vm(
        "main:\n    MOV rw0, 0xDEADBEEF\n    SHL rw0, 32\n    HALT\n",
        vec![],
    );
    assert_eq!(vm.files[0].regs[0], 0x00000000);
    assert!(vm.files[0].flags.is_set(Flag::Zero));
}

#[test]
fn shl_32bit_by_register_operand() {
    // Use rb1 as the shift amount
    let vm = run_vm(
        "main:\n    MOV rw0, 0x00000001\n    MOV rb1, 4\n    SHL rw0, rb1\n    HALT\n",
        vec![],
    );
    assert_eq!(vm.files[0].regs[0], 0x00000010);
}

#[test]
fn shr_32bit_logical_does_not_sign_extend() {
    // 0x80000000 >> 1 = 0x40000000 (zeros fill from left, no sign extension)
    let vm = run_vm(
        "main:\n    MOV rw0, 0x80000000\n    SHR rw0, 1\n    HALT\n",
        vec![],
    );
    assert_eq!(vm.files[0].regs[0], 0x4000_0000);
    assert!(!vm.files[0].flags.is_set(Flag::Negative));
    assert!(!vm.files[0].flags.is_set(Flag::Carry));
}

#[test]
fn shr_32bit_sets_carry_when_lsb_shifted_out() {
    // 0x00000001 >> 1 = 0; carry = bit 0 of original = 1
    let vm = run_vm(
        "main:\n    MOV rw0, 0x00000001\n    SHR rw0, 1\n    HALT\n",
        vec![],
    );
    assert_eq!(vm.files[0].regs[0], 0x00000000);
    assert!(vm.files[0].flags.is_set(Flag::Zero));
    assert!(vm.files[0].flags.is_set(Flag::Carry));
}

#[test]
fn shr_32bit_all_ones_by_four() {
    // 0xFFFFFFFF >> 4 = 0x0FFFFFFF, carry = bit 3 of original = 1, negative = 0
    let vm = run_vm(
        "main:\n    MOV rw0, 0xFFFFFFFF\n    SHR rw0, 4\n    HALT\n",
        vec![],
    );
    assert_eq!(vm.files[0].regs[0], 0x0FFF_FFFF);
    assert!(!vm.files[0].flags.is_set(Flag::Negative));
    assert!(vm.files[0].flags.is_set(Flag::Carry));
}

#[test]
fn sar_32bit_sign_extends_negative_value() {
    // 0x80000000 >> 4 (arithmetic) = 0xF8000000 (sign bit replicated)
    let vm = run_vm(
        "main:\n    MOV rw0, 0x80000000\n    SAR rw0, 4\n    HALT\n",
        vec![],
    );
    assert_eq!(vm.files[0].regs[0], 0xF800_0000);
    assert!(vm.files[0].flags.is_set(Flag::Negative));
    assert!(!vm.files[0].flags.is_set(Flag::Carry)); // bit 3 of 0x80000000 = 0
}

#[test]
fn sar_32bit_positive_value_does_not_sign_extend() {
    // 0x40000000 >> 4 = 0x04000000
    let vm = run_vm(
        "main:\n    MOV rw0, 0x40000000\n    SAR rw0, 4\n    HALT\n",
        vec![],
    );
    assert_eq!(vm.files[0].regs[0], 0x0400_0000);
    assert!(!vm.files[0].flags.is_set(Flag::Negative));
}

#[test]
fn rol_32bit_wraps_msb_to_lsb() {
    // ROL 0x80000001 by 1: bit 31 wraps to bit 0 → result = 0x00000003
    let vm = run_vm(
        "main:\n    MOV rw0, 0x80000001\n    ROL rw0, 1\n    HALT\n",
        vec![],
    );
    assert_eq!(vm.files[0].regs[0], 0x0000_0003);
    assert!(!vm.files[0].flags.is_set(Flag::Negative));
}

#[test]
fn ror_32bit_wraps_lsb_to_msb() {
    // ROR 0x00000001 by 1: bit 0 wraps to bit 31 → result = 0x80000000
    let vm = run_vm(
        "main:\n    MOV rw0, 0x00000001\n    ROR rw0, 1\n    HALT\n",
        vec![],
    );
    assert_eq!(vm.files[0].regs[0], 0x8000_0000);
    assert!(vm.files[0].flags.is_set(Flag::Negative));
}

#[test]
fn shl_8bit_overflow_zeroes_register_and_sets_flags() {
    // SHL rb0, 1 with 0x80: shifted bit goes out of top, result = 0
    // carry = (0x80 >> (8-1)) & 1 = bit 7 = 1
    let vm = run_vm(
        "main:\n    MOV rb0, 0x80\n    SHL rb0, 1\n    HALT\n",
        vec![],
    );
    assert_eq!(vm.files[0].regs[0] & 0xFF, 0x00);
    assert!(vm.files[0].flags.is_set(Flag::Zero));
    assert!(vm.files[0].flags.is_set(Flag::Carry));
}

#[test]
fn shl_8bit_only_modifies_low_byte_of_register() {
    // Upper bytes of rw0 must be preserved when operating on rb0.
    // MOV rw0, 0xDEAD1212 then SHL rb0, 4:
    //   rb0 = 0x12, shifted: (0x12 << 4) & 0xFF = 0x20
    //   rw0 after write_register for rb: 0xDEAD1220
    let vm = run_vm(
        "main:\n    MOV rw0, 0xDEAD1212\n    SHL rb0, 4\n    HALT\n",
        vec![],
    );
    assert_eq!(vm.files[0].regs[0], 0xDEAD_1220);
}

#[test]
fn sar_8bit_sign_extends_negative_into_low_byte() {
    // SAR rb0, 1 with 0x80 (i8 = -128): result = 0xC0 (i8 = -64)
    let vm = run_vm(
        "main:\n    MOV rb0, 0x80\n    SAR rb0, 1\n    HALT\n",
        vec![],
    );
    assert_eq!(vm.files[0].regs[0] & 0xFF, 0xC0);
    assert!(vm.files[0].flags.is_set(Flag::Negative));
}

#[test]
fn rol_16bit_wraps_high_bit_to_low_bit() {
    // ROL rh0, 1: 0x8001 → bit 15 wraps to bit 0 → 0x0003
    let vm = run_vm(
        "main:\n    MOV rh0, 0x8001\n    ROL rh0, 1\n    HALT\n",
        vec![],
    );
    assert_eq!(vm.files[0].regs[0] & 0xFFFF, 0x0003);
}

#[test]
fn ror_16bit_wraps_low_bit_to_high_bit() {
    // ROR rh0, 1: 0x0001 → bit 0 wraps to bit 15 → 0x8000
    let vm = run_vm(
        "main:\n    MOV rh0, 0x0001\n    ROR rh0, 1\n    HALT\n",
        vec![],
    );
    assert_eq!(vm.files[0].regs[0] & 0xFFFF, 0x8000);
    assert!(vm.files[0].flags.is_set(Flag::Negative));
}

// ============================================================
// Privilege Level and Kernel-Only Operations
// ============================================================

// Helper: build a VM with user file IP set to the kernel code start, enter user mode,
// and run. Asserts the VM halts due to a privilege violation (interrupt 6).
fn assert_privilege_fault_in_user_mode(source: &str) {
    let (mut vm, _rom) = build_vm(source);
    let code_base = vm.files[0].ip;
    vm.files[1].ip = code_base;
    vm.active = 1; // switch to user mode
    // ivt[6] = 0: unhandled privilege violation halts the VM
    vm.run().expect("run should not error");
    assert!(vm.halted);
    // raise_interrupt always switches back to kernel register file
    assert_eq!(vm.active, 0);
    // pending_interrupt is set to the index of the unhandled interrupt
    assert_eq!(vm.pending_interrupt, Some(PRIV_FAULT));
}

#[test]
fn sie_in_user_mode_raises_privilege_fault() {
    assert_privilege_fault_in_user_mode("main:\n    SIE rb0, main\n    HALT\n");
}

#[test]
fn dpl_in_user_mode_raises_privilege_fault() {
    assert_privilege_fault_in_user_mode("main:\n    DPL\n    HALT\n");
}

#[test]
fn mmap_in_user_mode_raises_privilege_fault() {
    assert_privilege_fault_in_user_mode(
        "main:\n    MMAP rw0, rw1, 4096\n    HALT\n",
    );
}

#[test]
fn munmap_in_user_mode_raises_privilege_fault() {
    assert_privilege_fault_in_user_mode(
        "main:\n    MUNMAP rw0, 4096\n    HALT\n",
    );
}

#[test]
fn tur_in_user_mode_raises_privilege_fault() {
    assert_privilege_fault_in_user_mode("main:\n    TUR rw0, rw1\n    HALT\n");
}

#[test]
fn tkr_in_user_mode_raises_privilege_fault() {
    assert_privilege_fault_in_user_mode("main:\n    TKR rw0, rw1\n    HALT\n");
}

#[test]
fn cr_write_in_user_mode_raises_privilege_fault() {
    // MOV cr, rw0 in user mode should raise interrupt 6
    assert_privilege_fault_in_user_mode("main:\n    MOV cr, rw0\n    HALT\n");
}

#[test]
fn mprotect_in_user_mode_raises_privilege_fault() {
    assert_privilege_fault_in_user_mode(
        "main:\n    MPROTECT rw0, rb1\n    HALT\n",
    );
}

#[test]
fn dpl_switches_execution_to_user_register_file() {
    // DPL is followed by HALT. IP advancement after DPL lands on HALT and executes
    // it in user mode (active stays 1 since HALT never changes it).
    let vm = run_vm("main:\n    DPL\n    HALT\n", vec![]);
    assert!(vm.halted);
    assert_eq!(vm.active, 1);
}

#[test]
fn tur_and_tkr_transfer_values_between_register_files() {
    // Kernel sets user rw0 = 42 via TKR, then reads it back into kernel rw1 via TUR.
    let vm = run_vm(
        "main:\n    MOV rw0, 42\n    TKR rw0, rw0\n    TUR rw1, rw0\n    HALT\n",
        vec![],
    );
    assert_eq!(vm.files[0].regs[1], 42); // kernel rw1 got the user value back
    assert_eq!(vm.files[1].regs[0], 42); // user rw0 was set by TKR
}

#[test]
fn sie_sets_interrupt_vector_entry() {
    // SIE rb0=15, handler_addr should place the address into ivt[15].
    let vm = run_vm(
        "main:\n    MOV rb0, 15\n    SIE rb0, main\n    HALT\n",
        vec![],
    );
    // ivt[15] should now hold the address of main (the code base).
    let code_base = vm.files[0].ip; // after halt, ip is past HALT, but ivt[15] was set
    // The assembler-resolved address of main is the code section start.
    // We can trust ivt[15] is non-zero and within reasonable code range.
    assert_ne!(vm.ivt[15], 0);
    // It should point into the code section, which is near code_base - 8 or code_base
    // (the program starts with MOV rb0=3b, SIE=6b, HALT=1b total 10 bytes; main is at start)
    assert!(vm.ivt[15] < code_base); // code_base is ip after HALT; main is a few bytes before
}

// ============================================================
// Memory Mapping and Context Register
// ============================================================

#[test]
fn cr_reads_as_kernel_context_after_initialization() {
    let vm = run_vm("main:\n    MOV rw0, cr\n    HALT\n", vec![]);
    assert_eq!(vm.files[0].regs[0], 0); // kernel context = 0
    assert_eq!(vm.cr, 0);
}

#[test]
fn cr_can_be_written_and_read_back_in_kernel_mode() {
    let vm = run_vm(
        "main:\n    MOV rw0, 1\n    MOV cr, rw0\n    MOV rw1, cr\n    HALT\n",
        vec![],
    );
    assert_eq!(vm.cr, 1);
    assert_eq!(vm.files[0].regs[1], 1);
}

#[test]
fn mmap_instruction_creates_accessible_virtual_region() {
    // Map physical 0x300000 to virtual 0xA00000, write a word, read it back.
    let source = r#"
.code
main:
    MOV rw0, 0x00A00000
    MOV rw1, 0x00300000
    MMAP rw0, rw1, 4096
    MOV rw2, 0xDEADBEEF
    STORE rw0, rw2
    LOAD rw3, rw0
    HALT
"#;
    let vm = run_vm(source, vec![]);
    assert!(vm.halted);
    assert_eq!(vm.files[0].regs[3], 0xDEAD_BEEF);
    // Also verify the physical memory was written through the virtual mapping.
    assert_eq!(vm.bus.read_u32(0x00A00000).unwrap(), 0xDEAD_BEEF);
}

#[test]
fn munmap_instruction_removes_virtual_mapping() {
    // Map a page then immediately unmap it. Afterwards the virtual address must fault.
    let source = r#"
.code
main:
    MOV rw0, 0x00A00000
    MOV rw1, 0x00300000
    MMAP rw0, rw1, 4096
    MUNMAP rw0, 4096
    HALT
"#;
    let vm = run_vm(source, vec![]);
    assert!(vm.halted);
    // Virtual page is gone; any access should return an error.
    assert!(
        vm.bus.read_byte(0x00A00000).is_err(),
        "unmapped virtual address should not be accessible"
    );
    // But the physical memory is still reachable via another mapping or physical write.
    vm.bus.write_physical_byte(0x00300000, 0xAB).unwrap();
}

#[test]
fn mprotect_instruction_changes_page_to_read_only() {
    // Map a page read-write, change it to read-only, verify writes now fail.
    let source = r#"
.code
main:
    MOV rw0, 0x00A00000
    MOV rw1, 0x00300000
    MMAP rw0, rw1, 4096
    MOV rb2, 0x01
    MPROTECT rw0, rb2
    HALT
"#;
    let vm = run_vm(source, vec![]);
    assert!(vm.halted);
    // Read should succeed.
    assert!(vm.bus.read_byte(0x00A00000).is_ok());
    // Write should be denied.
    assert!(
        vm.bus.write_byte(0x00A00000, 0xFF).is_err(),
        "write to read-only page should be denied"
    );
}

#[test]
fn mmap_with_register_size_operand_maps_correctly() {
    // MMAP rw0, rw1, rw2 (all-register form), mapping 4096 bytes.
    let source = r#"
.code
main:
    MOV rw0, 0x00B00000
    MOV rw1, 0x00310000
    MOV rw2, 4096
    MMAP rw0, rw1, rw2
    MOV rw3, 0xCAFEBABE
    STORE rw0, rw3
    LOAD rw4, rw0
    HALT
"#;
    let vm = run_vm(source, vec![]);
    assert!(vm.halted);
    assert_eq!(vm.files[0].regs[4], 0xCAFE_BABE);
}

// ============================================================
// Other Instructions
// ============================================================

#[test]
fn not_inverts_all_bits_of_32bit_register() {
    let vm = run_vm(
        "main:\n    MOV rw0, 0xAAAAAAAA\n    NOT rw0\n    HALT\n",
        vec![],
    );
    assert_eq!(vm.files[0].regs[0], 0x5555_5555);
    // 0x55... has bit 31 = 0, so negative flag is clear
    assert!(!vm.files[0].flags.is_set(Flag::Negative));
}

#[test]
fn not_inverts_only_low_byte_for_rb_view() {
    // MOV rb0, 0xAA → rw0 = 0x000000AA. NOT rb0 → rb0 = 0x55, upper bits unchanged.
    let vm = run_vm(
        "main:\n    MOV rb0, 0xAA\n    NOT rb0\n    HALT\n",
        vec![],
    );
    assert_eq!(vm.files[0].regs[0] & 0xFF, 0x55);
    assert_eq!(vm.files[0].regs[0] & 0xFFFF_FF00, 0); // upper bytes still zero
}

#[test]
fn push_and_pop_roundtrip_preserves_32bit_value() {
    let vm = run_vm(
        r#"main:
    MOV rw0, 0xDEADBEEF
    PUSH rw0
    MOV rw0, 0
    POP rw1
    HALT
"#,
        vec![],
    );
    assert_eq!(vm.files[0].regs[1], 0xDEAD_BEEF);
    assert_eq!(vm.files[0].regs[0], 0); // rw0 was zeroed before pop
}

#[test]
fn push_and_pop_byte_through_stack() {
    let vm = run_vm(
        r#"main:
    MOV rb0, 0xAB
    PUSH rb0
    POP rb1
    HALT
"#,
        vec![],
    );
    assert_eq!(vm.files[0].regs[1] & 0xFF, 0xAB);
}

#[test]
fn call_and_ret_invoke_subroutine_and_return() {
    let vm = run_vm(
        r#"main:
    MOV rw0, 0
    CALL sub
    HALT
sub:
    MOV rw0, 42
    RET
"#,
        vec![],
    );
    assert!(vm.halted);
    assert_eq!(vm.files[0].regs[0], 42);
}

#[test]
fn out_via_hex_device_writes_formatted_byte() {
    let output = TempFile::new("txt");
    let devices = vec![DeviceConfig::HexIo {
        port: 0,
        input: "stdin".to_string(),
        output: output.path().to_string_lossy().into_owned(),
        general: GeneralDeviceConfig::default(),
    }];
    let vm = run_vm(
        "main:\n    MOV rb0, 0xAB\n    OUT 0, rb0\n    HALT\n",
        devices,
    );
    drop(vm);
    let text = fs::read_to_string(output.path()).expect("failed to read output");
    assert_eq!(text, "0xAB\n");
}

#[test]
fn out_via_raw_device_writes_ascii_bytes() {
    let output = TempFile::new("txt");
    let devices = vec![DeviceConfig::RawIo {
        port: 0,
        input: "stdin".to_string(),
        output: output.path().to_string_lossy().into_owned(),
        general: GeneralDeviceConfig::default(),
    }];
    let source = "main:\n    MOV rb0, 'H'\n    OUT 0, rb0\n    MOV rb0, 'i'\n    OUT 0, rb0\n    HALT\n";
    let vm = run_vm(source, devices);
    drop(vm);
    let text = fs::read_to_string(output.path()).expect("failed to read output");
    assert_eq!(text, "Hi");
}

#[test]
fn load_and_store_32bit_via_data_section() {
    let source = r#"
.data
    buf: dw 0

.code
main:
    MOV rw0, buf
    MOV rw1, 0xDEADBEEF
    STORE rw0, rw1
    LOAD rw2, rw0
    HALT
"#;
    let vm = run_vm(source, vec![]);
    assert_eq!(vm.files[0].regs[2], 0xDEAD_BEEF);
}

#[test]
fn load_and_store_16bit_via_data_section() {
    let source = r#"
.data
    buf: dh 0

.code
main:
    MOV rw0, buf
    MOV rh1, 0x1234
    STORE rw0, rh1
    LOAD rh2, rw0
    HALT
"#;
    let vm = run_vm(source, vec![]);
    assert_eq!(vm.files[0].regs[2] & 0xFFFF, 0x1234);
    assert_eq!(vm.files[0].regs[2] >> 16, 0); // upper 16 untouched, started as zero
}

#[test]
fn load_and_store_byte_via_data_section() {
    let source = r#"
.data
    buf: db 0

.code
main:
    MOV rw0, buf
    MOV rb1, 0xAB
    STORE rw0, rb1
    LOAD rb2, rw0
    HALT
"#;
    let vm = run_vm(source, vec![]);
    assert_eq!(vm.files[0].regs[2] & 0xFF, 0xAB);
}
