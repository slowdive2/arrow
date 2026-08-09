// SPDX-License-Identifier: MIT
//
// Copyright (c) 2022 memN0ps
//
// this file is derived from the illusion-rs project:
// https://github.com/memN0ps/illusion-rs
//
// original source:
// https://github.com/memN0ps/illusion-rs/blob/main/hypervisor/src/intel/support.rs

#![allow(dead_code)]
use core::{
    arch::asm,
    sync::atomic::{AtomicBool, Ordering},
};
use wdk_sys::ntddk::KeIpiGenericCall;

pub fn vmxon(vmxon_region: u64) -> x86::vmx::Result<()> {
    unsafe { x86::bits64::vmx::vmxon(vmxon_region) }
}

pub fn vmxoff() -> x86::vmx::Result<()> {
    unsafe { x86::bits64::vmx::vmxoff() }
}

pub fn vmclear(vmcs_region: u64) -> x86::vmx::Result<()> {
    unsafe { x86::bits64::vmx::vmclear(vmcs_region) }
}

pub fn vmptrld(vmcs_region: u64) -> x86::vmx::Result<()> {
    unsafe { x86::bits64::vmx::vmptrld(vmcs_region) }
}

pub fn vmptrst() -> x86::vmx::Result<u64> {
    unsafe { x86::bits64::vmx::vmptrst() }
}

pub fn vmread(field: u32) -> x86::vmx::Result<u64> {
    unsafe { x86::bits64::vmx::vmread(field) }
}

pub fn vmwrite<T: Into<u64>>(field: u32, val: T) -> x86::vmx::Result<()>
where
    u64: From<T>,
{
    unsafe { x86::bits64::vmx::vmwrite(field, u64::from(val)) }
}

unsafe fn vmcall_watch_exec_here(gpa: u64) -> bool {
    use crate::exit::vmcall::{ARROW_HYPERCALL_MAGIC, HYPERCALL_ARM_EXECUTE_MONITOR};

    let status: u64;
    unsafe {
        asm!(
            "vmcall",
            in("rcx") HYPERCALL_ARM_EXECUTE_MONITOR,
            in("rdx") gpa,
            in("r10") ARROW_HYPERCALL_MAGIC,
            lateout("rax") status,
            options(nostack),
        );
    }
    status == 0
}

struct WatchExecBroadcast<'a> {
    gpa: u64,
    guard: &'a crate::vmm::VmmClientGuard,
    ok: AtomicBool,
    visited: AtomicBool,
}

unsafe extern "system" fn watch_exec_cpu(context: usize) -> usize {
    let context = unsafe { &*(context as *const WatchExecBroadcast<'_>) };
    if !unsafe { crate::vmm::current_cpu_virtualized(context.guard) } {
        // Hot-added or already-devirtualized CPUs have no EPT cache to flush,
        // and executing VMCALL on them would raise #UD.
        return 1;
    }

    context.visited.store(true, Ordering::Release);
    if !unsafe { vmcall_watch_exec_here(context.gpa) } {
        context.ok.store(false, Ordering::Release);
    }
    1
}

// each cpu runs vmcall so each one flushes its cached ept entry
pub unsafe fn vmcall_watch_exec(gpa: u64) -> bool {
    let Some(guard) = crate::vmm::VmmClientGuard::try_acquire() else {
        return false;
    };
    let context = WatchExecBroadcast {
        gpa,
        guard: &guard,
        ok: AtomicBool::new(true),
        visited: AtomicBool::new(false),
    };
    unsafe { KeIpiGenericCall(Some(watch_exec_cpu), core::ptr::from_ref(&context) as usize) };
    context.visited.load(Ordering::Acquire) && context.ok.load(Ordering::Acquire)
}

// write xcr0 when osxsave is on
pub fn xsetbv(val: u64) {
    unsafe {
        x86::controlregs::xcr0_write(x86::controlregs::Xcr0::from_bits_truncate(val));
    }
}

// flush all caches
#[inline(always)]
pub fn wbinvd() {
    unsafe {
        asm!("wbinvd", options(nostack, nomem));
    }
}

// read tsc
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

pub fn cr3_write(val: u64) {
    unsafe { x86::controlregs::cr3_write(val) };
}

pub fn cr4() -> u64 {
    unsafe { x86::controlregs::cr4() }.bits() as u64
}

pub fn cr4_write(val: u64) {
    unsafe { x86::controlregs::cr4_write(x86::controlregs::Cr4::from_bits_truncate(val as usize)) };
}

pub fn guest_cr0() -> x86::vmx::Result<u64> {
    let mask = vmread(x86::vmx::vmcs::control::CR0_GUEST_HOST_MASK)?;
    Ok(vmread(x86::vmx::vmcs::control::CR0_READ_SHADOW)? & mask
        | vmread(x86::vmx::vmcs::guest::CR0)? & !mask)
}

pub unsafe fn vmcall_shutdown() -> bool {
    use crate::exit::vmcall::{ARROW_HYPERCALL_MAGIC, HYPERCALL_SHUTDOWN};

    let status: u64;
    unsafe {
        asm!(
            "vmcall",
            in("rcx") HYPERCALL_SHUTDOWN,
            in("r10") ARROW_HYPERCALL_MAGIC,
            lateout("rax") status,
            options(nostack),
        );
    }
    status == 0
}

pub fn guest_cr4() -> x86::vmx::Result<u64> {
    let mask = vmread(x86::vmx::vmcs::control::CR4_GUEST_HOST_MASK)?;
    Ok(vmread(x86::vmx::vmcs::control::CR4_READ_SHADOW)? & mask
        | vmread(x86::vmx::vmcs::guest::CR4)? & !mask)
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

pub fn dr7_write(val: u64) {
    unsafe { x86::debugregs::dr7_write(x86::debugregs::Dr7(val as usize)) };
}

// disable maskable interrupts
pub fn cli() {
    unsafe { x86::irq::disable() };
}

pub fn hlt() {
    unsafe { x86::halt() };
}

// read an io byte
pub fn inb(port: u16) -> u8 {
    unsafe { x86::io::inb(port) }
}

// write an io byte
pub fn outb(port: u16, val: u8) {
    unsafe { x86::io::outb(port, val) };
}

// reads the idtr
pub fn sidt() -> x86::dtables::DescriptorTablePointer<u64> {
    let mut idtr = x86::dtables::DescriptorTablePointer::<u64>::default();
    unsafe { x86::dtables::sidt(&mut idtr) };
    idtr
}

// reads the gdtr
pub fn sgdt() -> x86::dtables::DescriptorTablePointer<u64> {
    let mut gdtr = x86::dtables::DescriptorTablePointer::<u64>::default();
    unsafe { x86::dtables::sgdt(&mut gdtr) };
    gdtr
}
