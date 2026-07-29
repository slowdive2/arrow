use bit_field::BitField;
use core::arch::x86_64::__cpuid_count;

use crate::vmm::Vcpu;

use super::vmexit::VmExitAction;

// https://learn.microsoft.com/en-us/virtualization/hyper-v-on-windows/tlfs/feature-discovery
#[allow(dead_code)]
pub enum CpuidLeaf {
    VendorInfo = 0x0,
    FeatureInformation = 0x1,
    CacheInformation = 0x2,
    ExtendedFeatureInformation = 0x7,
    HypervisorVendor = 0x40000000,
    HypervisorInterface = 0x40000001,
    HypervisorSystemIdentity = 0x40000002,
    HypervisorFeatureIdentification = 0x40000003,
    ImplementationRecommendations = 0x40000004,
    HypervisorImplementationLimits = 0x40000005,
    ImplementationHardwareFeatures = 0x40000006,
    NestedHypervisorFeatureIdentification = 0x40000009,
    HypervisorNestedVirtualizationFeatures = 0x4000000A,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[allow(dead_code)]
enum FeatureBits {
    // assumes eax = 1
    HypervisorVmxSupportBit = 5,
    HypervisorPresentBit = 31,
}

pub fn handle_cpuid(vcpu: &mut Vcpu) -> VmExitAction {
    let leaf = vcpu.guest_registers.rax as u32;
    let subleaf = vcpu.guest_registers.rcx as u32;

    let mut cpuid_result = unsafe { __cpuid_count(leaf, subleaf) };

    if leaf == CpuidLeaf::FeatureInformation as u32 {
        cpuid_result
            .ecx
            .set_bit(FeatureBits::HypervisorPresentBit as usize, false); // lie to guest about hypervisor status

        log::debug!("cpuid: FeatureInformation requested; clearing HypervisorPresent bit");
    } else {
        log::debug!("cpuid: generic leaf={leaf:#x}, subleaf={subleaf:#x}");
    }

    vcpu.guest_registers.rax = cpuid_result.eax as u64;
    vcpu.guest_registers.rbx = cpuid_result.ebx as u64;
    vcpu.guest_registers.rcx = cpuid_result.ecx as u64;
    vcpu.guest_registers.rdx = cpuid_result.edx as u64;

    VmExitAction::ResumeAndAdvance
}
