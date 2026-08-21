use std::io::{Read, Write};
use std::process::{Command, Stdio};

fn frame(value: &serde_json::Value) -> Vec<u8> {
    let body = serde_json::to_vec(value).unwrap();
    let mut out = format!("Content-Length: {}\r\n\r\n", body.len()).into_bytes();
    out.extend(body);
    out
}

fn read_frame(bytes: &[u8], cursor: &mut usize) -> serde_json::Value {
    let rest = &bytes[*cursor..];
    let header_end = rest.windows(4).position(|w| w == b"\r\n\r\n").unwrap();
    let header = std::str::from_utf8(&rest[..header_end]).unwrap();
    let length: usize = header
        .strip_prefix("Content-Length: ")
        .unwrap()
        .parse()
        .unwrap();
    let start = *cursor + header_end + 4;
    let end = start + length;
    *cursor = end;
    serde_json::from_slice(&bytes[start..end]).unwrap()
}

#[test]
fn lsp_binary_answers_initialize_and_shutdown_over_jsonrpc() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_bgraph-lsp"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut input = child.stdin.take().unwrap();
    input
        .write_all(&frame(&serde_json::json!({
            "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}
        })))
        .unwrap();
    input
        .write_all(&frame(&serde_json::json!({
            "jsonrpc": "2.0", "id": 2, "method": "shutdown", "params": null
        })))
        .unwrap();
    input
        .write_all(&frame(&serde_json::json!({
            "jsonrpc": "2.0", "id": 3, "method": "bgraph/unknown", "params": null
        })))
        .unwrap();
    input
        .write_all(&frame(&serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": { "textDocument": {
                "uri": "file:///tmp/contract.bd", "text": "node a"
            }}
        })))
        .unwrap();
    input
        .write_all(&frame(&serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didClose",
            "params": { "textDocument": { "uri": "file:///tmp/contract.bd" }}
        })))
        .unwrap();
    input
        .write_all(&frame(&serde_json::json!({
            "jsonrpc": "2.0", "method": "exit", "params": null
        })))
        .unwrap();
    drop(input);

    let mut output = Vec::new();
    child
        .stdout
        .take()
        .unwrap()
        .read_to_end(&mut output)
        .unwrap();
    assert!(child.wait().unwrap().success());

    let mut cursor = 0;
    let initialized = read_frame(&output, &mut cursor);
    assert_eq!(initialized["id"], 1);
    assert_eq!(
        initialized["result"]["capabilities"]["positionEncoding"][0],
        "utf-16"
    );
    assert_eq!(
        initialized["result"]["serverInfo"]["name"],
        "bgraph Language Server"
    );

    let shutdown = read_frame(&output, &mut cursor);
    assert_eq!(shutdown["id"], 2);
    assert!(shutdown["result"].is_null());

    let unknown = read_frame(&output, &mut cursor);
    assert_eq!(unknown["id"], 3);
    assert_eq!(unknown["error"]["code"], -32601);

    let opened = read_frame(&output, &mut cursor);
    assert_eq!(opened["method"], "textDocument/publishDiagnostics");
    assert_eq!(opened["params"]["uri"], "file:///tmp/contract.bd");
    assert!(opened["params"]["diagnostics"].is_array());

    let closed = read_frame(&output, &mut cursor);
    assert_eq!(closed["method"], "textDocument/publishDiagnostics");
    assert_eq!(closed["params"]["diagnostics"], serde_json::json!([]));
}
