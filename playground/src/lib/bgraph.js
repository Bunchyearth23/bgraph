// The wasm module, wrapped in the three calls a page actually makes.
//
// The boundary is a handful of `extern "C"` functions over linear memory —
// hand-rolled in `src/wasm.rs` rather than generated, so this file is the
// other half of that contract and the only place in the app that knows a
// pointer exists.

let api = null
let loading = null

const encoder = new TextEncoder()
const decoder = new TextDecoder()

/// Instantiate once, and hand every later caller the same promise.
export function load(url = new URL('./bgraph.wasm', document.baseURI).href) {
  if (loading) return loading
  loading = (async () => {
    const response = await fetch(url)
    if (!response.ok) throw new Error(`${response.status} fetching ${url}`)
    const { instance } = await WebAssembly.instantiate(
      await response.arrayBuffer(),
      {},
    )
    api = instance.exports
    return api
  })()
  return loading
}

export function ready() {
  return api !== null
}

// A fresh view every time: the module's memory may have grown since the last
// call, and a view onto a detached buffer throws rather than quietly reading
// the wrong bytes.
function result(at) {
  return decoder.decode(new Uint8Array(api.memory.buffer, at, api.bgraph_result_len()))
}

function withSource(text, call) {
  const bytes = encoder.encode(text)
  const pointer = api.bgraph_alloc(bytes.length)
  new Uint8Array(api.memory.buffer, pointer, bytes.length).set(bytes)
  try {
    return result(call(pointer, bytes.length))
  } finally {
    api.bgraph_free(pointer, bytes.length)
  }
}

/// The SVG, or a string starting `error:`.
///
/// One channel, so there is no second call to forget: an error announces
/// itself in the text rather than in a status nobody checks.
export function render(text) {
  return withSource(text, (p, n) => api.bgraph_render(p, n))
}

/// Every diagnostic with one-based lines and UTF-16 columns, like JS offsets.
///
/// A file can render *and* have four warnings, which is why this is not folded
/// into `render`.
export function check(text) {
  return JSON.parse(withSource(text, (p, n) => api.bgraph_check(p, n)))
}

/// The worked examples the module itself carries.
///
/// Not a copy kept beside it: a playground offering a genre the module cannot
/// render is a playground that lies about the thing it exists to show.
export function examples() {
  return JSON.parse(result(api.bgraph_examples()))
}
