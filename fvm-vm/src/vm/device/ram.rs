use std::cell::RefCell;

use crate::{
    error::{VmError, VmResult},
    vm::device::{Device, MemoryMappedDevice},
};

// RAM device: plain heap memory.
pub struct Ram {
    id: [u8; 8],
    data: RefCell<Vec<u8>>,
}

impl Ram {
    pub fn new(size: u32, id: Option<[u8; 8]>) -> Self {
        Self {
            id: id.unwrap_or(*b"RAM\0\0\0\0\0"),
            data: RefCell::new(vec![0u8; size as usize]),
        }
    }

    pub fn load_bytes(&self, offset: u32, bytes: &[u8]) -> VmResult<()> {
        let mut data = self.data.borrow_mut();
        let end = offset as usize + bytes.len();
        if end > data.len() {
            return Err(VmError::DeviceError {
                device: self.id,
                offset,
                message: format!("load_bytes out of bounds: end={end}, size={}", data.len()),
            });
        }
        data[offset as usize..end].copy_from_slice(bytes);
        Ok(())
    }
}

impl Device for Ram {
    fn id(&self) -> [u8; 8] {
        self.id
    }
}

impl MemoryMappedDevice for Ram {
    fn size(&self) -> u32 {
        self.data.borrow().len() as u32
    }

    fn read_byte(&self, offset: u32) -> VmResult<u8> {
        let data = self.data.borrow();
        data.get(offset as usize)
            .copied()
            .ok_or(VmError::DeviceError {
                device: self.id,
                offset,
                message: format!("read out of bounds: size={}", data.len()),
            })
    }

    fn write_byte(&self, offset: u32, value: u8) -> VmResult<()> {
        let mut data = self.data.borrow_mut();
        let len = data.len();
        data.get_mut(offset as usize)
            .map(|b| *b = value)
            .ok_or(VmError::DeviceError {
                device: self.id,
                offset,
                message: format!("write out of bounds: size={len}"),
            })
    }
}
