//! Custom LVGL allocator backend (`lv_malloc_core` / `lv_free_core` etc.)
//!
//! `lv_mem.c` provides the public API (`lv_malloc`, `lv_free`, …) and
//! forwards the real work to `lv_malloc_core`, `lv_free_core`, … which we
//! implement here — routing every LVGL allocation to PSRAM via
//! `esp_alloc::HEAP`.  This avoids exhaustion of the ~512 kB internal DRAM
//! that C `malloc` draws from.
//!
//! # Allocation header
//!
//! C `free(void*)` carries no size, but Rust's `GlobalAlloc::dealloc`
//! requires a `Layout`.  We store a `usize` header immediately before the
//! returned pointer:
//!
//! ```text
//!  ┌──────────┬──────────────────────────────┐
//!  │ size (4B)│         user data             │
//!  ├──────────┤                              │
//!  │ *header  │ *returned ptr                 │
//!  └──────────┴──────────────────────────────┘
//! ```

use core::alloc::{GlobalAlloc, Layout};
use core::ffi::c_void;
use core::ptr;

/// Alignment for every LVGL allocation (matches `LV_ATTRIBUTE_MEM_ALIGN = 4`).
const ALIGN: usize = 4;

/// Header stored before the user pointer (just the allocation size).
const HEADER: usize = core::mem::size_of::<usize>();

// ---------------------------------------------------------------------------
// lv_malloc_core
// ---------------------------------------------------------------------------

#[unsafe(no_mangle)]
pub unsafe extern "C" fn lv_malloc_core(size: usize) -> *mut c_void {
    if size == 0 {
        return ptr::null_mut();
    }
    let total = match size.checked_add(HEADER) {
        Some(n) => n,
        None => return ptr::null_mut(),
    };
    let layout = unsafe { Layout::from_size_align_unchecked(total, ALIGN) };
    let block = unsafe { esp_alloc::HEAP.alloc(layout) };
    if block.is_null() {
        return ptr::null_mut();
    }
    unsafe { ptr::write(block as *mut usize, size) };
    unsafe { block.add(HEADER) as *mut c_void }
}

// ---------------------------------------------------------------------------
// lv_free_core
// ---------------------------------------------------------------------------

#[unsafe(no_mangle)]
pub unsafe extern "C" fn lv_free_core(ptr: *mut c_void) {
    if ptr.is_null() {
        return;
    }
    let user = ptr as *mut u8;
    let header = unsafe { user.sub(HEADER) };
    let size = unsafe { ptr::read(header as *mut usize) };
    let total = size + HEADER;
    let layout = unsafe { Layout::from_size_align_unchecked(total, ALIGN) };
    unsafe { esp_alloc::HEAP.dealloc(header, layout) };
}

// ---------------------------------------------------------------------------
// lv_realloc_core
// ---------------------------------------------------------------------------

#[unsafe(no_mangle)]
pub unsafe extern "C" fn lv_realloc_core(ptr: *mut c_void, new_size: usize) -> *mut c_void {
    if new_size == 0 {
        unsafe { lv_free_core(ptr) };
        return ptr::null_mut();
    }
    if ptr.is_null() {
        return unsafe { lv_malloc_core(new_size) };
    }

    let user = ptr as *mut u8;
    let header = unsafe { user.sub(HEADER) };
    let old_size = unsafe { ptr::read(header as *mut usize) };

    let new_ptr = unsafe { lv_malloc_core(new_size) };
    if new_ptr.is_null() {
        return ptr::null_mut();
    }

    let copy = if old_size < new_size { old_size } else { new_size };
    unsafe { ptr::copy_nonoverlapping(ptr, new_ptr, copy) };
    unsafe { lv_free_core(ptr) };

    new_ptr
}

// ---------------------------------------------------------------------------
// lv_mem_init / lv_mem_deinit
// ---------------------------------------------------------------------------

#[unsafe(no_mangle)]
pub extern "C" fn lv_mem_init() {}

#[unsafe(no_mangle)]
pub extern "C" fn lv_mem_deinit() {}

// ---------------------------------------------------------------------------
// lv_mem_monitor_core  (stub)
// ---------------------------------------------------------------------------

#[unsafe(no_mangle)]
pub extern "C" fn lv_mem_monitor_core(_mon_p: *mut c_void) {}

// ---------------------------------------------------------------------------
// lv_mem_test_core  (stub — always ok)
// ---------------------------------------------------------------------------

#[unsafe(no_mangle)]
pub extern "C" fn lv_mem_test_core() -> i32 {
    1 // LV_RESULT_OK
}

// ---------------------------------------------------------------------------
// lv_mem_add_pool / lv_mem_remove_pool  (stubs — not supported)
// ---------------------------------------------------------------------------

#[unsafe(no_mangle)]
pub extern "C" fn lv_mem_add_pool(_mem: *mut c_void, _bytes: usize) -> *mut c_void {
    ptr::null_mut()
}

#[unsafe(no_mangle)]
pub extern "C" fn lv_mem_remove_pool(_pool: *mut c_void) {}
