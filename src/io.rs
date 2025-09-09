use core::arch::asm;
// todo: we can slightly anstract it into a Port structure
// which has read<u8>, read<u16>, read<u32>, write<u8>, write<u16>,
// write<u32>
pub unsafe fn read_u8(port: u16) -> u8 {
    let out: u8;
    unsafe { asm!("in {}, dx", out(reg_byte) out, in("dx") port) };
    out
}

pub unsafe fn read_u16(port: u16) -> u16 {
    let out: u16;
    unsafe { asm!("in {0:x}, dx", out(reg) out, in("dx") port,) };
    out
}

pub unsafe fn read_u32(port: u16) -> u32 {
    let out: u32;
    unsafe { asm!("in {0:e}, dx", out(reg) out, in("dx") port) }
    out
}

pub unsafe fn write_u8(port: u16, val: u8) {
    unsafe { asm!("out dx, al", in("dx") port, in("al") val) }
}

pub unsafe fn write_u32(port: u16, val: u32) {
    unsafe { asm!("out dx, eax", in("dx") port, in("eax") val) }
}
