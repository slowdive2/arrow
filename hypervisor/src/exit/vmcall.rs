// tiny vmcall interface for ept changes

use crate::vmm::Vcpu;

use super::vmexit::VmExitAction;

pub const ARROW_HYPERCALL_MAGIC: u64 = u64::from_le_bytes(*b"ArrowEPT");
pub const HYPERCALL_ARM_EXECUTE_MONITOR: u64 = 1;

const HYPERCALL_SUCCESS: u64 = 0;
const HYPERCALL_ERROR: u64 = u64::MAX;

// unknown vmcalls still get #ud
pub unsafe fn handle(vcpu: &mut Vcpu) -> Option<VmExitAction> {
    if vcpu.regs.r10 != ARROW_HYPERCALL_MAGIC {
        return None;
    }

    let result = match vcpu.regs.rcx {
        HYPERCALL_ARM_EXECUTE_MONITOR if !vcpu.ept.is_null() => unsafe {
            crate::ept::Ept::watch_exec(vcpu.ept, vcpu.regs.rdx)
        },
        service => {
            log::warn!("unknown Arrow hypercall service {service:#x}");
            vcpu.regs.rax = HYPERCALL_ERROR;
            return Some(VmExitAction::ResumeAndAdvance);
        }
    };

    match result {
        Ok(()) => vcpu.regs.rax = HYPERCALL_SUCCESS,
        Err(error) => {
            log::error!("EPT monitor hypercall failed: {error:?}");
            vcpu.regs.rax = HYPERCALL_ERROR;
        }
    }

    Some(VmExitAction::ResumeAndAdvance)
}
