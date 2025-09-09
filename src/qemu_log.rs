use core::fmt;
use core::fmt::Write;

use spin::mutex::SpinMutex;

#[macro_export]
macro_rules! qemu_print {
    ($($arg:tt)*) => ($crate::qemu_log::_print(format_args!($($arg)*)));
}

#[macro_export]
macro_rules! qemu_println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ($crate::qemu_print!("{}\n", format_args!($($arg)*)));
}

const QEMU_PORT: u16 = 0xe9;
pub struct QemuLogger;

pub static GLOBAL_LOGGER: SpinMutex<QemuLogger> = SpinMutex::new(QemuLogger {});

/// Safety: should only be ran when we're in qemu and with a lock if
/// it's in a multi-cpu environment
unsafe fn qemu_write(c: u8) {
    unsafe {
        crate::io::write_u8(QEMU_PORT, c);
    }
}

impl fmt::Write for QemuLogger {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for char in s.chars() {
            if let Some(ascii) = char.as_ascii() {
                unsafe { qemu_write(ascii.to_u8()) };
            }
        }
        Ok(())
    }
}

pub fn _print(args: fmt::Arguments) {
    GLOBAL_LOGGER.lock().write_fmt(args).unwrap();
}
