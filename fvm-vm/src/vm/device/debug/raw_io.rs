use std::cell::RefCell;
use std::io::{Read, Write};

use fvm_core::types::{Byte, Word};

use super::super::PortMappedDevice;
use crate::error::{VmError, VmResult};
use crate::vm::device::Device;
use crate::vm::interrupts::Interrupt;

pub struct RawIo {
    input: RefCell<Box<dyn Read>>,
    output: RefCell<Box<dyn Write>>,
    id: [u8; 8],
}

impl RawIo {
    pub fn new(input: Box<dyn Read>, output: Box<dyn Write>, id: Option<[u8; 8]>) -> Self {
        Self {
            input: RefCell::new(input),
            output: RefCell::new(output),
            id: id.unwrap_or(*b"RAWIN\0\0\0"),
        }
    }

    fn read_byte_raw(&self) -> VmResult<u8> {
        let mut buf = [0u8; 1];
        let mut input = self.input.borrow_mut();
        match input.read_exact(&mut buf) {
            Ok(()) => Ok(buf[0]),
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                Err(VmError::Interrupt(Interrupt::DeviceEndOfInput))
            }
            Err(e) => Err(VmError::DeviceError {
                device: self.id(),
                offset: 0,
                message: format!("read error: {e}"),
            }),
        }
    }
}

impl Device for RawIo {
    fn id(&self) -> [u8; 8] {
        self.id
    }
}

impl PortMappedDevice for RawIo {
    fn read_byte(&self, _port: Word) -> VmResult<Byte> {
        self.read_byte_raw()
    }

    fn write_byte(&self, _port: Word, value: Byte) -> VmResult<()> {
        let mut output = self.output.borrow_mut();
        output.write_all(&[value]).map_err(|e| VmError::DeviceError {
            device: self.id(),
            offset: 0,
            message: format!("write error: {e}"),
        })?;
        output.flush().map_err(|e| VmError::DeviceError {
            device: self.id(),
            offset: 0,
            message: format!("flush error: {e}"),
        })
    }

    fn read_half(&self, _port: Word) -> VmResult<u16> {
        let hi = self.read_byte_raw()? as u16;
        let lo = self.read_byte_raw()? as u16;
        Ok((hi << 8) | lo)
    }

    fn write_half(&self, _port: Word, value: u16) -> VmResult<()> {
        let hi = (value >> 8) as u8;
        let lo = value as u8;
        let mut output = self.output.borrow_mut();
        output.write_all(&[hi, lo]).map_err(|e| VmError::DeviceError {
            device: self.id(),
            offset: 0,
            message: format!("write error: {e}"),
        })?;
        output.flush().map_err(|e| VmError::DeviceError {
            device: self.id(),
            offset: 0,
            message: format!("flush error: {e}"),
        })
    }

    fn read_word(&self, _port: Word) -> VmResult<u32> {
        let hi = self.read_half(_port)? as u32;
        let lo = self.read_half(_port)? as u32;
        Ok((hi << 16) | lo)
    }

    fn write_word(&self, _port: Word, value: u32) -> VmResult<()> {
        let hi = (value >> 16) as u16;
        let lo = value as u16;
        self.write_half(_port, hi)?;
        self.write_half(_port, lo)
    }
}
