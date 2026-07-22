// hypervisor/src/lib.rs
#![no_std]

pub mod ept;
pub mod vmcs;
pub mod vmexit;
pub mod vmm;
pub mod vmx;

// no_std needs a panic handler somewhere in the final binary —
// put it in driver/, not here, since hypervisor/ might get unit-tested
// on host later (std enabled via cfg(test) is a future option, not now)
