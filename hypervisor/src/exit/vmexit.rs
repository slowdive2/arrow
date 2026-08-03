use crate::support::{vmread, vmwrite};
use crate::vmm::Vcpu;
use x86::vmx::vmcs;

use super::{cpuid, ept, genericvmx, msr, triplefault, vmcall};

const VM_ENTRY_FAILED: u64 = 0x41; // cf | zf, intel sdm 31.2 "conventions"

// intel sdm appendix c, table c-1 "basic exit reasons"
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
    pub const EPT_VIOLATION: u64 = 48;
    pub const EPT_MISCONFIGURATION: u64 = 49;
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

fn advance_rip() {
    let rip = vmread(vmcs::guest::RIP);
    let len = vmread(vmcs::ro::VMEXIT_INSTRUCTION_LEN);
    vmwrite(vmcs::guest::RIP, rip + len);
}

pub unsafe fn handle(rflags: u64, vcpu: &mut Vcpu) -> VmExitAction {
    if rflags & VM_ENTRY_FAILED != 0 {
        log::error!(
            "vmlaunch failed: rflags={:#x} vm-instruction-error={:#x}",
            rflags,
            vmread(vmcs::ro::VM_INSTRUCTION_ERROR),
        );
        return VmExitAction::Shutdown;
    }

    let reason = vmread(vmcs::ro::EXIT_REASON) & 0xffff;
    let qual = vmread(vmcs::ro::EXIT_QUALIFICATION);

    let action = match reason {
        exit_reason::CPUID => cpuid::handle(vcpu),
        exit_reason::RDMSR => unsafe { msr::handle(vcpu, false) },
        exit_reason::WRMSR => unsafe { msr::handle(vcpu, true) },
        exit_reason::TRIPLE_FAULT => unsafe { triplefault::handle(vcpu) },
        exit_reason::EPT_VIOLATION => unsafe {
            ept::handle_violation(vcpu, qual, vmread(vmcs::ro::GUEST_PHYSICAL_ADDR_FULL))
        },
        exit_reason::EPT_MISCONFIGURATION => {
            ept::handle_misconfig(vmread(vmcs::ro::GUEST_PHYSICAL_ADDR_FULL))
        }
        exit_reason::VMCALL => unsafe {
            vmcall::handle(vcpu).unwrap_or_else(|| genericvmx::handle(vcpu))
        },
        exit_reason::VMCLEAR
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
        | exit_reason::INVVPID => unsafe { genericvmx::handle(vcpu) },
        _ => {
            log::error!("unhandled vm-exit: reason={:#x} qual={:#x}", reason, qual,);
            VmExitAction::Shutdown
        }
    };

    if action == VmExitAction::ResumeAndAdvance {
        advance_rip();
    }

    action
}
