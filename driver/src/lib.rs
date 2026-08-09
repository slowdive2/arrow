#![no_std]

#[cfg(not(test))]
extern crate wdk_panic;

#[cfg(not(test))]
use wdk_alloc::WdkAllocator;
use wdk_sys::{DRIVER_OBJECT, NTSTATUS, PCUNICODE_STRING, STATUS_SUCCESS, STATUS_UNSUCCESSFUL};

#[cfg(not(test))]
#[global_allocator]
static GLOBAL_ALLOCATOR: WdkAllocator = WdkAllocator;

#[export_name = "DriverEntry"]
pub unsafe extern "system" fn driver_entry(
    driver: &mut DRIVER_OBJECT,
    _registry_path: PCUNICODE_STRING,
) -> NTSTATUS {
    hypervisor::logging::init(log::LevelFilter::Info);

    if unsafe { hypervisor::vmm::vmm_init() } {
        driver.DriverUnload = Some(driver_exit);
        log::info!("vmm_init succeeded");
        STATUS_SUCCESS
    } else {
        log::error!("vmm_init failed");
        STATUS_UNSUCCESSFUL
    }
}

unsafe extern "C" fn driver_exit(_driver: *mut DRIVER_OBJECT) {
    while !unsafe { hypervisor::vmm::vmm_shutdown() } {
        log::error!("driver unload incomplete; retrying");
        for _ in 0..1024 {
            core::hint::spin_loop();
        }
    }
    log::info!("driver unloading");
}
