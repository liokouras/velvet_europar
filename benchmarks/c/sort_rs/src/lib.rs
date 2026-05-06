use std::slice;

#[unsafe(no_mangle)]
pub extern "C" fn rust_sort_i32(ptr: *mut i32, len: usize) {
    // Safety:
    // - ptr must be valid for `len` elements
    // - memory must be writable
    let slice = unsafe {
        slice::from_raw_parts_mut(ptr, len)
    };

    slice.sort();
}
