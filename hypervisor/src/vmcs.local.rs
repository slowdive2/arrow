use {
    crate::{
        intel::{
            capture::GuestRegisters,
            controls::{adjust_vmx_controls, VmxControl},
            descriptor::Descriptors,
            invept::invept_single_context,
            invvpid::{invvpid_single_context, VPID_TAG},
            segmentation::{access_rights_from_native, lar, lsl},
            support::{cr3, rdmsr, sidt, vmread, vmwrite},
        },
    },
    bit_field::BitField,
    core::fmt,
    x86::{
        bits64::{paging::BASE_PAGE_SIZE, rflags},
        debugregs::dr7,
        msr,
        segmentation::{cs, ds, es, fs, gs, ss},
        vmx::vmcs,
    },
    x86_64::registers::control::{Cr0, Cr0Flags, Cr4, Cr4Flags},
};

