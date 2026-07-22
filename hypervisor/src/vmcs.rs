pub struct VmcsRegion {
    revision_identifier: u32,
    abort_indicator: bool,
    pub reserved: [u8; BASE_PAGE_SIZE - 8],
}
