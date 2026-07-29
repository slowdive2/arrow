use crate::support::{vmread, vmwrite};
use crate::vmm::Vcpu;
use x86::vmx::vmcs;

use super::{cpuid, genericvmx, msr, triplefault};

const VM_ENTRY_FAILED: u64 = 0x41; // CF | ZF, Intel SDM 31.2 "Conventions"

// Intel SDM Appendix C, Table C-1 "Basic Exit Reasons" 
mod exit_reason {
    pub const TRIPLE_FAULT: u64 = 2;
    pub const CPUID: u64 = 10;
    pub const VMCALL: u64 = 18;
    pub const VMCLEAR: u64 = 19;
    pub const VMLAUNCH: u64 = 20;
    pub const VMPTRLD: u64 = 21;
    pub const VMPTRST: u64 = 22;
    pub const VMREAD: u64 = 23;
    pub const VMRESUME: u64 = 24;
    pub const VMWRITE: u64 = 25;
    pub const VMXOFF: u64 = 26;
    pub const VMXON: u64 = 27;
    pub const RDMSR: u64 = 31;
    pub const WRMSR: u64 = 32;
    pub const INVEPT: u64 = 50;
    pub const INVVPID: u64 = 53;
    pub const VMFUNC: u64 = 59;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VmExitAction {
    ResumeAndAdvance,
    ResumeWithoutAdvance,
    Shutdown,
}

unsafe fn advance_guest_rip() {
    let rip = vmread(vmcs::guest::RIP);
    let len = vmread(vmcs::ro::VMEXIT_INSTRUCTION_LEN);
    unsafe {
        vmwrite(vmcs::guest::RIP, rip + len);
    }
}

pub unsafe fn handle_vmexit(rflags: u64, vcpu: &mut Vcpu) -> VmExitAction {
    if rflags & VM_ENTRY_FAILED != 0 {
        log::error!(
            "vmlaunch failed: rflags={:#x} vm-instruction-error={:#x}",
            rflags,
            vmread(vmcs::ro::VM_INSTRUCTION_ERROR),
        );
        return VmExitAction::Shutdown;
    }

    let reason = vmread(vmcs::ro::EXIT_REASON) & 0xffff;
    let qualification = vmread(vmcs::ro::EXIT_QUALIFICATION);

    let action = match reason {
        exit_reason::CPUID => cpuid::handle_cpuid(vcpu),
        exit_reason::RDMSR => unsafe { msr::handle_msr_access(vcpu, false) },
        exit_reason::WRMSR => unsafe { msr::handle_msr_access(vcpu, true) },
        exit_reason::TRIPLE_FAULT => unsafe { triplefault::handle_triple_fault(vcpu) },
        exit_reason::VMCALL
        | exit_reason::VMCLEAR
        | exit_reason::VMLAUNCH
        | exit_reason::VMPTRLD
        | exit_reason::VMPTRST
        | exit_reason::VMREAD
        | exit_reason::VMRESUME
        | exit_reason::VMWRITE
        | exit_reason::VMXOFF
        | exit_reason::VMXON
        | exit_reason::INVEPT
        | exit_reason::VMFUNC
        | exit_reason::INVVPID => unsafe { genericvmx::handle_generic_vmx(vcpu) },
        _ => {
            log::error!(
                "unhandled vm-exit: reason={:#x} qualification={:#x}",
                reason,
                qualification,
            );
            VmExitAction::Shutdown
        }
    };

    if action == VmExitAction::ResumeAndAdvance {
        unsafe { advance_guest_rip() };
    }

    action
}
