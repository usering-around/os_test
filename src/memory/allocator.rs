use core::alloc::GlobalAlloc;

use crate::memory::{
    paging::Page,
    physical::BasicPhysicalAllocator,
    virt::{BasicPageAllocator, GLOBAL_PAGE_ALLOCATOR, PageAllocation, PageAllocator, VirtAddr},
};

// TODO: Make a proper allocator instead of using the virtual page allocator
#[global_allocator]
static GLOBAL_ALLOCATOR: Allocator<BasicPageAllocator<BasicPhysicalAllocator>> = Allocator {
    page_allocator: &GLOBAL_PAGE_ALLOCATOR,
};

struct Allocator<T: PageAllocator + 'static> {
    page_allocator: &'static T,
}

unsafe impl<T: PageAllocator> GlobalAlloc for Allocator<T> {
    unsafe fn alloc(&self, layout: core::alloc::Layout) -> *mut u8 {
        let page_amount = ((layout.size() + (self.page_allocator.page_size() % layout.align()))
            / self.page_allocator.page_size())
            + 1;
        unsafe {
            let Some(allocation) = self.page_allocator.alloc_pages(page_amount) else {
                return core::ptr::null_mut::<u8>();
            };
            allocation
                .as_virt_addr()
                .0
                .next_multiple_of(layout.align() as u64) as *mut u8
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: core::alloc::Layout) {
        let page_amount = ((layout.size() + (self.page_allocator.page_size() % layout.align()))
            / self.page_allocator.page_size())
            + 1;
        unsafe {
            let allocation = PageAllocation {
                first_page: Page::from(VirtAddr(ptr as u64)),
                page_amount,
            };
            self.page_allocator.dealloc_pages(&allocation);
        }
    }
}

#[cfg(test)]
mod test {
    use alloc::{boxed::Box, vec};

    #[test_case]
    fn basic_alloc() {
        let mut vec = vec!["hello", "wassup", "bitch"];

        vec.push("value");
        vec.push("value");
        vec[0] = "test";
        vec[4] = "test";
        drop(vec);
        let ok = Box::new(str::from_utf8("what is up guyz this is my string".as_bytes()).unwrap());
        ok.chars().find(|&c| c == 'z');
        drop(ok);
    }

    #[test_case]
    fn big_alloc() {
        let mut big_box = Box::new([0; 10000]);
        big_box[0] = 1;
        big_box[big_box.len() - 1] = -3321;
    }
}
