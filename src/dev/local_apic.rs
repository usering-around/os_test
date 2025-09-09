use acpi::madt::Madt;
use spin::Lazy;

use crate::memory::{
    physical::PhyAddr,
    virt::{GLOBAL_PAGE_ALLOCATOR, PageAllocator, VirtAddr},
};

static LOCAL_APIC_ADDRESS: Lazy<VirtAddr> = Lazy::new(|| {
    let madt = crate::acpi::tables().find_table::<Madt>().unwrap();
    let lapic_phy_addr = PhyAddr(madt.get().local_apic_address as u64);
    unsafe { GLOBAL_PAGE_ALLOCATOR.map_physical(lapic_phy_addr, 1) }
        .unwrap()
        .1
});

pub struct LocalApic;

impl LocalApic {
    pub fn read(register: u32) -> u32 {
        unsafe {
            core::ptr::read_volatile((LOCAL_APIC_ADDRESS.0 + (register as u64)) as *const u32)
        }
    }
    pub fn write(register: u32, val: u32) {
        unsafe {
            core::ptr::write_volatile((LOCAL_APIC_ADDRESS.0 + (register as u64)) as *mut u32, val);
        }
    }

    pub fn addr() -> VirtAddr {
        *LOCAL_APIC_ADDRESS
    }

    pub fn version() -> u32 {
        Self::read(0x30) & 0xff
    }
    pub fn id() -> u32 {
        Self::read(0x20) >> 24
    }

    pub fn set_timer_init_count(count: u32) {
        Self::write(0x390, count);
    }

    pub fn set_lvt_timer_irq(irq: u32) {
        Self::write(0x320, irq);
    }

    pub fn set_timer_div(div: u32) {
        Self::write(0x3e0, div);
    }

    pub fn current_count() -> u32 {
        Self::read(0x390)
    }

    pub fn set_lvt_error_irq(irq: u32) {
        Self::write(0x370, irq);
    }

    pub fn init() {
        Self::set_lvt_error_irq(42);
        Self::set_lvt_timer_irq(32);
    }
}
