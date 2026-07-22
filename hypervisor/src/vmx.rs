use core::arch::x86_64::__cpuid;

use x86::controlregs::{cr0, cr0_write, cr4, cr4_write};
use x86::msr::{
    rdmsr, wrmsr, IA32_FEATURE_CONTROL, IA32_VMX_CR0_FIXED0, IA32_VMX_CR0_FIXED1,
    IA32_VMX_CR4_FIXED0, IA32_VMX_CR4_FIXED1,
};

pub const VMX_REGION_SIZE: usize = 0x1000;

#[repr(C, align(4096))]
pub struct VmxRegion {
    pub header: u32,
    pub abort_indicator: u32,
    pub data: [u8; VMX_REGION_SIZE - 8],
}

pub fn has_vmx_support() -> bool {
    let r = unsafe { __cpuid(1) };
    (r.ecx & (1 << 5)) != 0
}

pub fn vmxon(vmxon_region: u64) {
    unsafe { x86::bits64::vmx::vmxon(vmxon_region).unwrap() };
}

pub unsafe fn adjust_control_regs() {
    let c0 = unsafe { cr0() }.bits() as u64;
    let c0_f0 = unsafe { rdmsr(IA32_VMX_CR0_FIXED0) };
    let c0_f1 = unsafe { rdmsr(IA32_VMX_CR0_FIXED1) };
    unsafe {
        cr0_write(x86::controlregs::Cr0::from_bits_truncate(
            ((c0 | c0_f0) & c0_f1) as usize,
        ));
    }

    let c4 = unsafe { cr4() }.bits() as u64 | (1 << 13);
    let c4_f0 = unsafe { rdmsr(IA32_VMX_CR4_FIXED0) };
    let c4_f1 = unsafe { rdmsr(IA32_VMX_CR4_FIXED1) };
    unsafe {
        cr4_write(x86::controlregs::Cr4::from_bits_truncate(
            ((c4 | c4_f0) & c4_f1) as usize,
        ));
    }
}

pub unsafe fn enable_vmx_operation() -> bool {
    const VMX_LOCK_BIT: u64 = 1 << 0;
    const VMXON_OUTSIDE_SMX: u64 = 1 << 2;

    unsafe { adjust_control_regs() };

    let fc = unsafe { rdmsr(IA32_FEATURE_CONTROL) };
    if (fc & VMX_LOCK_BIT) == 0 {
        unsafe { wrmsr(IA32_FEATURE_CONTROL, fc | VMXON_OUTSIDE_SMX | VMX_LOCK_BIT) };
    } else if (fc & VMXON_OUTSIDE_SMX) == 0 {
        log::error!("VMX disabled by BIOS: FEATURE_CONTROL locked without VMXON_OUTSIDE_SMX");
        return false;
    }
    true
}
