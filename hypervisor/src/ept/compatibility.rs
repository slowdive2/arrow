// only check the ept bits used below

use bitfield_struct::bitfield;
use x86::msr::{IA32_MTRR_DEF_TYPE, IA32_VMX_EPT_VPID_CAP};

use crate::support::rdmsr;

const WALK_4: u64 = 1 << 6;
const EPT_WB: u64 = 1 << 14;
const PAGE_2MB: u64 = 1 << 16;
const INVEPT: u64 = 1 << 20;
const INVEPT_SINGLE: u64 = 1 << 25;

// ia32_mtrr_def_type
#[bitfield(u64)]
#[derive(PartialEq, Eq)]
pub struct MtrrDefType {
    #[bits(3)]
    pub default_type: u8,
    #[bits(7)]
    __: u8,
    pub fixed_enabled: bool,
    pub enabled: bool,
    #[bits(52)]
    __: u64,
}

pub fn ept_supported() -> bool {
    let caps = rdmsr(IA32_VMX_EPT_VPID_CAP);
    let needed = WALK_4 | EPT_WB | PAGE_2MB | INVEPT | INVEPT_SINGLE;

    if caps & needed != needed {
        log::error!("required ept features r missing");
        return false;
    }

    let def = MtrrDefType::from_bits(rdmsr(IA32_MTRR_DEF_TYPE));
    if !def.enabled() {
        log::error!("mtrrs r disabled");
        return false;
    }

    true
}

pub fn mtrr_default_type() -> u8 {
    MtrrDefType::from_bits(rdmsr(IA32_MTRR_DEF_TYPE)).default_type()
}
