use core::mem::{align_of, size_of};

use bitfield_struct::bitfield;

/// the ept pointer written into the vmcs
#[bitfield(u64)]
#[derive(PartialEq, Eq)]
pub struct EptPointer {
    /// use one of the paging-structure memory types
    #[bits(3)]
    pub memory_type: u8,
    /// encoded as `levels - 1`, so four levels is 3
    #[bits(3)]
    pub page_walk_length_minus_one: u8,
    /// lets hardware update accessed and dirty bits
    pub accessed_and_dirty_enabled: bool,
    /// reserved
    #[bits(5)]
    __: u8,
    /// physical pml4 page number, bits 51:12 of its hpa
    #[bits(40)]
    pub pml4_page_number: u64,
    /// reserved, including bits above the cpu's physical address width
    #[bits(12)]
    __: u16,
}

/// a non-leaf entry pointing to the next table
///
/// pml4e, non-leaf pdpte, and non-leaf pde all use this format.. bits 7:3 are
/// hidden on purpose since leaf-only fields r invalid here
#[bitfield(u64)]
#[derive(PartialEq, Eq)]
pub struct EptTableEntry {
    pub readable: bool,
    pub writable: bool,
    /// supervisor execute when mbec is enabled
    pub executable: bool,
    /// reserved, keep zero
    #[bits(5)]
    __: u8,
    pub accessed: bool,
    /// ignored, still nicer to keep zero
    #[bits(1)]
    __: u8,
    /// user execute when mbec is enabled
    pub user_executable: bool,
    /// ignored, keep zero
    #[bits(1)]
    __: u8,
    /// next table's physical 4 kib page number
    #[bits(40)]
    pub next_table_page_number: u64,
    /// ignored or reserved, keep zero
    #[bits(12)]
    __: u16,
}

/// a leaf entry mapping one 4 kib page
#[bitfield(u64)]
#[derive(PartialEq, Eq)]
pub struct Ept4KbPageEntry {
    pub readable: bool,
    pub writable: bool,
    /// supervisor execute when mbec is enabled
    pub executable: bool,
    /// use one of the leaf memory types, resolved from the mtrrs
    #[bits(3)]
    pub memory_type: u8,
    pub ignore_pat: bool,
    /// ignored, keep zero
    #[bits(1)]
    __: u8,
    pub accessed: bool,
    pub dirty: bool,
    /// user execute when mbec is enabled
    pub user_executable: bool,
    /// ignored, keep zero
    #[bits(1)]
    __: u8,
    /// physical 4 kib page number, bits 51:12 of the hpa
    #[bits(40)]
    pub page_number: u64,
    /// optional feature bits are left unused for now
    #[bits(11)]
    __: u16,
    /// suppress an ept-violation #ve when that control is enabled
    pub suppress_ve: bool,
}

/// the large-page pde view, mapping one 2 mib region
///
/// bits 20:12 are reserved, so `page_number` is the hpa shifted by 21 rather
/// than the usual 4 kib pfn shifted by 12
#[bitfield(u64)]
#[derive(PartialEq, Eq)]
pub struct Ept2MbPageEntry {
    pub readable: bool,
    pub writable: bool,
    /// supervisor execute when mbec is enabled
    pub executable: bool,
    /// use one of the leaf memory types, resolved from the mtrrs
    #[bits(3)]
    pub memory_type: u8,
    pub ignore_pat: bool,
    /// must be 1 for a 2 mib leaf
    pub large_page: bool,
    pub accessed: bool,
    pub dirty: bool,
    /// user execute when mbec is enabled
    pub user_executable: bool,
    /// ignored, keep zero
    #[bits(1)]
    __: u8,
    /// reserved 2 mib alignment bits
    #[bits(9)]
    __: u16,
    /// physical 2 mib page number, bits 51:21 of the hpa
    #[bits(31)]
    pub page_number: u64,
    /// optional feature bits are left unused for now
    #[bits(11)]
    __: u16,
    pub suppress_ve: bool,
}

/// a pde is either a 2 mib leaf or a pointer to a 4 kib page table
/// bit 7 selects the view check it before reading a union member
#[repr(C)]
#[derive(Clone, Copy)]
pub union EptPageDirectoryEntry {
    pub table: EptTableEntry,
    pub large_page: Ept2MbPageEntry,
    pub raw: u64,
}

/// 128-bit operand used by `invept`
#[repr(C, align(16))]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct InveptDescriptor {
    pub ept_pointer: u64,
    pub reserved: u64,
}

// entry layout checks
const _: () = {
    assert!(size_of::<EptPointer>() == size_of::<u64>());
    assert!(size_of::<EptTableEntry>() == size_of::<u64>());
    assert!(size_of::<Ept4KbPageEntry>() == size_of::<u64>());
    assert!(size_of::<Ept2MbPageEntry>() == size_of::<u64>());
    assert!(size_of::<EptPageDirectoryEntry>() == size_of::<u64>());
    assert!(size_of::<InveptDescriptor>() == 16);
    assert!(align_of::<InveptDescriptor>() == 16);
};
