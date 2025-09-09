use core::default::Default;
use core::marker::PhantomPinned;
use core::mem::MaybeUninit;
use core::pin::Pin;

use crate::arch_x86_64::{self, lidt};

type InterruptHandlerFn = unsafe extern "C" fn() -> !;
type TrapHandlerFn = unsafe extern "C" fn() -> !;

/// NOTE: ALLOCATIONS/ANY REASOURCE WHICH REQUIRES A LOCK IS NOT ALLOWED IN HERE EXCEPT A PANIC.
#[macro_export]
macro_rules! interrupt_handler_fn {
    (|| $func: block) => {{
        use core::arch::naked_asm;
        #[naked]
        extern "C" fn wrapper() -> ! {
            extern "C" fn ignore() {
                $func
            }
            unsafe {
                // and call the C function
                naked_asm!(

                    "
                    // save the registers which are not saved by C abi
                    push rdi;
                    push rsi;
                    push rdx;
                    push rcx;
                    push rax;
                    push r8;
                    push r9;
                    push r10;
                    push r11;
                    // c abi requires cld
                    cld;
                    // c abi requires stack alignment of 16 bytes
                    // we push 9, 8 bytes ptr, and the cpu aligns to 16 bytes without error code
                    sub rsp, 8
                    // call the actual handler
                    call {};
                    // restore the stack
                    add rsp, 8
                    pop r11;
                    pop r10;
                    pop r9;
                    pop r8;
                    pop rax;
                    pop rcx;
                    pop rdx;
                    pop rsi;
                    pop rdi;
                    iretq;",
                    sym ignore
                )
            }
        }
        wrapper
    }};
}
/// Create a new trap handler
/// A trap may not return. If you wish to recover from a trap, do it by your own code.
/// To assist with that, registers not preserved by the C abi are preserved. The1y're pushed to the stack in exactly the following order:
/// rdi, rsi, rdx, rcx, rax, r8, r9, r10, r11
/// where left = pushed first.
/// Additionally, 8 bytes of junk are pushed to the stack after r11 for alignment purposes.
/// NOTE: ALLOCATIONS/ANY REASOURCE WHICH REQUIRES A LOCK IS NOT ALLOWED IN HERE EXCEPT A PANIC.
#[macro_export]
macro_rules! trap_handler_fn {
    (|| $func: block) => {{
        use core::arch::naked_asm;
        #[naked]
        extern "C" fn wrapper() -> ! {
            extern "C" fn ignore() -> ! {
                $func
            }
            unsafe {
                // and call the C function
                naked_asm!(

                    "
                    // save the registers which are not saved by C abi
                    push rdi;
                    push rsi;
                    push rdx;
                    push rcx;
                    push rax;
                    push r8;
                    push r9;
                    push r10;
                    push r11;
                    // c abi requires cld
                    cld;
                    // c abi requires stack alignment of 16 bytes
                    // we push 9, 8 bytes ptr, and the cpu aligns to 16 bytes without error code
                    sub rsp, 8
                    // call the actual handler
                    call {};",
                    sym ignore
                )
            }
        }
        wrapper
    }};


}

#[macro_export]
macro_rules! insert_interrupt {
    ($idt: expr, $idx: literal, $idt_entry_type: expr) => {
        $idt.as_mut().insert(
            $idx,
            IdtEntry::new_with_current_cs(IdtEntryType::Interrupt($idt_entry_type)),
        );
    };
}

#[macro_export]
macro_rules! insert_trap {
    ($idt: expr, $idx: literal, $idt_entry_type: expr) => {
        $idt.as_mut().insert(
            $idx,
            IdtEntry::new_with_current_cs(IdtEntryType::Trap($idt_entry_type)),
        );
    };
}

/// Create a new trap handler function which also handles error codes.
/// A trap may not return. If you wish to recover from a trap, do it by your own code.  
/// To assist with that, registers not preserved by the C abi are preserved. They're pushed to the stack in exactly the following order:  
/// rdi, rsi, rdx, rcx, rax, r8, r9, r10, r11  
/// where left = pushed first.
/// NOTE: ALLOCATIONS/ANY REASOURCE WHICH REQUIRES A LOCK IS NOT ALLOWED IN HERE EXCEPT A PANIC.
#[macro_export]
macro_rules! trap_handler_fn_with_error {
    (|$num: ident| $func: block) => {{
        use core::arch::naked_asm;
        #[naked]
        extern "C" fn wrapper() -> ! {
            extern "C" fn ignore($num: u64) -> ! {
                $func
            }
            unsafe {
                // and call the C function
                naked_asm!(

                    "
                    // save the registers which are not saved by C abi
                    push rdi;
                    push rsi;
                    push rdx;
                    push rcx;
                    push rax;
                    push r8;
                    push r9;
                    push r10;
                    push r11; 
                    // move the error code to the first arg
                    mov rdi, [rsp + 8 * 9]
                    // c abi requires cld
                    cld;
                    // DUE TO THE ERROR CODE, THIS IS 16 BYTE ALIGNED: 9 * 8 + 8 = 5 * 16
                    // call the actual handler
                    call {};",
                    sym ignore
                )
            }
        }
        wrapper
    }};
}

