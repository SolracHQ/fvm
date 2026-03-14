pub mod bus;
pub mod config;
pub mod decoder;
pub mod device;
pub mod executor;
pub mod flags;
pub mod interrupts;
pub mod program_patcher;
pub mod registers;

use std::{collections::HashMap, rc::Rc};

use fvm_core::{
    instruction::Instruction,
    opcode::Op,
    types::{Address, Word},
};

use self::program_patcher::{SectionPlacement, load_and_patch_program};
use crate::{
    error::{VmError, VmResult},
    vm::{
        bus::PAGE_SIZE, config::VmConfig, device::Device, interrupts::Interrupt,
        registers::RegisterFile,
    },
};

const FAULT_INFO_SIZE: u32 = PAGE_SIZE;
const STACK_SIZE: u32 = 4 * 1024 * 1024;
const STACK_BASE: u32 = 0xFFC0_0000;
const STACK_TOP: u32 = 0xFFFF_FFFF;
const KERNEL_CONTEXT: u32 = 0;
const USER_CONTEXT: u32 = 1;

pub struct VM {
    pub files: [RegisterFile; 2], // 0 = kernel, 1 = user
    pub active: usize,            // must always be KERNEL_CONTEXT or USER_CONTEXT (as usize)
    pub mr: u32,
    pub ivt: [Address; 256],
    // Last triggered interrupt, If is a memory fault and the new interrupt is also a memory fault, the vm should halt.
    pub pending_interrupt: Option<u8>,
    pub bus: bus::Bus,
    pub port_devices: HashMap<Word, Rc<dyn device::PortMappedDevice>>,
    pub halted: bool,
    pub devices: Vec<Rc<dyn Device>>,
}

impl VM {
    pub fn new(config: VmConfig) -> VmResult<Self> {
        let devices = device::initializer::initialize_devices(&config)?;
        let all_devices: Vec<Rc<dyn Device>> = devices
            .memory_mapped
            .iter()
            .cloned()
            .map(|d| d as Rc<dyn Device>)
            .chain(
                devices
                    .port_mapped
                    .iter()
                    .cloned()
                    .map(|(_, d)| d as Rc<dyn Device>),
            )
            .collect();
        let bus = bus::Bus::new(devices.memory_mapped);
        let mut vm = Self {
            files: [RegisterFile::new(), RegisterFile::new()],
            active: 0,
            mr: 0,
            ivt: [0; 256],
            pending_interrupt: None,
            bus: bus?,
            port_devices: devices.port_mapped.iter().cloned().collect(),
            halted: false,
            devices: all_devices,
        };

        vm.initialize(config.rom)?;
        Ok(vm)
    }

    fn fetch(&mut self) -> VmResult<Op> {
        let address = self.files[self.active].ip;
        let byte = self.bus.fetch_byte(address)?;
        Op::try_from(byte).map_err(|_| VmError::InvalidOpcode {
            opcode: byte,
            address,
        })
    }

    fn decode(&mut self, opcode: Op) -> VmResult<Instruction> {
        decoder::decode_instruction(self, opcode)
    }

    fn execute(&mut self, instruction: Instruction) -> VmResult<()> {
        executor::execute_instruction(self, instruction)
    }

    pub fn step(&mut self) -> VmResult<()> {
        if self.halted {
            return Ok(());
        }

        self.bus.set_context(self.files[self.active].cr);

        if let Some(interrupt) = self.poll_devices()? {
            self.raise_interrupt(interrupt.index(), self.files[self.active].ip)?;
            return Ok(());
        }

        let start_ip = self.files[self.active].ip;
        let opcode = match self.fetch() {
            Ok(opcode) => opcode,
            Err(error) => return self.handle_step_error(start_ip.wrapping_add(1), error),
        };

        let instruction = match self.decode(opcode) {
            Ok(instruction) => instruction,
            Err(error) => return self.handle_step_error(start_ip.wrapping_add(1), error),
        };
        let instruction_size = instruction.size as u32;

        if let Err(error) = self.execute(instruction) {
            return self.handle_step_error(start_ip.wrapping_add(instruction_size), error);
        }

        if !self.halted && !opcode_controls_ip(opcode) {
            self.files[self.active].ip = start_ip.wrapping_add(instruction_size);
        }

        Ok(())
    }

    pub fn run(&mut self) -> VmResult<()> {
        while !self.halted {
            self.step()?;
        }
        Ok(())
    }

