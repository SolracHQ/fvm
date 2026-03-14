/// Minimal addressable unit is a byte (8 bits)
pub type Byte = u8;
/// A half-word is 16 bits, used for 16-bit instructions and 16-bit register views
pub type HalfWord = u16;
/// A word is 32 bits, used for 32-bit register views and memory access
pub type Word = u32;
/// Address is a 32-bit word, representing an offset into the VM's physical address space
pub type Address = u32;
