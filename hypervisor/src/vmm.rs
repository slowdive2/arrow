extern crate alloc;

use alloc::boxed::Box;
use core::ffi::c_void;
use core::mem::size_of;
use core::ptr::null_mut;
use core::sync::atomic::{AtomicBool, AtomicPtr, Ordering};

use wdk_sys::{
    ntddk::{
        ExAllocatePool2, ExFreePoolWithTag, KeGetProcessorNumberFromIndex,
        KeQueryActiveProcessorCountEx, KeRevertToUserGroupAffinityThread,
        KeSetSystemGroupAffinityThread, MmGetPhysicalAddress,
    },
    GROUP_AFFINITY, KAFFINITY, POOL_FLAG_NON_PAGED, PROCESSOR_NUMBER,
};

use x86::msr::{rdmsr, IA32_VMX_BASIC};

use crate::descriptor::Descriptors;
use crate::ept::{build_mtrr_map, ept_supported, mtrr_default_type, Ept};
use crate::exit::vmexit::{handle, VmExitAction, VM_ENTRY_FAILED};
use crate::support::{
    cr0, cr0_write, cr3_write, cr4, cr4_write, dr7_write, vmcall_shutdown, vmread, vmxoff, vmxon,
};
use crate::vmcs::{capture_registers, setup_vmcs, GuestRegs};
use crate::vmlaunch::{launch_vm, restore_guest};
use crate::vmx::{enable_vmx, has_vmx_support, VmxRegion, VMX_REGION_SIZE};
use x86::vmx::vmcs;

const VMM_TAG: u32 = u32::from_le_bytes(*b"Arro");
const ALL_PROCESSOR_GROUPS: u16 = 0xffff;
const PAGE_SIZE: usize = 0x1000;
pub const HOST_STACK_SIZE: usize = 0x6000;
const VMX_REVISION_ID_MASK: u32 = 0x7fff_ffff;

unsafe fn phys_of(ptr: *mut c_void) -> u64 {
    unsafe { MmGetPhysicalAddress(ptr).QuadPart as u64 }
}

#[inline]
unsafe fn free_pool<T>(ptr: *mut T) {
    if !ptr.is_null() {
        unsafe { ExFreePoolWithTag(ptr.cast(), VMM_TAG) };
    }
}

pub struct Vcpu {
    pub vmcs: *mut VmxRegion,
    pub vmcs_pa: u64,
    pub vmxon: *mut VmxRegion,
    pub vmxon_pa: u64,

    pub msr_bitmap: *mut u8,
    pub msr_bitmap_pa: u64,
    pub host_stack: *mut u8,
    // every cpu borrows the same ept
    pub ept: *mut Ept,
    pub regs: GuestRegs,

    pub guest_desc: Descriptors,
    pub host_desc: Descriptors,
    original_cr0: u64,
    original_cr4: u64,
    // True while lifecycle cleanup must obtain a successful VMXOFF before
    // freeing any allocation reachable through this vCPU's VMCS.
    active: AtomicBool,
}

#[inline]
unsafe fn free_ept(ept: *mut Ept) {
    if !ept.is_null() {
        drop(unsafe { Box::from_raw(ept) });
    }
}

pub struct Vmm {
    // exallocatepool2 zeroes memory by default
    cpu_count: u32,
    pub vcpus: *mut *mut Vcpu,
    pub ept: *mut Ept,
}

static VMM: AtomicPtr<Vmm> = AtomicPtr::new(null_mut());
static VMM_LIFECYCLE_LOCK: AtomicBool = AtomicBool::new(false);

struct VmmLifecycleGuard;

impl VmmLifecycleGuard {
    fn try_acquire() -> Option<Self> {
        VMM_LIFECYCLE_LOCK
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .ok()
            .map(|_| Self)
    }
}

impl Drop for VmmLifecycleGuard {
    fn drop(&mut self) {
        VMM_LIFECYCLE_LOCK.store(false, Ordering::Release);
    }
}

