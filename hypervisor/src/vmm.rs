use core::ffi::c_void;
use core::ptr::{null_mut, write_bytes};

use wdk_sys::{
    ntddk::{
        ExAllocatePool2, KeGetCurrentProcessorNumber, KeGetProcessorNumberFromIndex,
        KeQueryActiveProcessorCountEx, KeRevertToUserGroupAffinityThread,
        KeSetSystemGroupAffinityThread, MmGetPhysicalAddress,
    },
    GROUP_AFFINITY, KAFFINITY, POOL_FLAG_NON_PAGED, PROCESSOR_NUMBER,
};

use x86::msr::{rdmsr, IA32_VMX_BASIC};

use crate::vmx::{
    adjust_control_regs, enable_vmx_operation, has_vmx_support, VmxRegion, VMX_REGION_SIZE,
};

const VMM_TAG: u32 = u32::from_le_bytes(*b"Arro");
const ALL_PROCESSOR_GROUPS: u16 = 0xffff;
const PAGE_SIZE: usize = 0x1000;
const VMM_STACK_SIZE: usize = 0x6000;
const VMX_REVISION_ID_MASK: u32 = 0x7fff_ffff;

unsafe fn phys_of(ptr: *mut c_void) -> u64 {
    unsafe { MmGetPhysicalAddress(ptr).QuadPart as u64 }
}
pub struct Vcpu {
    pub vmcs: *mut VmxRegion,
    pub vmcs_physical: u64,
    pub vmxon: *mut VmxRegion,
    pub vmxon_physical: u64,

    pub msr_bitmap: *mut u8,
    pub msr_bitmap_physical: u64,

    pub vmm_context: *mut VmmContext,
}

pub struct VmmContext {
    // ExAllocatePool2 zeroes memory by default
    processor_count: u32,
    pub vcpu_table: *mut *mut Vcpu,
    pub stack: *mut u8,
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
    let revision_identifier = (basic as u32) & VMX_REVISION_ID_MASK;

