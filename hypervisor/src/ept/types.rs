/// entries in each four-level ept table
pub const EPT_ENTRY_COUNT: usize = 512;
/// single ept table page
pub const EPT_PAGE_SIZE: usize = 0x1000;
/// shift for a 4 kib page number
pub const EPT_PAGE_SHIFT: u32 = 12;
/// single 2 mib large page
pub const EPT_LARGE_PAGE_SIZE: u64 = 0x20_0000;
/// shift for a 2 mib page number
pub const EPT_LARGE_PAGE_SHIFT: u32 = 21;
/// single page directory covers 1 gib
pub const EPT_PAGE_DIRECTORY_SPAN: u64 = 0x4000_0000;
/// this layout covers the low 512 gib with a pml4 entry
pub const EPT_IDENTITY_MAP_SPAN: u64 = 0x80_0000_0000;
/// eptp stores the walk length as `levels - 1`
pub const EPT_FOUR_LEVEL_WALK_LENGTH: u8 = 3;

/// a guest physical address; keeping this separate from an hpa catches mixups
#[repr(transparent)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GuestPhysicalAddress(pub u64);

/// a host/system physical address stored in ept
#[repr(transparent)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct HostPhysicalAddress(pub u64);

/// valid memory types for leaf mappings vals 2, 3, and 7 r reserved
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EptMemoryType {
    Uncacheable = 0,
    WriteCombining = 1,
    WriteThrough = 4,
    WriteProtected = 5,
    WriteBack = 6,
}

/// valid eptp memory types, subject to `ia32_vmx_ept_vpid_cap`
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EptPagingStructureMemoryType {
    Uncacheable = 0,
    WriteBack = 6,
}
