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

/// How long the last result is.
#[unsafe(no_mangle)]
pub extern "C" fn bgraph_result_len() -> usize {
    RESULT.lock().map(|r| r.len()).unwrap_or(0)
}

/// Check the source at `pointer` and return the diagnostics as JSON.
///
/// Separate from [`bgraph_render`] because they answer different questions: a
/// file can render *and* have four warnings, and a page that only knew whether
/// the render failed could not show them. Positions come out as line and
/// column, one-based, which is what a reader counts in and what an editor
/// gutter wants.
///
/// # Safety
///
/// `pointer` must point at `len` initialised bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bgraph_check(pointer: *const u8, len: usize) -> *const u8 {
    let bytes = unsafe { std::slice::from_raw_parts(pointer, len) };
    let source = String::from_utf8_lossy(bytes);

    let mut out = String::from("[");
    match crate::parser::parse_diagram(&source) {
        Ok(diagram) => {
            let index = crate::lsp::LineIndex::new(&source);
            let mut diagnostics = crate::check::check(&diagram);
            if !diagnostics
                .iter()
                .any(|diag| diag.severity == crate::check::Severity::Error)
                && let Ok(layout) = crate::layout::solve(&diagram)
            {
                diagnostics.extend(crate::check::after_layout(&diagram, &layout));
            }
            for (i, diag) in diagnostics.iter().enumerate() {
                let (line, col) = index.line_col(diag.span.start);
                if i > 0 {
                    out.push(',');
                }
                let severity = match diag.severity {
                    crate::check::Severity::Error => "error",
                    crate::check::Severity::Warning => "warning",
                };
                out.push_str(&format!(
                    "{{\"severity\":\"{severity}\",\"line\":{},\"col\":{},\"message\":{}}}",
                    line + 1,
                    col + 1,
                    json_string(&diag.message)
                ));
            }
        }
        // A parse error has no `Diag` to carry it — it is the reason there is
        // no diagram to check — so it arrives here as the one diagnostic there
        // can be, rather than as an empty list that reads like "all well".
        Err(err) => out.push_str(&format!(
            "{{\"severity\":\"error\",\"line\":0,\"col\":0,\"message\":{}}}",
            json_string(&format!("{err:#}"))
        )),
    }
    out.push(']');

    let mut result = RESULT.lock().unwrap_or_else(|e| e.into_inner());
    *result = out.into_bytes();
    result.as_ptr()
}

/// Every worked example the binary carries, as JSON.
///
/// The page's genre menu is the crate's own `EXAMPLES` table rather than a
/// copy kept beside it — the same argument that made the reference manual
/// generated. A playground offering a genre the module cannot render, or
/// missing one it can, is a playground that lies about the thing it exists to
/// demonstrate.
#[unsafe(no_mangle)]
pub extern "C" fn bgraph_examples() -> *const u8 {
    let mut out = String::from("[");
    for (i, (name, source)) in crate::examples::EXAMPLES.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str(&format!(
            "{{\"name\":{},\"source\":{}}}",
            json_string(name),
            json_string(source)
        ));
    }
    out.push(']');

    let mut result = RESULT.lock().unwrap_or_else(|e| e.into_inner());
    *result = out.into_bytes();
    result.as_ptr()
}

/// A JSON string literal, escaped by hand for the same reason everything else
/// here is: pulling a serialiser in to quote four fields would cost more than
/// it saves.
fn json_string(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

// ------------------------------------------------------------ Typst plugins

/// Typst calls a plugin through a protocol of its own, and it is not the one
/// above: the host writes the arguments into the module's memory itself and
/// takes the result through a function it provides, so an exported function
/// receives only *lengths* and returns only a status.
///
/// A second entry point rather than a second module. The four functions above
/// are the general shape — a page, a Markdown preview, anything that can hold
/// a pointer — and this is the shape one particular host insists on.
///
/// Behind its own feature, and it has to be: these are *imports*, so a module
/// built with them cannot be instantiated by anything that does not supply
/// them — the playground and the Markdown preview both pass an empty import
/// object, and would simply fail to load. Two modules rather than one that
/// only one host can start.
#[cfg(all(target_arch = "wasm32", feature = "typst"))]
mod typst {
    #[link(wasm_import_module = "typst_env")]
    unsafe extern "C" {
        /// Typst writes the concatenated arguments here.
        fn wasm_minimal_protocol_write_args_to_buffer(ptr: *mut u8);
        /// And takes the answer back through this.
        fn wasm_minimal_protocol_send_result_to_host(ptr: *const u8, len: usize);
    }

    /// Render `len` bytes of source. Returns 0 for a diagram, 1 for an error,
    /// with the message sent back the same way — Typst shows the body either
    /// way, so a failure explains itself in the page rather than leaving a
    /// hole whose cause is three files away.
    #[unsafe(no_mangle)]
    pub extern "C" fn bgraph_typst_render(len: usize) -> i32 {
        let mut source = vec![0u8; len];
        unsafe { wasm_minimal_protocol_write_args_to_buffer(source.as_mut_ptr()) };

        // Lossy rather than fatal: a panic crossing the boundary takes the
        // document build down, which is never the right answer to bad bytes.
        let text = String::from_utf8_lossy(&source);
        let (status, answer) = match crate::render_source(&text) {
            Ok(svg) => (0, svg),
            Err(err) => (1, format!("error: {err:#}")),
        };

        let bytes = answer.into_bytes();
        unsafe { wasm_minimal_protocol_send_result_to_host(bytes.as_ptr(), bytes.len()) };
        status
    }
}
