/// 4 MiB stack size
pub const STACK_SIZE: u32 = 4 * 1024 * 1024;

/// Stack virtual base address (last 4 MiB of address space)
pub const STACK_BASE: u32 = 0xFFC0_0000;

/// Stack top (highest address in the stack region)
pub const STACK_TOP: u32 = 0xFFFF_FFFF;

/// Kernel mode context identifier
pub const KERNEL_CONTEXT: u32 = 0;

/// User mode context identifier
pub const USER_CONTEXT: u32 = 1;
