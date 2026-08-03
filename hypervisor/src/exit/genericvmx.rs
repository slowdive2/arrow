use crate::vmm::Vcpu;

use super::eventinjection;
use super::vmexit::VmExitAction;

pub unsafe fn handle(vcpu: &mut Vcpu) -> VmExitAction {
    unsafe { eventinjection::inject_ud(vcpu) }
}
