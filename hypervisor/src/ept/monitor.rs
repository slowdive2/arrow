// ept page splitting and execute traps

extern crate alloc;

use alloc::{boxed::Box, vec::Vec};
use core::{
    hint::spin_loop,
    ptr,
    ptr::NonNull,
    sync::atomic::{AtomicBool, Ordering},
};

use bitfield_struct::bitfield;
use wdk_sys::{
    ntddk::{ExAllocatePool2, ExFreePoolWithTag, MmFreeContiguousMemory},
    POOL_FLAG_NON_PAGED,
};

use super::{
    alloc_map, invept_single, mtrr_type, phys_of, Ept4KbPageEntry, EptPageDirectoryEntry,
    EptPageMap, EptPageTable, EptPagingStructureMemoryType, EptPointer, EptSplit, EptTableEntry,
    MtrrRange, EPT_FOUR_LEVEL_WALK_LENGTH, EPT_IDENTITY_MAP_SPAN, EPT_LARGE_PAGE_SHIFT,
    EPT_LARGE_PAGE_SIZE, EPT_PAGE_SHIFT, EPT_PAGE_SIZE, EPT_TAG,
};

// vm exits use these instead of calling the allocator
pub const SPLIT_COUNT: usize = 16;

const EPT_LARGE_PAGE_MASK: u64 = EPT_LARGE_PAGE_SIZE - 1;
const EPT_PAGE_MASK: u64 = EPT_PAGE_SIZE as u64 - 1;
const EPT_LARGE_PAGE_BIT: u64 = 1 << 7;

// one shared map means one tiny edit lock is enough
static EPT_LOCK: AtomicBool = AtomicBool::new(false);

fn lock_ept() {
    while EPT_LOCK
        .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
        .is_err()
    {
        spin_loop();
    }
}

