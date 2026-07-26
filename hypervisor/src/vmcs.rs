//! VMCS guest/host register-state setup.
//!
//! The `GuestRegisters` layout and capture-then-launch pattern are derived from
//! illusion-rs (Copyright © memN0ps), used under the MIT License.
//! Original: https://github.com/memN0ps/illusion-rs
//!

use core::{
    arch::{asm, global_asm},
    mem,
};

use bitfield_struct::bitfield;
use x86::msr::{rdmsr, IA32_FS_BASE, IA32_GS_BASE};
use x86::{
    bits64::rflags::RFlags,
    segmentation::{self, SegmentSelector},
};

use crate::{
    descriptors::Descriptors,
    vcpu::Vcpu,
    vmcs,
    vmx::{vmclear, vmptrld, vmwrite},
};

#[repr(C, align(16))]
#[derive(Clone, Copy, Default)]
pub struct M128A {
    pub low: u64,
    pub high: i64,
}

unsafe extern "win64" {
    pub fn capture_registers(registers: &mut GuestRegisters) -> bool;
}

/// Intel VMCS segment access-rights field.
#[bitfield(u32)]
#[derive(PartialEq, Eq)]
pub struct SegmentAccessRights {
    #[bits(4)]
    pub segment_type: u8,
    pub s: bool,
    #[bits(2)]
    pub dpl: u8,
    pub present: bool,
    #[bits(4)]
    __: u8,
    pub avl: bool,
    pub long_mode: bool,
    pub default_big: bool,
    pub granularity: bool,
    pub unusable: bool,
    #[bits(15)]
    __: u32,
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

global_asm!(
    r#"
.intel_syntax noprefix

// Captures the current GPRs, RFLAGS, caller RSP/RIP, and XMM registers.
//
// Windows x64 ABI:
//     RCX = &mut GuestRegisters
//
// Consequently, the saved RCX value is the argument pointer rather than the
// caller's pre-call RCX. The launch/resume design must account for that.
.global capture_registers
capture_registers:
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

    pushfq
    pop     rax
    mov     [rcx + {registers_rflags}], rax

    // Save the caller's stack pointer, not this function's entry RSP.
    lea     rax, [rsp + 8]
    mov     [rcx + {registers_rsp}], rax

    // The return address is the caller continuation RIP.
    mov     rax, [rsp]
    mov     [rcx + {registers_rip}], rax

    movaps  [rcx + {registers_xmm0}],  xmm0
    movaps  [rcx + {registers_xmm1}],  xmm1
    movaps  [rcx + {registers_xmm2}],  xmm2
    movaps  [rcx + {registers_xmm3}],  xmm3
    movaps  [rcx + {registers_xmm4}],  xmm4
    movaps  [rcx + {registers_xmm5}],  xmm5
    movaps  [rcx + {registers_xmm6}],  xmm6
    movaps  [rcx + {registers_xmm7}],  xmm7
    movaps  [rcx + {registers_xmm8}],  xmm8
    movaps  [rcx + {registers_xmm9}],  xmm9
    movaps  [rcx + {registers_xmm10}], xmm10
    movaps  [rcx + {registers_xmm11}], xmm11
    movaps  [rcx + {registers_xmm12}], xmm12
    movaps  [rcx + {registers_xmm13}], xmm13
    movaps  [rcx + {registers_xmm14}], xmm14
    movaps  [rcx + {registers_xmm15}], xmm15

    // false: execution has not yet resumed through the virtualized path.
    xor     eax, eax
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
    registers_r8 = const mem::offset_of!(GuestRegisters, r8),
    registers_r9 = const mem::offset_of!(GuestRegisters, r9),
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

/// Executes LAR and returns its architecturally formatted result.
pub fn lar(selector: SegmentSelector) -> u32 {
    let access_rights: u64;
    let flags: u64;

    unsafe {
        asm!(
            "lar {access_rights}, {selector}",
            "pushfq",
            "pop {flags}",
            access_rights = lateout(reg) access_rights,
            selector = in(reg) u64::from(selector.bits()),
            flags = lateout(reg) flags,
        );
    }

    assert!(
        RFlags::from_raw(flags).contains(RFlags::FLAGS_ZF),
        "LAR failed for selector {:#x}",
        selector.bits(),
    );

    access_rights as u32
}

/// Executes LSL and returns the segment limit.
pub fn lsl(selector: SegmentSelector) -> u32 {
    let limit: u64;
    let flags: u64;

    unsafe {
        asm!(
            "lsl {limit}, {selector}",
            "pushfq",
            "pop {flags}",
            limit = lateout(reg) limit,
            selector = in(reg) u64::from(selector.bits()),
            flags = lateout(reg) flags,
        );
    }

    assert!(
        RFlags::from_raw(flags).contains(RFlags::FLAGS_ZF),
        "LSL failed for selector {:#x}",
        selector.bits(),
    );

    limit as u32
}

/// Converts the LAR result to the VMCS access-rights encoding.
#[inline]
fn vmcs_access_rights(selector: SegmentSelector) -> u64 {
    u64::from((lar(selector) >> 8) & !0xF00)
}

#[inline]
fn unusable_access_rights() -> u64 {
    u64::from(SegmentAccessRights::new().with_unusable(true).into_bits())
}

#[inline]
fn host_selector(selector: SegmentSelector) -> u64 {
    // VM-entry validation requires RPL and TI to be zero in host selectors.
    u64::from(selector.bits() & !0x7)
}
pub unsafe fn setup_guest_registers_state(
    guest_descriptor: &Descriptors,
    guest_registers: &GuestRegisters,
) {
    let cs = segmentation::cs();
    let ss = segmentation::ss();
    let ds = segmentation::ds();
    let es = segmentation::es();
    let fs = segmentation::fs();
    let gs = segmentation::gs();

    unsafe {
        vmwrite(vmcs::guest::CR0, x86::controlregs::cr0().bits() as u64);
        vmwrite(vmcs::guest::CR3, x86::controlregs::cr3());
        vmwrite(vmcs::guest::CR4, x86::controlregs::cr4().bits() as u64);
        vmwrite(vmcs::guest::DR7, x86::debugregs::dr7().0 as u64);

        vmwrite(vmcs::guest::RSP, guest_registers.rsp);
        vmwrite(vmcs::guest::RIP, guest_registers.rip);
        vmwrite(vmcs::guest::RFLAGS, guest_registers.rflags);

        vmwrite(vmcs::guest::CS_SELECTOR, u64::from(cs.bits()));
        vmwrite(vmcs::guest::SS_SELECTOR, u64::from(ss.bits()));
        vmwrite(vmcs::guest::DS_SELECTOR, u64::from(ds.bits()));
        vmwrite(vmcs::guest::ES_SELECTOR, u64::from(es.bits()));
        vmwrite(vmcs::guest::FS_SELECTOR, u64::from(fs.bits()));
        vmwrite(vmcs::guest::GS_SELECTOR, u64::from(gs.bits()));
        vmwrite(vmcs::guest::LDTR_SELECTOR, 0);
        vmwrite(
            vmcs::guest::TR_SELECTOR,
            u64::from(guest_descriptor.tr.bits()),
        );

        // In 64-bit mode, CS/SS/DS/ES bases are architecturally zero.
        vmwrite(vmcs::guest::CS_BASE, 0);
        vmwrite(vmcs::guest::SS_BASE, 0);
        vmwrite(vmcs::guest::DS_BASE, 0);
        vmwrite(vmcs::guest::ES_BASE, 0);

        // TODO: read the real FS/GS base MSRs for the current Windows thread
        // Selector-derived bases are not sufficient in long mode.
        vmwrite(vmcs::guest::FS_BASE, rdmsr(IA32_FS_BASE));
        vmwrite(vmcs::guest::GS_BASE, rdmsr(IA32_GS_BASE));

        vmwrite(vmcs::guest::LDTR_BASE, 0);
        vmwrite(vmcs::guest::TR_BASE, guest_descriptor.tss_base);

        vmwrite(vmcs::guest::CS_LIMIT, u64::from(lsl(cs)));
        vmwrite(vmcs::guest::SS_LIMIT, u64::from(lsl(ss)));
        vmwrite(vmcs::guest::DS_LIMIT, u64::from(lsl(ds)));
        vmwrite(vmcs::guest::ES_LIMIT, u64::from(lsl(es)));
        vmwrite(vmcs::guest::FS_LIMIT, u64::from(lsl(fs)));
        vmwrite(vmcs::guest::GS_LIMIT, u64::from(lsl(gs)));
        vmwrite(vmcs::guest::LDTR_LIMIT, 0);
        vmwrite(vmcs::guest::TR_LIMIT, u64::from(guest_descriptor.tss_limit));

        vmwrite(vmcs::guest::CS_ACCESS_RIGHTS, vmcs_access_rights(cs));
        vmwrite(vmcs::guest::SS_ACCESS_RIGHTS, vmcs_access_rights(ss));
        vmwrite(vmcs::guest::DS_ACCESS_RIGHTS, vmcs_access_rights(ds));
        vmwrite(vmcs::guest::ES_ACCESS_RIGHTS, vmcs_access_rights(es));
        vmwrite(vmcs::guest::FS_ACCESS_RIGHTS, vmcs_access_rights(fs));
        vmwrite(vmcs::guest::GS_ACCESS_RIGHTS, vmcs_access_rights(gs));
        vmwrite(vmcs::guest::LDTR_ACCESS_RIGHTS, unusable_access_rights());
        vmwrite(
            vmcs::guest::TR_ACCESS_RIGHTS,
            vmcs_access_rights(guest_descriptor.tr),
        );

        vmwrite(vmcs::guest::GDTR_BASE, guest_descriptor.gdtr.base as u64);
        vmwrite(vmcs::guest::IDTR_BASE, guest_descriptor.idtr.base as u64);
        vmwrite(
            vmcs::guest::GDTR_LIMIT,
            u64::from(guest_descriptor.gdtr.limit),
        );
        vmwrite(vmcs::guest::IDTR_LIMIT, u64::from(idtr.limit));

        // No shadow VMCS is linked.
        vmwrite(vmcs::guest::LINK_PTR_FULL, u64::MAX);
    }
}

pub unsafe fn setup_host_registers_state(
    host_descriptor: &Descriptors,
    host_cr3: u64,
    host_rsp: u64,
    host_rip: u64,
) {
    let idtr = x86::dtables::sidt();

    unsafe {
        vmwrite(vmcs::host::CR0, x86::controlregs::cr0().bits() as u64);
        vmwrite(vmcs::host::CR3, host_cr3);
        vmwrite(vmcs::host::CR4, x86::controlregs::cr4().bits() as u64);

        vmwrite(vmcs::host::CS_SELECTOR, host_selector(segmentation::cs()));
        vmwrite(vmcs::host::SS_SELECTOR, host_selector(segmentation::ss()));
        vmwrite(vmcs::host::DS_SELECTOR, host_selector(segmentation::ds()));
        vmwrite(vmcs::host::ES_SELECTOR, host_selector(segmentation::es()));
        vmwrite(vmcs::host::FS_SELECTOR, host_selector(segmentation::fs()));
        vmwrite(vmcs::host::GS_SELECTOR, host_selector(segmentation::gs()));
        vmwrite(vmcs::host::TR_SELECTOR, host_selector(host_descriptor.tr));

<<<<<<< HEAD
        // TODO: write the real host FS/GS base
        vmwrite(vmcs::host::FS_BASE, 0);
        vmwrite(vmcs::host::GS_BASE, 0);
=======
        // TODO: write the real host FS/GS bases used by your Windows context.
        vmwrite(vmcs::host::FS_BASE, rdmsr(IA32_FS_BASE));
        vmwrite(vmcs::host::GS_BASE, rdmsr(IA32_GS_BASE));
>>>>>>> f3a1e08 (control helper fns)
        vmwrite(vmcs::host::TR_BASE, host_descriptor.tss_base);
        vmwrite(vmcs::host::GDTR_BASE, host_descriptor.gdtr.base as u64);
        vmwrite(vmcs::host::IDTR_BASE, host_descriptor.idtr.base as u64);

        vmwrite(vmcs::host::RSP, host_rsp);
        vmwrite(vmcs::host::RIP, host_rip);

        // TODO: also populate SYSENTER state and any MSR-dependent host fields
    }
}

pub unsafe fn setup_vmcs_control_fields(
    _vcpu: &mut Vcpu,
) -> Result<(), VmxError> {
    let pinbased = unsafe {
        adjust_pinbased_controls(PINBASED_CTL)
    };

    let primary = unsafe {
        adjust_primary_controls(PRIMARY_CTL)
    };

    let secondary = unsafe {
        adjust_secondary_controls(SECONDARY_CTL)
    };

    let entry = unsafe {
        adjust_entry_controls(ENTRY_CTL)
    };

    let exit = unsafe {
        adjust_exit_controls(EXIT_CTL)
    };

    unsafe {
        vmwrite(
            vmcs::control::PINBASED_EXEC_CONTROLS,
            u64::from(pinbased),
        )?;

        vmwrite(
            vmcs::control::PRIMARY_PROCBASED_EXEC_CONTROLS,
            u64::from(primary),
        )?;

        vmwrite(
            vmcs::control::SECONDARY_PROCBASED_EXEC_CONTROLS,
            u64::from(secondary),
        )?;

        vmwrite(
            vmcs::control::VMENTRY_CONTROLS,
            u64::from(entry),
        )?;

        vmwrite(
            vmcs::control::VMEXIT_CONTROLS,
            u64::from(exit),
        )?;
        vmwrite(vmcs::control::MSR_BITMAPS_ADDR_FULL, msr_bitmap)?;
    }
    

    Ok(())
}

pub unsafe fn setup_vmcs(vcpu: *mut Vcpu) {
    assert!(!vcpu.is_null(), "setup_vmcs received a null Vcpu pointer");

    let vcpu = unsafe { &mut *vcpu };

    unsafe {
        vmclear(vcpu.vmcs_physical);
        vmptrld(vcpu.vmcs_physical);

        setup_guest_registers_state(&vcpu.guest_descriptor, &vcpu.guest_registers);

        setup_host_registers_state(
            &vcpu.host_descriptor,
            x86::controlregs::cr3(),
            vcpu.host_stack_top,
            vcpu.vmexit_entry,
        );

        setup_vmcs_control_fields(vcpu);
    }
}