pub unsafe fn init_vmxon(vcpu: *mut Vcpu) -> bool {
    let vmxon: *mut VmxRegion =
        unsafe { ExAllocatePool2(POOL_FLAG_NON_PAGED, VMX_REGION_SIZE as u64, VMM_TAG).cast() };

    if vmxon.is_null() {
        log::error!(
            "vmm.rs: ExAllocatePool2 failed: size={} tag={:#x}",
            VMX_REGION_SIZE,
            VMM_TAG,
        );
        return false;
    };

    let basic = unsafe { rdmsr(IA32_VMX_BASIC) };
    let revision_id = (basic as u32) & VMX_REVISION_ID_MASK;

    unsafe {
        (*vmxon).header = revision_id;
        (*vcpu).vmxon = vmxon;
        (*vcpu).vmxon_pa = phys_of(vmxon.cast());
    }
    true
}

pub unsafe fn init_vmcs(vcpu: *mut Vcpu) -> bool {
    let vmcs: *mut VmxRegion =
        unsafe { ExAllocatePool2(POOL_FLAG_NON_PAGED, VMX_REGION_SIZE as u64, VMM_TAG).cast() };
    if vmcs.is_null() {
        log::error!(
            "vmm.rs: ExAllocatePool2 failed: size={} tag={:#x}",
            VMX_REGION_SIZE,
            VMM_TAG,
        );
        return false;
    };
    let basic = unsafe { rdmsr(IA32_VMX_BASIC) };
    let revision_id = (basic as u32) & VMX_REVISION_ID_MASK;

    unsafe {
        (*vmcs).header = revision_id;
        (*vcpu).vmcs = vmcs;
        (*vcpu).vmcs_pa = phys_of(vmcs.cast());
    }
    true
}

pub unsafe fn init_msr_bitmap(vcpu: *mut Vcpu) -> bool {
    let msr_bitmap: *mut u8 =
        unsafe { ExAllocatePool2(POOL_FLAG_NON_PAGED, PAGE_SIZE as u64, VMM_TAG).cast() };
    if msr_bitmap.is_null() {
        log::error!(
            "ExAllocatePool2 failed: size={} tag={:#x}",
            PAGE_SIZE,
            VMM_TAG
        );
        return false;
    }
    unsafe {
        (*vcpu).msr_bitmap = msr_bitmap;
        (*vcpu).msr_bitmap_pa = phys_of(msr_bitmap.cast());
    }
    true
}

pub unsafe fn init_host_stack(vcpu: *mut Vcpu) -> bool {
    let host_stack: *mut u8 =
        unsafe { ExAllocatePool2(POOL_FLAG_NON_PAGED, HOST_STACK_SIZE as u64, VMM_TAG).cast() };
    if host_stack.is_null() {
        log::error!(
            "ExAllocatePool2 failed: size={} tag={:#x}",
            HOST_STACK_SIZE,
            VMM_TAG
        );
        return false;
    }
    unsafe { (*vcpu).host_stack = host_stack };
    true
}

pub unsafe fn alloc_vmm(ept: *mut Ept) -> *mut Vmm {
    let cpu_count = unsafe { KeQueryActiveProcessorCountEx(ALL_PROCESSOR_GROUPS) };

    if cpu_count == 0 {
        log::error!("vmm.rs: No available processors",);
        return null_mut();
    };

    let table_size = size_of::<*mut Vcpu>() * cpu_count as usize;

    let ctx: *mut Vmm =
        unsafe { ExAllocatePool2(POOL_FLAG_NON_PAGED, size_of::<Vmm>() as u64, VMM_TAG).cast() };
    let vcpus: *mut *mut Vcpu =
        unsafe { ExAllocatePool2(POOL_FLAG_NON_PAGED, table_size as u64, VMM_TAG).cast() };

    if ctx.is_null() || vcpus.is_null() {
        if ctx.is_null() {
            log::error!(
                "vmm.rs: ExAllocatePool2 failed: size={} tag={:#x}",
                size_of::<Vmm>(),
                VMM_TAG,
            );
        }
        if vcpus.is_null() {
            log::error!(
                "vmm.rs: ExAllocatePool2 failed: size={} tag={:#x}",
                table_size as u64,
                VMM_TAG,
            );
        }
        unsafe {
            free_pool(vcpus);
            free_pool(ctx);
        }
        return null_mut();
    }

    unsafe {
        (*ctx).cpu_count = cpu_count;
        (*ctx).vcpus = vcpus;
        (*ctx).ept = ept;
    }

    ctx
}