fn unlock_ept() {
    EPT_LOCK.store(false, Ordering::Release);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EptError {
    OutOfRange,
    NoSplitPage,
    SplitNotFound,
    InveptFailed,
}

// ept violation exit qual
#[bitfield(u64)]
#[derive(PartialEq, Eq)]
pub struct EptViolationQualification {
    pub read: bool,
    pub write: bool,
    pub execute: bool,
    pub ept_read: bool,
    pub ept_write: bool,
    pub ept_exec: bool,
    pub ept_user_exec: bool,
    pub gla_valid: bool,
    pub translated: bool,
    pub user: bool,
    pub guest_rw: bool,
    pub guest_exec: bool,
    pub nmi_unblock: bool,
    #[bits(51)]
    __: u64,
}

// all vcpus point at this one map
pub struct Ept {
    map: NonNull<EptPageMap>,
    eptp: u64,
    default_type: u8,
    mtrrs: Vec<MtrrRange>,
    free_pts: Vec<NonNull<EptPageTable>>,
    splits: Vec<EptSplit>,
}

impl Ept {
    // allocate everything before vmx root mode
    pub unsafe fn new(mtrrs: &[MtrrRange], default_type: u8) -> Option<Box<Self>> {
        let map = NonNull::new(unsafe { alloc_map(mtrrs, default_type) })?;
        let pml4_pa = unsafe { phys_of(map.as_ptr().cast()) };
        let eptp = EptPointer::new()
            .with_mem_type(EptPagingStructureMemoryType::WriteBack as u8)
            .with_walk_len_minus_one(EPT_FOUR_LEVEL_WALK_LENGTH)
            .with_ad_enabled(false)
            .with_pml4_pfn(pml4_pa >> EPT_PAGE_SHIFT)
            .into_bits();

        let mut ept = Box::new(Self {
            map,
            eptp,
            default_type,
            mtrrs: mtrrs.to_vec(),
            free_pts: Vec::with_capacity(SPLIT_COUNT),
            splits: Vec::with_capacity(SPLIT_COUNT),
        });

        for _ in 0..SPLIT_COUNT {
            let pt = NonNull::new(unsafe {
                ExAllocatePool2(POOL_FLAG_NON_PAGED, EPT_PAGE_SIZE as u64, EPT_TAG)
                    .cast::<EptPageTable>()
            });

            let Some(pt) = pt else {
                log::error!("ept split page alloc failed");
                return None;
            };
            ept.free_pts.push(pt);
        }

        Some(ept)
    }

    pub const fn eptp(&self) -> u64 {
        self.eptp
    }

    // pde covering this gpa
    fn pde(&mut self, gpa: u64) -> Result<*mut EptPageDirectoryEntry, EptError> {
        if gpa >= EPT_IDENTITY_MAP_SPAN {
            return Err(EptError::OutOfRange);
        }

        let pdpt_i = ((gpa >> 30) & 0x1ff) as usize;
        let pd_i = ((gpa >> EPT_LARGE_PAGE_SHIFT) & 0x1ff) as usize;
        let map = unsafe { self.map.as_mut() };

        Ok(ptr::from_mut(&mut map.pds[pdpt_i].entries[pd_i]))
    }

    // pte covering this gpa after a split
    fn pte(&mut self, gpa: u64) -> Result<*mut Ept4KbPageEntry, EptError> {
        if gpa >= EPT_IDENTITY_MAP_SPAN {
            return Err(EptError::OutOfRange);
        }

        let page_base = gpa & !EPT_LARGE_PAGE_MASK;
        let pt_i = ((gpa >> EPT_PAGE_SHIFT) & 0x1ff) as usize;
        let split = self
            .splits
            .iter_mut()
            .find(|split| split.gpa == page_base)
            .ok_or(EptError::SplitNotFound)?;
        let pt = unsafe { split.pt.as_mut() };

        Ok(ptr::from_mut(&mut pt.entries[pt_i]))
    }

    // replace one 2 mib pde with 512 matching 4 kib ptes
    fn split_2mb(&mut self, gpa: u64) -> Result<(), EptError> {
        let pde = self.pde(gpa)?;
        let raw = unsafe { (*pde).raw };
        if raw & EPT_LARGE_PAGE_BIT == 0 {
            return Ok(());
        }

        let old = unsafe { (*pde).large_page };
        let pt = self.free_pts.pop().ok_or(EptError::NoSplitPage)?;
        unsafe { ptr::write_bytes(pt.as_ptr(), 0, 1) };

        let page_base = old.pfn() << EPT_LARGE_PAGE_SHIFT;
        for (i, pte) in unsafe { &mut *pt.as_ptr() }.entries.iter_mut().enumerate() {
            let pa = page_base + i as u64 * EPT_PAGE_SIZE as u64;
            *pte = Ept4KbPageEntry::new()
                .with_readable(old.readable())
                .with_writable(old.writable())
                .with_executable(old.executable())
                .with_mem_type(mtrr_type(&self.mtrrs, self.default_type, pa))
                .with_ignore_pat(old.ignore_pat())
                .with_accessed(old.accessed())
                .with_dirty(old.dirty())
                .with_user_executable(old.user_executable())
                .with_pfn(pa >> EPT_PAGE_SHIFT)
                .with_suppress_ve(old.suppress_ve());
        }

        let pt_pa = unsafe { phys_of(pt.as_ptr().cast()) };
        let new_pde = EptTableEntry::new()
            .with_readable(true)
            .with_writable(true)
            .with_executable(true)
            .with_pfn(pt_pa >> EPT_PAGE_SHIFT);

        // capacity was reserved in new(), so this does not allocate
        self.splits.push(EptSplit { gpa: page_base, pt });
        unsafe { (*pde).table = new_pde };

        Ok(())
    }

    // every broadcast caller also flushes its own cpu
    fn watch_exec_locked(&mut self, gpa: u64) -> Result<(), EptError> {
        self.split_2mb(gpa)?;
        let pte = self.pte(gpa & !EPT_PAGE_MASK)?;
        unsafe { ptr::write(pte, (*pte).with_executable(false)) };
        if !unsafe { invept_single(self.eptp) } {
            return Err(EptError::InveptFailed);
        }
        Ok(())
    }

    // restore execute and retry the faulting instruction
    fn handle_violation_locked(
        &mut self,
        qual: EptViolationQualification,
        gpa: u64,
    ) -> Result<bool, EptError> {
        if !qual.execute() || qual.ept_exec() {
            return Ok(false);
        }

        let pte = match self.pte(gpa & !EPT_PAGE_MASK) {
            Ok(pte) => pte,
            Err(EptError::SplitNotFound) => return Ok(false),
            Err(err) => return Err(err),
        };

        let old = unsafe { *pte };

        // another cpu restored it, but this cpu still cached the old pte
        if old.executable() {
            if !unsafe { invept_single(self.eptp) } {
                return Err(EptError::InveptFailed);
            }
            return Ok(true);
        }

        unsafe { ptr::write(pte, old.with_executable(true)) };
        if !unsafe { invept_single(self.eptp) } {
            return Err(EptError::InveptFailed);
        }
        Ok(true)
    }

    // serialize edits to the shared map
    pub unsafe fn watch_exec(ept: *mut Self, gpa: u64) -> Result<(), EptError> {
        lock_ept();
        let result = unsafe { (&mut *ept).watch_exec_locked(gpa) };
        unlock_ept();
        result
    }

    // serialize the one-shot restore too
    pub unsafe fn handle_violation(
        ept: *mut Self,
        qual: EptViolationQualification,
        gpa: u64,
    ) -> Result<bool, EptError> {
        lock_ept();
        let result = unsafe { (&mut *ept).handle_violation_locked(qual, gpa) };
        unlock_ept();
        result
    }
}

impl Drop for Ept {
    fn drop(&mut self) {
        while let Some(split) = self.splits.pop() {
            unsafe { ExFreePoolWithTag(split.pt.as_ptr().cast(), EPT_TAG) };
        }
        while let Some(pt) = self.free_pts.pop() {
            unsafe { ExFreePoolWithTag(pt.as_ptr().cast(), EPT_TAG) };
        }
        unsafe { MmFreeContiguousMemory(self.map.as_ptr().cast()) };
    }
}
