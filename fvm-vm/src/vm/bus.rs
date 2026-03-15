use std::{collections::HashMap, rc::Rc};

use super::device::MemoryMappedDevice;
use crate::error::{VmError, VmResult};

pub mod perm {
    pub const READ: u8 = 0b001;
    pub const WRITE: u8 = 0b010;
    pub const EXECUTE: u8 = 0b100;
}

pub const PAGE_BITS: u32 = 12;
pub const PAGE_SIZE: u32 = 1 << PAGE_BITS;
pub const PAGE_MASK: u32 = PAGE_SIZE - 1;

fn page_key(context: u32, page: u32) -> u64 {
    (context as u64) << 32 | page as u64
}

struct PhysicalRegion {
    phys_base: u32,
    device: Rc<dyn MemoryMappedDevice>,
}

pub struct PhysicalRegionInfo {
    pub phys_base: u32,
    pub size: u32,
    pub id: [u8; 8],
}

struct PageEntry {
    device_page_base: u32,
    device: Rc<dyn MemoryMappedDevice>,
    permissions: u8,
}

pub struct Bus {
    physical: Vec<PhysicalRegion>,
    pages: HashMap<u64, PageEntry>,
    current_context: u32,
}

impl Bus {
    pub fn new(devices: Vec<Rc<dyn MemoryMappedDevice>>) -> VmResult<Self> {
        let mut physical = Vec::new();
        let mut cursor: u32 = 0;
        for device in devices {
            match cursor.checked_add(device.size()) {
                Some(new_cursor) => {
                    physical.push(PhysicalRegion {
                        phys_base: cursor,
                        device: Rc::clone(&device),
                    });
                    cursor = new_cursor;
                }
                None => {
                    if device.fast_fail() {
                        return Err(VmError::Layout(format!(
                            "physical address space overflow while mapping device {:?}",
                            std::str::from_utf8(&device.id()).unwrap_or("???")
                        )));
                    } else {
                        eprintln!(
                            "warning: skipping device {:?} due to insufficient address space",
                            std::str::from_utf8(&device.id()).unwrap_or("???")
                        );
                    }
                }
            }
        }
        Ok(Self {
            physical,
            pages: HashMap::new(),
            current_context: 0,
        })
    }

    pub fn set_context(&mut self, context: u32) {
        self.current_context = context;
    }

    pub fn current_context(&self) -> u32 {
        self.current_context
    }

    pub fn physical_regions(&self) -> Vec<PhysicalRegionInfo> {
        self.physical
            .iter()
            .map(|region| PhysicalRegionInfo {
                phys_base: region.phys_base,
                size: region.device.size(),
                id: region.device.id(),
            })
            .collect()
    }

    fn resolve_physical(&self, phys_addr: u32) -> VmResult<(Rc<dyn MemoryMappedDevice>, u32)> {
        for region in &self.physical {
            let end = region.phys_base + region.device.size();
            if phys_addr >= region.phys_base && phys_addr < end {
                return Ok((Rc::clone(&region.device), phys_addr - region.phys_base));
            }
        }
        Err(VmError::UnmappedPhysicalAddress { address: phys_addr })
    }

    pub fn write_physical_byte(&self, phys_addr: u32, value: u8) -> VmResult<()> {
        let (device, offset) = self.resolve_physical(phys_addr)?;
        device.write_byte(offset, value)
    }

    pub fn write_physical_u32(&self, phys_addr: u32, value: u32) -> VmResult<()> {
        let (device, offset) = self.resolve_physical(phys_addr)?;
        device.write_word(offset, value)
    }

    pub fn write_physical_bytes(&self, phys_addr: u32, bytes: &[u8]) -> VmResult<()> {
        for (index, byte) in bytes.iter().copied().enumerate() {
            self.write_physical_byte(
                phys_addr
                    .checked_add(index as u32)
                    .ok_or(VmError::AddressOverflow)?,
                byte,
            )?;
        }
        Ok(())
    }

    pub fn read_physical_byte(&self, phys_addr: u32) -> VmResult<u8> {
        let (device, offset) = self.resolve_physical(phys_addr)?;
        device.read_byte(offset)
    }

    pub fn read_physical_u32(&self, phys_addr: u32) -> VmResult<u32> {
        let (device, offset) = self.resolve_physical(phys_addr)?;
        device.read_word(offset)
    }

    pub fn mmap(
        &mut self,
        context: u32,
        virt_page: u32,
        phys_page: u32,
        page_count: u32,
        permissions: u8,
    ) -> VmResult<()> {
        // Resolve all physical pages before mutating self.pages to avoid
        // borrowing self.physical and self.pages simultaneously.
        let mut resolved = Vec::with_capacity(page_count as usize);
        for i in 0..page_count {
            let phys_byte_addr = (phys_page + i)
                .checked_mul(PAGE_SIZE)
                .ok_or(VmError::Layout(format!(
                    "MMAP: physical page address overflow: phys_page={:08X} + i={:08X}, PAGE_SIZE={:08X}",
                    phys_page, i, PAGE_SIZE
                )))?;
            let (device, device_page_base) = self.resolve_physical(phys_byte_addr)?;
            resolved.push((i, device, device_page_base));
        }

        for (i, device, device_page_base) in resolved {
            self.pages.insert(
                page_key(context, virt_page + i),
                PageEntry {
                    device_page_base,
                    device,
                    permissions,
                },
            );
        }
        Ok(())
    }

