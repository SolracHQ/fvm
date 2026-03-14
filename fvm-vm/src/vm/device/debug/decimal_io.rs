use std::cell::RefCell;
use std::io::{BufRead, BufReader, BufWriter, Read, Write};

use fvm_core::types::{Byte, HalfWord, Word};

use super::super::PortMappedDevice;
use crate::error::{VmError, VmResult};
use crate::vm::device::Device;
use crate::vm::interrupts::Interrupt;

pub struct DecimalIo {
    input: RefCell<BufReader<Box<dyn Read>>>,
    output: RefCell<BufWriter<Box<dyn Write>>>,
    id: [u8; 8],
}

impl DecimalIo {
    pub fn new(input: Box<dyn Read>, output: Box<dyn Write>, id: Option<[u8; 8]>) -> Self {
        Self {
            input: RefCell::new(BufReader::new(input)),
            output: RefCell::new(BufWriter::new(output)),
            id: id.unwrap_or(*b"DECIO\0\0\0"),
        }
    }

    fn read_line_as_u32(&self) -> VmResult<u32> {
        let mut line = String::new();
        let bytes_read =
            self.input
                .borrow_mut()
                .read_line(&mut line)
                .map_err(|e| VmError::DeviceError {
                    device: self.id(),
                    offset: 0,
                    message: format!("read error: {e}"),
                })?;

        if bytes_read == 0 {
            return Err(VmError::Interrupt(Interrupt::DeviceEndOfInput));
        }

        line.trim()
            .parse::<u32>()
            .map_err(|_| VmError::DeviceError {
                device: self.id(),
                offset: 0,
                message: format!("expected decimal integer, got: {:?}", line.trim()),
            })
    }
}

impl Device for DecimalIo {
    fn id(&self) -> [u8; 8] {
        self.id
    }
}

impl PortMappedDevice for DecimalIo {
    fn read_byte(&self, _port: Word) -> VmResult<Byte> {
        let val = self.read_line_as_u32()?;
        if val > u8::MAX as u32 {
            return Err(VmError::DeviceError {
                device: self.id(),
                offset: 0,
                message: format!("value {val} out of range for u8"),
            });
        }
        Ok(val as Byte)
    }

    fn write_byte(&self, _port: Word, value: Byte) -> VmResult<()> {
        writeln!(self.output.borrow_mut(), "{value}").map_err(|e| VmError::DeviceError {
            device: self.id(),
            offset: 0,
            message: format!("write error: {e}"),
        })
    }

    fn read_half(&self, _port: Word) -> VmResult<HalfWord> {
        let val = self.read_line_as_u32()?;
        if val > u16::MAX as u32 {
            return Err(VmError::DeviceError {
                device: self.id(),
                offset: 0,
                message: format!("value {val} out of range for u16"),
            });
        }
        Ok(val as HalfWord)
    }

    fn write_half(&self, _port: Word, value: HalfWord) -> VmResult<()> {
        writeln!(self.output.borrow_mut(), "{value}").map_err(|e| VmError::DeviceError {
            device: self.id(),
            offset: 0,
            message: format!("write error: {e}"),
        })
    }

    fn read_word(&self, _port: Word) -> VmResult<Word> {
        self.read_line_as_u32()
    }

    fn write_word(&self, _port: Word, value: Word) -> VmResult<()> {
        writeln!(self.output.borrow_mut(), "{value}").map_err(|e| VmError::DeviceError {
            device: self.id(),
            offset: 0,
            message: format!("write error: {e}"),
        })
    }
}
