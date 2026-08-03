extern crate alloc;

use alloc::vec::Vec;
use core::{ffi::c_void, mem::size_of, ptr};

use bitfield_struct::bitfield;
use x86::msr::{IA32_MTRRCAP, IA32_MTRR_PHYSBASE0, IA32_MTRR_PHYSMASK0};

use wdk_sys::{
    ntddk::{MmAllocateContiguousMemory, MmGetPhysicalAddress},
    PAGED_CODE, PHYSICAL_ADDRESS,
};

use super::{
    Ept2MbPageEntry, EptMemoryType, EptPageDirectory, EptPageDirectoryPointerTable,
    EptPageMapLevel4, EptTableEntry, EPT_ENTRY_COUNT, EPT_LARGE_PAGE_SHIFT, EPT_LARGE_PAGE_SIZE,
    EPT_PAGE_SHIFT, EPT_PAGE_SIZE,
};
use crate::support::rdmsr;

pub(super) const EPT_TAG: u32 = u32::from_le_bytes(*b"tEpA");

#[inline]
pub(super) unsafe fn phys_of(ptr: *mut c_void) -> u64 {
    unsafe { MmGetPhysicalAddress(ptr).QuadPart as u64 }
}

// ia32_mtrrcap
#[bitfield(u64)]
#[derive(PartialEq, Eq)]
pub struct MtrrCap {
    #[bits(8)]
    pub var_count: u8,
    pub fixed_supported: bool,
    #[bits(1)]
    __: u8,
    pub wc_supported: bool,
    pub smrr_supported: bool,
    #[bits(52)]
    __: u64,
}

// ia32_mtrr_physbasen
#[bitfield(u64)]
#[derive(PartialEq, Eq)]
pub struct MtrrBase {
    #[bits(8)]
    pub mem_type: u8,
    #[bits(4)]
    __: u8,
    #[bits(36)]
    pub pfn: u64,
    #[bits(16)]
    __: u16,
}

// ia32_mtrr_physmaskn
#[bitfield(u64)]
#[derive(PartialEq, Eq)]
pub struct MtrrMask {
    #[bits(11)]
    __: u16,
    pub valid: bool,
    #[bits(36)]
    pub pfn: u64,
    #[bits(16)]
    __: u16,
}

// one enabled variable mtrr
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MtrrRange {
    pub phys_base: u64,
    pub phys_end: u64,
    pub mem_type: u8,
}

#[repr(C, align(4096))]
pub struct EptPageMap {
    pub pml4: EptPageMapLevel4,
    pub pdpt: EptPageDirectoryPointerTable,

    // 512 pds map the low 512 gib
    pub pds: [EptPageDirectory; EPT_ENTRY_COUNT],
}

const _: () = {
    assert!(size_of::<EptPageMap>() == EPT_PAGE_SIZE * (EPT_ENTRY_COUNT + 2));
};

pub fn build_mtrr_map() -> Vec<MtrrRange> {
    let cap = MtrrCap::from_bits(rdmsr(IA32_MTRRCAP));
    let mut ranges = Vec::with_capacity(cap.var_count().into());

    for reg in 0..u32::from(cap.var_count()) {
        let off = reg * 2;
        let base = MtrrBase::from_bits(rdmsr(IA32_MTRR_PHYSBASE0 + off));
        let mask = MtrrMask::from_bits(rdmsr(IA32_MTRR_PHYSMASK0 + off));

        if !mask.valid() {
            continue;
        }

        let phys_base = base.pfn() << EPT_PAGE_SHIFT;
        let phys_mask = mask.pfn() << EPT_PAGE_SHIFT;
        let Some(size) = 1u64.checked_shl(phys_mask.trailing_zeros()) else {
            continue;
        };

        ranges.push(MtrrRange {
            phys_base,
            phys_end: phys_base + size - 1,
            mem_type: base.mem_type(),
        });
    }

    log::debug!("committed {} MTRR ranges", ranges.len());
    ranges
}

// 11.11.4.1 MTRR Precedences 
// uc takes precedence.. wt wins when it overlaps wb
pub(super) fn mtrr_type(ranges: &[MtrrRange], default_type: u8, pa: u64) -> u8 {
    mtrr_type_for_range(ranges, default_type, pa, pa)
}

// check the whole leaf so a range in its middle is not missed
fn mtrr_type_for_range(ranges: &[MtrrRange], default_type: u8, start: u64, end: u64) -> u8 {
    let mut kind = default_type;

    for range in ranges
        .iter()
        .filter(|range| start <= range.phys_end && end >= range.phys_base)
    {
        if range.mem_type == EptMemoryType::Uncacheable as u8 {
            return range.mem_type;
        }

        if (kind == EptMemoryType::WriteBack as u8
            && range.mem_type == EptMemoryType::WriteThrough as u8)
            || (kind == EptMemoryType::WriteThrough as u8
                && range.mem_type == EptMemoryType::WriteBack as u8)
        {
            kind = EptMemoryType::WriteThrough as u8;
        } else {
            kind = range.mem_type;
        }
    }

    kind
}

// builds a 512 gib identity map with 2 mib pages
pub(super) unsafe fn alloc_map(mtrrs: &[MtrrRange], default_type: u8) -> *mut EptPageMap {
    PAGED_CODE!();

    let max_pa = PHYSICAL_ADDRESS { QuadPart: -1 };
    let map: *mut EptPageMap =
        unsafe { MmAllocateContiguousMemory(size_of::<EptPageMap>(), max_pa).cast() };

    if map.is_null() {
        log::error!("ept map alloc failed");
        return ptr::null_mut();
    }

    unsafe { ptr::write_bytes(map, 0, 1) };
    let ept = unsafe { &mut *map };

    // pml4[0] owns the low 512 gib
    let pdpt_pa = unsafe { phys_of(ptr::from_mut(&mut ept.pdpt).cast()) };
    ept.pml4.entries[0] = EptTableEntry::new()
        .with_readable(true)
        .with_writable(true)
        .with_executable(true)
        .with_pfn(pdpt_pa >> EPT_PAGE_SHIFT);

    // each pdpt entry points at a 1 gib pd
    for i in 0..EPT_ENTRY_COUNT {
        let pd_pa = unsafe { phys_of(ptr::from_mut(&mut ept.pds[i]).cast()) };

        ept.pdpt.entries[i] = EptTableEntry::new()
            .with_readable(true)
            .with_writable(true)
            .with_executable(true)
            .with_pfn(pd_pa >> EPT_PAGE_SHIFT);
    }

    // every pde maps its matching 2 mib physical page
    for pd_i in 0..EPT_ENTRY_COUNT {
        for i in 0..EPT_ENTRY_COUNT {
            let pa = (pd_i * EPT_ENTRY_COUNT + i) as u64 * EPT_LARGE_PAGE_SIZE;
            let kind = mtrr_type_for_range(mtrrs, default_type, pa, pa + EPT_LARGE_PAGE_SIZE - 1);

            ept.pds[pd_i].entries[i].large_page = Ept2MbPageEntry::new()
                .with_readable(true)
                .with_writable(true)
                .with_executable(true)
                .with_mem_type(kind)
                .with_large_page(true)
                .with_pfn(pa >> EPT_LARGE_PAGE_SHIFT);
        }
    }

    ptr::from_mut(ept)
}
