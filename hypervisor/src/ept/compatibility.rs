use x86::msr::{rdmsr,};

/// `IA32_MTRR_DEF_TYPE`
#[bitfield(u64)]
#[derive(PartialEq, Eq)]
pub struct Ia32MtrrDefTypeRegister {
    /// default memory type used when no enabled mtrr covers an address
    #[bits(3)]
    pub default_memory_type: u8,
    /// reserved, keep zero
    #[bits(7)]
    __: u8,
    /// enables the fixed-range mtrrs when mtrrs are globally enabled
    pub fixed_range_mtrr_enabled: bool,
    /// globally enables fixed-range and variable-range mtrrs
    pub mtrr_enabled: bool,
    /// reserved, keep zero
    #[bits(52)]
    __: u64,
}

const IA32_MTRR_CAP : u32 = 0xFE;

pub const unsafe fn ept_check_features() -> bool {

    let mtrr_def_type = rdmsr(Ia32MtrrDefTypeRegister);

    if !mtrr_def_type.mtrr_enabled {
        log::error!("mtrr not enabled (mtrr dynamic ranges not supported)");
        return 0;
    }


}   