    pub fn munmap(&mut self, context: u32, virt_page: u32, page_count: u32) -> VmResult<()> {
        for i in 0..page_count {
            if self
                .pages
                .remove(&page_key(context, virt_page + i))
                .is_none()
            {
                let virt_page_i = virt_page.checked_add(i).ok_or(VmError::Layout(
                    format!("MUNMAP: virtual page overflow: base=0x{:08X}, offset=0x{:08X}", virt_page, i)
                ))?;
                let address = virt_page_i.checked_mul(PAGE_SIZE).ok_or(VmError::Layout(
                    format!("MUNMAP: address overflow: page=0x{:08X}, PAGE_SIZE=0x{:X}", virt_page_i, PAGE_SIZE)
                ))?;
                return Err(VmError::UnmappedAddress {
                    context,
                    address,
                });
            }
        }
        Ok(())
    }

    pub fn mprotect(&mut self, context: u32, virt_page: u32, page_count: u32, permissions: u8) -> VmResult<()> {
        for i in 0..page_count {
            let key = page_key(context, virt_page + i);
            let virt_page_i = virt_page.checked_add(i).ok_or(VmError::Layout(
                format!("MPROTECT: virtual page overflow: base=0x{:08X}, offset=0x{:08X}", virt_page, i)
            ))?;
            let address = virt_page_i.checked_mul(PAGE_SIZE).ok_or(VmError::Layout(
                format!("MPROTECT: address overflow: page=0x{:08X}, PAGE_SIZE=0x{:X}", virt_page_i, PAGE_SIZE)
            ))?;
            let entry = self.pages.get_mut(&key).ok_or(VmError::UnmappedAddress {
                context,
                address,
            })?;
            entry.permissions = permissions;
        }
        Ok(())
    }

    fn lookup(&self, virt_addr: u32, required_perm: u8) -> VmResult<(&PageEntry, u32)> {
        let key = page_key(self.current_context, virt_addr >> PAGE_BITS);
        let entry = self.pages.get(&key).ok_or(VmError::UnmappedAddress {
            context: self.current_context,
            address: virt_addr,
        })?;

        if entry.permissions & required_perm == 0 {
            return Err(VmError::PermissionDenied {
                context: self.current_context,
                address: virt_addr,
                required: perm_name(required_perm),
                got: entry.permissions,
            });
        }

        Ok((entry, virt_addr & PAGE_MASK))
    }

    pub fn read_byte(&self, virt_addr: u32) -> VmResult<u8> {
        let (entry, offset) = self.lookup(virt_addr, perm::READ)?;
        entry.device.read_byte(entry.device_page_base + offset)
    }

    pub fn write_byte(&self, virt_addr: u32, value: u8) -> VmResult<()> {
        let (entry, offset) = self.lookup(virt_addr, perm::WRITE)?;
        entry
            .device
            .write_byte(entry.device_page_base + offset, value)
    }

    pub fn read_u16(&self, virt_addr: u32) -> VmResult<u16> {
        let (entry, offset) = self.lookup(virt_addr, perm::READ)?;
        entry.device.read_half(entry.device_page_base + offset)
    }

    pub fn write_u16(&self, virt_addr: u32, value: u16) -> VmResult<()> {
        let (entry, offset) = self.lookup(virt_addr, perm::WRITE)?;
        entry
            .device
            .write_half(entry.device_page_base + offset, value)
    }

    pub fn read_u32(&self, virt_addr: u32) -> VmResult<u32> {
        let (entry, offset) = self.lookup(virt_addr, perm::READ)?;
        entry.device.read_word(entry.device_page_base + offset)
    }

    pub fn write_u32(&self, virt_addr: u32, value: u32) -> VmResult<()> {
        let (entry, offset) = self.lookup(virt_addr, perm::WRITE)?;
        entry
            .device
            .write_word(entry.device_page_base + offset, value)
    }

    pub fn fetch_byte(&self, virt_addr: u32) -> VmResult<u8> {
        let (entry, offset) = self.lookup(virt_addr, perm::EXECUTE)?;
        entry.device.read_byte(entry.device_page_base + offset)
    }

    // Removes all page mappings for a context. Called when a context is destroyed.
    pub fn destroy_context(&mut self, context: u32) {
        let prefix = (context as u64) << 32;
        let prefix_end = prefix | 0xFFFF_FFFF;
        self.pages.retain(|&k, _| k < prefix || k > prefix_end);
    }
}

fn perm_name(perm: u8) -> &'static str {
    match perm {
        perm::READ => "READ",
        perm::WRITE => "WRITE",
        perm::EXECUTE => "EXECUTE",
        _ => "UNKNOWN",
    }
}
