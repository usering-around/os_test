#![no_std]
#![no_main]
#![feature(ptr_as_ref_unchecked)]
#![feature(ascii_char)]
#![feature(naked_functions)]
#![feature(custom_test_frameworks)]
#![feature(let_chains)]
#![test_runner(crate::test::test_runner)]
#![reexport_test_harness_main = "lib_test"]
use core::{cell::LazyCell, fmt::Write, mem::MaybeUninit};

use console::{Console, ThreadSafeConsole};
#[cfg(feature = "smp")]
use limine::request::MpRequest;
use limine::{
    BaseRevision,
    request::{RequestsEndMarker, RequestsStartMarker},
};
use limine::{
    modules::{InternalModule, ModuleFlags},
    request::{
        ExecutableAddressRequest, FramebufferRequest, HhdmRequest, MemoryMapRequest, ModuleRequest,
        RsdpRequest,
    },
};

use screen::Screen;

pub mod arch_x86_64;
pub mod console;
pub mod cpu;
pub mod dev;
pub mod fs;
pub mod hexdump;
pub mod idt;
pub mod interrupts;
pub mod io;
pub mod memory;
pub mod msr;
#[cfg(not(test))]
pub mod panic;
pub mod qemu_log;
pub mod screen;
pub mod stack_trace;
#[cfg(test)]
mod test;
pub extern crate alloc;
pub mod acpi;

// use the .requests section since limine supports it
#[used]
#[unsafe(link_section = ".requests")]
pub static BASE_REVISION: BaseRevision = BaseRevision::new();

/// Define the stand and end markers for Limine requests.
#[used]
#[unsafe(link_section = ".requests_start_marker")]
static _START_MARKER: RequestsStartMarker = RequestsStartMarker::new();
#[used]
#[unsafe(link_section = ".requests_end_marker")]
static _END_MARKER: RequestsEndMarker = RequestsEndMarker::new();

#[used]
#[unsafe(link_section = ".requests")]
pub static FRAMEBUFFER_REQUEST: FramebufferRequest = FramebufferRequest::new();

#[used]
#[unsafe(link_section = ".requests")]
pub static KERNEL_SYMBOL_MODULE: InternalModule = InternalModule::new()
    .with_path(unsafe { core::ffi::CStr::from_bytes_with_nul_unchecked(b"kernel.symbols\0") })
    .with_flags(ModuleFlags::REQUIRED);
#[used]
#[unsafe(link_section = ".requests")]
pub static MODULE_REQUEST: ModuleRequest =
    ModuleRequest::new().with_internal_modules(&[&KERNEL_SYMBOL_MODULE]);

#[used]
#[unsafe(link_section = ".requests")]
pub static EXECUTABLE_REQUEST: ExecutableAddressRequest = ExecutableAddressRequest::new();

#[used]
#[unsafe(link_section = ".requests")]
pub static HIGHER_HALF_DIRECT_MAP: HhdmRequest = HhdmRequest::new();

#[used]
#[unsafe(link_section = ".requests")]
pub static LIMINE_MEMORY_MAP: MemoryMapRequest = MemoryMapRequest::new();

#[cfg(feature = "smp")]
#[used]
#[unsafe(link_section = ".requests")]
pub static LIMINE_CPU_REQUEST: MpRequest = MpRequest::new();

#[used]
#[unsafe(link_section = ".requests")]
pub static LIMINE_RSDP_REQUEST: RsdpRequest = RsdpRequest::new();

unsafe extern "C" {
    // shared from linker
    /// a pointer to this value = address at which the kernel begins
    pub safe static AT_KERNEL_BEGIN: usize;
    /// a pointer to this value = address at which the kernel ends
    pub safe static AT_KERNEL_END: usize;
}

pub fn kernel_size() -> u64 {
    let kern_begin = (&AT_KERNEL_BEGIN) as *const _ as u64;
    let kern_end = (&AT_KERNEL_END) as *const _ as u64;
    kern_end - kern_begin
}

