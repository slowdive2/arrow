// SPDX-License-Identifier: MIT
//
// Copyright (c) 2022 memN0ps
//
// This file is derived from the illusion-rs project:
// https://github.com/memN0ps/illusion-rs
//
// Original source:
// https://github.com/memN0ps/illusion-rs/blob/main/hypervisor/src/intel/support.rs



#![allow(dead_code)]
use core::arch::asm;

pub fn vmxon(vmxon_region: u64) {
    unsafe { x86::bits64::vmx::vmxon(vmxon_region).unwrap() };
}

pub fn vmxoff() -> bool {
    match unsafe { x86::bits64::vmx::vmxoff() } {
        Ok(()) => true,
        Err(_) => {
            log::error!("VMXOFF failed");
            false
        }
    }
}

pub fn vmclear(vmcs_region: u64) {
    unsafe { x86::bits64::vmx::vmclear(vmcs_region).unwrap() };
}

pub fn vmptrld(vmcs_region: u64) {
    unsafe { x86::bits64::vmx::vmptrld(vmcs_region).unwrap() }
}

pub fn vmptrst() -> u64 {
    unsafe { x86::bits64::vmx::vmptrst().unwrap() }
}

pub fn vmread(field: u32) -> u64 {
    unsafe { x86::bits64::vmx::vmread(field) }.unwrap_or(0)
}

pub fn vmwrite<T: Into<u64>>(field: u32, val: T)
where
    u64: From<T>,
{
    unsafe { x86::bits64::vmx::vmwrite(field, u64::from(val)) }.unwrap();
}

/// wr to Extended Control Register XCR0 ; only supported if CR4_ENABLE_OS_XSAVE is set
pub fn xsetbv(val: u64) {
    unsafe {
        x86::controlregs::xcr0_write(x86::controlregs::Xcr0::from_bits_truncate(val));
    }
}

/// wr back all modified cache contents to memory and invalidate the caches
#[inline(always)]
pub fn wbinvd() {
    unsafe {
        asm!("wbinvd", options(nostack, nomem));
    }
}

/// returns the timestamp counter value
pub fn rdtsc() -> u64 {
    unsafe { core::arch::x86_64::_rdtsc() }
}

pub fn rdmsr(msr: u32) -> u64 {
    unsafe { x86::msr::rdmsr(msr) }
}

pub fn wrmsr(msr: u32, value: u64) {
    unsafe { x86::msr::wrmsr(msr, value) };
}

pub fn cr0() -> x86::controlregs::Cr0 {
    unsafe { x86::controlregs::cr0() }
}

pub fn cr0_write(val: u64) {
    unsafe { x86::controlregs::cr0_write(x86::controlregs::Cr0::from_bits_truncate(val as usize)) };
}

pub fn cr3() -> u64 {
    unsafe { x86::controlregs::cr3() }
}

pub fn cr4() -> u64 {
    unsafe { x86::controlregs::cr4() }.bits() as u64
}

pub fn cr4_write(val: u64) {
    unsafe { x86::controlregs::cr4_write(x86::controlregs::Cr4::from_bits_truncate(val as usize)) };
}

pub fn read_effective_guest_cr0() -> u64 {
    let mask = vmread(x86::vmx::vmcs::control::CR0_GUEST_HOST_MASK);
    vmread(x86::vmx::vmcs::control::CR0_READ_SHADOW) & mask | vmread(x86::vmx::vmcs::guest::CR0) & !mask
}

pub fn read_effective_guest_cr4() -> u64 {
    let mask = vmread(x86::vmx::vmcs::control::CR4_GUEST_HOST_MASK);
    vmread(x86::vmx::vmcs::control::CR4_READ_SHADOW) & mask | vmread(x86::vmx::vmcs::guest::CR4) & !mask
}

pub fn cr2_write(val: u64) {
    unsafe { x86::controlregs::cr2_write(val) };
}

pub fn dr0_write(val: u64) {
    unsafe { x86::debugregs::dr0_write(val as _) };
}

pub fn dr1_write(val: u64) {
    unsafe { x86::debugregs::dr1_write(val as _) };
}

pub fn dr2_write(val: u64) {
    unsafe { x86::debugregs::dr2_write(val as _) };
}

pub fn dr3_write(val: u64) {
    unsafe { x86::debugregs::dr3_write(val as _) };
}

pub fn dr6_write(val: u64) {
    let dr6 = x86::debugregs::Dr6::from_bits_truncate(val as _);
    unsafe { x86::debugregs::dr6_write(dr6) };
}

pub fn dr0_read() -> u64 {
    unsafe { x86::debugregs::dr0() as u64 }
}

pub fn dr1_read() -> u64 {
    unsafe { x86::debugregs::dr1() as u64 }
}

pub fn dr2_read() -> u64 {
    unsafe { x86::debugregs::dr2() as u64 }
}

pub fn dr3_read() -> u64 {
    unsafe { x86::debugregs::dr3() as u64 }
}

pub fn dr6_read() -> u64 {
    unsafe { x86::debugregs::dr6().bits() as u64 }
}

pub fn dr7_read() -> u64 {
    unsafe { x86::debugregs::dr7().0 as u64 }
}

/// disables maskable interrupts.
pub fn cli() {
    unsafe { x86::irq::disable() };
}

pub fn hlt() {
    unsafe { x86::halt() };
}

/// reads 8-bits from an IO port
pub fn inb(port: u16) -> u8 {
    unsafe { x86::io::inb(port) }
}

/// writes 8-bits to an IO port
pub fn outb(port: u16, val: u8) {
    unsafe { x86::io::outb(port, val) };
}

/// reads the IDTR
pub fn sidt() -> x86::dtables::DescriptorTablePointer<u64> {
    let mut idtr = x86::dtables::DescriptorTablePointer::<u64>::default();
    unsafe { x86::dtables::sidt(&mut idtr) };
    idtr
}

/// reads the GDTR
pub fn sgdt() -> x86::dtables::DescriptorTablePointer<u64> {
    let mut gdtr = x86::dtables::DescriptorTablePointer::<u64>::default();
    unsafe { x86::dtables::sgdt(&mut gdtr) };
    gdtr
}