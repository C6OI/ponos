use crate::allocator::Locked;
use core::alloc::{GlobalAlloc, Layout};
use core::ptr;
use core::ptr::NonNull;

/// The block sizes to use.
///
/// The sizes must each be power of 2 because they are also used as
/// the block alignment (alignments must be always powers of 2).
const BLOCK_SIZES: &[usize] = &[8, 16, 32, 64, 128, 256, 512, 1024, 2048];

/// Choose an appropriate block size for the given `layout`.
///
/// Returns an index into the `BLOCK_SIZES` array.
fn list_index(layout: &Layout) -> Option<usize> {
    let required_block_size = layout.size().max(layout.align());
    BLOCK_SIZES.iter().position(|&s| s >= required_block_size)
}

struct ListNode {
    next: Option<&'static mut Self>,
}

pub struct FixedSizeBlockAllocator {
    list_heads: [Option<&'static mut ListNode>; BLOCK_SIZES.len()],
    fallback_allocator: linked_list_allocator::Heap,
}

impl FixedSizeBlockAllocator {
    /// Creates an empty `FixedSizeBlockAllocator`.
    #[must_use]
    pub const fn new() -> Self {
        const EMPTY: Option<&'static mut ListNode> = None;
        Self {
            list_heads: [EMPTY; BLOCK_SIZES.len()],
            fallback_allocator: linked_list_allocator::Heap::empty(),
        }
    }

    /// Initialize the allocator with the given heap bounds.
    ///
    /// # Safety
    /// This function is unsafe because the caller must guarantee that the given
    /// heap bounds are valid and that the heap is unused. This method must be
    /// called only once.
    pub unsafe fn init(&mut self, heap_start: usize, heap_size: usize) {
        unsafe {
            self.fallback_allocator.init(heap_start, heap_size);
        }
    }

    /// Allocates using the fallback allocator.
    fn fallback_alloc(&mut self, layout: Layout) -> *mut u8 {
        self.fallback_allocator
            .allocate_first_fit(layout)
            .map_or_else(|()| ptr::null_mut(), ptr::NonNull::as_ptr)
    }
}

impl Default for FixedSizeBlockAllocator {
    fn default() -> Self {
        Self::new()
    }
}

unsafe impl GlobalAlloc for Locked<FixedSizeBlockAllocator> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let mut allocator = self.lock();
        match list_index(&layout) {
            Some(index) => {
                if let Some(node) = allocator.list_heads[index].take() {
                    allocator.list_heads[index] = node.next.take();
                    ptr::from_mut::<ListNode>(node).cast::<u8>()
                } else {
                    // no block exists in list => allocate new block
                    let block_size = BLOCK_SIZES[index];
                    // only works if all block sizes are a power of 2
                    let block_align = block_size;
                    let layout = Layout::from_size_align(block_size, block_align).unwrap();
                    allocator.fallback_alloc(layout)
                }
            }
            None => allocator.fallback_alloc(layout),
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let mut allocator = self.lock();
        if let Some(index) = list_index(&layout) {
            let new_node = ListNode {
                next: allocator.list_heads[index].take(),
            };
            // verify that block has size and alignment required for storing node
            assert!(size_of::<ListNode>() <= BLOCK_SIZES[index]);
            assert!(align_of::<ListNode>() <= BLOCK_SIZES[index]);
            let new_node_ptr = ptr.cast::<ListNode>();
            unsafe {
                new_node_ptr.write(new_node);
                allocator.list_heads[index] = Some(&mut *new_node_ptr);
            }
        } else {
            let ptr = NonNull::new(ptr).unwrap();
            unsafe {
                allocator.fallback_allocator.deallocate(ptr, layout);
            }
        }
    }
}
