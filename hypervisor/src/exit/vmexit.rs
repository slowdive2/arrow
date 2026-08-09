use crate::support::{vmread, vmwrite};
use crate::vmm::Vcpu;
use x86::vmx::vmcs;

use super::{cpuid, ept, genericvmx, msr, triplefault, vmcall};

pub const VM_ENTRY_FAILED: u64 = 0x41; // cf | zf, intel sdm 31.2 "conventions"

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
    ShutdownAndAdvance,
}

fn read_vmcs(field: u32) -> Option<u64> {
    match vmread(field) {
        Ok(value) => Some(value),
        Err(error) => {
            log::error!("vmread failed for field {field:#x}: {error:?}");
            None
        }
    }
}

fn refresh_guest_state(vcpu: &mut Vcpu) -> bool {
    let state = (
        read_vmcs(vmcs::guest::RIP),
        read_vmcs(vmcs::guest::RSP),
        read_vmcs(vmcs::guest::RFLAGS),
    );

    let (Some(rip), Some(rsp), Some(rflags)) = state else {
        return false;
    };

    vcpu.regs.rip = rip;
    vcpu.regs.rsp = rsp;
    vcpu.regs.rflags = rflags;
    true
}

fn advance_rip(vcpu: &mut Vcpu) -> bool {
    let Some(len) = read_vmcs(vmcs::ro::VMEXIT_INSTRUCTION_LEN) else {
        return false;
    };
    let Some(next_rip) = vcpu.regs.rip.checked_add(len) else {
        log::error!("guest RIP overflow while advancing by {len}");
        return false;
    };

    match vmwrite(vmcs::guest::RIP, next_rip) {
        Ok(()) => {
            vcpu.regs.rip = next_rip;
            true
        }
        Err(error) => {
            log::error!("vmwrite failed for guest RIP: {error:?}");
            false
        }
    }
}

pub unsafe fn handle(rflags: u64, vcpu: &mut Vcpu) -> VmExitAction {
    if rflags & VM_ENTRY_FAILED != 0 {
        match vmread(vmcs::ro::VM_INSTRUCTION_ERROR) {
            Ok(error) => log::error!(
                "vmlaunch failed: rflags={:#x} vm-instruction-error={:#x}",
                rflags,
                error,
            ),
            Err(error) => log::error!(
                "vmlaunch failed: rflags={:#x} vmread failed: {error:?}",
                rflags,
            ),
        }
        return VmExitAction::Shutdown;
    }

    let Some(reason) = read_vmcs(vmcs::ro::EXIT_REASON) else {
        return VmExitAction::Shutdown;
    };
    let reason = reason & 0xffff;
    let Some(qual) = read_vmcs(vmcs::ro::EXIT_QUALIFICATION) else {
        return VmExitAction::Shutdown;
    };
    if !refresh_guest_state(vcpu) {
        return VmExitAction::Shutdown;
    }

    let action = match reason {
        exit_reason::CPUID => cpuid::handle(vcpu),
        exit_reason::RDMSR => unsafe { msr::handle(vcpu, false) },
        exit_reason::WRMSR => unsafe { msr::handle(vcpu, true) },
        exit_reason::TRIPLE_FAULT => unsafe { triplefault::handle(vcpu) },
        exit_reason::EPT_VIOLATION => unsafe {
            let Some(gpa) = read_vmcs(vmcs::ro::GUEST_PHYSICAL_ADDR_FULL) else {
                return VmExitAction::Shutdown;
            };
            ept::handle_violation(vcpu, qual, gpa)
        },
        exit_reason::EPT_MISCONFIGURATION => {
            let Some(gpa) = read_vmcs(vmcs::ro::GUEST_PHYSICAL_ADDR_FULL) else {
                return VmExitAction::Shutdown;
            };
            ept::handle_misconfig(gpa)
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

    if matches!(
        action,
        VmExitAction::ResumeAndAdvance | VmExitAction::ShutdownAndAdvance
    ) && !advance_rip(vcpu)
    {
        log::error!("cannot safely resume or devirtualize without advancing guest RIP");
        loop {
            core::hint::spin_loop();
        }
    }

    match action {
        VmExitAction::ShutdownAndAdvance => VmExitAction::Shutdown,
        _ => action,
    }
}
