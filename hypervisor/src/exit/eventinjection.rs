use bit_field::BitField;

use crate::support::vmwrite;
use crate::vmm::Vcpu;
use x86::vmx::vmcs;

use super::vmexit::VmExitAction;

const RFLAGS_RF_BIT: usize = 16;
const VECTOR_INVALID_OPCODE: u8 = 6;
const VECTOR_GENERAL_PROTECTION: u8 = 13;

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InterruptionType {
    ExternalInterrupt = 0,
    Reserved = 1,
    Nmi = 2,
    HardwareException = 3,
    SoftwareInterrupt = 4,
    PrivilegedSoftwareException = 5,
    SoftwareException = 6,
    OtherEvent = 7,
}

#[derive(Debug, Clone, Copy)]
pub struct VmEntryEvent {
    pub vector: u8,
    pub interruption_type: InterruptionType,
    pub error_code: Option<u32>,
}

impl VmEntryEvent {
    pub const fn exception(vector: u8, error_code: Option<u32>) -> Self {
        Self {
            vector,
            interruption_type: InterruptionType::HardwareException,
            error_code,
        }
    }

    pub const fn interruption_info(self) -> u32 {
        let mut value = self.vector as u32;

        value |= (self.interruption_type as u32) << 8;

        if self.error_code.is_some() {
            value |= 1 << 11;
        }

        value | (1 << 31)
    }
}

unsafe fn inject_exception(vcpu: &mut Vcpu, event: VmEntryEvent) {
    unsafe {
        vmwrite(
            vmcs::control::VMENTRY_INTERRUPTION_INFO_FIELD,
            u64::from(event.interruption_info()),
        );
        vmwrite(vmcs::control::VMENTRY_INSTRUCTION_LEN, 0u64);

        if let Some(error_code) = event.error_code {
            vmwrite(
                vmcs::control::VMENTRY_EXCEPTION_ERR_CODE,
                u64::from(error_code),
            );
        }
    }

    // rf avoids retriggering the same fault on entry
    // https://revers.engineering/day-5-vmexits-interrupts-cpuid-emulation/
    vcpu.regs.rflags.set_bit(RFLAGS_RF_BIT, true);
    unsafe {
        vmwrite(vmcs::guest::RFLAGS, vcpu.regs.rflags);
    }
}

pub unsafe fn inject_ud(vcpu: &mut Vcpu) -> VmExitAction {
    // make the instruction look unsupported
    unsafe { inject_exception(vcpu, VmEntryEvent::exception(VECTOR_INVALID_OPCODE, None)) };
    VmExitAction::ResumeWithoutAdvance
}

pub unsafe fn inject_gp(vcpu: &mut Vcpu) -> VmExitAction {
    unsafe {
        inject_exception(
            vcpu,
            VmEntryEvent::exception(VECTOR_GENERAL_PROTECTION, Some(0)),
        )
    };
    VmExitAction::ResumeWithoutAdvance
}
