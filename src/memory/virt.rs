use core::fmt::Debug;
use limine::memory_map::EntryType;
use spin::Mutex;

use crate::{
    LIMINE_MEMORY_MAP,
    arch_x86_64::invlpg,
    memory::{
        paging::{PAGE_SIZE, Page, PageIter, PageTable, PageTableEntryFlags},
        physical::{BasicPhysicalAllocator, PhyAddr, PhysicalAllocator},
    },
};

// TODO:
// Create a better memory allocator, the current one simply searches for a contigous set of pages in the page table,
// and then save the last page it allocated, and searches from that place the next time.
// If we fill up the entire memory space, it will fail. NEED TO FIX THIS!

/// The kernel's global page allocator.
pub static GLOBAL_PAGE_ALLOCATOR: BasicPageAllocator<BasicPhysicalAllocator> =
    BasicPageAllocator::new_const();

pub fn init() {
    let usable_mem = LIMINE_MEMORY_MAP
        .get_response()
        .unwrap()
        .entries()
        .iter()
        .filter(|e| e.entry_type == EntryType::USABLE)
        .max_by_key(|e| e.length)
        .unwrap();
    crate::console_println!(
        "base mem: 0x{:x}, size: {} bytes, max_mem: {}",
        usable_mem.base,
        usable_mem.length,
        LIMINE_MEMORY_MAP
            .get_response()
            .unwrap()
            .entries()
            .iter()
            .filter(|e| e.entry_type == EntryType::USABLE)
            .map(|e| e.length)
            .sum::<u64>()
    );
    unsafe {
        GLOBAL_PAGE_ALLOCATOR.configure_physical_area(PhyAddr(usable_mem.base), usable_mem.length);
    }
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct VirtAddr(pub u64);

impl VirtAddr {
    // x86_64 can usually address 48 bits (we can extend), so some virtual addresses
    // are considered invalid, or rather formally "not canonical". This checks for it
    pub fn is_valid(&self) -> bool {
        let last_16_bits = self.0 >> 48;
        last_16_bits == 0xffff || last_16_bits == 0
    }
}

impl Debug for VirtAddr {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "0x{:x}", self.0)
    }
}

pub struct PageAllocation {
    pub first_page: Page,
    pub page_amount: usize,
}

impl PageAllocation {
    /// Get a new page allocation from the amount of pages and an address in the first page.
    pub fn new(virt_addr: VirtAddr, page_amount: usize) -> Self {
        Self {
            first_page: Page::from(virt_addr),
            page_amount,
        }
    }

    /// Get the virtual address of the first page in the allocation.
    pub fn as_virt_addr(&self) -> VirtAddr {
        VirtAddr::from(self.first_page)
    }
}
pub trait PageAllocator {
    /// Allocate page_amount of pages. Returns None if it is not possible to allocate them.
    unsafe fn alloc_pages(&self, page_amount: usize) -> Option<PageAllocation>;
    /// Deallocate an allocation.
    unsafe fn dealloc_pages(&self, alloc: &PageAllocation);
    /// Map a physical address to some amount of pages. Allocates at least page_amount * self.page_size()
    /// amount of memory after the address. Returns the allocation along with virtual address which corresponds to the physical one.
    /// Note: the physical address need not be aligned, and the given PageAllocation may be bigger than page_amount.
    unsafe fn map_physical(
        &self,
        addr: PhyAddr,
        page_amount: usize,
    ) -> Option<(PageAllocation, VirtAddr)>;

    fn page_size(&self) -> usize {
        PAGE_SIZE as usize
    }
}

pub struct BasicPageAllocator<T: PhysicalAllocator> {
    pub inner: Mutex<BasicPageAllocatorInner<T>>,
}
pub struct BasicPageAllocatorInner<T: PhysicalAllocator> {
    pub physical_allocator: T,
    last_page_alloc: Page,
}

impl BasicPageAllocator<BasicPhysicalAllocator> {
    pub const fn new_const() -> Self {
        BasicPageAllocator {
            inner: Mutex::new(BasicPageAllocatorInner {
                physical_allocator: unsafe { BasicPhysicalAllocator::init(PhyAddr(0)) },
                last_page_alloc: Page::new(1),
            }),
        }
    }