/// Represents a single entry in the IDT
#[derive(Debug, Clone)]
pub struct IdtEntry {
    /// The type of entry, either a trap or gate.
    entry_type: IdtEntryType,
    /// The kernel code segment. If IdtEntry::new is used in kernel context, you might want to simply use arch_x86_64::cs() for this value.
    gdt_kernel_cs: u16,
}

/// The type of entry. Trap and Interrupt have minor differences; read their documentation
#[derive(Debug, Clone, Copy)]
pub enum IdtEntryType {
    /// An interrupt differs by trap in the fact that it saves the NEXT instruction to execute, and it clears the interrupt flag
    /// i.e. interrupts are disabled.
    Interrupt(InterruptHandlerFn),
    /// A Trap differs from interrupts in the fact that it saves the CURRENT INSTRUCTION (i.e. the instruction which called the TRAP), and it does
    /// not clear the interrupt flag. RETURNING FROM A TRAP CAN CAUSE A LOOP, since it will return to the instruction which called the TRAP.
    Trap(TrapHandlerFn),
}

#[repr(C, packed)]
#[derive(Debug)]
struct IdtRaw([IdtEntryRaw; 256]);

#[derive(Debug)]
pub struct Idt {
    raw: IdtRaw,
    ptr: IdtPtr,
    _phantom_pinned: PhantomPinned,
}

impl Idt {
    /// load the IDT onto the processor. Safety: Read the intel manual about when it is safe to load this thing (lol).
    /// If the IDT is dropped before it was replaced with some other IDT,
    /// it is considered undefined behvaior.
    pub unsafe fn load(self: Pin<&Self>) {
        // safety: the Idt type is unmmovable, hence so is self.ptr,
        // which makes this safe as long as the IDT lives
        unsafe { lidt(&self.ptr) }
    }

    /* todo: return this when memory allocation is implemneted
    pub fn zeroed_boxed() -> Pin<Box<Idt>> {
        let mut idt = Box::new(Self {
            raw: unsafe { core::mem::zeroed() },
            ptr: IdtPtr {
                base: 0 as *const _,
                limit: (core::mem::size_of::<IdtRaw>() - 1) as u16,
            },
        });
        idt.ptr.base = &raw const idt.raw;
        Pin::new(idt)
    } */

    /// Initialize an IDT from uninitialized data  
    /// We take in uninitalized data because we need to know the memory location of
    /// the IDT struct before creating it.  
    /// We return Pin<&mut self> since we cannot have the buffer which holds the IDT entries
    /// move.
    pub fn init(uninit: Pin<&mut MaybeUninit<Self>>) -> Pin<&mut Self> {
        unsafe {
            uninit.map_unchecked_mut(|m| {
                let idt = m.write(Self {
                    raw: IdtRaw(core::mem::zeroed()),
                    ptr: IdtPtr {
                        base: 0 as *const _,
                        limit: (core::mem::size_of::<IdtRaw>() - 1) as u16,
                    },
                    _phantom_pinned: PhantomPinned {},
                });
                idt.ptr.base = &raw const idt.raw;
                idt
            })
        }
    }

    pub fn insert(self: Pin<&mut Self>, index: usize, entry: IdtEntry) {
        unsafe { *self.get_unchecked_mut().raw.0.get_mut(index).unwrap() = entry.to_raw() }
    }
}

impl IdtEntry {
    pub fn new(entry_type: IdtEntryType, gdt_kernel_cs: u16) -> Self {
        Self {
            entry_type,
            gdt_kernel_cs,
        }
    }

    pub fn new_with_current_cs(entry_type: IdtEntryType) -> Self {
        Self::new(entry_type, arch_x86_64::cs())
    }

    fn to_raw(&self) -> IdtEntryRaw {
        let fn_ptr = match self.entry_type {
            IdtEntryType::Interrupt(f) => f as u64,
            IdtEntryType::Trap(f) => f as u64,
        };
        let fn_ptr_low = (fn_ptr & 0xffff) as u16;
        let fn_ptr_mid = (fn_ptr >> 16) as u16;
        let fn_ptr_high = (fn_ptr >> 32) as u32;
        let options = match self.entry_type {
            IdtEntryType::Interrupt(_) => 0x8E00,
            IdtEntryType::Trap(_) => 0x8F00,
        };
        let raw = IdtEntryRaw {
            fn_ptr_low,
            gdt_kernel_cs: self.gdt_kernel_cs,
            options: options,
            fn_ptr_mid,
            fn_ptr_high,
            reserved: 0,
        };
        raw
    }
}

/// Actual representation of the IDT given to the processor
#[repr(C, packed)]
#[derive(Default, Debug, Clone, Copy)]
struct IdtEntryRaw {
    fn_ptr_low: u16,
    gdt_kernel_cs: u16,
    options: u16,
    fn_ptr_mid: u16,
    fn_ptr_high: u32,
    reserved: u32,
}

#[repr(C, packed)]
#[derive(Debug)]
pub struct IdtPtr {
    /// size of the Idt minus 1
    limit: u16,
    /// Pointer to the IDT
    base: *const IdtRaw,
}

unsafe impl Send for IdtPtr {}