    /// This function executes the initialization steps for the VM, kinda the bios work, is a vm so is magic
    /// Steps:
    /// 1. Write the discovery table to ram start address and map it to the first few pages of the kernel context.
    ///    The discovery table contains an entry for each physical memory region, including its base address, size, and device ID.
    /// 2. Load the program sections into memory and map them to the appropriate virtual addresses.
    /// 3. Map the fixed stack region to the end of main RAM.
    fn initialize(&mut self, rom_path: String) -> VmResult<()> {
        let physical_regions = self.bus.physical_regions();
        let main_ram = physical_regions
            .first()
            .ok_or_else(|| VmError::Layout("VM requires a main RAM device".to_string()))?;

        if main_ram.size < STACK_SIZE {
            return Err(VmError::Layout(format!(
                "main RAM must be at least {} bytes to back the fixed 4 MiB stack",
                STACK_SIZE
            )));
        }

        self.bus.set_context(KERNEL_CONTEXT);
        self.files[0].cr = KERNEL_CONTEXT;

        self.write_discovery_table(&physical_regions)?;

        let discovery_pages = discovery_table_pages(physical_regions.len() as u32);
        let fault_info_base = discovery_pages
            .checked_mul(PAGE_SIZE)
            .ok_or(VmError::AddressOverflow)?;
        let reserved_bytes = fault_info_base
            .checked_add(FAULT_INFO_SIZE)
            .ok_or(VmError::AddressOverflow)?;
        let patched_program = load_and_patch_program(
            &rom_path,
            reserved_bytes,
            main_ram
                .size
                .checked_sub(STACK_SIZE)
                .ok_or(VmError::AddressOverflow)?,
        )?;

        self.bus
            .mmap(KERNEL_CONTEXT, 0, 0, discovery_pages, bus::perm::READ)?;
        self.bus.mmap(
            KERNEL_CONTEXT,
            fault_info_base / PAGE_SIZE,
            fault_info_base / PAGE_SIZE,
            FAULT_INFO_SIZE / PAGE_SIZE,
            bus::perm::READ,
        )?;

        self.load_section(
            &patched_program.ro_data,
            patched_program.sections[0],
            bus::perm::READ,
        )?;
        self.load_section(
            &patched_program.code,
            patched_program.sections[1],
            bus::perm::READ | bus::perm::EXECUTE,
        )?;
        self.load_section(
            &patched_program.rw_data,
            patched_program.sections[2],
            bus::perm::READ | bus::perm::WRITE,
        )?;

        let stack_phys_base = main_ram.size - STACK_SIZE;
        self.bus.mmap(
            KERNEL_CONTEXT,
            STACK_BASE / PAGE_SIZE,
            stack_phys_base / PAGE_SIZE,
            STACK_SIZE / PAGE_SIZE,
            bus::perm::READ | bus::perm::WRITE,
        )?;

        self.files[0].ip = patched_program.entry_point;
        self.files[0].sp = STACK_TOP;
        self.files[1] = RegisterFile::new();
        self.active = KERNEL_CONTEXT as usize;

        Ok(())
    }

    fn write_discovery_table(&self, regions: &[bus::PhysicalRegionInfo]) -> VmResult<()> {
        self.bus.write_physical_u32(0, regions.len() as u32)?;
        let mut cursor = 4u32;

        for region in regions {
            self.bus.write_physical_u32(cursor, region.phys_base)?;
            self.bus.write_physical_u32(cursor + 4, region.size)?;
            self.bus.write_physical_bytes(cursor + 8, &region.id)?;
            self.bus
                .write_physical_byte(cursor + 16, default_permissions_for_device(region.id))?;
            self.bus.write_physical_bytes(cursor + 17, &[0; 7])?;
            cursor = cursor.checked_add(24).ok_or(VmError::AddressOverflow)?;
        }

        Ok(())
    }

    fn load_section(
        &mut self,
        bytes: &[u8],
        placement: Option<SectionPlacement>,
        permissions: u8,
    ) -> VmResult<()> {
        if let Some(placement) = placement {
            self.bus.write_physical_bytes(placement.phys_base, bytes)?;
            self.bus.mmap(
                KERNEL_CONTEXT,
                placement.actual_base / PAGE_SIZE,
                placement.phys_base / PAGE_SIZE,
                placement.aligned_len / PAGE_SIZE,
                permissions,
            )?;
        }
        Ok(())
    }

    fn poll_devices(&self) -> VmResult<Option<Interrupt>> {
        for device in &self.devices {
            match device.fetch_interrupt() {
                Ok(Some(interrupt)) => return Ok(Some(interrupt)),
                Ok(None) => {}
                Err(VmError::Interrupt(interrupt)) => return Ok(Some(interrupt)),
                Err(error) => return Err(error),
            }
        }
        Ok(None)
    }

    fn handle_step_error(&mut self, resume_ip: u32, error: VmError) -> VmResult<()> {
        match interrupt_for_error(&error) {
            Some(interrupt) => self.raise_interrupt(interrupt.index(), resume_ip),
            None => Err(error),
        }
    }

