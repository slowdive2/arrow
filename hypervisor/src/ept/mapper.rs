extern crate alloc;

use alloc::vec::Vec;
use core::ffi::c_void;

use bitfield_struct::bitfield;

use wdk_sys::{
    ntddk::{ExAllocatePool2, ExFreePoolWithTag, KeGetCurrentIrql, MmGetPhysicalAddress},
    APC_LEVEL, PAGED_CODE, POOL_FLAG_NON_PAGED,
};

use super::{
    Ept4KbPageEntry, EptMemoryType, EptPageDirectory, EptPageDirectoryPointerTable,
    EptPageMapLevel4, EptPageTable, EptPagingStructureMemoryType, EptPointer, EptTableEntry,
    EPT_FOUR_LEVEL_WALK_LENGTH, EPT_PAGE_SHIFT, EPT_PAGE_SIZE,
};
use crate::support::rdmsr;

const EPT_TAG: u32 = u32::from_le_bytes(*b"tEpA");
const PAGES_TO_ALLOCATE: usize = 10;

#[inline]
unsafe fn phys_of(ptr: *mut c_void) -> u64 {
    unsafe { MmGetPhysicalAddress(ptr).QuadPart as u64 }
}

#[inline]
unsafe fn free_if_non_null<T>(ptr: *mut T) {
    if !ptr.is_null() {
        unsafe { ExFreePoolWithTag(ptr.cast(), EPT_TAG) };
    }
}

/// `IA32_MTRRCAP`
#[bitfield(u64)]
#[derive(PartialEq, Eq)]
pub struct Ia32MtrrCapabilityRegister {
    /// number of supported variable-range mtrr register pairs
    #[bits(8)]
    pub variable_range_count: u8,
    /// fixed-range mtrrs are supported
    pub fixed_range_mtrrs_supported: bool,
    /// reserved
    #[bits(1)]
    __: u8,
    /// the write-combining memory type is supported
    pub write_combining_supported: bool,
    /// system-management range registers are supported
    pub smrr_supported: bool,
    /// reserved
    #[bits(52)]
    __: u64,
}

/// `IA32_MTRR_PHYSBASEn`
#[bitfield(u64)]
#[derive(PartialEq, Eq)]
pub struct Ia32MtrrPhysBaseRegister {
    /// memory type for this variable range
    #[bits(8)]
    pub memory_type: u8,
    /// reserved, keep zero
    #[bits(4)]
    __: u8,
    /// physical base address shifted right by 12
    #[bits(36)]
    pub page_frame_number: u64,
    /// reserved
    #[bits(16)]
    __: u16,
}

/// `IA32_MTRR_PHYSMASKn`
#[bitfield(u64)]
#[derive(PartialEq, Eq)]
pub struct Ia32MtrrPhysMaskRegister {
    /// reserved
    #[bits(11)]
    __: u16,
    /// enables this variable-range mtrr pair
    pub valid: bool,
    /// physical address mask shifted right by 12
    #[bits(36)]
    pub page_frame_number: u64,
    /// reserved
    #[bits(16)]
    __: u16,
}

/// one enabled variable-range mtrr
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MtrrRangeDescriptor {
    /// inclusive physical start address
    pub physical_base_address: u64,
    /// inclusive physical end address
    pub physical_end_address: u64,
    pub memory_type: u8,
}

const IA32_MTRRCAP_MSR: u32 = 0xfe;
const IA32_MTRR_PHYSBASE0_MSR: u32 = 0x200;
const IA32_MTRR_PHYSMASK0_MSR: u32 = 0x201;

