use std::fs::File;
use std::io;
use std::rc::Rc;

use crate::error::VmError;
use crate::vm::config::{DeviceConfig, VmConfig};
use crate::vm::device::debug::{DecimalIo, HexIo, RawIo};
use fvm_core::types::Word;

pub struct InitializedDevices {
    pub memory_mapped: Vec<Rc<dyn super::MemoryMappedDevice>>,
    // Each entry pairs the base port with the device.
    pub port_mapped: Vec<(Word, Rc<dyn super::PortMappedDevice>)>,
}

fn round_up_to_page(size: Word) -> Word {
    const PAGE_SIZE: Word = 4096;
    size.div_ceil(PAGE_SIZE) * PAGE_SIZE
}

pub fn initialize_devices(config: &VmConfig) -> Result<InitializedDevices, VmError> {
    let mut memory_mapped: Vec<Rc<dyn super::MemoryMappedDevice>> = Vec::new();
    let mut port_mapped: Vec<(Word, Rc<dyn super::PortMappedDevice>)> = Vec::new();

    let rounded_main_memory = round_up_to_page(config.mem_size);
    if rounded_main_memory != config.mem_size {
        eprintln!(
            "warning: rounding main_memory_size from {} to {} bytes for 4 KiB alignment",
            config.mem_size, rounded_main_memory
        );
    }

    memory_mapped.push(Rc::new(super::ram::Ram::new(rounded_main_memory, None)));

    for device in &config.devices {
        match device {
            DeviceConfig::Ram { size, general } => {
                memory_mapped.push(Rc::new(super::ram::Ram::new(*size, general.id)));
            }

            DeviceConfig::DecimalIo {
                port,
                input,
                output,
                general,
            } => {
                let reader = open_input(input, *b"DECIO\0\0\0")?;
                let writer = open_output(output, *b"DECIO\0\0\0")?;

                let device = Rc::new(DecimalIo::new(reader, writer, general.id));
                port_mapped.push((*port, device));
            }

            DeviceConfig::HexIo {
                port,
                input,
                output,
                general,
            } => {
                let reader = open_input(input, *b"HEXIO\0\0\0")?;
                let writer = open_output(output, *b"HEXIO\0\0\0")?;

                let device = Rc::new(HexIo::new(reader, writer, general.id));
                port_mapped.push((*port, device));
            }

            DeviceConfig::RawIo {
                port,
                input,
                output,
                general,
            } => {
                let reader = open_input(input, *b"RAWIN\0\0\0")?;
                let writer = open_output(output, *b"RAWIN\0\0\0")?;

                let device = Rc::new(RawIo::new(reader, writer, general.id));
                port_mapped.push((*port, device));
            }
        }
    }

    Ok(InitializedDevices {
        memory_mapped,
        port_mapped,
    })
}

fn open_input(path: &str, device: [u8; 8]) -> Result<Box<dyn io::Read>, VmError> {
    match path {
        "stdin" => Ok(Box::new(io::stdin())),
        other => Ok(Box::new(File::open(other).map_err(|e| VmError::DeviceError {
            device,
            offset: 0,
            message: format!("failed to open input file '{other}': {e}"),
        })?)),
    }
}

fn open_output(path: &str, device: [u8; 8]) -> Result<Box<dyn io::Write>, VmError> {
    match path {
        "stdout" => Ok(Box::new(io::stdout())),
        "stderr" => Ok(Box::new(io::stderr())),
        other => Ok(Box::new(File::create(other).map_err(|e| VmError::DeviceError {
            device,
            offset: 0,
            message: format!("failed to open output file '{other}': {e}"),
        })?)),
    }
}
