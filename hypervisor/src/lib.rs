#![no_std]

pub mod descriptor;
pub mod ept;
pub mod exit;
pub mod logging;
pub mod support;
pub mod vmcs;
pub mod vmlaunch;
pub mod vmm;
pub mod vmx;

// the driver owns the panic handler
