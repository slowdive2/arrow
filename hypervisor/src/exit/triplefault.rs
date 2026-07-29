use bitfield_struct::bitfield;
use x86::io::{inb, outb};
use x86::vmx::vmcs;

use crate::support::vmread;
use crate::vmm::Vcpu;

const RST_CNT_IO_PORT: u16 = 0x0cf9;

#[bitfield(u8)]
struct ResetControlRegister {
    reserved0: bool,
    system_reset: bool,
    reset_cpu: bool,
    full_reset: bool,

    #[bits(4)]
    __: u8,
}

// requests a physical platform reset through I/O port 0xCF9 - doesnt return
pub unsafe fn platform_hard_reset() -> ! {
    let raw_register = unsafe { inb(RST_CNT_IO_PORT) };

    let mut reset_register = ResetControlRegister::from(raw_register);

    reset_register.set_reset_cpu(true);
    reset_register.set_system_reset(true);

    let raw_register: u8 = reset_register.into();

    unsafe {
        outb(RST_CNT_IO_PORT, raw_register);
    }

    // dont continue executing if the reset request fails.. shouldnt happen anyways
    loop {
        core::hint::spin_loop();
    }
}

fn dump_vcpu_state(vcpu: &Vcpu) {
    log::error!(
        "triple fault: rip={:#x} rsp={:#x} rax={:#x} rbx={:#x} rcx={:#x} rdx={:#x}",
        vmread(vmcs::guest::RIP),
        vmread(vmcs::guest::RSP),
        vcpu.guest_registers.rax,
        vcpu.guest_registers.rbx,
        vcpu.guest_registers.rcx,
        vcpu.guest_registers.rdx,
    );
}

pub unsafe fn handle_triple_fault(vcpu: &Vcpu) -> ! {
    dump_vcpu_state(vcpu);

    unsafe { platform_hard_reset() }
}
