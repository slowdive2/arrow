// changing an entry does nothing until stale ept translations r flushed

use core::arch::asm;

use super::InveptDescriptor;

const INVEPT_SINGLE: u64 = 1;

unsafe fn invept(kind: u64, desc: &InveptDescriptor) -> bool {
    let cf: u8;
    let zf: u8;

    unsafe {
        asm!(
            "invept {kind}, [{desc}]",
            "setc {cf}",
            "setz {zf}",
            kind = in(reg) kind,
            desc = in(reg) desc,
            cf = lateout(reg_byte) cf,
            zf = lateout(reg_byte) zf,
        );
    }

    cf == 0 && zf == 0
}

pub unsafe fn invept_single(eptp: u64) -> bool {
    let desc = InveptDescriptor { eptp, reserved: 0 };
    unsafe { invept(INVEPT_SINGLE, &desc) }
}
