#[repr(u8)]
#[derive(Clone, Copy)]
pub enum DataWidth {
    U8 = 0,
    U16 = 1,
    U32 = 2,
    U64 = 3,
}

pub fn mask_width(width: DataWidth, value: u64) -> u64 {
    match width {
        DataWidth::U8 => value & 0xff,
        DataWidth::U16 => value & 0xffff,
        DataWidth::U32 => value & 0xffff_ffff,
        DataWidth::U64 => value,
    }
}
