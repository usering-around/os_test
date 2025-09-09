/// Bunch of functions relating to the x86_64 arch.
/// Register get/set functions will always be inlined (since calling a function may change the output of certain registers,
/// and also there isn't really a need for a whole function procedures for these functions)
use crate::idt::IdtPtr;
use core::arch::asm;
// get the cs register
#[inline(always)]
pub fn cs() -> u16 {
    let out: u16;
    unsafe {
        asm!("mov {0:x}, cs", out(reg) out);
    }
    out
}

pub unsafe fn lidt(idt_ptr: &IdtPtr) {
    unsafe { asm!("lidt [{}]", in(reg) idt_ptr) }
}

/// get the cr2 register
#[inline(always)]
pub fn cr2() -> u64 {
    let out: u64;
    unsafe { asm!("mov {}, cr2", out(reg) out) };
    out
}

/// get the cr3 register
#[inline(always)]
pub fn cr3() -> u64 {
    let out: u64;
    unsafe { asm!("mov {}, cr3", out(reg) out) };
    out
}

/// get the rbp register
#[inline(always)]
pub fn rbp() -> u64 {
    let out: u64;
    unsafe { asm!("mov {}, rbp", out(reg) out) };
    out
}

/// ## Safety:
/// if no interrupt is called, this will lock up the computer.
pub unsafe fn hlt() {
    unsafe { asm!("hlt") }
}

/// get the rip register
#[inline(always)]
pub fn rip() -> u64 {
    let out: u64;
    unsafe { asm!("lea {}, [rip]", out(reg) out) };
    out
}

#[inline(always)]
pub unsafe fn invlpg(addr: u64) {
    unsafe {
        asm!("invlpg ({})", in(reg) addr, options(att_syntax, nostack, preserves_flags));
    }
}

#[inline(always)]
pub unsafe fn reload_cr3() {
    let cr3 = cr3();
    unsafe {
        asm!("mov {0:r}, cr3", in(reg) 3);
        asm!("mov {}, cr3", in(reg) cr3)
    }
}

#[inline(always)]
pub unsafe fn sti() {
    unsafe { asm!("sti") }
}

#[inline(always)]
pub unsafe fn cli() {
    unsafe {
        asm!("cli");
    }
}

#[inline(always)]
pub unsafe fn rflags() -> u64 {
    let rflags: u64;
    unsafe {
        asm!("pushf
            pop {}",
        out(reg) rflags)
    }
    rflags
}
