use std::cell::RefCell;
use std::io::{BufRead, BufReader, BufWriter, Read, Write};

use fvm_core::types::{Byte, Word};

use super::super::PortMappedDevice;
use crate::error::{VmError, VmResult};
use crate::vm::device::Device;
use crate::vm::interrupts::Interrupt;

pub struct HexIo {
    input: RefCell<BufReader<Box<dyn Read>>>,
    output: RefCell<BufWriter<Box<dyn Write>>>,
    id: [u8; 8],
}

impl HexIo {
    pub fn new(input: Box<dyn Read>, output: Box<dyn Write>, id: Option<[u8; 8]>) -> Self {
        Self {
            input: RefCell::new(BufReader::new(input)),
            output: RefCell::new(BufWriter::new(output)),
            id: id.unwrap_or(*b"HEXIO\0\0\0"),
        }
    }

    fn read_line_as_byte(&self) -> VmResult<u8> {
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

        let trimmed = line.trim();
        if trimmed.is_empty() {
            return Err(VmError::Interrupt(Interrupt::DeviceEndOfInput));
        }

        let digits = trimmed
            .strip_prefix("0x")
            .or_else(|| trimmed.strip_prefix("0X"))
            .unwrap_or(trimmed);

        u8::from_str_radix(digits, 16).map_err(|_| VmError::DeviceError {
            device: self.id(),
            offset: 0,
            message: format!("expected hex byte, got: {trimmed:?}"),
        })
    }
}

impl Device for HexIo {
    fn id(&self) -> [u8; 8] {
        self.id
    }
}

impl PortMappedDevice for HexIo {
    fn read_byte(&self, _port: Word) -> VmResult<Byte> {
        self.read_line_as_byte()
    }

    fn write_byte(&self, _port: Word, value: Byte) -> VmResult<()> {
        let mut output = self.output.borrow_mut();
        writeln!(output, "0x{value:02X}").map_err(|e| VmError::DeviceError {
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
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    #[test]
    fn reads_prefixed_and_plain_hex_bytes() {
        let input = Cursor::new(b"0x41\n7f\n".to_vec());
        let output = Cursor::new(Vec::new());
        let device = HexIo::new(Box::new(input), Box::new(output), None);

        assert_eq!(device.read_byte(0).unwrap(), 0x41);
        assert_eq!(device.read_byte(0).unwrap(), 0x7F);
    }
}
