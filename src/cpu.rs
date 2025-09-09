use limine::mp::Cpu;

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
    interrupts::{SHARED_IDT, irq_enable},
};

#[cfg(feature = "smp")]
use crate::LIMINE_CPU_REQUEST;

// testing; needs to be removed once testing is done
fn hpet_init() {
    if let Some(irq) = (0..=24).find(|irq| unsafe { Hpet::timer(0).can_route_irq_to(*irq) }) {
        let irq_redirection = IoApicRedirectEntry {
            dest: LocalApic::id() as u8,
            mask: false,
            trigger_mode: unsafe { Hpet::timer(0).trigger_mode() },
            interrupt_polarity: InterruptPolarity::HighActive,
            destination_mode: DestinationMode::Physical,
            delivery_mode: DeilveryMode::Fixed,
            redirected_irq_num: 32,
        };
        unsafe {
            Hpet::timer(0).route_irq_to(irq).unwrap();
            let ticks = Hpet::tick_rate_ns() * 10_u64.pow(9);
            Hpet::timer(0).set_counter_raw(ticks as u64);
            Hpet::timer(0).enable();
        }
        IoApic::redirect_irq(irq as u8, irq_redirection);
        let ticks = Hpet::tick_rate_ns() * 10_u64.pow(9);
        Hpet::set_main_counter_raw(ticks as u64);
        Hpet::enable();
        SHARED_IDT.guard(|idt| {
            idt.lock().as_mut().insert(
                32,
                IdtEntry::new_with_current_cs(IdtEntryType::Interrupt(interrupt_handler_fn!(
                    || {
                        panic!("got 32 interrupt!");
                    }
                ))),
            );
        });

        console_println!("hpet initialized! irq: {}", irq);
        console_println!(
            "is using legacy mapping: {}",
            Hpet::is_using_legacy_mapping()
        );
    } else {
        panic!("cannot initialize hpet");
    }
}

pub fn init() {
    SHARED_IDT.guard(|idt| unsafe {
        idt.lock().as_ref().load();
    });
    console_println!("loaded shared idt!");
    IoApic::init();
    hpet_init();
    console_println!("ver: {}", LocalApic::version());
    console_println!("hpet tick rate: {:?}", Hpet::tick_rate_ns());
    console_println!("io apic version: {:?}", IoApic::version());
    console_println!(
        "io apic maximum redirection: {:?}",
        IoApic::maximum_redirections()
    );
    console_println!("io apic id: {:?}", IoApic::id());

    unsafe {
        irq_enable();
    }

    #[cfg(feature = "smp")]
    {
        let cpu_response = LIMINE_CPU_REQUEST.get_response().unwrap();
        for cpu in cpu_response.cpus() {
            cpu.goto_address.write(cpu_main);
        }
    }
}

#[naked]
#[unsafe(no_mangle)]
unsafe extern "C" fn cpu_main(cpu: &Cpu) -> ! {
    unsafe {
        core::arch::naked_asm!(
            // initialize stack frame and pass the cpu as an arguement
            // (implicitly since we didn't touch the RDI register)
            "xor rbp, rbp;
            call {};",
            sym cpu_main_rs
        )
    }
}
#[unsafe(no_mangle)]
unsafe extern "C" fn cpu_main_rs(cpu: &Cpu) -> ! {
    console_println!(
        "cpu {} arrived! lapic id: {}, lapic version: {:x}",
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
    unsafe {
        loop {
            hlt();
        }
    }
}
