#![no_std]
#![no_main]
#![feature(ascii_char)]
#![feature(naked_functions)]

use core::arch::naked_asm;
use core::mem::MaybeUninit;
use core::pin::pin;

use os_test::arch_x86_64::hlt;
use os_test::{
    BASE_REVISION, FRAMEBUFFER_REQUEST, console_println, create_init_idt, kernel_phy_begin,
    kernel_virt_begin, memory,
};

#[naked]
#[unsafe(no_mangle)]
unsafe extern "C" fn kmain() -> ! {
    unsafe {
        naked_asm!(
            // initialize stack frame
            "xor rbp, rbp;
            call {};",
            sym kmain_rs
        )
    }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn kmain_rs() -> ! {
    // ensure that the screen is functional
    if FRAMEBUFFER_REQUEST.get_response().is_none()
        || FRAMEBUFFER_REQUEST
            .get_response()
            .is_some_and(|r| r.framebuffers().next().is_none())
    {
        // WE HAVE NO SCREEN TO WRITE TO. WE CAN'T NOTIFY THE USER OF ANYTHING BASICALLY. CURRENTLY THE OS IS USELESS IF IT DOESN'T HAVE A SCREEN.
        // PERHAPS IN THE FUTURE REMOVE THE REQUIREMENT OF A SCREEN
        unsafe {
            hlt();
        }
    }
    // All limine requests must also be referenced in a called function, otherwise they may be
    // removed by the linker.
    assert!(BASE_REVISION.is_supported());

    // create initial idt
    let uninit_idt = pin!(MaybeUninit::uninit());
    let init = create_init_idt(uninit_idt);
    unsafe { init.as_ref().load() };
    console_println!("IDT has been loaded");
    memory::init();
    console_println!("memory has been loaded!");

    console_println!(
        "phy_addr: {:x}, virt_addr: {:x}",
        kernel_phy_begin(),
        kernel_virt_begin()
    );

    os_test::cpu::init();
    unsafe {
        loop {
            hlt();
        }
    }
}
