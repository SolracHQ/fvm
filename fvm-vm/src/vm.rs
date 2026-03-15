pub mod bus;
pub mod config;
pub mod decoder;
pub mod device;
pub mod executor;
pub mod flags;
pub mod interrupts;
pub mod program_patcher;
pub mod registers;
pub mod initializer;
pub mod constants;

use std::{collections::HashMap, rc::Rc};

use fvm_core::{
    instruction::Instruction,
    opcode::Op,
    types::{Address, Word},
};

use crate::{
    error::{VmError, VmResult},
    vm::{
        config::VmConfig, device::Device, interrupts::Interrupt,
        registers::RegisterFile,
    },
};
use self::constants::{STACK_TOP, KERNEL_CONTEXT, USER_CONTEXT};

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
        Op::try_from(byte).map_err(|_| {
            VmError::FetchError {
                address,
                reason: format!("0x{:02X} is not a valid opcode byte", byte),
            }
        })
    }

    fn decode(&mut self, opcode: Op) -> VmResult<Instruction> {
        let ip = self.files[self.active].ip;
        decoder::decode_instruction(self, opcode).map_err(|e| match e {
            VmError::DecodeError { .. } | VmError::FetchError { .. } => e,
            other => VmError::DecodeError {
                address: ip,
                opcode: opcode as u8,
                arg_index: 0,
                reason: other.to_string(),
            },
        })
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

    /// Initialize the VM by setting up loader info, loading the program, and preparing execution.
    /// All initialization logic is delegated to the initializer module.
    fn initialize(&mut self, rom_path: String) -> VmResult<()> {
        initializer::initialize(self, rom_path)
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
            return Err(VmError::DoubleFault {
                first_interrupt: 1,
                second_interrupt: interrupt_index,
            });
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
        self.push_kernel_byte(interrupted.flags.bits())?;
        self.push_kernel_byte(u8::from(came_from_user))?;

        let handler = self.ivt[interrupt_index as usize];
        if handler == 0 {
            return Err(VmError::NoInterruptHandler {
                interrupt: interrupt_index,
                address: resume_ip,
            });
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
        let flags = self.pop_kernel_byte()?;
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
            | Op::Dpl
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
