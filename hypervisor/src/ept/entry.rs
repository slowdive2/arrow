use core::mem::{align_of, size_of};

use bitfield_struct::bitfield;

// ept pointer written into the vmcs
#[bitfield(u64)]
#[derive(PartialEq, Eq)]
pub struct EptPointer {
    // cache type for the ept tables
    #[bits(3)]
    pub mem_type: u8,
    // levels - 1, so four levels is 3
    #[bits(3)]
    pub walk_len_minus_one: u8,
    // lets the cpu update accessed and dirty bits
    pub ad_enabled: bool,
    // reserved
    #[bits(5)]
    __: u8,
    // pml4 physical page number
    #[bits(40)]
    pub pml4_pfn: u64,
    // reserved
    #[bits(12)]
    __: u16,
}

// pml4e, pdpte, and non-leaf pde all use this
#[bitfield(u64)]
#[derive(PartialEq, Eq)]
pub struct EptTableEntry {
    pub readable: bool,
    pub writable: bool,
    // supervisor execute with mbec
    pub executable: bool,
    // reserved
    #[bits(5)]
    __: u8,
    pub accessed: bool,
    // ignored
    #[bits(1)]
    __: u8,
    // user execute with mbec
    pub user_executable: bool,
    // ignored
    #[bits(1)]
    __: u8,
    // next table physical page number
    #[bits(40)]
    pub pfn: u64,
    // reserved
    #[bits(12)]
    __: u16,
}

// maps one 4 kib page
#[bitfield(u64)]
#[derive(PartialEq, Eq)]
pub struct Ept4KbPageEntry {
    pub readable: bool,
    pub writable: bool,
    // supervisor execute with mbec
    pub executable: bool,
    // cache type from the mtrrs
    #[bits(3)]
    pub mem_type: u8,
    pub ignore_pat: bool,
    // ignored
    #[bits(1)]
    __: u8,
    pub accessed: bool,
    pub dirty: bool,
    // user execute with mbec
    pub user_executable: bool,
    // ignored
    #[bits(1)]
    __: u8,
    // physical page number, pa >> 12
    #[bits(40)]
    pub pfn: u64,
    // unused for now
    #[bits(11)]
    __: u16,
    // suppress #ve
    pub suppress_ve: bool,
}

// maps one 2 mib page
#[bitfield(u64)]
#[derive(PartialEq, Eq)]
pub struct Ept2MbPageEntry {
    pub readable: bool,
    pub writable: bool,
    // supervisor execute with mbec
    pub executable: bool,
    // cache type from the mtrrs
    #[bits(3)]
    pub mem_type: u8,
    pub ignore_pat: bool,
    // must be 1 for a 2 mib leaf
    pub large_page: bool,
    pub accessed: bool,
    pub dirty: bool,
    // user execute with mbec
    pub user_executable: bool,
    // ignored
    #[bits(1)]
    __: u8,
    // reserved because 2 mib pages are aligned
    #[bits(9)]
    __: u16,
    // physical page number, pa >> 21
    #[bits(31)]
    pub pfn: u64,
    // unused for now
    #[bits(11)]
    __: u16,
    pub suppress_ve: bool,
}

// bit 7 picks the large page view
#[repr(C)]
#[derive(Clone, Copy)]
pub union EptPageDirectoryEntry {
    pub table: EptTableEntry,
    pub large_page: Ept2MbPageEntry,
    pub raw: u64,
}

// 128-bit invept operand
#[repr(C, align(16))]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct InveptDescriptor {
    pub eptp: u64,
    pub reserved: u64,
}

// layout checks
const _: () = {
    assert!(size_of::<EptPointer>() == size_of::<u64>());
    assert!(size_of::<EptTableEntry>() == size_of::<u64>());
    assert!(size_of::<Ept4KbPageEntry>() == size_of::<u64>());
    assert!(size_of::<Ept2MbPageEntry>() == size_of::<u64>());
    assert!(size_of::<EptPageDirectoryEntry>() == size_of::<u64>());
    assert!(size_of::<InveptDescriptor>() == 16);
    assert!(align_of::<InveptDescriptor>() == 16);
};
