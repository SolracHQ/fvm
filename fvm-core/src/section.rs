use num_enum::TryFromPrimitive;

#[derive(Clone, Copy, TryFromPrimitive, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Section {
    RoData,
    Code,
    Data,
}
