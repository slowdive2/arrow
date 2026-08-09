use crate::vmm::Vcpu;

use super::eventinjection;
use super::vmexit::VmExitAction;

pub unsafe fn handle(vcpu: &mut Vcpu, _write: bool) -> VmExitAction {
    unsafe { eventinjection::inject_gp(vcpu) }
}
