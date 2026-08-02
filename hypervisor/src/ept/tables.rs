use core::{
    marker::PhantomPinned,
    mem::{align_of, offset_of, size_of},
    ptr::NonNull,
};

use super::{
    Ept2MbPageEntry, Ept4KbPageEntry, EptPageDirectoryEntry, EptTableEntry, GuestPhysicalAddress,
    HostPhysicalAddress, EPT_ENTRY_COUNT, EPT_PAGE_SIZE,
};

/// root table
#[repr(C, align(4096))]
pub struct EptPageMapLevel4 {
    pub entries: [EptTableEntry; EPT_ENTRY_COUNT],
}

/// the pdpt selected by `pml4[0]`
#[repr(C, align(4096))]
pub struct EptPageDirectoryPointerTable {
    pub entries: [EptTableEntry; EPT_ENTRY_COUNT],
}

/// one 1 gib page directory
#[repr(C, align(4096))]
pub struct EptPageDirectory {
    pub entries: [EptPageDirectoryEntry; EPT_ENTRY_COUNT],
}

/// one page table, covering 2 mib with 512 4 kib leaves
#[repr(C, align(4096))]
pub struct EptPageTable {
    pub entries: [Ept4KbPageEntry; EPT_ENTRY_COUNT],
}

/// dense 512 gib identity-map skeleton built around 2 mib leaves
///
/// the low 2 mib gets a ready 4 kib table because fixed mtrrs often give that
/// area mixed memory types other large pages can be split on demand
///
/// once its entries point back into this allocation it cannot move, hence the
/// pin marker
#[repr(C, align(4096))]
pub struct Ept {
    pub pml4: EptPageMapLevel4,
    pub pdpt: EptPageDirectoryPointerTable,
    pub page_directories: [EptPageDirectory; EPT_ENTRY_COUNT],
    pub low_2mb_page_table: EptPageTable,
    _pin: PhantomPinned,
}

/// software-side record for one decomposed 2 mib page
///
/// ept needs the table's hpa..the monitor needs its kernel va keeping
/// the old leaf makes restoration lossless
#[derive(Debug)]
pub struct EptSplit {
    pub guest_physical_base: GuestPhysicalAddress,
    pub page_table: NonNull<EptPageTable>,
    pub page_table_physical_address: HostPhysicalAddress,
    pub original_large_page_entry: Ept2MbPageEntry,
}

// table layout checks
const _: () = {
    assert!(size_of::<EptPageMapLevel4>() == EPT_PAGE_SIZE);
    assert!(size_of::<EptPageDirectoryPointerTable>() == EPT_PAGE_SIZE);
    assert!(size_of::<EptPageDirectory>() == EPT_PAGE_SIZE);
    assert!(size_of::<EptPageTable>() == EPT_PAGE_SIZE);

    assert!(align_of::<EptPageMapLevel4>() == EPT_PAGE_SIZE);
    assert!(align_of::<EptPageDirectoryPointerTable>() == EPT_PAGE_SIZE);
    assert!(align_of::<EptPageDirectory>() == EPT_PAGE_SIZE);
    assert!(align_of::<EptPageTable>() == EPT_PAGE_SIZE);

    assert!(offset_of!(Ept, pml4) == 0);
    assert!(offset_of!(Ept, pdpt) == EPT_PAGE_SIZE);
    assert!(offset_of!(Ept, page_directories) == EPT_PAGE_SIZE * 2);
    assert!(offset_of!(Ept, low_2mb_page_table) == EPT_PAGE_SIZE * 514);
    assert!(size_of::<Ept>() == EPT_PAGE_SIZE * 515);
};
