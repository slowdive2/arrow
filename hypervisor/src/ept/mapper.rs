use core::ffi::c_void;

use wdk_sys::{
    ntddk::{ExAllocatePool2, ExFreePoolWithTag, KeGetCurrentIrql, MmGetPhysicalAddress},
    APC_LEVEL, PAGED_CODE, POOL_FLAG_NON_PAGED,
};

use super::{
    Ept4KbPageEntry, EptMemoryType, EptPageDirectory, EptPageDirectoryPointerTable,
    EptPageMapLevel4, EptPageTable, EptPagingStructureMemoryType, EptPointer, EptTableEntry,
    EPT_FOUR_LEVEL_WALK_LENGTH, EPT_PAGE_SHIFT, EPT_PAGE_SIZE,
};

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

pub unsafe fn initialize_ept() -> u64 {
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