pub unsafe fn init_vcpu(ept: *mut Ept) -> *mut Vcpu {
    let vcpu: *mut Vcpu =
        unsafe { ExAllocatePool2(POOL_FLAG_NON_PAGED, size_of::<Vcpu>() as u64, VMM_TAG).cast() };
    if vcpu.is_null() {
        log::error!(
            "vmm.rs: ExAllocatePool2 failed: size={} tag={:#x}",
            size_of::<Vcpu>(),
            VMM_TAG,
        );
        return null_mut();
    };

    let vmxon_ok = unsafe { init_vmxon(vcpu) };
    let vmcs_ok = unsafe { init_vmcs(vcpu) };
    let msr_ok = unsafe { init_msr_bitmap(vcpu) };
    let host_stack_ok = unsafe { init_host_stack(vcpu) };
    unsafe { (*vcpu).ept = ept };

    if !vmxon_ok || !vmcs_ok || !msr_ok || !host_stack_ok {
        unsafe {
            free_pool((*vcpu).host_stack);
            free_pool((*vcpu).msr_bitmap);
            free_pool((*vcpu).vmcs);
            free_pool((*vcpu).vmxon);
            free_pool(vcpu);
        }
        return null_mut();
    }

    vcpu
}

unsafe fn free_vcpu(vcpu: *mut Vcpu) {
    if vcpu.is_null() {
        return;
    }

    unsafe {
        free_pool((*vcpu).host_stack);
        free_pool((*vcpu).msr_bitmap);
        free_pool((*vcpu).vmcs);
        free_pool((*vcpu).vmxon);
        free_pool(vcpu);
    }
}

unsafe fn free_vmm(ctx: *mut Vmm) {
    if ctx.is_null() {
        return;
    }

    unsafe {
        for i in 0..(*ctx).cpu_count {
            free_vcpu(*(*ctx).vcpus.add(i as usize));
        }
        free_ept((*ctx).ept);
        free_pool((*ctx).vcpus);
        free_pool(ctx);
    }
}

unsafe fn switch_to_processor(index: u32) -> Option<GROUP_AFFINITY> {
    let mut cpu_num = unsafe { core::mem::zeroed::<PROCESSOR_NUMBER>() };
    if unsafe { KeGetProcessorNumberFromIndex(index, &mut cpu_num) } < 0 {
        log::error!("cannot find processor {}", index);
        return None;
    }

    let mut affinity = unsafe { core::mem::zeroed::<GROUP_AFFINITY>() };
    let mut old_affinity = unsafe { core::mem::zeroed::<GROUP_AFFINITY>() };
    affinity.Group = cpu_num.Group;
    affinity.Mask = (1 as KAFFINITY) << cpu_num.Number;
    unsafe { KeSetSystemGroupAffinityThread(&affinity, &mut old_affinity) };
    Some(old_affinity)
}

unsafe fn shutdown_vcpu(ctx: *mut Vmm, index: u32) -> bool {
    let vcpu = unsafe { *(*ctx).vcpus.add(index as usize) };
    if vcpu.is_null() || !unsafe { (*vcpu).active.load(Ordering::Acquire) } {
        return true;
    }

    let Some(old_affinity) = (unsafe { switch_to_processor(index) }) else {
        return false;
    };
    let ok = unsafe { vmcall_shutdown() };
    unsafe { KeRevertToUserGroupAffinityThread(&old_affinity) };

    ok && !unsafe { (*vcpu).active.load(Ordering::Acquire) }
}

unsafe fn shutdown_all(ctx: *mut Vmm) -> bool {
    let mut ok = true;
    for i in 0..unsafe { (*ctx).cpu_count } {
        ok &= unsafe { shutdown_vcpu(ctx, i) };
    }
    ok
}

unsafe fn rollback(ctx: *mut Vmm) {
    if !unsafe { shutdown_all(ctx) } {
        log::error!("rollback failed");
        loop {
            core::hint::spin_loop();
        }
    }
    unsafe { free_vmm(ctx) };
}

