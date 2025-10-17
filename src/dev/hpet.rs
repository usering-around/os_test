use acpi::HpetInfo;
use spin::Lazy;

use crate::{
    dev::ioapic::TriggerMode,
    memory::{
        physical::PhyAddr,
        virt::{GLOBAL_PAGE_ALLOCATOR, PageAllocator, VirtAddr},
    },
};

const GENERAL_CAPABILITIES_REGISTER: u64 = 0;
const GENERAL_CONFIGURATION_REGISTER: u64 = 0x10;
const MAIN_COUNTER_VAL_REGISTER: u64 = 0xf0;
/// enable legacy replacement mapping
const LEG_RT_CNF: u64 = 0b10;
/// enable the counter and start receiving interrupts
const ENABLE_CNF: u64 = 0b1;

static HPET_BASE_ADDR: Lazy<VirtAddr> = Lazy::new(|| {
    let hpet_info = HpetInfo::new(&crate::acpi::tables()).unwrap();
    if !hpet_info.main_counter_is_64bits() {
        panic!("HPET IS NOT CAPABLE OF 64 BITS!");
    }
    unsafe { GLOBAL_PAGE_ALLOCATOR.map_physical(PhyAddr(hpet_info.base_address as u64), 1) }
        .unwrap()
        .1
});
pub struct Hpet;

impl Hpet {
    pub unsafe fn read(reg: u64) -> u64 {
        unsafe { core::ptr::read_volatile((HPET_BASE_ADDR.0 + reg) as *const u64) }
    }

    pub unsafe fn write(reg: u64, val: u64) {
        unsafe {
            core::ptr::write_volatile((HPET_BASE_ADDR.0 + reg) as *mut u64, val);
        }
    }

    /// get the amount of femto seconds (10e-15) which pass per single tick
    pub fn fs_per_tick() -> u64 {
        let fs_per_tick = unsafe { Self::read(GENERAL_CAPABILITIES_REGISTER) >> 32 };
        assert!(fs_per_tick <= 0x05F5E100);
        fs_per_tick
    }
    /// get the amount of ticks per 1 miliseconds
    // ms is chosen since the resolution is at least 100ns, which gives us 10e-7 margin of error with division rounding worst case scenario
    pub fn tick_rate_ms() -> u64 {
        10_u64.pow(12) / Self::fs_per_tick()
    }

    pub fn num_timers() -> u64 {
        unsafe { ((Self::read(GENERAL_CAPABILITIES_REGISTER) >> 8) & 0xf) + 1 }
    }

    pub fn is_using_legacy_mapping() -> bool {
        unsafe { (Self::read(GENERAL_CONFIGURATION_REGISTER) >> 1) & 1 != 0 }
    }

    pub fn enable_legacy_mapping() {
        unsafe {
            let old = Self::read(GENERAL_CONFIGURATION_REGISTER);
            let new = old | LEG_RT_CNF;
            Self::write(GENERAL_CONFIGURATION_REGISTER, new);
        }
    }

    pub fn disable_legacy_mapping() {
        unsafe {
            let old = Self::read(GENERAL_CONFIGURATION_REGISTER);
            let new = old & !LEG_RT_CNF;
            Self::write(GENERAL_CONFIGURATION_REGISTER, new);
        }
    }

    pub fn is_enabled() -> bool {
        unsafe { Self::read(GENERAL_CONFIGURATION_REGISTER) & ENABLE_CNF != 0 }
    }
    pub fn is_disabled() -> bool {
        !Self::is_enabled()
    }

    pub fn enable() {
        unsafe {
            let old = Self::read(GENERAL_CONFIGURATION_REGISTER);
            let new = old | ENABLE_CNF;
            Self::write(GENERAL_CONFIGURATION_REGISTER, new);
        }
    }

    pub fn disable() {
        unsafe {
            let old = Self::read(GENERAL_CONFIGURATION_REGISTER);
            let new = old & !ENABLE_CNF;
            Self::write(GENERAL_CONFIGURATION_REGISTER, new);
        }
    }

    pub unsafe fn set_main_counter_raw_unchecked(val: u64) {
        unsafe {
            Self::write(MAIN_COUNTER_VAL_REGISTER, val);
        }
    }

    pub fn set_main_counter_raw(val: u64) {
        if Self::is_enabled() {
            panic!("ATTEMPTING TO SET THE MAIN COUNTER WHILE HPET IS ENABLED");
        } else {
            unsafe {
                Self::set_main_counter_raw_unchecked(val);
            }
        }
    }

    pub fn read_main_counter() -> u64 {
        unsafe { Self::read(MAIN_COUNTER_VAL_REGISTER) }
    }

    /// get a HPET timer
    /// ## Safety
    /// you must ensure that you're the sole owner of this timer.
    /// ## Panic
    /// Panics if
    /// ```rs
    /// num < Self::num_timers()
    /// ```
    pub unsafe fn timer(num: u64) -> Timer {
        assert!(num < Self::num_timers());
        Timer { num }
    }
}

pub struct Timer {
    num: u64,
}

impl Timer {
    fn general_capabilties_reg_num(&self) -> u64 {
        0x100 + 0x20 * self.num
    }

    fn comparator_value_reg_num(&self) -> u64 {
        0x108 + 0x20 * self.num
    }

    pub fn can_route_irq_to(&self, irq: u64) -> bool {
        unsafe { (Hpet::read(self.general_capabilties_reg_num()) >> 32) & (1 << irq) != 0 }
    }

    pub fn route_irq_to(&self, irq: u64) -> Option<u64> {
        if self.can_route_irq_to(irq) {
            assert!(irq <= 23);
            unsafe {
                let old = Hpet::read(self.general_capabilties_reg_num());
                let new = old | (irq << 9);
                Hpet::write(self.general_capabilties_reg_num(), new);
            }
            Some(irq)
        } else {
            None
        }
    }

    pub fn trigger_mode(&self) -> TriggerMode {
        if unsafe { (Hpet::read(self.general_capabilties_reg_num()) >> 1) & 1 != 0 } {
            TriggerMode::LevelSensetive
        } else {
            TriggerMode::EdgeSensetive
        }
    }

    pub fn enable(&self) {
        unsafe {
            let old = Hpet::read(self.general_capabilties_reg_num());
            let new = old | 0b100;
            Hpet::write(self.general_capabilties_reg_num(), new);
        }
    }

    pub fn disable(&self) {
        unsafe {
            let old = Hpet::read(self.general_capabilties_reg_num());
            let new = old & !0b100;
            Hpet::write(self.general_capabilties_reg_num(), new);
        }
    }

    pub fn set_counter_raw(&self, val: u64) {
        unsafe {
            Hpet::write(self.comparator_value_reg_num(), val);
        }
    }

    pub fn read_counter(&self) -> u64 {
        unsafe { Hpet::read(self.comparator_value_reg_num()) }
    }
}
