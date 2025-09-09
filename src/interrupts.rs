use core::pin::Pin;

use alloc::boxed::Box;
use spin::{Lazy, Mutex};

use crate::{
    arch_x86_64::{cli, rflags, sti},
    create_init_idt,
    idt::Idt,
};

pub unsafe fn irq_disable() {
    unsafe {
        cli();
    }
}

pub unsafe fn irq_enable() {
    unsafe {
        sti();
    }
}

/// An IDT shared between all the processor with lifetime until the end of the kernel.
/// We use an InterruptGuard to ensure no interrupts occur while we modify.
pub static SHARED_IDT: Lazy<InterruptGuard<Mutex<Pin<&mut Idt>>>> = Lazy::new(|| {
    let idt_static = Box::leak(Box::new_uninit());
    let idt = create_init_idt(Pin::static_mut(idt_static));
    InterruptGuard::new(Mutex::new(idt))
});

#[inline(always)]
pub fn irq_is_enabled() -> bool {
    if unsafe { rflags() } & 0x0200 == 1 {
        true
    } else {
        false
    }
}

/// Temporarily stop interrupts in the given function.
/// Note: you can use it in a nested manner.
pub fn interrupt_guard<F, R>(func: F) -> R
where
    F: FnOnce() -> R,
{
    unsafe {
        let interrupts_enabled = irq_is_enabled();
        if interrupts_enabled {
            cli();
        }
        let result = func();
        if interrupts_enabled {
            sti();
        }
        result
    }
}

/// Guard a type from being modified with interrupts enabled.
/// Functions in a nested manner.
pub struct InterruptGuard<T> {
    inner: T,
}

impl<T> InterruptGuard<T> {
    pub fn new(inner: T) -> Self {
        Self { inner }
    }

    pub fn guard<F, R>(&self, func: F) -> R
    where
        F: FnOnce(&T) -> R,
    {
        interrupt_guard(|| func(&self.inner))
    }
    pub fn guard_mut<F, R>(&mut self, func: F) -> R
    where
        F: FnOnce(&mut T) -> R,
    {
        interrupt_guard(|| func(&mut self.inner))
    }
}
