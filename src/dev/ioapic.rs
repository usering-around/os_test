use acpi::madt::{Madt, MadtEntry};
use spin::Lazy;

use crate::memory::{
    physical::PhyAddr,
    virt::{GLOBAL_PAGE_ALLOCATOR, PageAllocator, VirtAddr},
};

pub struct IoApic;

static IO_APIC_ADDR: Lazy<VirtAddr> = Lazy::new(|| {
    let madt = crate::acpi::tables().find_table::<Madt>().unwrap();
    let io_apic_entry = madt
        .get()
        .entries()
        .find(|e| matches!(e, MadtEntry::IoApic(_)))
        .unwrap();
    let MadtEntry::IoApic(data) = io_apic_entry else {
        panic!("not possible");
    };
    let ok = data.global_system_interrupt_base;
    // in the future we should handle this better...
    assert_eq!(ok, 0);
    crate::qemu_println!("io apic data: {:?}", data);
    let io_apic_phy_addr = PhyAddr(data.io_apic_address as u64);
    crate::qemu_println!("io apic phy addr: {:?}", io_apic_phy_addr);
    unsafe { GLOBAL_PAGE_ALLOCATOR.map_physical(io_apic_phy_addr, 1) }
        .unwrap()
        .1
});

impl IoApic {
    const IO_REG_SELECT_OFFSET: u64 = 0;
    const IO_WINDOW_OFFSET: u64 = 0x10;
    const IOAPICVER_REG: u32 = 0x1;
    const IOAPIC_ID_REG: u32 = 0;
    unsafe fn reg_select(reg: u32) {
        unsafe {
            core::ptr::write_volatile(
                (IO_APIC_ADDR.0 + Self::IO_REG_SELECT_OFFSET) as *mut u32,
                reg,
            )
        }
    }
    pub unsafe fn write_u32(reg: u32, val: u32) {
        unsafe {
            Self::reg_select(reg);
            core::ptr::write_volatile((IO_APIC_ADDR.0 + Self::IO_WINDOW_OFFSET) as *mut u32, val);
        };
    }

    unsafe fn read_u32(reg: u32) -> u32 {
        unsafe {
            Self::reg_select(reg);
            core::ptr::read_volatile((IO_APIC_ADDR.0 + Self::IO_WINDOW_OFFSET) as *const u32)
        }
    }

    pub fn version() -> u32 {
        unsafe { Self::read_u32(Self::IOAPICVER_REG) & 0xff }
    }

    pub fn maximum_redirections() -> u32 {
        unsafe {
            crate::qemu_println!("max redirect: 0x{:x}", Self::read_u32(Self::IOAPICVER_REG));
        }
        unsafe { (Self::read_u32(Self::IOAPICVER_REG) >> 16) & 0xff }
    }

    pub fn id() -> u32 {
        unsafe { (Self::read_u32(Self::IOAPIC_ID_REG) >> 24) & 0xf }
    }

    /// redirect irq 0-23 into an arbitrary irq number
    /// Note that irqs 0-15 are for legacy irqs. Use 16-23 for arbitrary interrupts.
    pub fn redirect_irq(irq_num: u8, entry: IoApicRedirectEntry) {
        unsafe {
            let reg_num = irq_num + 0x10;
            let low = (entry.as_raw().0 & 0xffff_ffff) as u32;

            let high = (entry.as_raw().0 >> 32) as u32;
            Self::write_u32(reg_num as u32, low);
            Self::write_u32(reg_num as u32 + 1, high);
        }
    }
    pub fn init() {
        let madt = crate::acpi::tables().find_table::<Madt>().unwrap();
        for entry in madt.get().entries() {
            match entry {
                MadtEntry::InterruptSourceOverride(over) => {
                    crate::qemu_println!("{:?}", over);
                }
                _ => (),
            }
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum DeilveryMode {
    Fixed = 0,
    LowestPriority = 1,
    Smi = 2,
    Nmi = 4,
    Init = 5,
    ExtInit = 7,
}

#[derive(Clone, Copy, Debug)]
pub enum DestinationMode {
    Physical = 0,
    Logical = 1,
}

#[derive(Clone, Copy, Debug)]
pub enum InterruptPolarity {
    HighActive = 0,
    LowActive = 1,
}

#[derive(Clone, Copy, Debug)]
pub enum TriggerMode {
    EdgeSensetive = 0,
    LevelSensetive = 1,
}

pub struct IoApicRedirectEntry {
    pub dest: u8,
    pub mask: bool,
    pub trigger_mode: TriggerMode,
    pub interrupt_polarity: InterruptPolarity,
    pub destination_mode: DestinationMode,
    pub delivery_mode: DeilveryMode,
    pub redirected_irq_num: u8,
}
struct IoApicRedirectEntryRaw(u64);

impl IoApicRedirectEntry {
    fn as_raw(&self) -> IoApicRedirectEntryRaw {
        let num = self.redirected_irq_num as u64
            | ((self.delivery_mode as u64) << 8)
            | ((self.destination_mode as u64) << 11)
            | ((self.interrupt_polarity as u64) << 13)
            | ((self.trigger_mode as u64) << 15)
            | (if self.mask { 1 } else { 0 } << 16);
        IoApicRedirectEntryRaw(num)
    }
}