    unsafe {
        (*vmxon).header = revision_identifier;
        (*vcpu).vmxon = vmxon;
        (*vcpu).vmxon_physical = phys_of(vmxon.cast());
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
    let revision_identifier = (basic as u32) & VMX_REVISION_ID_MASK;

    unsafe {
        (*vmcs).header = revision_identifier;
        (*vcpu).vmcs = vmcs;
        (*vcpu).vmcs_physical = phys_of(vmcs.cast());
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
        (*vcpu).msr_bitmap_physical = phys_of(msr_bitmap.cast());
    }
    true
}

pub unsafe fn allocate_vmm_context() -> *mut VmmContext {
    let ctx: *mut VmmContext = unsafe {
        ExAllocatePool2(POOL_FLAG_NON_PAGED, size_of::<VmmContext>() as u64, VMM_TAG).cast()
    };

    if ctx.is_null() {
        log::error!(
            "vmm.rs: ExAllocatePool2 failed: size={} tag={:#x}",
            size_of::<VmmContext>(),
            VMM_TAG,
        );
        return null_mut();
    };

    let processor_count = unsafe { KeQueryActiveProcessorCountEx(ALL_PROCESSOR_GROUPS) };

    if processor_count == 0 {
        log::error!("vmm.rs: No available processors",);
        return null_mut();
    };

    unsafe { (*ctx).processor_count = processor_count };

    let vcpu_tbl_len = size_of::<*mut Vcpu>() * processor_count as usize;

    let vcpu_tbl: *mut *mut Vcpu =
        unsafe { ExAllocatePool2(POOL_FLAG_NON_PAGED, vcpu_tbl_len as u64, VMM_TAG).cast() };

    if vcpu_tbl.is_null() {
        log::error!(
            "vmm.rs: ExAllocatePool2 failed: size={} tag={:#x}",
            vcpu_tbl_len as u64,
            VMM_TAG,
        );
        return null_mut();
    };

    unsafe { (*ctx).vcpu_table = vcpu_tbl };

    let stack: *mut u8 =
        unsafe { ExAllocatePool2(POOL_FLAG_NON_PAGED, VMM_STACK_SIZE as u64, VMM_TAG).cast() };

    if stack.is_null() {
        log::error!(
            "vmm.rs: ExAllocatePool2 failed: size={} tag={:#x}",
            VMM_STACK_SIZE as u64,
            VMM_TAG,
        );
        return null_mut();
    };

    unsafe {
        write_bytes(stack, 0xcc, VMM_STACK_SIZE);
        (*ctx).stack = stack;
    }

    ctx
}

pub unsafe fn init_vcpu() -> *mut Vcpu {
    let vcpu: *mut Vcpu =
        unsafe { ExAllocatePool2(POOL_FLAG_NON_PAGED, size_of::<Vcpu>() as u64, VMM_TAG).cast() };
    if vcpu.is_null() {
        log::error!(
            "vmm.rs: ExAllocatePool2 failed: size={} tag={:#x}",
            size_of::<VmmContext>(),
            VMM_TAG,
        );
        return null_mut();
    };

    if !unsafe { init_vmxon(vcpu) } {
        return null_mut();
    }
    if !unsafe { init_vmcs(vcpu) } {
        return null_mut();
    }
    if !unsafe { init_msr_bitmap(vcpu) } {
        return null_mut();
    }

    vcpu
}

pub unsafe fn vmm_init() -> bool {
    let vmm_context: *mut VmmContext = unsafe { allocate_vmm_context() };
    if vmm_context.is_null() {
        return false;
    }

    let processor_count = unsafe { (*vmm_context).processor_count };

    // 1: allocate vcpus
    for index in 0..processor_count {
        let vcpu = unsafe { init_vcpu() };
        if vcpu.is_null() {
            log::error!("vcpu alloc failed for processor {}", index);
            return false;
        }
        unsafe {
            *(*vmm_context).vcpu_table.add(index as usize) = vcpu;
            (*vcpu).vmm_context = vmm_context;
        }
    }

    // 2: pin to each core and enter VMX
    for index in 0..processor_count {
        let mut proc_num = unsafe { core::mem::zeroed::<PROCESSOR_NUMBER>() };
        let mut aff = unsafe { core::mem::zeroed::<GROUP_AFFINITY>() };
        let mut old_aff = unsafe { core::mem::zeroed::<GROUP_AFFINITY>() };

        unsafe { KeGetProcessorNumberFromIndex(index, &mut proc_num) };
        aff.Group = proc_num.Group;
        aff.Mask = (1 as KAFFINITY) << proc_num.Number;

        unsafe { KeSetSystemGroupAffinityThread(&aff, &mut old_aff) };
        let ok = unsafe { init_logical_processor(vmm_context) };
        unsafe { KeRevertToUserGroupAffinityThread(&old_aff) };

        if !ok {
            log::error!("VMX init failed on processor {}", index);
            return false;
        }
    }

    true
}

pub unsafe fn init_logical_processor(vmm_context: *mut VmmContext) -> bool {
    let processor_number = unsafe { KeGetCurrentProcessorNumber() };

    let vcpu = unsafe { *(*vmm_context).vcpu_table.add(processor_number as usize) };
    if vcpu.is_null() {
        log::error!("no vcpu for processor {}", processor_number);
        return false;
    }

    if !has_vmx_support() {
        log::error!("VMX not supported on processor {}", processor_number);
        return false;
    }

    if !unsafe { enable_vmx_operation() } {
        log::error!("enable_vmx_operation failed on processor {}", processor_number);
        return false;
    }

    if unsafe { x86::bits64::vmx::vmxon((*vcpu).vmxon_physical) }.is_err() {
        log::error!("VMXON instruction failed on vcpu {:p}", vcpu);
        return false;
    }

    log::info!("vcpu {:p} in VMX operation on processor {}", vcpu, processor_number);
    true
}
