use core::arch::x86_64::__cpuid;

use {
    bitfield::BitMut,
    x86::{current::paging::BASE_PAGE_SIZE, msr},
    x86_64::registers::control::{Cr0, Cr4},
};

pub fn has_vmx_support() -> bool {
    let cpuid_resp = unsafe { __cpuid(1) };
    (cpuid_resp.ecx & (1 << 5)) != 0
}

pub fn enable_vmx_operation() -> bool {
    const VMX_LOCK_BIT: u64 = 1 << 0;
    const VMXON_OUTSIDE_SMX: u64 = 1 << 2;

    let mut cr4 = Cr4::read_raw();
    cr4.set_bit(13, true);
    unsafe { Cr4::write_raw(cr4) };

    let ia32_fc = unsafe { msr::rdmsr(msr::IA32_FEATURE_CONTROL) };
    if (ia32_fc & VMX_LOCK_BIT) == 0 {
        unsafe { msr::wrmsr(msr::IA32_FEATURE_CONTROL, VMXON_OUTSIDE_SMX | VMX_LOCK_BIT | ia32_fc) };
    } else if (ia32_fc & VMXON_OUTSIDE_SMX) == 0 {
        // TODO: ERR ON BIOS LOCK
        return false;
    }
    true
}

