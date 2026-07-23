use core::{arch::global_asm, fmt, mem};
use bitfield_struct::bitfield;
//! VMCS guest register state.
//!
//! The `GuestRegisters` layout and the capture-then-launch pattern
//! are from illusion-rs (Copyright © memN0ps, MIT License)
//! Original: https://github.com/memN0ps/illusion-rs

extern "efiapi" {
    pub fn capture_registers(registers: &mut GuestRegisters) -> bool;
}

#[bitfield(u32)]
#[derive(PartialEq, Eq)]
pub struct SegmentAccessRights {
    #[bits(4)] pub segment_type: u8,
    pub s: bool,
    #[bits(2)] pub dpl: u8,
    pub present: bool,
    #[bits(4)] __: u8,
    pub avl: bool,
    pub long_mode: bool,
    pub default_big: bool,
    pub granularity: bool,
    pub unusable: bool,
    #[bits(15)] __: u32,
}

#[repr(C, align(16))]
#[derive(Clone, Copy, Default)]
pub struct GuestRegisters {
    pub rax: u64,
    pub rcx: u64,
    pub rdx: u64,
    pub rbx: u64,
    pub rsp: u64,
    pub rbp: u64,
    pub rsi: u64,
    pub rdi: u64,
    pub r8: u64,
    pub r9: u64,
    pub r10: u64,
    pub r11: u64,
    pub r12: u64,
    pub r13: u64,
    pub r14: u64,
    pub r15: u64,
    pub rip: u64,
    pub rflags: u64,
    pub xmm0: M128A,
    pub xmm1: M128A,
    pub xmm2: M128A,
    pub xmm3: M128A,
    pub xmm4: M128A,
    pub xmm5: M128A,
    pub xmm6: M128A,
    pub xmm7: M128A,
    pub xmm8: M128A,
    pub xmm9: M128A,
    pub xmm10: M128A,
    pub xmm11: M128A,
    pub xmm12: M128A,
    pub xmm13: M128A,
    pub xmm14: M128A,
    pub xmm15: M128A,
    pub original_lstar: u64,
    pub hook_lstar: u64,
}

unsafe fn lar(selector: u16) -> u32 {
    let result: u32;
    core::arch::asm!(
        "lar {result:e}, {sel:x}",
        result = out(reg) result,
        sel = in(reg) selector as u32,
        options(nomem, nostack, preserves_flags),
    );
    result
}

