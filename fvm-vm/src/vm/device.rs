use fvm_core::types::{Byte, HalfWord, Word};

use crate::{error::VmResult, vm::interrupts::Interrupt};

pub mod debug;
pub mod initializer;
pub mod ram;

pub use debug::{DecimalIo, HexIo, RawIo};

pub trait Device {
    /// 8-byte ASCII identifier for debugging. Should be unique across devices, but this is not enforced.
    fn id(&self) -> [u8; 8];
    /// FVM is and will always be single-threaded, so we cannot have real hardware interrupts.
    /// Instead, on each step of the VM loop, the VM will call `fetch_interrupt` on each device to check if it has an interrupt pending.
    /// If so, the VM will handle the interrupt before executing the next instruction.
    fn fetch_interrupt(&self) -> VmResult<Option<Interrupt>> {
        Ok(None)
    }
    /// Whether the VM should fail if there is not enough address space to map this device.
    /// If true, the VM will fail to start if the device cannot be mapped.
    /// If false, the device will be skipped with a warning if there is not enough address space.
    fn fast_fail(&self) -> bool {
        true
    }
}

/// A memory-mapped device. The VM will route reads/writes to the appropriate device based on the current context's page tables.
/// FVM is big-endian, so multi-byte reads/writes are in big-endian order (e.g. reading a u16 reads two consecutive bytes and combines them as (hi << 8) | lo).
pub trait MemoryMappedDevice: Device {
    /// Byte length of the device's addressable space.
    fn size(&self) -> Word;

    /// Read a byte from the device at the given offset.
    /// Offset is relative to the start of the device's address space, not the physical address.
    fn read_byte(&self, offset: Word) -> VmResult<Byte>;
    /// Read a 16-bit half-word from the device at the given offset.
    /// Offset is relative to the start of the device's address space, not the physical address.
    fn read_half(&self, offset: Word) -> VmResult<HalfWord> {
        let hi = self.read_byte(offset)? as HalfWord;
        let lo = self.read_byte(offset + 1)? as HalfWord;
        Ok((hi << 8) | lo)
    }
    /// Read a 32-bit word from the device at the given offset.
    /// Offset is relative to the start of the device's address space, not the physical address.
    fn read_word(&self, offset: Word) -> VmResult<Word> {
        let hi = self.read_half(offset)? as Word;
        let lo = self.read_half(offset + 2)? as Word;
        Ok((hi << 16) | lo)
    }

    /// Write a byte to the device at the given offset.
    /// Offset is relative to the start of the device's address space, not the physical address.
    fn write_byte(&self, offset: Word, value: Byte) -> VmResult<()>;
    /// Write a 16-bit half-word to the device at the given offset.
    /// Offset is relative to the start of the device's address space, not the physical address.
    fn write_half(&self, offset: Word, value: HalfWord) -> VmResult<()> {
        let hi = (value >> 8) as Byte;
        let lo = value as Byte;
        self.write_byte(offset, hi)?;
        self.write_byte(offset + 1, lo)
    }
    /// Write a 32-bit word to the device at the given offset.
    /// Offset is relative to the start of the device's address space, not the physical address.
    fn write_word(&self, offset: Word, value: Word) -> VmResult<()> {
        let hi = (value >> 16) as HalfWord;
        let lo = value as HalfWord;
        self.write_half(offset, hi)?;
        self.write_half(offset + 2, lo)
    }
}

pub trait PortMappedDevice: Device {
    /// Read a byte from the device
    fn read_byte(&self, port: Word) -> VmResult<Byte>;

    /// Write a byte to the device
    fn write_byte(&self, port: Word, value: Byte) -> VmResult<()>;

    /// Read a 16-bit half-word from the device
    fn read_half(&self, port: Word) -> VmResult<HalfWord> {
        let hi = self.read_byte(port)? as HalfWord;
        let lo = self.read_byte(port + 1)? as HalfWord;
        Ok((hi << 8) | lo)
    }

    /// Write a 16-bit half-word to the device
    fn write_half(&self, port: Word, value: HalfWord) -> VmResult<()> {
        let hi = (value >> 8) as Byte;
        let lo = value as Byte;
        self.write_byte(port, hi)?;
        self.write_byte(port + 1, lo)
    }

    /// Read a 32-bit word from the device
    fn read_word(&self, port: Word) -> VmResult<Word> {
        let hi = self.read_half(port)? as Word;
        let lo = self.read_half(port + 2)? as Word;
        Ok((hi << 16) | lo)
    }

    /// Write a 32-bit word to the device
    fn write_word(&self, port: Word, value: Word) -> VmResult<()> {
        let hi = (value >> 16) as HalfWord;
        let lo = value as HalfWord;
        self.write_half(port, hi)?;
        self.write_half(port + 2, lo)
    }
}
