use crate::support::{vmread, vmxoff};
use x86::vmx::vmcs;

const VM_ENTRY_FAILED: u64 = 0x41; // CF | ZF, per Intel SDM 31.2 "Conventions"

pub fn handle_vmexit(rflags: u64) -> bool {
    if rflags & VM_ENTRY_FAILED != 0 {
        log::error!(
            "vmlaunch failed: rflags={:#x} vm-instruction-error={:#x}",
            rflags,
            vmread(vmcs::ro::VM_INSTRUCTION_ERROR),
        );
        vmxoff();
        return false;
    }

    let reason = vmread(vmcs::ro::EXIT_REASON) & 0xffff;
    let qualification = vmread(vmcs::ro::EXIT_QUALIFICATION);

    log::info!(
        "vm-exit: reason={:#x} qualification={:#x}",
        reason,
        qualification,
    );

    vmxoff();
    true
}
