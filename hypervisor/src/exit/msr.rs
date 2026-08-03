use crate::support::{rdmsr, wrmsr};
use crate::vmm::Vcpu;

use super::eventinjection;
use super::vmexit::VmExitAction;

const RESERVED_MSR_RANGE_LOW: u32 = 0x4000_0000;
const RESERVED_MSR_RANGE_HIGH: u32 = 0x4000_00ff;
const MSR_MASK_LOW: u64 = 0xffff_ffff;

pub unsafe fn handle(vcpu: &mut Vcpu, write: bool) -> VmExitAction {
    let msr_id = vcpu.regs.rcx as u32;

    if (RESERVED_MSR_RANGE_LOW..=RESERVED_MSR_RANGE_HIGH).contains(&msr_id) {
        return unsafe { eventinjection::inject_gp(vcpu) };
    }

    if write {
        let high = (vcpu.regs.rdx & MSR_MASK_LOW) << 32;
        let low = vcpu.regs.rax & MSR_MASK_LOW;
        let msr_value = high | low;

        wrmsr(msr_id, msr_value);
    } else {
        let msr_value = rdmsr(msr_id);

        vcpu.regs.rdx = msr_value >> 32;
        vcpu.regs.rax = msr_value & MSR_MASK_LOW;
    }

    VmExitAction::ResumeAndAdvance
}

// todo: invalid msrs should inject #gp
