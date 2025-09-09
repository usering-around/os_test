use core::fmt::Debug;

use crate::{HIGHER_HALF_DIRECT_MAP, memory::virt::VirtAddr};

// Todo: the current align_up/down methods are bad,
// we align to the power of 2 all the time anyways, we should use something like:
// #define ALIGN_PAGE_FRAME_UPPER(addr) (((addr) + PAGE_FRAME_SIZE - 1) & (~(PAGE_FRAME_SIZE - 1))) // Page align an address. Upper means it aligns upwards

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct PhyAddr(pub u64);

impl Debug for PhyAddr {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "0x{:x}", self.0)
    }
}

impl PhyAddr {
    pub fn as_virtual(&self) -> VirtAddr {
        VirtAddr(self.0 + HIGHER_HALF_DIRECT_MAP.get_response().unwrap().offset())
    }

    pub fn align_up(&self, alignment: usize) -> PhyAddr {
        let ok = self.0 % alignment as u64;
        if ok == 0 {
            PhyAddr(self.0)
        } else {
            PhyAddr(self.0 - ok + alignment as u64)
        }
    }

    pub fn align_down(&self, alignment: usize) -> PhyAddr {
        let ok = self.0 % alignment as u64;
        PhyAddr(self.0 - ok)
    }
}

//TODO: rework this mess, a bunch of unsafe where it's probably not necessary,
// this bitmap allocator is horribly designed

/// A physical allocator is simply a struct which configures itself
/// based on contigous usable physical memory, and is able to
/// give and free physical memory.
pub unsafe trait PhysicalAllocator {
    /// allocate one singular frame
    unsafe fn allocate_frame(&mut self) -> PhyAddr;
    /// free a frame
    unsafe fn free_frame(&mut self, frame: PhyAddr);

    /// allocate a frames contigously at a specific address. Returns None if the address is already allocated.
    /// Address must be aligned to Self::frame_size()
    unsafe fn alloc_phy_addr(&mut self, phy_addr: PhyAddr, frame_count: usize) -> Option<PhyAddr>;
    // frame size in bytes
    fn frame_size() -> u64;
}

const BITMAP_SIZE: usize = 8388608;
pub struct BasicPhysicalAllocator {
    // can handle up to 32GiB of ram
    bitmap: *mut [bool; BITMAP_SIZE],
    offset: PhyAddr,
    limit: u64,
}

static mut BITMAP: [bool; BITMAP_SIZE] = [false; BITMAP_SIZE];

impl BasicPhysicalAllocator {
    /// create a BasicPhysicalAllocator
    /// due to reasons (BAD DESIGN) calling this more than one time is unsafe
    /// and hence this function is marked as unsafe.
    /// ## Safety:
    /// DO NOT CREATE MULTIPLE BASIC PHYSICAL ALLOCATORS!
    pub const unsafe fn init(offset: PhyAddr) -> Self {
        BasicPhysicalAllocator {
            // safety: this function should only be called once and hence
            // this bitmap is only owned by one singular BasicPhysicalAllocator
            bitmap: &raw mut BITMAP,
            offset,
            limit: 0,
        }
    }

    pub unsafe fn set_offset(&mut self, offset: PhyAddr) {
        assert!(offset.0.is_multiple_of(Self::frame_size()));
        self.offset = offset;
    }

    pub unsafe fn limit_mut(&mut self) -> &mut u64 {
        &mut self.limit
    }
}

/// safety: you need unsafe to use the pointer anyways
unsafe impl Send for BasicPhysicalAllocator {}

unsafe impl PhysicalAllocator for BasicPhysicalAllocator {
    unsafe fn allocate_frame(&mut self) -> PhyAddr {
        let bitmap = unsafe { self.bitmap.as_mut().unwrap() };
        if let Some(index) = bitmap
            .iter()
            .enumerate()
            .find(|&(_i, &b)| b == false)
            .map(|(i, _)| i)
        {
            bitmap[index] = true;

            PhyAddr((index as u64 * Self::frame_size()) + self.offset.0)
        } else {
            PhyAddr(0)
        }
    }

    unsafe fn free_frame(&mut self, frame: PhyAddr) {
        if frame.0 % Self::frame_size() != 0 {
            panic!("WHAT IS THIS ALIGNMENT?! ptr: {:x}", frame.0);
        }
        let bitmap = unsafe { self.bitmap.as_mut().unwrap() };

        let index = (frame.0 - self.offset.0) / Self::frame_size();
        if bitmap[index as usize] == false {
            panic!("DBG: ATTEMPTING TO DEALLOCATE ALLOCATED MEMORY???");
        }
        bitmap[index as usize] = false;
    }

    unsafe fn alloc_phy_addr(&mut self, phy_addr: PhyAddr, frame_count: usize) -> Option<PhyAddr> {
        if phy_addr.0 % Self::frame_size() != 0 {
            panic!("bad alignment. ptr: {:?}", phy_addr);
        }

        let bitmap = unsafe { self.bitmap.as_mut().unwrap() };
        let index = (phy_addr.0 - self.offset.0) / Self::frame_size();
        for i in 0..frame_count {
            if bitmap[index as usize + i] == true {
                return None;
            }
        }

        for i in 0..frame_count {
            bitmap[index as usize + i] = true;
        }
        Some(phy_addr)
    }
    fn frame_size() -> u64 {
        4096
    }
}

#[cfg(test)]
mod test {
    use super::*;
    #[test_case]
    fn align_up() {
        let addr = PhyAddr(0);
        assert_eq!(addr.align_up(1), addr);
        let addr = PhyAddr(0x2);
        assert_eq!(addr.align_up(0x4), PhyAddr(0x4));
        let addr = PhyAddr(0xdeadbeef);
        assert_eq!(addr.align_up(0x1000), PhyAddr(0xdeadc000))
    }

    #[test_case]
    fn align_down() {
        let addr = PhyAddr(1);
        assert_eq!(addr.align_down(1), addr);
        let addr = PhyAddr(0x4);
        assert_eq!(addr.align_up(0x2), PhyAddr(0x4));
        let addr = PhyAddr(0xdeadbeef);
        assert_eq!(addr.align_down(0x1000), PhyAddr(0xdeadb000));
    }
}