global_asm!(
    r#"
// Captures current general purpose registers, RFLAGS, RSP, RIP, and XMM registers.
//
// extern "efiapi" fn capture_registers(registers: &mut GuestRegisters)
.global capture_registers
capture_registers:
    // Capture general purpose registers.
    mov     [rcx + {registers_rax}], rax
    mov     [rcx + {registers_rcx}], rcx
    mov     [rcx + {registers_rdx}], rdx
    mov     [rcx + {registers_rbx}], rbx
    mov     [rcx + {registers_rsp}], rsp
    mov     [rcx + {registers_rbp}], rbp
    mov     [rcx + {registers_rsi}], rsi
    mov     [rcx + {registers_rdi}], rdi
    mov     [rcx + {registers_r8}],  r8
    mov     [rcx + {registers_r9}],  r9
    mov     [rcx + {registers_r10}], r10
    mov     [rcx + {registers_r11}], r11
    mov     [rcx + {registers_r12}], r12
    mov     [rcx + {registers_r13}], r13
    mov     [rcx + {registers_r14}], r14
    mov     [rcx + {registers_r15}], r15

    // Capture RFLAGS.
    pushfq
    pop     rax
    mov     [rcx + {registers_rflags}], rax

    // Capture RSP.
    mov     rax, rsp
    add     rax, 8
    mov     [rcx + {registers_rsp}], rax

    // Capture RIP.
    mov     rax, [rsp]
    mov     [rcx + {registers_rip}], rax

    // Capture XMM registers.
    movaps  [rcx + {registers_xmm0}], xmm0
    movaps  [rcx + {registers_xmm1}], xmm1
    movaps  [rcx + {registers_xmm2}], xmm2
    movaps  [rcx + {registers_xmm3}], xmm3
    movaps  [rcx + {registers_xmm4}], xmm4
    movaps  [rcx + {registers_xmm5}], xmm5
    movaps  [rcx + {registers_xmm6}], xmm6
    movaps  [rcx + {registers_xmm7}], xmm7
    movaps  [rcx + {registers_xmm8}], xmm8
    movaps  [rcx + {registers_xmm9}], xmm9
    movaps  [rcx + {registers_xmm10}], xmm10
    movaps  [rcx + {registers_xmm11}], xmm11
    movaps  [rcx + {registers_xmm12}], xmm12
    movaps  [rcx + {registers_xmm13}], xmm13
    movaps  [rcx + {registers_xmm14}], xmm14
    movaps  [rcx + {registers_xmm15}], xmm15

    // Return false to indicate that the processor is not virtualized currently.
    xor rax, rax

    ret
"#,
    registers_rax = const mem::offset_of!(GuestRegisters, rax),
    registers_rcx = const mem::offset_of!(GuestRegisters, rcx),
    registers_rdx = const mem::offset_of!(GuestRegisters, rdx),
    registers_rbx = const mem::offset_of!(GuestRegisters, rbx),
    registers_rsp = const mem::offset_of!(GuestRegisters, rsp),
    registers_rbp = const mem::offset_of!(GuestRegisters, rbp),
    registers_rsi = const mem::offset_of!(GuestRegisters, rsi),
    registers_rdi = const mem::offset_of!(GuestRegisters, rdi),
    registers_r8  = const mem::offset_of!(GuestRegisters, r8),
    registers_r9  = const mem::offset_of!(GuestRegisters, r9),
    registers_r10 = const mem::offset_of!(GuestRegisters, r10),
    registers_r11 = const mem::offset_of!(GuestRegisters, r11),
    registers_r12 = const mem::offset_of!(GuestRegisters, r12),
    registers_r13 = const mem::offset_of!(GuestRegisters, r13),
    registers_r14 = const mem::offset_of!(GuestRegisters, r14),
    registers_r15 = const mem::offset_of!(GuestRegisters, r15),
    registers_rip = const mem::offset_of!(GuestRegisters, rip),
    registers_rflags = const mem::offset_of!(GuestRegisters, rflags),
    registers_xmm0 = const mem::offset_of!(GuestRegisters, xmm0),
    registers_xmm1 = const mem::offset_of!(GuestRegisters, xmm1),
    registers_xmm2 = const mem::offset_of!(GuestRegisters, xmm2),
    registers_xmm3 = const mem::offset_of!(GuestRegisters, xmm3),
    registers_xmm4 = const mem::offset_of!(GuestRegisters, xmm4),
    registers_xmm5 = const mem::offset_of!(GuestRegisters, xmm5),
    registers_xmm6 = const mem::offset_of!(GuestRegisters, xmm6),
    registers_xmm7 = const mem::offset_of!(GuestRegisters, xmm7),
    registers_xmm8 = const mem::offset_of!(GuestRegisters, xmm8),
    registers_xmm9 = const mem::offset_of!(GuestRegisters, xmm9),
    registers_xmm10 = const mem::offset_of!(GuestRegisters, xmm10),
    registers_xmm11 = const mem::offset_of!(GuestRegisters, xmm11),
    registers_xmm12 = const mem::offset_of!(GuestRegisters, xmm12),
    registers_xmm13 = const mem::offset_of!(GuestRegisters, xmm13),
    registers_xmm14 = const mem::offset_of!(GuestRegisters, xmm14),
    registers_xmm15 = const mem::offset_of!(GuestRegisters, xmm15),
);

fn access_rights_from_native(lar_result: u32) -> u32 {
    // LAR returns descriptor bits 15:0 shifted appropriately
    // Extract just the type/S/DPL/P/AVL/L/D/G bits, drop reserved noise
    let native = (lar_result >> 8) as u16;
    SegmentAccessRights::new()
        .with_segment_type((native & 0xF) as u8)
        .with_s(native & (1 << 4) != 0)
        .with_dpl(((native >> 5) & 0x3) as u8)
        .with_present(native & (1 << 7) != 0)
        .with_avl(native & (1 << 12) != 0)
        .with_long_mode(native & (1 << 13) != 0)
        .with_default_big(native & (1 << 14) != 0)
        .with_granularity(native & (1 << 15) != 0)
        .into_bits()
}

// For unusable segments (like LDTR when not in use):
fn unusable_access_rights() -> u32 {
    SegmentAccessRights::new().with_unusable(true).into_bits()
}

