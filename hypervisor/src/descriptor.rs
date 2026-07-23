use x86::{
    dtables::{sgdt, sidt, DescriptorTablePointer},
    segmentation::SegmentSelector,
};

#[derive(Default, Copy, Clone)]
pub struct Descriptors {
    pub gdtr: DescriptorTablePointer<u64>,
    pub idtr: DescriptorTablePointer<u64>,
    pub tr: SegmentSelector,
    pub tss_base: u64,
    pub tss_limit: u32,
    pub tss_access_rights: u32,
}

impl Descriptors {
    /// snapshot current cpu state for guest
    pub unsafe fn capture_current() -> Self {
        let mut gdtr = DescriptorTablePointer::default();
        let mut idtr = DescriptorTablePointer::default();
        unsafe { sgdt(&mut gdtr) };
        unsafe { sidt(&mut idtr) };

        let tr = unsafe { read_tr() };
        let (tss_base, tss_limit, tss_ar) = unsafe {
            resolve_tss_descriptor(gdtr.base as *const u64, tr)
        };

        Self {
            gdtr,
            idtr,
            tr,
            tss_base,
            tss_limit,
            tss_access_rights: tss_ar,
        }
    }
}

unsafe fn read_tr() -> SegmentSelector {
    let sel: u16;
    core::arch::asm!("str {sel:x}", sel = out(reg) sel, options(nomem, nostack, preserves_flags));
    SegmentSelector::from_raw(sel)
}

unsafe fn resolve_tss_descriptor(gdt_base: *const u64, tr: SegmentSelector) -> (u64, u32, u32) {
    // tss descriptor in long mode is 16 bytes (2 GDT slots)
    // base = 63:56 of high | 39:16 of low
    // limit = 19:16 of high | 15:0 of low
    // Access rights derived from low bits 47:40 + G bit + AVL
    let index = tr.index() as usize;
    let low  = unsafe { *gdt_base.add(index) };
    let high = unsafe { *gdt_base.add(index + 1) };

    let base_low  = ((low >> 16) & 0xff_ffff) | (((low >> 56) & 0xff) << 24);
    let base_high = high & 0xffff_ffff;
    let base = base_low | (base_high << 32);

    let limit_low = (low & 0xffff) as u32;
    let limit_high = ((low >> 48) & 0xf) as u32;
    let limit = limit_low | (limit_high << 16);

    let ar_native = ((low >> 40) & 0xff) as u32 | (((low >> 52) & 0xf) << 12);

    (base, limit, ar_native)
}