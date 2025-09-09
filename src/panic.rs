use crate::arch_x86_64::hlt;
use crate::qemu_println;
use crate::stack_trace::StackTrace;
use crate::{CONSOLE, screen::Color};
use core::fmt::Write;
use core::panic::PanicInfo;

// think of a better system rather than doing this,
// since it doesn't help against multi-cpu
unsafe fn _force_unlock_panic_outputs() {
    unsafe {
        crate::CONSOLE.force_unlock();
        crate::qemu_log::GLOBAL_LOGGER.force_unlock();
    }
}

#[panic_handler]
fn panic(inf: &PanicInfo) -> ! {
    // Note: it is fine to to use the SCREEN/CONSOLE here since if the screen is not functional we're doing something else
    // ensure that the console/logger aren't locked
    // safety: currently the computer only runs on 1 cpu,
    // so panic = nothing else runs
    unsafe { _force_unlock_panic_outputs() }
    qemu_println!("{}", inf);

    let mut console = CONSOLE.lock();
    console.bg_color = Color::blue();
    console.fg_color = Color::white();
    console.clear();
    writeln!(console, "{}", inf).unwrap();

    let mut trace = StackTrace::new();

    writeln!(console, "\nstack trace:").unwrap();
    let mut func_name = unsafe { StackTrace::lookup_current_function().unwrap() };
    while let Some(addr) = unsafe { trace.next() } {
        writeln!(console, "{} <called at {:#x}>", func_name, addr).unwrap();
        func_name = if let Some(sym) = unsafe { StackTrace::lookup_symbol_from_return_addr(addr) } {
            sym
        } else {
            "unknown_func"
        };
    }
    writeln!(console, "{}", func_name).unwrap();

    loop {
        // we keep the CONSOLE locked so that no other CPU writes to it
        // in the future we should make some kind of PANIC cpu interrupt which stops other CPUs
        unsafe {
            hlt();
        }
    }
}
