pub mod allocator;
pub mod paging;
pub mod physical;
pub mod virt;

pub fn init() {
    virt::init();
}
