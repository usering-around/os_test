use core::fmt::Debug;

use crate::{
    arch_x86_64::cr3,
    memory::{
        physical::{PhyAddr, PhysicalAllocator},
        virt::VirtAddr,
    },
};

#[derive(Clone, Copy)]
pub struct PageTableEntry {
    entry: u64,
}

impl Debug for PageTableEntry {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{:?}", PageTableEntryFlags::from_bits_retain(self.entry))
    }
}

impl PageTableEntry {
    pub const fn default() -> Self {
        Self { entry: 0 }
    }
    pub const fn new() -> Self {
        Self::default()
    }

    pub const fn clear(&mut self) {
        self.entry = 0
    }

    pub const fn addr(&self) -> PhyAddr {
        PhyAddr(self.entry & Self::physical_address_mask())
    }
    const fn physical_address_mask() -> u64 {
        0x000f_ffff_ffff_f000u64
    }

    pub const fn set_flags(&mut self, flags: PageTableEntryFlags) {
        self.entry = self.addr().0 | flags.bits();
    }

    pub fn set_addr(&mut self, addr: PhyAddr, flags: PageTableEntryFlags) {
        assert!(addr.0.is_multiple_of(PAGE_SIZE));
        self.entry = addr.0 | flags.bits()
    }

    pub const fn flags(&self) -> PageTableEntryFlags {
        PageTableEntryFlags::from_bits_retain(self.entry & !Self::physical_address_mask())
    }

    /// Reintepret the entry as a PageTable. Panics if the entry is not present.
    /// ## Safety:
    /// The physical pointer must point to a valid address.
    /// Only use it on level4/level3/level2 page tables. Using it on a level1 page table is undefined behvaior
    /// (realistically, what would happen is you would interpret a page as a page table, which would be complete nonsense)
    pub unsafe fn as_page_table(&self) -> &PageTable {
        assert!(self.present());
        let ptr = self.addr().as_virtual().0 as *const PageTable;
        unsafe { ptr.as_ref().unwrap() }
    }

    /// Reintepret the entry as a PageTable. Panics if the entry is not present.
    /// ## Safety:
    /// The physical pointer must point to a valid address.
    /// Only use it on level4/level3/level2 page tables. Using it on a level1 page table is undefined behvaior
    /// (realistically, what would happen is you would interpret a page as a page table, which would be complete nonsense)
    pub unsafe fn as_page_table_mut(&mut self) -> &mut PageTable {
        let ptr = self.addr().as_virtual().0 as *mut PageTable;
        unsafe { ptr.as_mut().unwrap() }
    }

    pub fn present(&self) -> bool {
        self.flags().contains(PageTableEntryFlags::PRESENT)
    }
}

bitflags::bitflags! {
    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    pub struct PageTableEntryFlags: u64 {
        const PRESENT = 1;
        const WRITABLE = 1 << 1;
        const USER_ALLOWED = 1 << 2;
        const CACHE_WRITE_THROUGH = 1 << 3;
        const NO_CACHE = 1 << 4;
        const ACCESSED = 1 << 5;
        const DIRTY = 1 << 6;
        const HUGE_PAGE = 1 << 7;
        const GLOBAL = 1 << 8;
        const NO_EXECUTE = 1 << 63;
    }

}

pub const PAGE_TABLE_ENTRY_NUM: usize = 512;

#[repr(align(4096))]
pub struct PageTable {
    entries: [PageTableEntry; PAGE_TABLE_ENTRY_NUM],
}

#[derive(Clone, Copy, PartialEq, PartialOrd, Debug)]
pub struct Page {
    num: u64,
}

pub const PAGE_SIZE: u64 = 0x1000;

impl From<VirtAddr> for Page {
    fn from(value: VirtAddr) -> Self {
        assert!(value.is_valid());
        Page {
            num: value.0 / PAGE_SIZE,
        }
    }
}

impl From<Page> for VirtAddr {
    fn from(value: Page) -> Self {
        VirtAddr(value.num * PAGE_SIZE)
    }
}

