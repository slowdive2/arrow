// guest register layout follows illusion-rs
// https://github.com/memN0ps/illusion-rs

use core::{
    arch::{asm, global_asm},
    mem,
};

use bitfield_struct::bitfield;
use x86::msr::{rdmsr, IA32_FS_BASE, IA32_GS_BASE};
use x86::vmx::vmcs::{
    self,
    control::{EntryControls, ExitControls, PrimaryControls, SecondaryControls},
};
use x86::{
    bits64::rflags::RFlags,
    segmentation::{self, SegmentSelector},
};

use crate::{
    descriptor::Descriptors,
    support::{vmclear, vmptrld, vmwrite},
    vmm::Vcpu,
    vmx::{
        adjust_entry_controls, adjust_exit_controls, adjust_pinbased_controls,
        adjust_primary_controls, adjust_secondary_controls,
    },
};

#[repr(C, align(16))]
#[derive(Clone, Copy, Default)]
pub struct M128A {
    pub low: u64,
    pub high: i64,
}

unsafe extern "win64" {
    pub fn capture_registers(registers: &mut GuestRegs) -> bool;
}

// vmcs segment access rights
#[bitfield(u32)]
#[derive(PartialEq, Eq)]
pub struct SegAccess {
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
pub struct GuestRegs {
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
    pub orig_lstar: u64,
    pub hook_lstar: u64,
}

global_asm!(
    r#"

// rcx holds the output ptr, so the original rcx is already gone
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

    // caller rsp
    lea     rax, [rsp + 8]
    mov     [rcx + {registers_rsp}], rax

    // caller rip
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

    // first pass returns false
    xor     eax, eax
    ret
"#,
    registers_rax = const mem::offset_of!(GuestRegs, rax),
    registers_rcx = const mem::offset_of!(GuestRegs, rcx),
    registers_rdx = const mem::offset_of!(GuestRegs, rdx),
    registers_rbx = const mem::offset_of!(GuestRegs, rbx),
    registers_rsp = const mem::offset_of!(GuestRegs, rsp),
    registers_rbp = const mem::offset_of!(GuestRegs, rbp),
    registers_rsi = const mem::offset_of!(GuestRegs, rsi),
    registers_rdi = const mem::offset_of!(GuestRegs, rdi),
    registers_r8 = const mem::offset_of!(GuestRegs, r8),
    registers_r9 = const mem::offset_of!(GuestRegs, r9),
    registers_r10 = const mem::offset_of!(GuestRegs, r10),
    registers_r11 = const mem::offset_of!(GuestRegs, r11),
    registers_r12 = const mem::offset_of!(GuestRegs, r12),
    registers_r13 = const mem::offset_of!(GuestRegs, r13),
    registers_r14 = const mem::offset_of!(GuestRegs, r14),
    registers_r15 = const mem::offset_of!(GuestRegs, r15),
    registers_rip = const mem::offset_of!(GuestRegs, rip),
    registers_rflags = const mem::offset_of!(GuestRegs, rflags),
    registers_xmm0 = const mem::offset_of!(GuestRegs, xmm0),
    registers_xmm1 = const mem::offset_of!(GuestRegs, xmm1),
    registers_xmm2 = const mem::offset_of!(GuestRegs, xmm2),
    registers_xmm3 = const mem::offset_of!(GuestRegs, xmm3),
    registers_xmm4 = const mem::offset_of!(GuestRegs, xmm4),
    registers_xmm5 = const mem::offset_of!(GuestRegs, xmm5),
    registers_xmm6 = const mem::offset_of!(GuestRegs, xmm6),
    registers_xmm7 = const mem::offset_of!(GuestRegs, xmm7),
    registers_xmm8 = const mem::offset_of!(GuestRegs, xmm8),
    registers_xmm9 = const mem::offset_of!(GuestRegs, xmm9),
    registers_xmm10 = const mem::offset_of!(GuestRegs, xmm10),
    registers_xmm11 = const mem::offset_of!(GuestRegs, xmm11),
    registers_xmm12 = const mem::offset_of!(GuestRegs, xmm12),
    registers_xmm13 = const mem::offset_of!(GuestRegs, xmm13),
    registers_xmm14 = const mem::offset_of!(GuestRegs, xmm14),
    registers_xmm15 = const mem::offset_of!(GuestRegs, xmm15),
);

// read access rights with lar
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

// read a segment limit
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

// lar and vmcs store these bits differently
#[inline]
fn vmcs_access_rights(selector: SegmentSelector) -> u64 {
    u64::from((lar(selector) >> 8) & !0xF00)
}

#[inline]
fn unusable_access_rights() -> u64 {
    u64::from(SegAccess::new().with_unusable(true).into_bits())
}

#[inline]
fn host_selector(selector: SegmentSelector) -> u64 {
    u64::from(selector.bits() & !0x7)
}
pub unsafe fn setup_guest_state(guest_desc: &Descriptors, regs: &GuestRegs) {
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

        vmwrite(vmcs::guest::RSP, regs.rsp);
        vmwrite(vmcs::guest::RIP, regs.rip);
        vmwrite(vmcs::guest::RFLAGS, regs.rflags);

        vmwrite(vmcs::guest::CS_SELECTOR, u64::from(cs.bits()));
        vmwrite(vmcs::guest::SS_SELECTOR, u64::from(ss.bits()));
        vmwrite(vmcs::guest::DS_SELECTOR, u64::from(ds.bits()));
        vmwrite(vmcs::guest::ES_SELECTOR, u64::from(es.bits()));
        vmwrite(vmcs::guest::FS_SELECTOR, u64::from(fs.bits()));
        vmwrite(vmcs::guest::GS_SELECTOR, u64::from(gs.bits()));
        vmwrite(vmcs::guest::LDTR_SELECTOR, 0u64);
        vmwrite(vmcs::guest::TR_SELECTOR, u64::from(guest_desc.tr.bits()));

        vmwrite(vmcs::guest::CS_BASE, 0u64);
        vmwrite(vmcs::guest::SS_BASE, 0u64);
        vmwrite(vmcs::guest::DS_BASE, 0u64);
        vmwrite(vmcs::guest::ES_BASE, 0u64);

        // long mode
        vmwrite(vmcs::guest::FS_BASE, rdmsr(IA32_FS_BASE));
        vmwrite(vmcs::guest::GS_BASE, rdmsr(IA32_GS_BASE));

        vmwrite(vmcs::guest::LDTR_BASE, 0u64);
        vmwrite(vmcs::guest::TR_BASE, guest_desc.tss_base);

        vmwrite(vmcs::guest::CS_LIMIT, u64::from(lsl(cs)));
        vmwrite(vmcs::guest::SS_LIMIT, u64::from(lsl(ss)));
        vmwrite(vmcs::guest::DS_LIMIT, u64::from(lsl(ds)));
        vmwrite(vmcs::guest::ES_LIMIT, u64::from(lsl(es)));
        vmwrite(vmcs::guest::FS_LIMIT, u64::from(lsl(fs)));
        vmwrite(vmcs::guest::GS_LIMIT, u64::from(lsl(gs)));
        vmwrite(vmcs::guest::LDTR_LIMIT, 0u64);
        vmwrite(vmcs::guest::TR_LIMIT, u64::from(guest_desc.tss_limit));

        vmwrite(vmcs::guest::CS_ACCESS_RIGHTS, vmcs_access_rights(cs));
        vmwrite(vmcs::guest::SS_ACCESS_RIGHTS, vmcs_access_rights(ss));
        vmwrite(vmcs::guest::DS_ACCESS_RIGHTS, vmcs_access_rights(ds));
        vmwrite(vmcs::guest::ES_ACCESS_RIGHTS, vmcs_access_rights(es));
        vmwrite(vmcs::guest::FS_ACCESS_RIGHTS, vmcs_access_rights(fs));
        vmwrite(vmcs::guest::GS_ACCESS_RIGHTS, vmcs_access_rights(gs));
        vmwrite(vmcs::guest::LDTR_ACCESS_RIGHTS, unusable_access_rights());
        vmwrite(
            vmcs::guest::TR_ACCESS_RIGHTS,
            vmcs_access_rights(guest_desc.tr),
        );

        vmwrite(vmcs::guest::GDTR_BASE, guest_desc.gdtr.base as u64);
        vmwrite(vmcs::guest::IDTR_BASE, guest_desc.idtr.base as u64);
        vmwrite(vmcs::guest::GDTR_LIMIT, u64::from(guest_desc.gdtr.limit));
        vmwrite(vmcs::guest::IDTR_LIMIT, u64::from(guest_desc.idtr.limit));

        // no shadow vmcs is linked
        vmwrite(vmcs::guest::LINK_PTR_FULL, u64::MAX);
    }
}

