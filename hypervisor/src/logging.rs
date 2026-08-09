use core::ffi::c_char;
use core::fmt::{self, Write};
use log::{Log, Metadata, Record};
use wdk_sys::ntddk::DbgPrintEx;

const LOG_BUFFER_SIZE: usize = 512;
const DPFLTR_IHVDRIVER_ID: u32 = 77;
const DPFLTR_INFO_LEVEL: u32 = 3;

struct LogBuffer {
    bytes: [u8; LOG_BUFFER_SIZE],
    len: usize,
}

impl LogBuffer {
    const fn new() -> Self {
        Self {
            bytes: [0; LOG_BUFFER_SIZE],
            len: 0,
        }
    }

    fn as_ptr(&self) -> *const c_char {
        self.bytes.as_ptr().cast()
    }
}

impl Write for LogBuffer {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        let available = LOG_BUFFER_SIZE - self.len - 1;
        let count = available.min(text.len());
        self.bytes[self.len..self.len + count].copy_from_slice(&text.as_bytes()[..count]);
        self.len += count;
        self.bytes[self.len] = 0;

        if count == text.len() {
            Ok(())
        } else {
            Err(fmt::Error)
        }
    }
}

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
        let _ = writeln!(
            buf,
            "vcpu-{} {}: {}",
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
    core::arch::x86_64::__cpuid(1).ebx >> 24
}
