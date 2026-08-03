use core::ffi::c_char;
use core::fmt::{self, Write};
use log::{Log, Metadata, Record};
use wdk_sys::ntddk::DbgPrintEx;

const LOG_BUFFER_SIZE: usize = 512;
const DPFLTR_IHVDRIVER_ID: u32 = 77;
const DPFLTR_INFO_LEVEL: u32 = 3;

struct DbgPrintLogger;

impl Log for DbgPrintLogger {
    fn enabled(&self, m: &Metadata) -> bool {
        m.level() <= log::Level::Trace
    }

    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }

        let mut buf = LogBuffer::new();
        let _ = write!(
            buf,
            "vcpu-{} {}: {}\n",
            apic_id(),
            record.level(),
            record.args(),
        );

        unsafe {
            DbgPrintEx(
                DPFLTR_IHVDRIVER_ID,
                DPFLTR_INFO_LEVEL,
                c"%s".as_ptr(),
                buf.as_ptr(),
            );
        }
    }

    fn flush(&self) {}
}

static LOGGER: DbgPrintLogger = DbgPrintLogger;

pub fn init(level: log::LevelFilter) {
    log::set_logger(&LOGGER).unwrap();
    log::set_max_level(level);
}

fn apic_id() -> u32 {
    unsafe { core::arch::x86_64::__cpuid(1).ebx >> 24 }
}
