#[derive(Clone, Copy, PartialEq, Eq)]
pub struct RegisterEncoding(pub u8);

impl std::fmt::Debug for RegisterEncoding {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.view() {
            view if view == Self::VIEW_RW => write!(f, "rw{}", self.index()),
            view if view == Self::VIEW_RH => write!(f, "rh{}", self.index()),
            view if view == Self::VIEW_RB => write!(f, "rb{}", self.index()),
            view if view == Self::VIEW_SP => write!(f, "sp"),
            view if view == Self::VIEW_CR => write!(f, "cr"),
            view if view == Self::VIEW_IP => write!(f, "ip"),
            view if view == Self::VIEW_MR => write!(f, "mr"),
            _ => write!(f, "Register(0x{:02X})", self.0),
        }
    }
}

impl RegisterEncoding {
    const VIEW_MASK: u8 = 0b1110_0000;
    const INDEX_MASK: u8 = 0b0000_1111;

    const VIEW_RW: u8 = 0b001_0_0000;
    const VIEW_RH: u8 = 0b010_0_0000;
    const VIEW_RB: u8 = 0b011_0_0000;
    const VIEW_SP: u8 = 0b100_0_0000;
    const VIEW_CR: u8 = 0b101_0_0000; // context register, privileged
    const VIEW_IP: u8 = 0b110_0_0000; // instruction pointer, privileged
    const VIEW_MR: u8 = 0b111_0_0000; // memory-mapped context register, privileged

    pub fn rw(index: u8) -> Self {
        debug_assert!(index < 16);
        Self(Self::VIEW_RW | (index & Self::INDEX_MASK))
    }

    pub fn rh(index: u8) -> Self {
        debug_assert!(index < 16);
        Self(Self::VIEW_RH | (index & Self::INDEX_MASK))
    }

    pub fn rb(index: u8) -> Self {
        debug_assert!(index < 16);
        Self(Self::VIEW_RB | (index & Self::INDEX_MASK))
    }

    pub fn sp() -> Self {
        Self(Self::VIEW_SP)
    }

    pub fn cr() -> Self {
        Self(Self::VIEW_CR)
    }

    pub fn ip() -> Self {
        Self(Self::VIEW_IP)
    }

    pub fn mr() -> Self {
        Self(Self::VIEW_MR)
    }

    pub fn view(self) -> u8 {
        self.0 & Self::VIEW_MASK
    }

    pub fn index(self) -> u8 {
        self.0 & Self::INDEX_MASK
    }

    pub fn is_rw(self) -> bool {
        self.view() == Self::VIEW_RW
    }
    pub fn is_rh(self) -> bool {
        self.view() == Self::VIEW_RH
    }
    pub fn is_rb(self) -> bool {
        self.view() == Self::VIEW_RB
    }
    pub fn is_sp(self) -> bool {
        self.view() == Self::VIEW_SP
    }
    pub fn is_ip(self) -> bool {
        self.view() == Self::VIEW_IP
    }
    pub fn is_mr(self) -> bool {
        self.view() == Self::VIEW_MR
    }
    pub fn is_cr(self) -> bool {
        self.view() == Self::VIEW_CR
    }

    pub fn is_privileged(self) -> bool {
        matches!(self.view(), Self::VIEW_CR | Self::VIEW_IP | Self::VIEW_MR)
    }

    pub fn width_bytes(self) -> u8 {
        match self.view() {
            Self::VIEW_RW => 4,
            Self::VIEW_RH => 2,
            Self::VIEW_RB => 1,
            Self::VIEW_SP => 4,
            Self::VIEW_CR => 4,
            Self::VIEW_IP => 4,
            Self::VIEW_MR => 4,
            _ => panic!("invalid register encoding: 0x{:02X}", self.0),
        }
    }

    pub fn from_byte(byte: u8) -> Option<Self> {
        let view = byte & Self::VIEW_MASK;
        let valid_view = matches!(
            view,
            Self::VIEW_RW
                | Self::VIEW_RH
                | Self::VIEW_RB
                | Self::VIEW_SP
                | Self::VIEW_CR
                | Self::VIEW_IP
                | Self::VIEW_MR
        );
        if valid_view { Some(Self(byte)) } else { None }
    }
}