impl Page {
    // addresses are only up to 48 bit; everything after that in the address should be either 1 or 0,
    // intel calls this canonical address. The Page abstraction doesn't have the notion of "canonical addresses",
    // thus in order to index the correct place in the page table, we need some kind of canonical page number.
    // this gives us it. Using & with this mask gives only the first 36 bits, which are the bits that matter
    // since the first 12 bits of the 64 bit address were deleted, giving a number with 52 bits
    // and we want to delete the last 16 bit of such number.
    const CANOINCAL_MASK: u64 = 0xfffffffff;
    const MAX_PAGE_CANOINCAL_NUM: usize =
        PAGE_TABLE_ENTRY_NUM * PAGE_TABLE_ENTRY_NUM * PAGE_TABLE_ENTRY_NUM * PAGE_TABLE_ENTRY_NUM;

    const fn canonical_num(&self) -> usize {
        (self.num & Self::CANOINCAL_MASK) as usize
    }

    pub const fn new(num: u64) -> Self {
        let this = Page { num };
        assert!(this.canonical_num() < Self::MAX_PAGE_CANOINCAL_NUM);
        this
    }
    pub fn num(&self) -> u64 {
        self.num
    }
    pub fn level4_idx(&self) -> usize {
        self.canonical_num() / (PAGE_TABLE_ENTRY_NUM * PAGE_TABLE_ENTRY_NUM * PAGE_TABLE_ENTRY_NUM)
    }

    pub fn level3_idx(&self) -> usize {
        (self.canonical_num() / (PAGE_TABLE_ENTRY_NUM * PAGE_TABLE_ENTRY_NUM))
            % PAGE_TABLE_ENTRY_NUM
    }
    pub fn level2_idx(&self) -> usize {
        (self.canonical_num() as usize / PAGE_TABLE_ENTRY_NUM) % PAGE_TABLE_ENTRY_NUM
    }
    pub fn level1_idx(&self) -> usize {
        self.canonical_num() as usize % PAGE_TABLE_ENTRY_NUM
    }

    pub fn next(&self) -> Option<Page> {
        self.next_by(1)
    }
    pub fn next_by(&self, num: u64) -> Option<Page> {
        if self.canonical_num() + (num as usize) < Self::MAX_PAGE_CANOINCAL_NUM {
            Some(Page {
                num: self.num + num,
            })
        } else {
            None
        }
    }
}

#[derive(Debug)]
pub struct PageIter {
    pub start: Page,
    pub end: Page,
}
impl PageIter {
    pub fn first(&self) -> Page {
        self.start
    }
    pub fn last_page(&self) -> Page {
        self.end
    }
}

impl Iterator for PageIter {
    type Item = Page;
    fn next(&mut self) -> Option<Self::Item> {
        if self.start <= self.end {
            let out = self.start;
            // this unwrap will panic if self.end == limit
            self.start = self.start.next().unwrap();
            Some(out)
        } else {
            None
        }
    }
}

#[derive(Debug)]
pub enum PageEntryError {
    HugePage,
    PageTableLevelIsNotPresent { level: usize },
}

impl PageTable {
    pub fn iter(&self) -> impl Iterator<Item = &PageTableEntry> {
        self.entries.iter()
    }

    pub unsafe fn clear_all_entries(&mut self) {
        for entry in self.entries.iter_mut() {
            entry.clear();
        }
    }

