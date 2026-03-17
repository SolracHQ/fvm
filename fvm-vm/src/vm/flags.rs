#[derive(Clone, Copy)]
pub enum Flag {
    Zero,
    Negative,
    Carry,
    Overflow,
}

#[derive(Clone, Copy)]
pub struct Flags(u8);

impl Flags {
    pub fn new() -> Self {
        Flags(0)
    }

    pub fn from_bits(bits: u8) -> Self {
        Flags(bits & 0b1111)
    }

    pub fn bits(self) -> u8 {
        self.0 & 0b1111
    }

    pub fn set(&mut self, flag: Flag) {
        match flag {
            Flag::Zero => self.0 |= 1 << 0,
            Flag::Negative => self.0 |= 1 << 1,
            Flag::Carry => self.0 |= 1 << 2,
            Flag::Overflow => self.0 |= 1 << 3,
        }
    }

    pub fn clear(&mut self, flag: Flag) {
        match flag {
            Flag::Zero => self.0 &= !(1 << 0),
            Flag::Negative => self.0 &= !(1 << 1),
            Flag::Carry => self.0 &= !(1 << 2),
            Flag::Overflow => self.0 &= !(1 << 3),
        }
    }

    pub fn is_set(&self, flag: Flag) -> bool {
        match flag {
            Flag::Zero => (self.0 & (1 << 0)) != 0,
            Flag::Negative => (self.0 & (1 << 1)) != 0,
            Flag::Carry => (self.0 & (1 << 2)) != 0,
            Flag::Overflow => (self.0 & (1 << 3)) != 0,
        }
    }
}

impl std::fmt::Debug for Flags {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut flags = Vec::new();
        if self.is_set(Flag::Zero) {
            flags.push("Z");
        }
        if self.is_set(Flag::Negative) {
            flags.push("N");
        }
        if self.is_set(Flag::Carry) {
            flags.push("C");
        }
        if self.is_set(Flag::Overflow) {
            flags.push("O");
        }
        write!(f, "Flags({})", flags.join(""))
    }
}