/// Stops all virtual processors and releases the complete VMM ownership graph.
///
/// # Safety
///
/// The caller must run at PASSIVE_LEVEL, must not independently manipulate VMX
/// state on these processors, and must prevent new hypervisor client work from
/// beginning during driver teardown.
pub unsafe fn vmm_shutdown() -> bool {
    let Some(_lifecycle_guard) = VmmLifecycleGuard::try_acquire() else {
        log::error!("VMM lifecycle operation already in progress");
        return false;
    };

    let ctx = VMM.load(Ordering::Acquire);
    if ctx.is_null() {
        return true;
    }

    if !unsafe { shutdown_all(ctx) } {
        log::error!("failed to stop every virtual processor");
        return false;
    }

    VMM.store(null_mut(), Ordering::Release);
    unsafe { free_vmm(ctx) };
    true
}

/// Allocates and launches the VMM on every active logical processor.
///
/// # Safety
///
/// The caller must run at PASSIVE_LEVEL and must have exclusive authority to
/// establish VMX operation on the selected processors.
pub unsafe fn vmm_init() -> bool {
    let Some(_lifecycle_guard) = VmmLifecycleGuard::try_acquire() else {
        log::error!("VMM lifecycle operation already in progress");
        return false;
    };

    if !VMM.load(Ordering::Acquire).is_null() {
        log::error!("vmm already initialized");
        return false;
    }

    // check before putting any eptp in a vmcs
    if !has_vmx_support() || !ept_supported() {
        return false;
    }

    // firmware keeps mtrrs in sync, so read them once
    let mtrrs = build_mtrr_map();
    let default_type = mtrr_default_type();
    // this needs special consideration.. lifecycle is roughly:
    // ept::new -> box::into_raw to avoid destruction -> ... free_ept..box::from_raw MANUALLY reconstructs to free
    // this kinda (definitely) goes against rust's philosophy
    let Some(ept) = (unsafe { Ept::new(&mtrrs, default_type) }) else {
        return false;
    };
    let ept = Box::into_raw(ept);

    let ctx: *mut Vmm = unsafe { alloc_vmm(ept) };
    if ctx.is_null() {
        unsafe { free_ept(ept) };
        return false;
    }
    let cpu_count = unsafe { (*ctx).cpu_count };

    // allocate each vcpu first
    let mut alloc_failed = false;
    for i in 0..cpu_count {
        let vcpu = unsafe { init_vcpu(ept) };
        alloc_failed |= vcpu.is_null();
        if vcpu.is_null() {
            log::error!("vcpu alloc failed for processor {}", i);
        }
        unsafe {
            *(*ctx).vcpus.add(i as usize) = vcpu;
        }
    }

    if alloc_failed {
        unsafe { free_vmm(ctx) };
        return false;
    }

    // then enter vmx on each cpu
    for i in 0..cpu_count {
        let Some(old_affinity) = (unsafe { switch_to_processor(i) }) else {
            unsafe { rollback(ctx) };
            return false;
        };
        let vcpu = unsafe { *(*ctx).vcpus.add(i as usize) };
        let ok = unsafe { init_cpu(vcpu, i) };
        unsafe { KeRevertToUserGroupAffinityThread(&old_affinity) };

        if !ok {
            log::error!("VMX init failed on processor {}", i);
            unsafe { rollback(ctx) };
            return false;
        }
    }

    VMM.store(ctx, Ordering::Release);
    true
}

fn vmxoff_or_halt() {
    if let Err(error) = vmxoff() {
        log::error!("VMXOFF failed: {error:?}");
        loop {
            core::hint::spin_loop();
        }
    }
}