pub unsafe fn setup_host_state(
    host_desc: &Descriptors,
    host_cr3: u64,
    host_rsp: u64,
    host_rip: u64,
) {
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
        vmwrite(vmcs::host::TR_SELECTOR, host_selector(host_desc.tr));

        vmwrite(vmcs::host::FS_BASE, rdmsr(IA32_FS_BASE));
        vmwrite(vmcs::host::GS_BASE, rdmsr(IA32_GS_BASE));
        vmwrite(vmcs::host::TR_BASE, host_desc.tss_base);
        vmwrite(vmcs::host::GDTR_BASE, host_desc.gdtr.base as u64);
        vmwrite(vmcs::host::IDTR_BASE, host_desc.idtr.base as u64);

        vmwrite(vmcs::host::RSP, host_rsp);
        vmwrite(vmcs::host::RIP, host_rip);
    }
}

const PINBASED_CTL: u32 = 0;
const PRIMARY_CTL: u32 = PrimaryControls::SECONDARY_CONTROLS.bits();
const SECONDARY_CTL: u32 = SecondaryControls::ENABLE_EPT.bits();
// both sides run in 64-bit mode
const ENTRY_CTL: u32 = EntryControls::IA32E_MODE_GUEST.bits();
const EXIT_CTL: u32 = ExitControls::HOST_ADDRESS_SPACE_SIZE.bits();