    unsafe fn configure_physical_area(&self, start: PhyAddr, size: u64) {
        unsafe {
            let phy_alloc = &mut self.inner.lock().physical_allocator;
            phy_alloc.set_offset(start);
            *phy_alloc.limit_mut() = size;
        }
    }
}

impl<T: PhysicalAllocator> PageAllocator for BasicPageAllocator<T> {
    unsafe fn alloc_pages(&self, page_amount: usize) -> Option<PageAllocation> {
        let mut inner = self.inner.lock();
        // safety: we have mutual exclusion over other threads since we locked ourselves
        // and this is only (or at least should be only) accessed by the page allocator.
        let page_table = unsafe { PageTable::current_mut() };

        let Some(free_pages) =
            page_table.find_free_pages(inner.last_page_alloc, page_amount as usize)
        else {
            return None;
        };

        let first_page = free_pages.first();
        inner.last_page_alloc = free_pages.last_page();
        for page in free_pages {
            unsafe {
                let frame = inner.physical_allocator.allocate_frame();
                page_table.map_page_unchecked(
                    page,
                    frame,
                    PageTableEntryFlags::PRESENT | PageTableEntryFlags::WRITABLE,
                    &mut inner.physical_allocator,
                );
                invlpg(VirtAddr::from(page).0);
            }
        }

        Some(PageAllocation {
            first_page,
            page_amount,
        })
    }

    unsafe fn dealloc_pages(&self, alloc: &PageAllocation) {
        let pages_to_free = PageIter {
            start: alloc.first_page,
            end: alloc
                .first_page
                .next_by(alloc.page_amount as u64 - 1)
                .unwrap(),
        };

        let mut inner = self.inner.lock();
        // safety: we have mutual exclusion due to locking ourselves and the page table should only be accessed by us.
        let page_table = unsafe { PageTable::current_mut() };
        for page in pages_to_free {
            unsafe {
                let page_entry = page_table.page_entry_mut(page).unwrap();
                inner.physical_allocator.free_frame(page_entry.addr());
                page_entry.clear();
                invlpg(VirtAddr::from(page).0);
            }
        }
    }

    unsafe fn map_physical(
        &self,
        addr: PhyAddr,
        page_amount: usize,
    ) -> Option<(PageAllocation, VirtAddr)> {
        let mut inner = self.inner.lock();
        unsafe {
            let aligned = addr.align_down(T::frame_size() as usize);
            let align_offset = addr.0 - aligned.0;
            let add_page = if align_offset == 0 { 0 } else { 1 };
            let page_amount = page_amount + add_page;
            let Some(phy_addr) = inner
                .physical_allocator
                .alloc_phy_addr(aligned, page_amount)
            else {
                return None;
            };
            // safety: mutual exlcusion via inner, only the page allocator has access to the page table
            let page_table = PageTable::current_mut();
            let Some(pages) = page_table.find_free_pages(inner.last_page_alloc, page_amount) else {
                let mut addr = phy_addr;
                for _ in 0..page_amount {
                    inner.physical_allocator.free_frame(addr);
                    addr.0 += T::frame_size();
                }
                return None;
            };
            let first_page = pages.first();

            let mut phy_addr = phy_addr;
            for page in pages {
                page_table.map_page_unchecked(
                    page,
                    phy_addr,
                    PageTableEntryFlags::PRESENT | PageTableEntryFlags::WRITABLE,
                    &mut inner.physical_allocator,
                );
                if phy_addr.0 == 0xfee00000 {
                    for i in 0..self.page_size() {
                        let byte = (VirtAddr::from(page).0 + i as u64) as *mut u8;
                        *byte = 0;
                    }
                }

                phy_addr.0 += T::frame_size();
            }

            Some((
                PageAllocation {
                    first_page,
                    page_amount,
                },
                VirtAddr(VirtAddr::from(first_page).0 + align_offset),
            ))
        }
    }
}
