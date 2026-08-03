use core::{
    mem::{align_of, size_of},
    ptr::NonNull,
};

use super::{
    Ept4KbPageEntry, EptPageDirectoryEntry, EptTableEntry, EPT_ENTRY_COUNT, EPT_PAGE_SIZE,
};

// root table
#[repr(C, align(4096))]
pub struct EptPageMapLevel4 {
    pub entries: [EptTableEntry; EPT_ENTRY_COUNT],
}

// pml4[0] points here
#[repr(C, align(4096))]
pub struct EptPageDirectoryPointerTable {
    pub entries: [EptTableEntry; EPT_ENTRY_COUNT],
}

// one pd maps 1 gib
#[repr(C, align(4096))]
pub struct EptPageDirectory {
    pub entries: [EptPageDirectoryEntry; EPT_ENTRY_COUNT],
}

// one pt maps 2 mib using 4 kib pages
#[repr(C, align(4096))]
pub struct EptPageTable {
    pub entries: [Ept4KbPageEntry; EPT_ENTRY_COUNT],
}

// tracks one 2 mib page split into 4 kib pages
#[derive(Debug)]
pub struct EptSplit {
    pub gpa: u64,
    pub pt: NonNull<EptPageTable>,
}

// catch layout changes at compile time
const _: () = {
    assert!(size_of::<EptPageMapLevel4>() == EPT_PAGE_SIZE);
    assert!(size_of::<EptPageDirectoryPointerTable>() == EPT_PAGE_SIZE);
    assert!(size_of::<EptPageDirectory>() == EPT_PAGE_SIZE);
    assert!(size_of::<EptPageTable>() == EPT_PAGE_SIZE);

    assert!(align_of::<EptPageMapLevel4>() == EPT_PAGE_SIZE);
    assert!(align_of::<EptPageDirectoryPointerTable>() == EPT_PAGE_SIZE);
    assert!(align_of::<EptPageDirectory>() == EPT_PAGE_SIZE);
    assert!(align_of::<EptPageTable>() == EPT_PAGE_SIZE);
};