pub unsafe fn setup_controls(vcpu: &mut Vcpu) -> bool {
    let pinbased = unsafe { adjust_pinbased_controls(PINBASED_CTL) };
    let primary = unsafe { adjust_primary_controls(PRIMARY_CTL) };

    // secondary controls need primary bit 31
    let secondary = if primary & (1 << 31) != 0 {
        unsafe { adjust_secondary_controls(SECONDARY_CTL) }
    } else {
        0
    };

    let entry = unsafe { adjust_entry_controls(ENTRY_CTL) };
    let exit = unsafe { adjust_exit_controls(EXIT_CTL) };

    if primary & PRIMARY_CTL != PRIMARY_CTL
        || secondary & SECONDARY_CTL != SECONDARY_CTL
        || entry & ENTRY_CTL != ENTRY_CTL
        || exit & EXIT_CTL != EXIT_CTL
    {
        log::error!("required EPT or VM-entry/VM-exit controls unavailable on this CPU");
        return false;
    }

    if vcpu.ept.is_null() {
        log::error!("cannot enable EPT without shared EPT state");
        return false;
    }

    unsafe {
        vmwrite(vmcs::control::PINBASED_EXEC_CONTROLS, u64::from(pinbased));
        vmwrite(
            vmcs::control::PRIMARY_PROCBASED_EXEC_CONTROLS,
            u64::from(primary),
        );
        vmwrite(
            vmcs::control::SECONDARY_PROCBASED_EXEC_CONTROLS,
            u64::from(secondary),
        );
        vmwrite(vmcs::control::VMENTRY_CONTROLS, u64::from(entry));
        vmwrite(vmcs::control::VMEXIT_CONTROLS, u64::from(exit));
        vmwrite(vmcs::control::MSR_BITMAPS_ADDR_FULL, vcpu.msr_bitmap_pa);
        // eptp cache type is for the tables, not mapped ram
        vmwrite(vmcs::control::EPTP_FULL, (*vcpu.ept).eptp());

        vmwrite(vmcs::control::CR0_GUEST_HOST_MASK, 0u64);
        vmwrite(vmcs::control::CR4_GUEST_HOST_MASK, 0u64);
        vmwrite(
            vmcs::control::CR0_READ_SHADOW,
            x86::controlregs::cr0().bits() as u64,
        );
        vmwrite(
            vmcs::control::CR4_READ_SHADOW,
            x86::controlregs::cr4().bits() as u64,
        );

        vmwrite(vmcs::control::EXCEPTION_BITMAP, 0u64);
        vmwrite(vmcs::control::PAGE_FAULT_ERR_CODE_MASK, 0u64);
        vmwrite(vmcs::control::PAGE_FAULT_ERR_CODE_MATCH, 0u64);
        vmwrite(vmcs::control::CR3_TARGET_COUNT, 0u64);
        vmwrite(vmcs::control::VMENTRY_INTERRUPTION_INFO_FIELD, 0u64);
        vmwrite(vmcs::control::VMENTRY_MSR_LOAD_COUNT, 0u64);
        vmwrite(vmcs::control::VMEXIT_MSR_STORE_COUNT, 0u64);
        vmwrite(vmcs::control::VMEXIT_MSR_LOAD_COUNT, 0u64);
    }

    true
}

pub unsafe fn setup_vmcs(vcpu: *mut Vcpu) -> bool {
    assert!(!vcpu.is_null(), "setup_vmcs received a null Vcpu pointer");

    let vcpu = unsafe { &mut *vcpu };

    unsafe {
        vmclear(vcpu.vmcs_pa);
        vmptrld(vcpu.vmcs_pa);

        setup_guest_state(&vcpu.guest_desc, &vcpu.regs);

        // launch_vm fills host rsp/rip right before vmlaunch
        setup_host_state(&vcpu.host_desc, x86::controlregs::cr3(), 0, 0);

        setup_controls(vcpu)
    }
}
