// every ept table has 512 entries
pub const EPT_ENTRY_COUNT: usize = 512;
pub const EPT_PAGE_SIZE: usize = 0x1000;
pub const EPT_PAGE_SHIFT: u32 = 12;
pub const EPT_LARGE_PAGE_SIZE: u64 = 0x20_0000;
pub const EPT_LARGE_PAGE_SHIFT: u32 = 21;
pub const EPT_PAGE_DIRECTORY_SPAN: u64 = 0x4000_0000;
pub const EPT_IDENTITY_MAP_SPAN: u64 = 0x80_0000_0000;
// four levels is encoded as 3 in eptp
pub const EPT_FOUR_LEVEL_WALK_LENGTH: u8 = 3;

// valid leaf memory types. 2, 3, and 7 are reserved
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EptMemoryType {
    Uncacheable = 0,
    WriteCombining = 1,
    WriteThrough = 4,
    WriteProtected = 5,
    WriteBack = 6,
}

// eptp only allows uc or wb
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EptPagingStructureMemoryType {
    Uncacheable = 0,
    WriteBack = 6,
}