pub fn kernel_virt_begin() -> u64 {
    EXECUTABLE_REQUEST.get_response().unwrap().virtual_base()
}

pub fn kernel_virt_end() -> u64 {
    EXECUTABLE_REQUEST.get_response().unwrap().virtual_base() + kernel_size()
}

/// start of physical address of the kernel
pub fn kernel_phy_begin() -> u64 {
    EXECUTABLE_REQUEST.get_response().unwrap().physical_base()
}
/// Screen to draw pixels in
pub const SCREEN: LazyCell<Screen> = LazyCell::new(|| unsafe {
    // safety: limine protocol should give us accurate data
    // and also this cannot panic since main.rs ensure we die if there isn't at least one framebuffer
    Screen::new(
        FRAMEBUFFER_REQUEST
            .get_response()
            .unwrap()
            .framebuffers()
            .next()
            .unwrap(),
    )
});

/// global console
pub static CONSOLE: spin::Lazy<ThreadSafeConsole> = spin::Lazy::new(|| {
    ThreadSafeConsole::new(Console::new(
        SCREEN.clone(),
        crate::screen::Color::black(),
        crate::screen::Color::blue(),
    ))
});

pub fn _console_print(args: core::fmt::Arguments) {
    CONSOLE.lock().write_fmt(args).unwrap();
}

#[macro_export]
macro_rules! console_print {
    ($($arg:tt)*) => ($crate::_console_print(format_args!($($arg)*)));
}

#[macro_export]
macro_rules! console_println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ($crate::console_print!("{}\n", format_args!($($arg)*)));
}

