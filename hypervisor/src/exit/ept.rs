// ept exits retry the access, so rip stays put

use crate::{ept::EptViolationQualification, vmm::Vcpu};

use super::vmexit::VmExitAction;

pub unsafe fn handle_violation(vcpu: &mut Vcpu, qual: u64, gpa: u64) -> VmExitAction {
    if vcpu.ept.is_null() {
        log::error!("EPT violation without shared EPT state");
        return VmExitAction::Shutdown;
    }

    let qual = EptViolationQualification::from_bits(qual);
    match unsafe { crate::ept::Ept::handle_violation(vcpu.ept, qual, gpa) } {
        Ok(true) => VmExitAction::ResumeWithoutAdvance,
        Ok(false) => {
            log::error!(
                "unhandled EPT violation: gpa={gpa:#x} qual={qual:#x}",
                qual = qual.into_bits(),
            );
            VmExitAction::Shutdown
        }
        Err(error) => {
            log::error!("EPT violation handling failed: gpa={gpa:#x} error={error:?}");
            VmExitAction::Shutdown
        }
    }
}

// a misconfig means we wrote a bad entry
pub fn handle_misconfig(gpa: u64) -> VmExitAction {
    log::error!("EPT misconfiguration at guest physical address {gpa:#x}");
    VmExitAction::Shutdown
}
