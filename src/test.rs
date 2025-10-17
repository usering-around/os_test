use crate::{qemu_print, qemu_println};
use core::{panic::PanicInfo, pin::pin};

// Todo: add colors
// Todo: ensure that tests don't interfere with each other by making sure memory is the same for each test

/// the runtime of tests
// Note: while we could in theory remove the Tests::next and instead use a loop,
// but the current implementation allows us to ignore the stack pretty seemlessly,
// which allows for interrupts to become panics seemlessly.
// If we were to implement it in a loop,
// after an interrupt we would need to slightly hack around to find the return value,
// which seems much more complicated than this.
pub struct Tests {
    pub should_current_test_panic: bool,
    current_test: usize,
    tests: &'static [&'static dyn Testable],
    failed_tests_num: usize,
    success_tests_num: usize,
}

impl Tests {
    unsafe fn next_test() {
        unsafe {
            TESTS.current_test += 1;
            #[allow(static_mut_refs)]
            let tests_len = TESTS.tests.len();
            if TESTS.current_test >= tests_len {
                #[allow(static_mut_refs)]
                {
                    qemu_println!(
                        "tests done; summary: {} succeeded, {} failed",
                        TESTS.success_tests_num,
                        TESTS.failed_tests_num
                    );
                }
                exit_qemu(QemuExitCode::Success);
            } else {
                TESTS.should_current_test_panic = false;
                TESTS.tests[TESTS.current_test].run_test();
            }
        }
    }

    fn success() {
        unsafe {
            TESTS.success_tests_num += 1;
        }
    }

    fn failed() {
        unsafe {
            TESTS.failed_tests_num += 1;
        }
    }
}
// safety: we never use it in multi-threaded context
unsafe impl Send for Tests {}
unsafe impl Sync for Tests {}

pub trait Testable {
    fn run_test(&self);
}

impl<T: Fn()> Testable for T {
    fn run_test(&self) {
        qemu_print!("{}... ", core::any::type_name::<T>());
        self();
        if unsafe { TESTS.should_current_test_panic } {
            qemu_println!("[failed] (did not panic)");
            Tests::failed();
        } else {
            qemu_println!("[success]");
            Tests::success();
        }
        unsafe {
            Tests::next_test();
        }
    }
}

const DUMMY: &'static [&'static dyn Testable] = &[];
pub static mut TESTS: Tests = Tests {
    should_current_test_panic: false,
    current_test: 0,
    tests: DUMMY,
    success_tests_num: 0,
    failed_tests_num: 0,
};
pub fn test_runner(tests: &[&dyn Testable]) {
    qemu_println!("\n\nrunning {} lib tests\n", tests.len());
    unsafe {
        TESTS.current_test = 0;
        TESTS.should_current_test_panic = false;
        // wildly unsafe
        let ok: &'static [&'static dyn Testable] =
            core::slice::from_raw_parts(tests.as_ptr() as *const _, tests.len());
        TESTS.tests = ok;
        TESTS.tests[TESTS.current_test].run_test();
    }
}

#[panic_handler]
fn panic(inf: &PanicInfo) -> ! {
    unsafe {
        if TESTS.should_current_test_panic {
            qemu_println!("[success] (panicked)");
            Tests::success();
        } else {
            qemu_println!("[failed]");
            qemu_println!("{}\n", inf);
            Tests::failed();
        }
        Tests::next_test()
    }

    loop {}
}

#[unsafe(naked)]
#[unsafe(no_mangle)]
unsafe extern "C" fn kmain() -> ! {
    use core::arch::naked_asm;

    naked_asm!(
        // initialize stack frame
        "xor rbp, rbp;
        call {};",
        sym kmain_rs
    )
}

#[unsafe(no_mangle)]
unsafe extern "C" fn kmain_rs() -> ! {
    use crate::{create_init_idt, memory};
    use core::mem::MaybeUninit;
    // create initial idt
    let uninit_idt = pin!(MaybeUninit::uninit());
    let init = create_init_idt(uninit_idt);
    unsafe { init.as_ref().load() };
    memory::init();
    crate::lib_test();
    loop {}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum QemuExitCode {
    Success = 0x10,
    #[allow(dead_code)]
    Failed = 0x11,
}
fn exit_qemu(exit_code: QemuExitCode) {
    unsafe {
        crate::io::write_u32(0xf4, exit_code as u32);
    }
}

/// Use this if the test should panic, before the actual panic.
/// Note that you can put it in the end and then you'll have a test
/// which checks the start of the test and the panic at the end.
/// # Example
/// ```rust
/// #[test_case]
/// fn test() {
///     // blah blah random tests    
///     assert_eq!(1, 1);
///     assert_eq!(2, 2);
///     // test that should panic
///     should_panic!();
///     assert_eq!(1,2);
///     // the test would pass in this case.
///     // if the above did not panic, the test would have failed.
/// }
/// ```
#[macro_export]
macro_rules! should_panic {
    () => {
        #[allow(unused_unsafe)]
        unsafe {
            crate::test::TESTS.should_current_test_panic = true;
        }
    };
}

#[test_case]
fn should_panic_test() {
    should_panic!();
    panic!()
}