unsafe fn stop_cpu(vcpu: *mut Vcpu) -> ! {
    let state = (
        vmread(vmcs::guest::RIP),
        vmread(vmcs::guest::RSP),
        vmread(vmcs::guest::RFLAGS),
        vmread(vmcs::guest::CR0),
        vmread(vmcs::guest::CR3),
        vmread(vmcs::guest::CR4),
        vmread(vmcs::guest::DR7),
    );

    let (rip, rsp, rflags, guest_cr0, guest_cr3, guest_cr4, guest_dr7) = match state {
        (Ok(rip), Ok(rsp), Ok(rflags), Ok(cr0), Ok(cr3), Ok(cr4), Ok(dr7)) => {
            (rip, rsp, rflags, cr0, cr3, cr4, dr7)
        }
        error => {
            log::error!("failed to read guest state during shutdown: {error:?}");
            loop {
                core::hint::spin_loop();
            }
        }
    };

    unsafe {
        (*vcpu).regs.rip = rip;
        (*vcpu).regs.rsp = rsp;
        (*vcpu).regs.rflags = rflags;
    }

    vmxoff_or_halt();

    // Once VMXOFF succeeds, hardware can no longer reference this vCPU's
    // VMXON region, VMCS, host stack, MSR bitmap, or EPT through this VMCS.
    unsafe { (*vcpu).active.store(false, Ordering::Release) };

    unsafe {
        cr0_write(guest_cr0);
        cr3_write(guest_cr3);
        cr4_write((guest_cr4 & !(1 << 13)) | ((*vcpu).original_cr4 & (1 << 13)));
        dr7_write(guest_dr7);
    }

    unsafe { restore_guest(&(*vcpu).regs) }
}

unsafe fn restore_control_registers(vcpu: *mut Vcpu) {
    unsafe {
        cr0_write((*vcpu).original_cr0);
        cr4_write((*vcpu).original_cr4);
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "win64" fn vmexit_handler(vcpu: *mut Vcpu) -> ! {
    loop {
        if unsafe { handle(0, &mut *vcpu) } == VmExitAction::Shutdown {
            unsafe { stop_cpu(vcpu) }
        }

        let rflags = unsafe { launch_vm(&mut (*vcpu).regs, 1) };
        if rflags & VM_ENTRY_FAILED != 0 {
            unsafe { handle(rflags, &mut *vcpu) };
            unsafe { stop_cpu(vcpu) }
        }
    }
}

pub unsafe fn init_cpu(vcpu: *mut Vcpu, cpu: u32) -> bool {
    if vcpu.is_null() {
        log::error!("no vcpu for processor {}", cpu);
        return false;
    }

    if !has_vmx_support() {
        log::error!("VMX not supported on processor {}", cpu);
        return false;
    }

    unsafe {
        (*vcpu).original_cr0 = cr0().bits() as u64;
        (*vcpu).original_cr4 = cr4();
    }

    if !unsafe { enable_vmx() } {
        log::error!("enable_vmx failed on processor {}", cpu);
        unsafe { restore_control_registers(vcpu) };
        return false;
    }

    if let Err(error) = vmxon(unsafe { (*vcpu).vmxon_pa }) {
        log::error!("VMXON instruction failed on vcpu {:p}: {error:?}", vcpu);
        unsafe { restore_control_registers(vcpu) };
        return false;
    }

    (*vcpu).guest_desc = Descriptors::capture_current();
    // no separate host address space yet
    (*vcpu).host_desc = Descriptors::capture_current();

    log::info!("vcpu {:p} in VMX operation on processor {}", vcpu, cpu);

    if unsafe { capture_registers(&mut (*vcpu).regs) } {
        return unsafe { (*vcpu).active.load(Ordering::Acquire) };
    }
    unsafe { (*vcpu).regs.rax = 1 };

    match unsafe { setup_vmcs(vcpu) } {
        Ok(true) => {}
        Ok(false) => {
            log::error!("setup_vmcs failed on processor {}", cpu);
            vmxoff_or_halt();
            unsafe { restore_control_registers(vcpu) };
            return false;
        }
        Err(error) => {
            log::error!("VMCS instruction failed on processor {}: {error:?}", cpu);
            vmxoff_or_halt();
            unsafe { restore_control_registers(vcpu) };
            return false;
        }
    }

    // From this point until a successful VMXOFF, hardware may consume every
    // VMX allocation reachable from this vCPU.
    unsafe { (*vcpu).active.store(true, Ordering::Release) };
    let rflags = unsafe { launch_vm(&mut (*vcpu).regs, 0) };
    unsafe { handle(rflags, &mut *vcpu) };
    vmxoff_or_halt();
    unsafe { (*vcpu).active.store(false, Ordering::Release) };
    unsafe { restore_control_registers(vcpu) };
    false
}
