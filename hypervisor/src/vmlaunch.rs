// based on https://github.com/tandasat/Hypervisor-101-in-Rust/blob/main/hypervisor/src/hardware_vt/vmx_run_vm.S
// and https://github.com/daaximus
// and https://github.com/drew-gpf

// SPDX-License-Identifier: MIT
// Copyright (c) 2022 memN0ps
// this file is derived from the illusion-rs project:
// https://github.com/memN0ps/illusion-rs
// https://github.com/memN0ps/illusion-rs/blob/main/hypervisor/src/intel/vmlaunch.rs

use {
    crate::vmcs::GuestRegs,
    core::{arch::global_asm, mem},
};

extern "efiapi" {
    pub fn launch_vm(regs: &mut GuestRegs, launched: u64) -> u64;
}

global_asm!(
    r#"


.macro PUSHAQ
    push    rax
    push    rcx
    push    rdx
    push    rbx
    push    rbp
    push    rsi
    push    rdi
    push    r8
    push    r9
    push    r10
    push    r11
    push    r12
    push    r13
    push    r14
    push    r15
.endm


.macro POPAQ
    pop     r15
    pop     r14
    pop     r13
    pop     r12
    pop     r11
    pop     r10
    pop     r9
    pop     r8
    pop     rdi
    pop     rsi
    pop     rbp
    pop     rbx
    pop     rdx
    pop     rcx
    pop     rax
.endm


.macro SAVE_XMM
    sub rsp, 0x100

    movaps xmmword ptr [rsp], xmm0
    movaps xmmword ptr [rsp + 0x10], xmm1
    movaps xmmword ptr [rsp + 0x20], xmm2
    movaps xmmword ptr [rsp + 0x30], xmm3
    movaps xmmword ptr [rsp + 0x40], xmm4
    movaps xmmword ptr [rsp + 0x50], xmm5
    movaps xmmword ptr [rsp + 0x60], xmm6
    movaps xmmword ptr [rsp + 0x70], xmm7
    movaps xmmword ptr [rsp + 0x80], xmm8
    movaps xmmword ptr [rsp + 0x90], xmm9
    movaps xmmword ptr [rsp + 0xA0], xmm10
    movaps xmmword ptr [rsp + 0xB0], xmm11
    movaps xmmword ptr [rsp + 0xC0], xmm12
    movaps xmmword ptr [rsp + 0xD0], xmm13
    movaps xmmword ptr [rsp + 0xE0], xmm14
    movaps xmmword ptr [rsp + 0xF0], xmm15
.endm


.macro RESTORE_XMM
movaps xmm0, xmmword ptr [rsp]
    movaps xmm1, xmmword ptr [rsp + 0x10]
    movaps xmm2, xmmword ptr [rsp + 0x20]
    movaps xmm3, xmmword ptr [rsp + 0x30]
    movaps xmm4, xmmword ptr [rsp + 0x40]
    movaps xmm5, xmmword ptr [rsp + 0x50]
    movaps xmm6, xmmword ptr [rsp + 0x60]
    movaps xmm7, xmmword ptr [rsp + 0x70]
    movaps xmm8, xmmword ptr [rsp + 0x80]
    movaps xmm9, xmmword ptr [rsp + 0x90]
    movaps xmm10, xmmword ptr [rsp + 0xA0]
    movaps xmm11, xmmword ptr [rsp + 0xB0]
    movaps xmm12, xmmword ptr [rsp + 0xC0]
    movaps xmm13, xmmword ptr [rsp + 0xD0]
    movaps xmm14, xmmword ptr [rsp + 0xE0]
    movaps xmm15, xmmword ptr [rsp + 0xF0]

    add rsp, 0x100
.endm

.global launch_vm
launch_vm:
    PUSHAQ

    SAVE_XMM

    mov     r15, rcx    // regs ptr
    mov     r14, rdx    // launch flag
    push    rcx         // keep regs ptr across vm entry

    mov     rax, [r15 + {registers_rax}]
    mov     rbx, [r15 + {registers_rbx}]
    mov     rcx, [r15 + {registers_rcx}]
    mov     rdx, [r15 + {registers_rdx}]
    mov     rdi, [r15 + {registers_rdi}]
    mov     rsi, [r15 + {registers_rsi}]
    mov     rbp, [r15 + {registers_rbp}]
    mov     r8,  [r15 + {registers_r8}]
    mov     r9,  [r15 + {registers_r9}]
    mov     r10, [r15 + {registers_r10}]
    mov     r11, [r15 + {registers_r11}]
    mov     r12, [r15 + {registers_r12}]

    movaps  xmm0, [r15 + {registers_xmm0}]
    movaps  xmm1, [r15 + {registers_xmm1}]
    movaps  xmm2, [r15 + {registers_xmm2}]
    movaps  xmm3, [r15 + {registers_xmm3}]
    movaps  xmm4, [r15 + {registers_xmm4}]
    movaps  xmm5, [r15 + {registers_xmm5}]
    movaps  xmm6, [r15 + {registers_xmm6}]
    movaps  xmm7, [r15 + {registers_xmm7}]
    movaps  xmm8, [r15 + {registers_xmm8}]
    movaps  xmm9, [r15 + {registers_xmm9}]
    movaps  xmm10, [r15 + {registers_xmm10}]
    movaps  xmm11, [r15 + {registers_xmm11}]
    movaps  xmm12, [r15 + {registers_xmm12}]
    movaps  xmm13, [r15 + {registers_xmm13}]
    movaps  xmm14, [r15 + {registers_xmm14}]
    movaps  xmm15, [r15 + {registers_xmm15}]

    test    r14, r14
    je      .Launch

    mov     r13, [r15 + {registers_r13}]
    mov     r14, [r15 + {registers_r14}]
    mov     r15, [r15 + {registers_r15}]
    vmresume
    jmp     .VmEntryFailure

.Launch:
    mov     r14, 0x6C14 // vmcs_host_rsp
    vmwrite r14, rsp
    lea     r13, [rip + .VmExit]
    mov     r14, 0x6C16 // vmcs_host_rip
    vmwrite r14, r13
    mov     r13, [r15 + {registers_r13}]
    mov     r14, [r15 + {registers_r14}]
    mov     r15, [r15 + {registers_r15}]
    vmlaunch

.VmEntryFailure:
    jmp     .Exit

.VmExit:
    xchg    r15, [rsp]  // recover regs ptr and save guest r15
    mov     [r15 + {registers_rax}], rax
    mov     [r15 + {registers_rbx}], rbx
    mov     [r15 + {registers_rcx}], rcx
    mov     [r15 + {registers_rdx}], rdx
    mov     [r15 + {registers_rsi}], rsi
    mov     [r15 + {registers_rdi}], rdi
    mov     [r15 + {registers_rbp}], rbp
    mov     [r15 + {registers_r8}],  r8
    mov     [r15 + {registers_r9}],  r9
    mov     [r15 + {registers_r10}], r10
    mov     [r15 + {registers_r11}], r11
    mov     [r15 + {registers_r12}], r12
    mov     [r15 + {registers_r13}], r13
    mov     [r15 + {registers_r14}], r14

    movaps  [r15 + {registers_xmm0}], xmm0
    movaps  [r15 + {registers_xmm1}], xmm1
    movaps  [r15 + {registers_xmm2}], xmm2
    movaps  [r15 + {registers_xmm3}], xmm3
    movaps  [r15 + {registers_xmm4}], xmm4
    movaps  [r15 + {registers_xmm5}], xmm5
    movaps  [r15 + {registers_xmm6}], xmm6
    movaps  [r15 + {registers_xmm7}], xmm7
    movaps  [r15 + {registers_xmm8}], xmm8
    movaps  [r15 + {registers_xmm9}], xmm9
    movaps  [r15 + {registers_xmm10}], xmm10
    movaps  [r15 + {registers_xmm11}], xmm11
    movaps  [r15 + {registers_xmm12}], xmm12
    movaps  [r15 + {registers_xmm13}], xmm13
    movaps  [r15 + {registers_xmm14}], xmm14
    movaps  [r15 + {registers_xmm15}], xmm15

    mov     rax, [rsp]  // guest r15
    mov     [r15 + {registers_r15}], rax

.Exit:
    pop     rax

    RESTORE_XMM

    POPAQ

    pushfq
    pop     rax
    ret
"#,
    registers_rax = const mem::offset_of!(GuestRegs, rax),
    registers_rcx = const mem::offset_of!(GuestRegs, rcx),
    registers_rdx = const mem::offset_of!(GuestRegs, rdx),
    registers_rbx = const mem::offset_of!(GuestRegs, rbx),
    registers_rbp = const mem::offset_of!(GuestRegs, rbp),
    registers_rsi = const mem::offset_of!(GuestRegs, rsi),
    registers_rdi = const mem::offset_of!(GuestRegs, rdi),
    registers_r8  = const mem::offset_of!(GuestRegs, r8),
    registers_r9  = const mem::offset_of!(GuestRegs, r9),
    registers_r10 = const mem::offset_of!(GuestRegs, r10),
    registers_r11 = const mem::offset_of!(GuestRegs, r11),
    registers_r12 = const mem::offset_of!(GuestRegs, r12),
    registers_r13 = const mem::offset_of!(GuestRegs, r13),
    registers_r14 = const mem::offset_of!(GuestRegs, r14),
    registers_r15 = const mem::offset_of!(GuestRegs, r15),
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
