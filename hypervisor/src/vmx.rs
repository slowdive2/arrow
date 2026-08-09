use core::arch::x86_64::__cpuid;

use x86::controlregs::{cr0, cr0_write, cr4, cr4_write};
use x86::msr::{
    self, rdmsr, wrmsr, IA32_FEATURE_CONTROL, IA32_VMX_BASIC, IA32_VMX_CR0_FIXED0,
    IA32_VMX_CR0_FIXED1, IA32_VMX_CR4_FIXED0, IA32_VMX_CR4_FIXED1,
};

pub const VMX_REGION_SIZE: usize = 0x1000;

#[repr(C, align(4096))]
pub struct VmxRegion {
    pub header: u32,
    pub abort_indicator: u32,
    pub data: [u8; VMX_REGION_SIZE - 8],
}

pub fn has_vmx_support() -> bool {
    let result = __cpuid(1);
    result.ecx & (1 << 5) != 0
}

pub unsafe fn adjust_control_regs() {
    let current_cr0 = unsafe { cr0() }.bits() as u64;
    let cr0_fixed0 = unsafe { rdmsr(IA32_VMX_CR0_FIXED0) };
    let cr0_fixed1 = unsafe { rdmsr(IA32_VMX_CR0_FIXED1) };

    let adjusted_cr0 = (current_cr0 | cr0_fixed0) & cr0_fixed1;

    unsafe {
        cr0_write(x86::controlregs::Cr0::from_bits_truncate(
            adjusted_cr0 as usize,
        ));
    }

    const CR4_VMXE: u64 = 1 << 13;

    let current_cr4 = unsafe { cr4() }.bits() as u64;
    let cr4_fixed0 = unsafe { rdmsr(IA32_VMX_CR4_FIXED0) };
    let cr4_fixed1 = unsafe { rdmsr(IA32_VMX_CR4_FIXED1) };

    let adjusted_cr4 = ((current_cr4 | CR4_VMXE) | cr4_fixed0) & cr4_fixed1;

    unsafe {
        cr4_write(x86::controlregs::Cr4::from_bits_truncate(
            adjusted_cr4 as usize,
        ));
    }
}

pub unsafe fn enable_vmx() -> bool {
    const VMX_LOCK_BIT: u64 = 1 << 0;
    const VMXON_OUTSIDE_SMX: u64 = 1 << 2;

    unsafe {
        adjust_control_regs();
    }

    let feature_control = unsafe { rdmsr(IA32_FEATURE_CONTROL) };

    if feature_control & VMX_LOCK_BIT == 0 {
        unsafe {
            wrmsr(
                IA32_FEATURE_CONTROL,
                feature_control | VMXON_OUTSIDE_SMX | VMX_LOCK_BIT,
            );
        }
    } else if feature_control & VMXON_OUTSIDE_SMX == 0 {
        log::error!("VMX disabled by BIOS: FEATURE_CONTROL locked without VMXON_OUTSIDE_SMX");
        return false;
    }

    true
}

const IA32_VMX_BASIC_TRUE_CONTROLS: u64 = 1 << 55;

fn has_true_controls() -> bool {
    let basic = unsafe { rdmsr(IA32_VMX_BASIC) };
    basic & IA32_VMX_BASIC_TRUE_CONTROLS != 0
}

unsafe fn adjust_cv(capability_msr: u32, requested: u32) -> u32 {
    let capability = unsafe { rdmsr(capability_msr) };

    let allowed_zero = capability as u32;
    let allowed_one = (capability >> 32) as u32;

    (requested | allowed_zero) & allowed_one
}

unsafe fn adjust_true_control(requested: u32, legacy_msr: u32, true_msr: u32) -> u32 {
    let capability_msr = if has_true_controls() {
        true_msr
    } else {
        legacy_msr
    };

    unsafe { adjust_cv(capability_msr, requested) }
}

pub unsafe fn adjust_pinbased_controls(requested: u32) -> u32 {
    unsafe {
        adjust_true_control(
            requested,
            msr::IA32_VMX_PINBASED_CTLS,
            msr::IA32_VMX_TRUE_PINBASED_CTLS,
        )
    }
}

pub unsafe fn adjust_primary_controls(requested: u32) -> u32 {
    unsafe {
        adjust_true_control(
            requested,
            msr::IA32_VMX_PROCBASED_CTLS,
            msr::IA32_VMX_TRUE_PROCBASED_CTLS,
        )
    }
}

pub unsafe fn adjust_secondary_controls(requested: u32) -> u32 {
    unsafe { adjust_cv(msr::IA32_VMX_PROCBASED_CTLS2, requested) }
}

pub unsafe fn adjust_exit_controls(requested: u32) -> u32 {
    unsafe {
        adjust_true_control(
            requested,
            msr::IA32_VMX_EXIT_CTLS,
            msr::IA32_VMX_TRUE_EXIT_CTLS,
        )
    }
}

pub unsafe fn adjust_entry_controls(requested: u32) -> u32 {
    unsafe {
        adjust_true_control(
            requested,
            msr::IA32_VMX_ENTRY_CTLS,
            msr::IA32_VMX_TRUE_ENTRY_CTLS,
        )
    }
}
