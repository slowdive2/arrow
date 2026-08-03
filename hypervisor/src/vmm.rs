extern crate alloc;

use alloc::boxed::Box;
use core::ffi::c_void;
use core::mem::size_of;
use core::ptr::null_mut;

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
use crate::exit::vmexit::{handle, VmExitAction};
use crate::support::vmxoff;
use crate::vmcs::{capture_registers, setup_vmcs, GuestRegs};
use crate::vmlaunch::launch_vm;
use crate::vmx::{enable_vmx, has_vmx_support, VmxRegion, VMX_REGION_SIZE};

const VMM_TAG: u32 = u32::from_le_bytes(*b"Arro");
const ALL_PROCESSOR_GROUPS: u16 = 0xffff;
const PAGE_SIZE: usize = 0x1000;
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
    // every cpu borrows the same ept
    pub ept: *mut Ept,
    pub regs: GuestRegs,

    pub guest_desc: Descriptors,
    pub host_desc: Descriptors,
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
    unsafe { (*vcpu).ept = ept };

    if !vmxon_ok || !vmcs_ok || !msr_ok {
        unsafe {
            free_pool((*vcpu).msr_bitmap);
            free_pool((*vcpu).vmcs);
            free_pool((*vcpu).vmxon);
            free_pool(vcpu);
        }
        return null_mut();
    }

    vcpu
}

pub unsafe fn vmm_init() -> bool {
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
        unsafe {
            for i in 0..cpu_count {
                let vcpu = *(*ctx).vcpus.add(i as usize);
                if !vcpu.is_null() {
                    free_pool((*vcpu).msr_bitmap);
                    free_pool((*vcpu).vmcs);
                    free_pool((*vcpu).vmxon);
                }
                free_pool(vcpu);
            }
            free_ept((*ctx).ept);
            free_pool((*ctx).vcpus);
            free_pool(ctx);
        }
        return false;
    }

    // then enter vmx on each cpu
    for i in 0..cpu_count {
        let mut cpu_num = unsafe { core::mem::zeroed::<PROCESSOR_NUMBER>() };
        let mut aff = unsafe { core::mem::zeroed::<GROUP_AFFINITY>() };
        let mut old_aff = unsafe { core::mem::zeroed::<GROUP_AFFINITY>() };

        unsafe { KeGetProcessorNumberFromIndex(i, &mut cpu_num) };
        aff.Group = cpu_num.Group;
        aff.Mask = (1 as KAFFINITY) << cpu_num.Number;

        unsafe { KeSetSystemGroupAffinityThread(&aff, &mut old_aff) };
        let vcpu = unsafe { *(*ctx).vcpus.add(i as usize) };
        let ok = unsafe { init_cpu(vcpu, i) };
        unsafe { KeRevertToUserGroupAffinityThread(&old_aff) };

        if !ok {
            log::error!("VMX init failed on processor {}", i);
            return false;
        }
    }

    true
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

    if !unsafe { enable_vmx() } {
        log::error!("enable_vmx failed on processor {}", cpu);
        return false;
    }

    if unsafe { x86::bits64::vmx::vmxon((*vcpu).vmxon_pa) }.is_err() {
        log::error!("VMXON instruction failed on vcpu {:p}", vcpu);
        return false;
    }

    (*vcpu).guest_desc = Descriptors::capture_current();
    // no separate host address space yet
    (*vcpu).host_desc = Descriptors::capture_current();

    log::info!("vcpu {:p} in VMX operation on processor {}", vcpu, cpu);

    unsafe { capture_registers(&mut (*vcpu).regs) };

    if !unsafe { setup_vmcs(vcpu) } {
        log::error!("setup_vmcs failed on processor {}", cpu);
        vmxoff();
        return false;
    }

    let mut launched = 0u64;
    loop {
        let rflags = unsafe { launch_vm(&mut (*vcpu).regs, launched) };

        if unsafe { handle(rflags, &mut *vcpu) } == VmExitAction::Shutdown {
            vmxoff();
            return true;
        }

        launched = 1;
    }
}
