pub mod bump;
pub mod fixed_size_block;
pub mod linked_list;

use crate::allocator::fixed_size_block::FixedSizeBlockAllocator;
use alloc::alloc::{GlobalAlloc, Layout};
use core::ptr::null_mut;
use x86_64::{
    structures::paging::{
        mapper::MapToError, FrameAllocator, Mapper, Page, PageTableFlags, Size4KiB,
    },
    VirtAddr,
};

pub const HEAP_START: usize = 0x_4444_4444_0000;
pub const HEAP_SIZE: usize = 100 * 1024; // 100 KiB

#[global_allocator]
static ALLOCATOR: Locked<FixedSizeBlockAllocator> = Locked::new(FixedSizeBlockAllocator::new());

pub struct Dummy;

unsafe impl GlobalAlloc for Dummy {
    unsafe fn alloc(&self, _layout: Layout) -> *mut u8 {
        null_mut()
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {
        panic!("dealloc should be never called")
    }
}

/// A wrapper around `spin::Mutex` to permit trait implementations.
pub struct Locked<A> {
    inner: spin::Mutex<A>,
}

impl<A> Locked<A> {
    pub const fn new(inner: A) -> Self {
        Self {
            inner: spin::Mutex::new(inner),
        }
    }

    pub fn lock(&'_ self) -> spin::MutexGuard<'_, A> {
        self.inner.lock()
    }
}

pub fn init_heap(
    mapper: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) -> Result<(), MapToError<Size4KiB>> {
    let page_range = {
        let heap_start = VirtAddr::new(HEAP_START as u64);
        let heap_end = heap_start + HEAP_SIZE as u64 - 1u64;
        let heap_start_page = Page::containing_address(heap_start);
        let heap_end_page = Page::containing_address(heap_end);
        Page::range_inclusive(heap_start_page, heap_end_page)
    };

    for page in page_range {
        let frame = frame_allocator
            .allocate_frame()
            .ok_or(MapToError::FrameAllocationFailed)?;
        let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;
        unsafe { mapper.map_to(page, frame, flags, frame_allocator)?.flush() };
    }

    unsafe {
        ALLOCATOR.lock().init(HEAP_START, HEAP_SIZE);
    }

    Ok(())
}

/// Align the given address `addr` upwards to alignment `align`.
///
/// Requires that `align` is a power of two.
///
/// # Remarks
/// Since `align` is a power of two, its binary representation has only a single bit set (e.g. `0b000100000`).
/// This means that `align - 1` has all the lower bits set (e.g. `0b00011111`).<br/>
/// By creating the bitwise `NOT` through the `!` operator, we get a number that has all the bits set
/// except for the bits lower than `align` (e.g. `0b…111111111100000`).<br/>
/// By performing a bitwise `AND` on an address and `!(align - 1)`, we align the address downwards.
/// This works by clearing all the bits that are lower than `align`.<br/>
/// Since we want to align upwards instead of downwards, we increase the `addr` by `align - 1`
/// before performing the bitwise `AND`.
/// This way, already aligned addresses remain the same while non-aligned addresses are rounded
/// to the next alignment boundary.
const fn align_up(addr: usize, align: usize) -> usize {
    (addr + align - 1) & !(align - 1)
}
