use revier_analysis::overlay::line_diff::{diff_lines, LineDiffPart};
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

struct CountingAllocator;

static CURRENT_BYTES: AtomicUsize = AtomicUsize::new(0);
static PEAK_BYTES: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ptr = System.alloc(layout);
        if !ptr.is_null() {
            add_live_bytes(layout.size());
        }
        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        System.dealloc(ptr, layout);
        subtract_live_bytes(layout.size());
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let new_ptr = System.realloc(ptr, layout, new_size);
        if !new_ptr.is_null() {
            if new_size > layout.size() {
                add_live_bytes(new_size - layout.size());
            } else {
                subtract_live_bytes(layout.size() - new_size);
            }
        }
        new_ptr
    }
}

#[global_allocator]
static GLOBAL: CountingAllocator = CountingAllocator;

#[test]
fn builds_asymmetric_disjoint_diff_with_bounded_peak_live_memory() {
    let old_text = numbered_lines("old", 2_048);
    let new_text = numbered_lines("new", 8_192);

    let baseline = CURRENT_BYTES.load(Ordering::SeqCst);
    PEAK_BYTES.store(baseline, Ordering::SeqCst);
    let parts = diff_lines(&old_text, &new_text);
    let peak_delta = PEAK_BYTES.load(Ordering::SeqCst) - baseline;

    assert_eq!(parts.len(), 2);
    assert!(matches!(parts[0], LineDiffPart::Removed(_)));
    assert!(matches!(parts[1], LineDiffPart::Added(_)));
    assert!(
        peak_delta < 1_500_000,
        "行级 diff 峰值存活内存不应保留递归 score buffer，实际峰值增量 {peak_delta} 字节"
    );
}

fn add_live_bytes(size: usize) {
    let current = CURRENT_BYTES.fetch_add(size, Ordering::SeqCst) + size;
    let mut peak = PEAK_BYTES.load(Ordering::SeqCst);
    while current > peak {
        match PEAK_BYTES.compare_exchange(peak, current, Ordering::SeqCst, Ordering::SeqCst) {
            Ok(_) => break,
            Err(next_peak) => peak = next_peak,
        }
    }
}

fn subtract_live_bytes(size: usize) {
    CURRENT_BYTES.fetch_sub(size, Ordering::SeqCst);
}

fn numbered_lines(prefix: &str, count: usize) -> String {
    (0..count)
        .map(|index| format!("{prefix}-{index}\n"))
        .collect::<String>()
}
