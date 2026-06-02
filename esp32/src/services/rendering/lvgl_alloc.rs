use core::alloc::{GlobalAlloc, Layout};
use core::ffi::c_void;
use core::ptr;

const ALIGN: usize = 4;

const CANARY_VAL: usize = 0xDEAD_BEEFusize;

// Layout: [hdr_canary(4) | size(4) | user_data(size) | foot_canary(4)]
//           block+0         block+4    block+8          block+8+size
const HDR_CANARY: usize = 0;
const HDR_SIZE: usize = 4;
const OFFSET_USER: usize = 8;

const OVERHEAD: usize = 12;

/// Round up to nearest multiple of 4 for footer canary alignment.
const fn aligned(n: usize) -> usize {
    (n + 3) & !3
}

fn footer_off(size: usize) -> usize {
    OFFSET_USER + aligned(size)
}

unsafe fn write_canaries(block: *mut u8, size: usize) {
    unsafe {
        ptr::write(block.add(HDR_CANARY) as *mut u32, CANARY_VAL as u32);
        ptr::write(block.add(HDR_SIZE) as *mut usize, size);
        ptr::write(block.add(footer_off(size)) as *mut u32, CANARY_VAL as u32);
    }
}

fn check_canaries(block: *mut u8, size: usize) {
    let h = unsafe { ptr::read(block.add(HDR_CANARY) as *const u32) };
    if h != CANARY_VAL as u32 {
        panic!("LVGL canary fail: header at {:p}", block);
    }
    let f = unsafe { ptr::read(block.add(footer_off(size)) as *const u32) };
    if f != CANARY_VAL as u32 {
        panic!("LVGL canary fail: footer at {:p}", unsafe {
            block.add(footer_off(size))
        });
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn lv_malloc_core(size: usize) -> *mut c_void {
    if size == 0 {
        return ptr::null_mut();
    }
    let total = match size.checked_add(OVERHEAD) {
        Some(n) => n,
        None => return ptr::null_mut(),
    };
    let layout = unsafe { Layout::from_size_align_unchecked(total, ALIGN) };
    let block = unsafe { esp_alloc::HEAP.alloc(layout) };
    if block.is_null() {
        return ptr::null_mut();
    }
    unsafe {
        write_canaries(block, size);
        block.add(OFFSET_USER) as *mut c_void
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn lv_free_core(ptr: *mut c_void) {
    unsafe {
        if ptr.is_null() {
            return;
        }
        let block = (ptr as *mut u8).sub(OFFSET_USER);
        let size = ptr::read(block.add(HDR_SIZE) as *const usize);
        check_canaries(block, size);
        let total = size + OVERHEAD;
        let layout = Layout::from_size_align_unchecked(total, ALIGN);
        esp_alloc::HEAP.dealloc(block, layout);
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn lv_realloc_core(ptr: *mut c_void, new_size: usize) -> *mut c_void {
    unsafe {
        if new_size == 0 {
            lv_free_core(ptr);
            return ptr::null_mut();
        }
        if ptr.is_null() {
            return lv_malloc_core(new_size);
        }

        let block = (ptr as *mut u8).sub(OFFSET_USER);
        let old_size = ptr::read(block.add(HDR_SIZE) as *const usize);
        check_canaries(block, old_size);

        let new_ptr = lv_malloc_core(new_size);
        if new_ptr.is_null() {
            return ptr::null_mut();
        }

        let copy = if old_size < new_size {
            old_size
        } else {
            new_size
        };
        ptr::copy_nonoverlapping(ptr, new_ptr, copy);
        lv_free_core(ptr);

        new_ptr
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn lv_mem_init() {}

#[unsafe(no_mangle)]
pub extern "C" fn lv_mem_deinit() {}

#[unsafe(no_mangle)]
pub extern "C" fn lv_mem_monitor_core(_mon_p: *mut c_void) {}

#[unsafe(no_mangle)]
pub extern "C" fn lv_mem_test_core() -> i32 {
    1
}

#[unsafe(no_mangle)]
pub extern "C" fn lv_mem_add_pool(_mem: *mut c_void, _bytes: usize) -> *mut c_void {
    ptr::null_mut()
}

#[unsafe(no_mangle)]
pub extern "C" fn lv_mem_remove_pool(_pool: *mut c_void) {}