pub fn setup_guest_registers_state(guest_descriptor: &Descriptors, guest_registers: &GuestRegisters) {

        let idtr = x86::dtables::sidt(&mut idtr);

        vmwrite(vmcs::guest::CR0, x86::controlregs::cr0().bits() as u64);
        vmwrite(vmcs::guest::CR3, x86::controlregs::cr3());
        vmwrite(vmcs::guest::CR4, x86::controlregs::cr4().bits() as u64);
        vmwrite(vmcs::guest::DR7, unsafe { x86::debugregs::dr7().0 as u64 });
        vmwrite(vmcs::guest::RSP, guest_registers.rsp);
        vmwrite(vmcs::guest::RIP, guest_registers.rip);
        vmwrite(vmcs::guest::RFLAGS, x86::bits64::rflags::read().bits());
        vmwrite(vmcs::guest::CS_SELECTOR, x86::segmentation::cs());
        vmwrite(vmcs::guest::SS_SELECTOR, x86::segmentation::ss());
        vmwrite(vmcs::guest::DS_SELECTOR, x86::segmentation::ds());
        vmwrite(vmcs::guest::ES_SELECTOR, x86::segmentation::es());
        vmwrite(vmcs::guest::FS_SELECTOR, x86::segmentation::fs());
        vmwrite(vmcs::guest::GS_SELECTOR, x86::segmentation::gs());
        vmwrite(vmcs::guest::LDTR_SELECTOR, 0u16);
        vmwrite(vmcs::guest::TR_SELECTOR, guest_descriptor.tr.bits());
        vmwrite(vmcs::guest::TR_BASE, guest_descriptor.tss.base);
        vmwrite(vmcs::guest::CS_LIMIT, x86::bits64::segmentation::lsl(cs));
        vmwrite(vmcs::guest::SS_LIMIT, x86::bits64::segmentation::lsl(ss));
        vmwrite(vmcs::guest::DS_LIMIT, x86::bits64::segmentation::lsl(ds));
        vmwrite(vmcs::guest::ES_LIMIT, x86::bits64::segmentation::lsl(es));
        vmwrite(vmcs::guest::FS_LIMIT, x86::bits64::segmentation::lsl(fs));
        vmwrite(vmcs::guest::GS_LIMIT, x86::bits64::segmentation::lsl(gs));
        vmwrite(vmcs::guest::LDTR_LIMIT, 0u32);
        vmwrite(vmcs::guest::TR_LIMIT, guest_descriptor.tss.limit);
        vmwrite(vmcs::guest::CS_ACCESS_RIGHTS, unsafe { access_rights_from_native(lar(cs().bits())) as u64 });
        vmwrite(vmcs::guest::SS_ACCESS_RIGHTS, unsafe { access_rights_from_native(lar(ss().bits())) } as u64);
        vmwrite(vmcs::guest::DS_ACCESS_RIGHTS, unsafe { access_rights_from_native(lar(ds().bits())) } as u64);
        vmwrite(vmcs::guest::ES_ACCESS_RIGHTS, unsafe { access_rights_from_native(lar(es().bits())) } as u64);
        vmwrite(vmcs::guest::FS_ACCESS_RIGHTS, unsafe { access_rights_from_native(lar(fs().bits())) } as u64);
        vmwrite(vmcs::guest::GS_ACCESS_RIGHTS, unsafe { access_rights_from_native(lar(gs().bits())) } as u64);
        vmwrite(vmcs::guest::LDTR_ACCESS_RIGHTS, unusable_access_rights() as u64);
        vmwrite(vmcs::guest::TR_ACCESS_RIGHTS, access_rights_from_native(guest_descriptor.tss.ar));
        vmwrite(vmcs::guest::GDTR_BASE, guest_descriptor.gdtr.base as u64);
        vmwrite(vmcs::guest::IDTR_BASE, idtr.base as u64);
        vmwrite(vmcs::guest::GDTR_LIMIT, guest_descriptor.gdtr.limit as u64);
        vmwrite(vmcs::guest::IDTR_LIMIT, idtr.limit as u64);
        vmwrite(vmcs::guest::LINK_PTR_FULL, u64::MAX);
}

pub fn setup_host_registers_state(host_descriptor: &Descriptors, pml4_pa: u64) {

        vmwrite(vmcs::host::CR0, x86::controlregs::cr0().bits() as u64);
        vmwrite(vmcs::host::CR3, pml4_pa);
        vmwrite(vmcs::host::CR4, unsafe { x86::controlregs::cr4() }.bits() as u64);

        vmwrite(vmcs::host::CS_SELECTOR, x86::segmentation::cs());
        vmwrite(vmcs::host::TR_SELECTOR, guest_descriptor.tr.bits());

        vmwrite(vmcs::host::TR_BASE, guest_descriptor.tss.base);
        vmwrite(vmcs::host::GDTR_BASE, guest_descriptor.gdtr.base as u64);
        vmwrite(vmcs::host::IDTR_BASE, u64::MAX);
}

pub unsafe fn init_vmcs_control_fields(vcpu : *mut Vcpu) {
    vmwrite(vmcs::control::CR0)
}

pub unsafe fn setup_vmcs(vcpu : *mut Vcpu) {
    vmclear((*vcpu).vmcs_physical);
    vmptrld((*vcpu).vmcs_physical);

    setup_guest_registers_state((*vcpu).guest_descriptor, (*vcpu).guest_registers);
    setup_host_registers_state((*vcpu).host_descriptor, pml4_pa);

}