    /// ## safety:
    /// ensure that the current page table actually exists before calling this
    /// also, of course the lifetime isn't static; it is as long as the page table
    /// is not replaced. it is up to the caller to manage this lifetime.
    /// Lastly, there shouldn't be any thread holding a mutable refrence to this page table.
    /// Only the page allocator should use this method.
    pub unsafe fn current() -> &'static Self {
        let phy_addr = PhyAddr(cr3());
        let virt_addr = phy_addr.as_virtual();
        let page_table = virt_addr.0 as *const PageTable;
        unsafe { page_table.as_ref().unwrap() }
    }

    /// ## safety:
    /// ensure that the current page table actually exists before calling this
    /// also, of course the lifetime isn't static; it is as long as the page table
    /// is not replaced. it is up to the caller to manage this lifetime.
    /// Lastly, this refrence shouldn't be held by multiple threads/
    /// cpus at the same time.
    /// Only the page allocator should use this method.
    pub unsafe fn current_mut() -> &'static mut Self {
        let phy_addr = PhyAddr(cr3());
        let virt_addr = phy_addr.as_virtual();
        let page_table = virt_addr.0 as *mut PageTable;
        unsafe { page_table.as_mut().unwrap() }
    }

    pub fn page_entry(&self, page: Page) -> Result<&PageTableEntry, PageEntryError> {
        let page_dir_table_ptr_entry = self.entries[page.level4_idx()];
        let mut page_level = 4;
        if page_dir_table_ptr_entry.present() {
            page_level = 3;
            unsafe {
                let page_dir_entry =
                    page_dir_table_ptr_entry.as_page_table().entries[page.level3_idx()];

                if page_dir_entry.present() {
                    page_level = 2;
                    let page_table_entry =
                        page_dir_entry.as_page_table().entries[page.level2_idx()];
                    if page_table_entry
                        .flags()
                        .contains(PageTableEntryFlags::HUGE_PAGE)
                    {
                        return Err(PageEntryError::HugePage);
                    }
                    if page_table_entry.present() {
                        return Ok(page_table_entry
                            .as_page_table()
                            .entries
                            .get(page.level1_idx())
                            // this is safe since the lifetime of this is tied to the pagetable anyways
                            .map(|p| (p as *const PageTableEntry).as_ref().unwrap())
                            .unwrap());
                    }
                }
            }
        }
        Err(PageEntryError::PageTableLevelIsNotPresent { level: page_level })
    }

    pub fn page_entry_mut(&mut self, page: Page) -> Option<&mut PageTableEntry> {
        let page_dir_table_ptr_entry = self.entries.get_mut(page.level4_idx()).unwrap();

        if page_dir_table_ptr_entry.present() {
            unsafe {
                let page_dir_entry = page_dir_table_ptr_entry
                    .as_page_table_mut()
                    .entries
                    .get_mut(page.level3_idx())
                    .unwrap();

                if page_dir_entry.present() {
                    let page_table_entry = page_dir_entry
                        .as_page_table_mut()
                        .entries
                        .get_mut(page.level2_idx())
                        .unwrap();

                    if page_table_entry.present() {
                        return page_table_entry
                            .as_page_table_mut()
                            .entries
                            .get_mut(page.level1_idx())
                            // this is safe since the lifetime of this is tied to the pagetable anyways
                            .map(|p| (p as *mut PageTableEntry).as_mut().unwrap());
                    }
                }
            }
        }
        None
    }

    pub fn is_present(&self, page: Page) -> bool {
        match self.page_entry(page) {
            Ok(p) => p.present(),
            Err(PageEntryError::HugePage) => true,
            Err(PageEntryError::PageTableLevelIsNotPresent { .. }) => false,
        }
    }

    /// Maps a single page to a physical address. Assumes you have already allocated the memory necessary.    
    /// Note: may allocate level4/level3/level2/level1 page tables (this is why it takes a physical allocator).
    /// ## Safety:
    /// the PhysicalAllocator should be valid, and you are responsible for making sure you're not overriding some important page.
    /// Note: this does not check if the given page is already present/have some flags. You're responsible for that.
    pub unsafe fn map_page_unchecked(
        &mut self,
        page: Page,
        phy_addr: PhyAddr,
        flags: PageTableEntryFlags,
        phy_mem_alloc: &mut impl PhysicalAllocator,
    ) {
        assert!(phy_addr.0.is_multiple_of(PAGE_SIZE));
        let page_dir_ptr_table_entry = self.entries.get_mut(page.level4_idx()).unwrap();

        if !page_dir_ptr_table_entry.present() {
            let frame = unsafe { phy_mem_alloc.allocate_frame() };
            page_dir_ptr_table_entry.set_addr(frame, flags);
            unsafe {
                page_dir_ptr_table_entry
                    .as_page_table_mut()
                    .clear_all_entries();
            }
        }
        let page_dir_entry = unsafe {
            page_dir_ptr_table_entry
                .as_page_table_mut()
                .entries
                .get_mut(page.level3_idx())
                .unwrap()
        };

        if !page_dir_entry.present() {
            let frame = unsafe { phy_mem_alloc.allocate_frame() };
            page_dir_entry.set_addr(frame, flags);
            unsafe {
                page_dir_entry.as_page_table_mut().clear_all_entries();
            }
        }

        let page_table_entry = unsafe {
            page_dir_entry
                .as_page_table_mut()
                .entries
                .get_mut(page.level2_idx())
                .unwrap()
        };

        if !page_table_entry.present() {
            let frame = unsafe { phy_mem_alloc.allocate_frame() };
            page_table_entry.set_addr(frame, flags);
            unsafe {
                page_table_entry.as_page_table_mut().clear_all_entries();
            }
        }
        let page_entry = unsafe {
            page_table_entry
                .as_page_table_mut()
                .entries
                .get_mut(page.level1_idx())
                .unwrap()
        };
        page_entry.set_addr(phy_addr, flags);
    }

    pub fn find_free_pages(&self, start_page: Page, num_pages: usize) -> Option<PageIter> {
        // we don't start with num 0 for obvious reasons
        let mut first_page = start_page;
        let mut free_page_count = if self.is_present(first_page) { 0 } else { 1 };
        let mut page = first_page;
        while free_page_count < num_pages
            && let Some(next_page) = page.next()
        {
            if !self.is_present(next_page) {
                if free_page_count == 0 {
                    first_page = next_page;
                }
                free_page_count += 1;
            } else {
                free_page_count = 0;
            }
            page = next_page;
        }
        if free_page_count == num_pages {
            Some(PageIter {
                start: first_page,
                end: page,
            })
        } else {
            None
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::should_panic;

    #[test_case]
    fn page_to_idx() {
        let page = Page { num: 0 };
        assert_eq!(page.level4_idx(), 0);
        assert_eq!(page.level3_idx(), 0);
        assert_eq!(page.level2_idx(), 0);
        assert_eq!(page.level1_idx(), 0);
        let page = Page { num: 511 };
        assert_eq!(page.level4_idx(), 0);
        assert_eq!(page.level3_idx(), 0);
        assert_eq!(page.level2_idx(), 0);
        assert_eq!(page.level1_idx(), 511);
        let page = Page {
            num: PAGE_TABLE_ENTRY_NUM as u64,
        };
        assert_eq!(page.level4_idx(), 0);
        assert_eq!(page.level3_idx(), 0);
        assert_eq!(page.level2_idx(), 1);
        assert_eq!(page.level1_idx(), 0);
        let page = Page {
            num: (PAGE_TABLE_ENTRY_NUM as u64 * 5) + 27,
        };
        assert_eq!(page.level4_idx(), 0);
        assert_eq!(page.level3_idx(), 0);
        assert_eq!(page.level2_idx(), 5);
        assert_eq!(page.level1_idx(), 27);
        let page = Page {
            num: (PAGE_TABLE_ENTRY_NUM * PAGE_TABLE_ENTRY_NUM) as u64 + 112,
        };
        assert_eq!(page.level4_idx(), 0);
        assert_eq!(page.level3_idx(), 1);
        assert_eq!(page.level2_idx(), 0);
        assert_eq!(page.level1_idx(), 112);
        let page = Page {
            num: (PAGE_TABLE_ENTRY_NUM * PAGE_TABLE_ENTRY_NUM * 112) as u64
                + (PAGE_TABLE_ENTRY_NUM * 69) as u64
                + 378,
        };
        assert_eq!(page.level4_idx(), 0);
        assert_eq!(page.level3_idx(), 112);
        assert_eq!(page.level2_idx(), 69);
        assert_eq!(page.level1_idx(), 378);

        let page = Page {
            num: (PAGE_TABLE_ENTRY_NUM * PAGE_TABLE_ENTRY_NUM * PAGE_TABLE_ENTRY_NUM * 5) as u64
                + (PAGE_TABLE_ENTRY_NUM * PAGE_TABLE_ENTRY_NUM * 78) as u64
                + (PAGE_TABLE_ENTRY_NUM * 4) as u64
                + 33,
        };
        assert_eq!(page.level4_idx(), 5);
        assert_eq!(page.level3_idx(), 78);
        assert_eq!(page.level2_idx(), 4);
        assert_eq!(page.level1_idx(), 33);
    }

    #[test_case]
    fn page_to_virtual() {
        let page = Page { num: 1 };
        assert_eq!(VirtAddr::from(page), VirtAddr(0x1000));
        let page = Page { num: 513 };
        assert_eq!(VirtAddr::from(page), VirtAddr(0x201000));
        let page = Page {
            num: (PAGE_TABLE_ENTRY_NUM * PAGE_TABLE_ENTRY_NUM * 3) as u64
                + PAGE_TABLE_ENTRY_NUM as u64,
        };
        assert_eq!(VirtAddr::from(page), VirtAddr(0xc0200000));
        assert_eq!(
            VirtAddr::from(Page::from(VirtAddr(0xffffffffffffffff))),
            VirtAddr(0xfffffffffffff000)
        );
    }

    #[test_case]
    fn virtual_to_page() {
        let virt_addr = VirtAddr(0x1000);
        assert_eq!(Page::from(virt_addr), Page { num: 1 });
        let virt_addr = VirtAddr(0x201000);
        assert_eq!(Page::from(virt_addr), Page { num: 513 });
        let virt_addr = VirtAddr(0xc0200000);
        assert_eq!(
            Page::from(virt_addr),
            Page {
                num: (PAGE_TABLE_ENTRY_NUM * PAGE_TABLE_ENTRY_NUM * 3) as u64
                    + PAGE_TABLE_ENTRY_NUM as u64,
            }
        );
    }

    #[test_case]
    fn canoincal_pages() {
        fn canonical_counterpart(virt_addr: &VirtAddr) -> VirtAddr {
            let first_48_bits_mask = 0xffffffffffff;
            VirtAddr(virt_addr.0 & first_48_bits_mask)
        }
        let virt_addr = VirtAddr(0xffffffffffffffff);
        assert_ne!(
            Page::from(virt_addr),
            Page::from(canonical_counterpart(&virt_addr))
        );
        assert_eq!(
            Page::from(virt_addr).canonical_num(),
            Page::from(canonical_counterpart(&virt_addr)).canonical_num()
        );
    }

    #[test_case]
    fn addr_set() {
        unsafe {
            let page_table = PageTable::current_mut();
            let entry = page_table
                .entries
                .iter_mut()
                .find(|e| !e.present())
                .unwrap();

            let addr = PhyAddr(0x1000);
            entry.set_addr(addr, PageTableEntryFlags::PRESENT);
            assert!(entry.present());
            assert_eq!(entry.addr(), addr);
            let entry = page_table.entries.iter_mut().nth(3).unwrap();
            should_panic!();
            entry.set_addr(PhyAddr(0x123), PageTableEntryFlags::PRESENT);
        }
    }

    #[test_case]
    fn page_present() {
        unsafe {
            let page_table = PageTable::current_mut();
            let empty_page_dir_idx = page_table
                .entries
                .iter()
                .enumerate()
                .find(|(_, e)| !e.present())
                .map(|(i, _)| i)
                .unwrap();
            let page = Page {
                num: empty_page_dir_idx as u64
                    * PAGE_TABLE_ENTRY_NUM as u64
                    * PAGE_TABLE_ENTRY_NUM as u64
                    * PAGE_TABLE_ENTRY_NUM as u64
                    + 3,
            };
            assert!(!page_table.is_present(page));
            // need more through checking,
        }
    }
}