// todo: move this somewhere else
use crate::idt::{Idt, IdtEntry, IdtEntryType};
use core::pin::Pin;
pub fn create_init_idt(uninit: Pin<&mut MaybeUninit<Idt>>) -> Pin<&mut Idt> {
    let mut idt = Idt::init(uninit);
    /*
    for i in 0..256 {
        idt.as_mut().insert(
            i,
            IdtEntry::new_with_current_cs(IdtEntryType::Trap(trap_handler_fn!(|| {
                panic!("trap");
            }))),
        );
    }
    */
    idt.as_mut().insert(
        0,
        IdtEntry::new_with_current_cs(IdtEntryType::Trap(trap_handler_fn!(|| {
            panic!("divide by 0 exception (0)");
        }))),
    );
    idt.as_mut().insert(
        1,
        IdtEntry::new_with_current_cs(IdtEntryType::Trap(trap_handler_fn!(|| {
            panic!("dbg (1)")
        }))),
    );
    idt.as_mut().insert(
        2,
        IdtEntry::new_with_current_cs(IdtEntryType::Interrupt(interrupt_handler_fn!(|| {
            panic!("NMI interrupt? (2)");
        }))),
    );
    insert_trap!(
        idt,
        3,
        trap_handler_fn!(|| { panic!("exception 3; breakpoint") })
    );
    insert_trap!(
        idt,
        4,
        trap_handler_fn!(|| { panic!("exception 4; overflow") })
    );
    insert_trap!(
        idt,
        5,
        trap_handler_fn!(|| { panic!("exception 5; bound ranger exceeded") })
    );
    insert_trap!(
        idt,
        6,
        trap_handler_fn!(|| { panic!("exception 6; invalid opcode") })
    );
    insert_trap!(
        idt,
        7,
        trap_handler_fn!(|| { panic!("exception 7; device not available") })
    );
    insert_trap!(
        idt,
        8,
        trap_handler_fn_with_error!(|err| {
            panic!("exception 8; double fault; err code: {}", err)
        })
    );
    insert_trap!(
        idt,
        9,
        trap_handler_fn!(|| { panic!("exception 9; coprocessor segment overrun") })
    );
    insert_trap!(
        idt,
        10,
        trap_handler_fn_with_error!(|err| {
            panic!("exception 10; invalid tss; err code {}", err)
        })
    );
    insert_trap!(
        idt,
        11,
        trap_handler_fn_with_error!(|err| {
            panic!("exception 11; segment not present; err code: {}", err)
        })
    );
    insert_trap!(
        idt,
        12,
        trap_handler_fn_with_error!(|err| {
            panic!("exception 12; stack segment fault; err code: {}", err)
        })
    );
    insert_trap!(
        idt,
        13,
        trap_handler_fn_with_error!(|err| {
            let is_external = if err & 1 != 0 { true } else { false };
            let desc_table = match (err >> 1) & 0b11 {
                0b00 => "GDT",
                0b01 | 0b11 => "IDT",
                0b10 => "LDT",
                _ => unreachable!(),
            };
            let idx = (err >> 3) & 0x1fff;
            panic!(
                "exception 13; general protection fault; err code: {}\nis_external: {}\ncaused by: {}\nindex: {}",
                err, is_external, desc_table, idx
            );
        })
    );
    //insert_trap!(idt, 14, trap_handler_fn!(|| { panic!("exception 3") }));
    insert_trap!(
        idt,
        15,
        trap_handler_fn!(|| { panic!("exception 15; reserved") })
    );
    insert_trap!(
        idt,
        16,
        trap_handler_fn!(|| { panic!("exception 16; x87 floating point exception") })
    );
    insert_trap!(
        idt,
        17,
        trap_handler_fn_with_error!(|err| {
            panic!("exception 17; alignment check; err code: {}", err)
        })
    );
    insert_trap!(
        idt,
        18,
        trap_handler_fn!(|| { panic!("exception 18; machine check") })
    );
    insert_trap!(
        idt,
        19,
        trap_handler_fn!(|| { panic!("exception 19; simd floating point exception") })
    );
    insert_trap!(
        idt,
        20,
        trap_handler_fn!(|| { panic!("exception 20; virtualization exception") })
    );
    insert_trap!(
        idt,
        21,
        trap_handler_fn_with_error!(|err| {
            panic!(
                "exception 21; control protection exception; err code: {}",
                err
            )
        })
    );
    insert_trap!(
        idt,
        22,
        trap_handler_fn!(|| { panic!("exception 22; reserved") })
    );
    insert_trap!(
        idt,
        23,
        trap_handler_fn!(|| { panic!("exception 23; reserved") })
    );
    insert_trap!(
        idt,
        24,
        trap_handler_fn!(|| { panic!("exception 24; reserved") })
    );
    insert_trap!(
        idt,
        25,
        trap_handler_fn!(|| { panic!("exception 25; reserved") })
    );
    insert_trap!(
        idt,
        26,
        trap_handler_fn!(|| { panic!("exception 26; reserved") })
    );
    insert_trap!(
        idt,
        27,
        trap_handler_fn!(|| { panic!("exception 27; reserved") })
    );
    insert_trap!(
        idt,
        28,
        trap_handler_fn!(|| { panic!("exception 28; hypervisor injection exception") })
    );
    insert_trap!(
        idt,
        29,
        trap_handler_fn_with_error!(|err| {
            panic!(
                "exception 29; vmm communication exception; err code: {}",
                err
            )
        })
    );
    insert_trap!(
        idt,
        30,
        trap_handler_fn_with_error!(|err| {
            panic!("exception 30; security exception; err code: {}", err)
        })
    );
    insert_trap!(
        idt,
        31,
        trap_handler_fn!(|| { panic!("exception 31; reserved") })
    );
    idt.as_mut().insert(
        14,
        IdtEntry::new_with_current_cs(IdtEntryType::Trap(trap_handler_fn_with_error!(|err| {
            panic!(
                "page protection fault; addr: 0x{:x}; err_code: {:b}",
                arch_x86_64::cr2(),
                err
            );
        }))),
    );

    idt
}
