//! The wasm entry points: four `extern "C"` functions over linear memory.
//!
//! Hand-rolled for the same reason the SVG writer, the PDF writer and the HTTP
//! preview are: `wasm-bindgen` is a dependency *and* a build step, for four
//! functions whose entire job is to move bytes across a boundary. The crate
//! that keeps four dependencies does not take a fifth to pass a string.
//!
//! # The protocol
//!
//! Strings cross as `(pointer, length)` in the module's own memory.
//!
//! 1. `bgraph_alloc(n)` gives you `n` bytes; write the source into them.
//! 2. `bgraph_render(ptr, len)` returns a pointer to the result, and
//!    `bgraph_result_len()` says how long it is.
//! 3. `bgraph_free(ptr, n)` hands back what you allocated.
//!
//! The result is UTF-8: the SVG on success, or `error: …` on failure. One
//! channel rather than two, because a caller that has to check a second
//! function to know whether the first worked is a caller that will forget.
//!
//! # What it cannot do
//!
//! [`crate::render_source`] takes text and touches no filesystem, which is
//! exactly why it is the function exposed here (`D-029`). `include` and
//! `from:` both need a path to resolve against and are refused with a message
//! saying so — in a browser there is no disk to resolve them against, and
//! pretending otherwise would be worse than saying no.

use std::sync::Mutex;

/// The last result, kept alive until the next call.
///
/// A `static` rather than a returned length, because the caller has to read
/// the length *after* the pointer and a wasm function returns one value.
static RESULT: Mutex<Vec<u8>> = Mutex::new(Vec::new());

/// Reserve `len` bytes for the caller to write source into.
///
/// # Safety
///
/// The returned pointer is valid for `len` bytes until it is passed back to
/// [`bgraph_free`] with the same length.
#[unsafe(no_mangle)]
pub extern "C" fn bgraph_alloc(len: usize) -> *mut u8 {
    let mut buffer = Vec::with_capacity(len);
    let pointer = buffer.as_mut_ptr();
    std::mem::forget(buffer);
    pointer
}

/// Give back what [`bgraph_alloc`] handed out.
///
/// # Safety
///
/// `pointer` must have come from [`bgraph_alloc`] with the same `len`, and
/// must not be used afterwards.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bgraph_free(pointer: *mut u8, len: usize) {
    if !pointer.is_null() {
        drop(unsafe { Vec::from_raw_parts(pointer, 0, len) });
    }
}

/// Render the source at `pointer` and return a pointer to the result.
///
/// # Safety
///
/// `pointer` must point at `len` initialised bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bgraph_render(pointer: *const u8, len: usize) -> *const u8 {
    let bytes = unsafe { std::slice::from_raw_parts(pointer, len) };
    // Invalid UTF-8 is a caller mistake, not a reason to trap: a panic
    // crossing the wasm boundary takes the whole module down and the page
    // with it, which is the one thing this must never do.
    let source = String::from_utf8_lossy(bytes);

    let answer = match crate::render_source(&source) {
        Ok(svg) => svg,
        Err(err) => format!("error: {err:#}"),
    };

    let mut result = RESULT.lock().unwrap_or_else(|e| e.into_inner());
    *result = answer.into_bytes();
    result.as_ptr()
}

/// How long the last [`bgraph_render`] result is.
#[unsafe(no_mangle)]
pub extern "C" fn bgraph_result_len() -> usize {
    RESULT.lock().map(|r| r.len()).unwrap_or(0)
}
