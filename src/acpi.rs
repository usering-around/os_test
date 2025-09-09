use acpi::AcpiTables;

use crate::{
    LIMINE_RSDP_REQUEST,
    memory::{
        physical::PhyAddr,
        virt::{GLOBAL_PAGE_ALLOCATOR, PageAllocation, PageAllocator, VirtAddr},
    },
};
use core::ptr::NonNull;

#[derive(Clone, Copy, Debug)]
pub struct AcpiTableHandler;
impl AcpiTableHandler {
    pub fn new() -> Self {
        Self {}
    }
}

impl acpi::AcpiHandler for AcpiTableHandler {
    unsafe fn map_physical_region<T>(
        &self,
        physical_address: usize,
        size: usize,
    ) -> acpi::PhysicalMapping<Self, T> {
        let page_amount = ((size
            + (GLOBAL_PAGE_ALLOCATOR.page_size() % core::mem::align_of::<T>()))
            / GLOBAL_PAGE_ALLOCATOR.page_size())
            + 1;
        let addr = PhyAddr(physical_address as u64);
        let (alloc, virt_addr) = unsafe {
            GLOBAL_PAGE_ALLOCATOR
                .map_physical(addr, page_amount)
                .expect("ACPI TABLES SHOULDN'T BE IN USABLE MEMORY")
        };
        let ptr = virt_addr
            .0
            .next_multiple_of(core::mem::align_of::<T>() as u64) as *mut T;
        let ptr = NonNull::new(ptr).unwrap();
        unsafe {
            acpi::PhysicalMapping::new(
                physical_address,
                ptr,
                size,
                alloc.page_amount * GLOBAL_PAGE_ALLOCATOR.page_size(),
                self.clone(),
            )
        }
    }

    fn unmap_physical_region<T>(region: &acpi::PhysicalMapping<Self, T>) {
        let allocation = PageAllocation::new(
            VirtAddr(region.virtual_start().as_ptr() as u64),
            region.mapped_length() / GLOBAL_PAGE_ALLOCATOR.page_size(),
        );
        unsafe {
            GLOBAL_PAGE_ALLOCATOR.dealloc_pages(&allocation);
        }
    }
}

pub fn tables() -> AcpiTables<AcpiTableHandler> {
    unsafe {
        let rsdp = LIMINE_RSDP_REQUEST.get_response().unwrap().address();
        let handler = crate::acpi::AcpiTableHandler::new();
        acpi::AcpiTables::from_rsdp(handler, rsdp).unwrap()
    }
}
