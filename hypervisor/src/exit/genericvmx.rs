use crate::vmm::Vcpu;

use super::eventinjection;
use super::vmexit::VmExitAction;

pub unsafe fn handle_generic_vmx(vcpu: &mut Vcpu) -> VmExitAction {
    unsafe { eventinjection::inject_invalidopcode(vcpu) }
}
