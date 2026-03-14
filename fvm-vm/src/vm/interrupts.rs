use num_enum::TryFromPrimitive;

#[derive(Debug, Clone, Copy, PartialEq, Eq, TryFromPrimitive)]
#[repr(u8)]
pub enum Interrupt {
    BusFault = 1,
    InvalidOpcode = 2,
    PrivilegeViolation = 6,
    Syscall = 15,
    DeviceEndOfInput = 16,
}

impl Interrupt {
    pub fn index(self) -> u8 {
        self as u8
    }
}
