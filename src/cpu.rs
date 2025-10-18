use core::{mem::MaybeUninit, u32};

use limine::mp::Cpu;

use crate::LIMINE_CPU_REQUEST;
use crate::{
    arch_x86_64::hlt,
    console_println,
    dev::{
        hpet::Hpet,
        ioapic::{DeilveryMode, DestinationMode, InterruptPolarity, IoApic, IoApicRedirectEntry},
        local_apic::LocalApic,
    },
    idt::{IdtEntry, IdtEntryType},
    interrupt_handler_fn,
    interrupts::SHARED_IDT,
};

#[derive(Debug)]
pub struct PerCpu {
    pub lapic_ticks_per_ms: u32,
}

/// Safety: call this after PerCpu has been initialized
pub unsafe fn percpu() -> &'static PerCpu {
    // safety: this value is only accessible via this function after it has been initialized,
    // and it's only accessed in an Immutable manner.
    let maybe_uninit = unsafe { &PERCPUS[LocalApic::id() as usize] };
    // we shouldn't call this without first initializing the struct
    // we should probably just check if it has been initializede
    unsafe { maybe_uninit.assume_init_ref() }
}

// probably enough for now
pub const MAX_CPU_COUNT: usize = 32;

static mut PERCPUS: [MaybeUninit<PerCpu>; MAX_CPU_COUNT] =
    [const { MaybeUninit::uninit() }; MAX_CPU_COUNT];

fn hpet_init() {
    // safety: we are the sole owner of the timer
    let timer = unsafe { Hpet::timer(0) };
    timer.enable();
    let irq_redirection = IoApicRedirectEntry {
        dest: LocalApic::id() as u8,
        mask: false,
        trigger_mode: timer.trigger_mode(),
        interrupt_polarity: InterruptPolarity::HighActive,
        destination_mode: DestinationMode::Physical,
        delivery_mode: DeilveryMode::Fixed,
        redirected_irq_num: 32,
    };

    // currently we can't mask PIT ourselves currently, so we use the legacy mapping to stop it from throwing interrupts
    // in the future we should probably just route the IRQ ourselves and explicitly mask the PIT
    Hpet::enable_legacy_mapping();
    IoApic::redirect_irq(2 as u8, irq_redirection);
    Hpet::enable();
    SHARED_IDT.guard(|idt| {
        idt.lock().as_mut().insert(
            32,
            IdtEntry::new_with_current_cs(IdtEntryType::Interrupt(interrupt_handler_fn!(|| {
                LocalApic::eoi();
            }))),
        );
    });

    console_println!("hpet initialized! irq: {}", 2);
}

fn local_apic_init() -> u32 {
    // should probably create an array/table of all IRQs instead of this
    LocalApic::set_spurious_interrupt_irq(33);
    LocalApic::set_lvt_timer_irq(34);
    LocalApic::set_lvt_error_irq(35);

    SHARED_IDT.guard(|idt| {
        let mut idt = idt.lock();
        idt.as_mut().insert(
            33,
            IdtEntry::new_with_current_cs(IdtEntryType::Interrupt(interrupt_handler_fn!(|| {
                panic!("spurious interrupt 33");
            }))),
        );
        idt.as_mut().insert(
            34,
            IdtEntry::new_with_current_cs(IdtEntryType::Interrupt(interrupt_handler_fn!(|| {
                panic!("lapic timer");
            }))),
        );
        idt.as_mut().insert(
            35,
            IdtEntry::new_with_current_cs(IdtEntryType::Interrupt(interrupt_handler_fn!(|| {
                panic!("LAPIC error");
            }))),
        );
    });

    // best resolution
    LocalApic::set_timer_div(1);
    // calibrate
    let init_ticks = u32::MAX;
    LocalApic::set_timer_init_count(init_ticks);
    crate::time::poll_sleep(core::time::Duration::from_millis(1));
    // we woke up after 1 ms,
    let ticks_per_ms = u32::MAX - LocalApic::current_count();
    LocalApic::set_timer_init_count(0);
    ticks_per_ms
}

pub fn init() {
    SHARED_IDT.guard(|idt| unsafe {
        idt.lock().as_ref().load();
    });
    console_println!("loaded shared idt!");
    IoApic::init();
    hpet_init();
    console_println!("lapic ver: {}", LocalApic::version());
    console_println!("apic ver: {}", LocalApic::id());
    console_println!("hpet tick rate: {:?}", Hpet::tick_rate_ms());
    console_println!("io apic version: {:?}", IoApic::version());
    console_println!(
        "io apic maximum redirection: {:?}",
        IoApic::maximum_redirections()
    );
    console_println!("io apic id: {:?}", IoApic::id());

    let cpu_response = LIMINE_CPU_REQUEST.get_response().unwrap();
    #[cfg(feature = "smp")]
    {
        for cpu in cpu_response.cpus() {
            cpu.goto_address.write(cpu_main);
        }
    }
    unsafe { cpu_main_rs(cpu_response.cpus()[cpu_response.bsp_lapic_id() as usize]) }
}

#[unsafe(naked)]
#[unsafe(no_mangle)]
unsafe extern "C" fn cpu_main(cpu: &Cpu) -> ! {
    core::arch::naked_asm!(
        // initialize stack frame and pass the cpu as an arguement
        // (implicitly since we didn't touch the RDI register)
        "xor rbp, rbp;
        call {};",
        sym cpu_main_rs
    )
}
#[unsafe(no_mangle)]
unsafe extern "C" fn cpu_main_rs(cpu: &Cpu) -> ! {
    console_println!(
        "cpu {} online! lapic id: {}, lapic version: {:x}",
        cpu.id,
        LocalApic::id(),
        LocalApic::version(),
    );
    {
        unsafe {
            SHARED_IDT.guard(|idt| {
                idt.lock().as_ref().load();
            });
        }
    }
    let lapic_ticks_per_ms = local_apic_init();
    // safety: LocalApic::id() should be different between each CPU,
    // hence this changes different elements of PERCPUS
    unsafe {
        PERCPUS[LocalApic::id() as usize].write(PerCpu { lapic_ticks_per_ms });
    }
    console_println!("CPU {} init done; data: {:?}", LocalApic::id(), unsafe {
        percpu()
    },);
    unsafe {
        loop {
            hlt();
        }
    }
}
