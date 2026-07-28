// hypervisor/src/lib.rs
#![no_std]

pub mod descriptor;
pub mod ept;
pub mod logging;
pub mod support;
pub mod vmcs;
pub mod vmexit;
pub mod vmlaunch;
pub mod vmm;
pub mod vmx;

// no_std needs a panic handler somewhere in the final binary —
// put it in driver/, not here