    pub(crate) fn raise_interrupt(&mut self, interrupt_index: u8, resume_ip: u32) -> VmResult<()> {
        self.bus.set_context(self.files[0].cr);

        if self.pending_interrupt == Some(1) && interrupt_index == 1 {
            self.halted = true;
            println!("Double bus fault detected, halting VM");
            return Ok(());
        }

        self.pending_interrupt = Some(interrupt_index);
        let came_from_user = self.active == USER_CONTEXT as usize;
        let interrupted = self.files[self.active].clone();
        self.active = KERNEL_CONTEXT as usize;

        for value in interrupted.regs {
            self.push_kernel_u32(value)?;
        }
        self.push_kernel_u32(resume_ip)?;
        self.push_kernel_u32(interrupted.cr)?;
        self.push_kernel_u32(interrupted.flags.bits() as u32)?;
        self.push_kernel_byte(u8::from(came_from_user))?;

        let handler = self.ivt[interrupt_index as usize];
        if handler == 0 {
            self.halted = true;
            return Ok(());
        }

        self.files[0].ip = handler;
        self.pending_interrupt = None;
        Ok(())
    }

    pub fn iret(&mut self) -> VmResult<()> {
        if self.files[0].sp == STACK_TOP {
            return self.raise_interrupt(Interrupt::PrivilegeViolation.index(), self.files[0].ip);
        }

        let came_from_user = self.pop_kernel_byte()? != 0;
        let flags = self.pop_kernel_u32()? as u8;
        let cr = self.pop_kernel_u32()?;
        let ip = self.pop_kernel_u32()?;
        let mut regs = [0u32; 16];
        for reg in &mut regs {
            *reg = self.pop_kernel_u32()?;
        }

        let target = if came_from_user {
            USER_CONTEXT as usize
        } else {
            KERNEL_CONTEXT as usize
        };
        self.files[target].regs = regs;
        self.files[target].ip = ip;
        self.files[target].cr = cr;
        self.files[target].flags = flags::Flags::from_bits(flags);
        self.active = target;
        Ok(())
    }

    fn push_kernel_byte(&mut self, value: u8) -> VmResult<()> {
        self.files[0].sp = self.files[0]
            .sp
            .checked_sub(1)
            .ok_or(VmError::AddressOverflow)?;
        self.bus.write_byte(self.files[0].sp, value)
    }

    fn push_kernel_u32(&mut self, value: u32) -> VmResult<()> {
        self.files[0].sp = self.files[0]
            .sp
            .checked_sub(4)
            .ok_or(VmError::AddressOverflow)?;
        self.bus.write_u32(self.files[0].sp, value)
    }

    fn pop_kernel_byte(&mut self) -> VmResult<u8> {
        let value = self.bus.read_byte(self.files[0].sp)?;
        self.files[0].sp = self.files[0]
            .sp
            .checked_add(1)
            .ok_or(VmError::AddressOverflow)?;
        Ok(value)
    }

    fn pop_kernel_u32(&mut self) -> VmResult<u32> {
        let value = self.bus.read_u32(self.files[0].sp)?;
        self.files[0].sp = self.files[0]
            .sp
            .checked_add(4)
            .ok_or(VmError::AddressOverflow)?;
        Ok(value)
    }
}

fn opcode_controls_ip(opcode: Op) -> bool {
    matches!(
        opcode,
        Op::Halt
            | Op::Jmp
            | Op::JmpReg
            | Op::Jz
            | Op::JzReg
            | Op::Jnz
            | Op::JnzReg
            | Op::Jc
            | Op::JcReg
            | Op::Jn
            | Op::JnReg
            | Op::Call
            | Op::CallReg
            | Op::Ret
            | Op::Iret
            | Op::IntImm
            | Op::IntReg
    )
}

fn interrupt_for_error(error: &VmError) -> Option<Interrupt> {
    match error {
        VmError::Interrupt(interrupt) => Some(*interrupt),
        VmError::UnmappedAddress { .. }
        | VmError::PermissionDenied { .. }
        | VmError::DeviceError { .. }
        | VmError::UnmappedPhysicalAddress { .. } => Some(Interrupt::BusFault),
        VmError::InvalidOpcode { .. } => Some(Interrupt::InvalidOpcode),
        _ => None,
    }
}

fn default_permissions_for_device(_id: [u8; 8]) -> u8 {
    bus::perm::READ | bus::perm::WRITE
}

fn discovery_table_pages(entry_count: u32) -> u32 {
    (4 + entry_count * 24).div_ceil(PAGE_SIZE)
}