pub unsafe fn ept_initialize() -> u64 {
    PAGED_CODE!();

    // pool2 zeroes these pages for us
    let ept_pml4: *mut EptPageMapLevel4 =
        unsafe { ExAllocatePool2(POOL_FLAG_NON_PAGED, EPT_PAGE_SIZE as u64, EPT_TAG).cast() };
    let ept_pdpt: *mut EptPageDirectoryPointerTable =
        unsafe { ExAllocatePool2(POOL_FLAG_NON_PAGED, EPT_PAGE_SIZE as u64, EPT_TAG).cast() };
    let ept_pd: *mut EptPageDirectory =
        unsafe { ExAllocatePool2(POOL_FLAG_NON_PAGED, EPT_PAGE_SIZE as u64, EPT_TAG).cast() };
    let ept_pt: *mut EptPageTable =
        unsafe { ExAllocatePool2(POOL_FLAG_NON_PAGED, EPT_PAGE_SIZE as u64, EPT_TAG).cast() };
    let guest_memory: *mut u8 = unsafe {
        ExAllocatePool2(
            POOL_FLAG_NON_PAGED,
            (PAGES_TO_ALLOCATE * EPT_PAGE_SIZE) as u64,
            EPT_TAG,
        )
        .cast()
    };

    if ept_pml4.is_null()
        || ept_pdpt.is_null()
        || ept_pd.is_null()
        || ept_pt.is_null()
        || guest_memory.is_null()
    {
        unsafe {
            free_if_non_null(guest_memory);
            free_if_non_null(ept_pt);
            free_if_non_null(ept_pd);
            free_if_non_null(ept_pdpt);
            free_if_non_null(ept_pml4);
        }
        return 0;
    }

    // pt setup
    for index in 0..PAGES_TO_ALLOCATE {
        let guest_page = unsafe { guest_memory.add(index * EPT_PAGE_SIZE) };
        let guest_page_phys = unsafe { phys_of(guest_page.cast()) };

        unsafe {
            (*ept_pt).entries[index] = Ept4KbPageEntry::new()
                .with_readable(true)
                .with_writable(true)
                .with_executable(true)
                .with_memory_type(EptMemoryType::WriteBack as u8)
                .with_ignore_pat(false)
                .with_accessed(false)
                .with_dirty(false)
                .with_user_executable(false)
                .with_page_number(guest_page_phys >> EPT_PAGE_SHIFT)
                .with_suppress_ve(false);
        }
    }

    // pd setup
    let ept_pt_phys = unsafe { phys_of(ept_pt.cast()) };
    unsafe {
        (*ept_pd).entries[0].table = EptTableEntry::new()
            .with_readable(true)
            .with_writable(true)
            .with_executable(true)
            .with_accessed(false)
            .with_user_executable(false)
            .with_next_table_page_number(ept_pt_phys >> EPT_PAGE_SHIFT);
    }

    // pdpt setup
    let ept_pd_phys = unsafe { phys_of(ept_pd.cast()) };
    unsafe {
        (*ept_pdpt).entries[0] = EptTableEntry::new()
            .with_readable(true)
            .with_writable(true)
            .with_executable(true)
            .with_accessed(false)
            .with_user_executable(false)
            .with_next_table_page_number(ept_pd_phys >> EPT_PAGE_SHIFT);
    }

    // pml4e setup
    let ept_pdpt_phys = unsafe { phys_of(ept_pdpt.cast()) };
    unsafe {
        (*ept_pml4).entries[0] = EptTableEntry::new()
            .with_readable(true)
            .with_writable(true)
            .with_executable(true)
            .with_accessed(false)
            .with_user_executable(false)
            .with_next_table_page_number(ept_pdpt_phys >> EPT_PAGE_SHIFT);
    }

    // eptp setup
    let ept_pml4_phys = unsafe { phys_of(ept_pml4.cast()) };

    let ept_ptr = EptPointer::new()
        .with_memory_type(EptPagingStructureMemoryType::WriteBack as u8)
        .with_page_walk_length_minus_one(EPT_FOUR_LEVEL_WALK_LENGTH)
        .with_accessed_and_dirty_enabled(true)
        .with_pml4_page_number(ept_pml4_phys >> EPT_PAGE_SHIFT)
        .into_bits();

    log::debug!("EPT pointer allocated at {ept_ptr:#x}");

    ept_ptr
}

pub fn ept_build_mtrr_map() -> Vec<MtrrRangeDescriptor> {
    let capabilities = Ia32MtrrCapabilityRegister::from_bits(rdmsr(IA32_MTRRCAP_MSR));
    let variable_range_count = capabilities.variable_range_count();
    let mut ranges = Vec::with_capacity(variable_range_count.into());

    for register in 0..u32::from(variable_range_count) {
        let register_offset = register * 2;
        let base =
            Ia32MtrrPhysBaseRegister::from_bits(rdmsr(IA32_MTRR_PHYSBASE0_MSR + register_offset));
        let mask =
            Ia32MtrrPhysMaskRegister::from_bits(rdmsr(IA32_MTRR_PHYSMASK0_MSR + register_offset));

        if !mask.valid() {
            continue;
        }

        let physical_base_address = base.page_frame_number() << EPT_PAGE_SHIFT;
        let physical_mask = mask.page_frame_number() << EPT_PAGE_SHIFT;
        let Some(range_size) = 1u64.checked_shl(physical_mask.trailing_zeros()) else {
            continue;
        };

        let descriptor = MtrrRangeDescriptor {
            physical_base_address,
            physical_end_address: physical_base_address + range_size - 1,
            memory_type: base.memory_type(),
        };

        if descriptor.memory_type != EptMemoryType::WriteBack as u8 {
            ranges.push(descriptor);
        }
    }

    log::debug!("committed {} MTRR ranges", ranges.len());
    ranges
}
