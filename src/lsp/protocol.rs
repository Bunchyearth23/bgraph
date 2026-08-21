use std::io::{BufRead, Write};

use serde_json::{Value, json};

pub(super) fn capabilities() -> Value {
    json!({
        "capabilities": {
            // Zed 1.15.1 deserializes the legacy singular field.  Sending
            // the newer array form makes initialization fail before the
            // server can advertise any other capability.
            "positionEncoding": "utf-16",
            "textDocumentSync": {
                "openClose": true,
                "change": 1,
                "save": { "includeText": false }
            },
            "completionProvider": {
                "triggerCharacters": [".", ":", "{"],
                "resolveProvider": false
            },
            "hoverProvider": true,
            "signatureHelpProvider": { "triggerCharacters": [" "], "retriggerCharacters": [" "] },
            "colorProvider": true,
            "documentHighlightProvider": true,
            "definitionProvider": true,
            "documentSymbolProvider": true,
        },
        "serverInfo": {
            "name": "bgraph Language Server",
            "version": env!("CARGO_PKG_VERSION"),
            "description": "Diagnostics, completion, hover and navigation for the bgraph diagram DSL."
        },
    })
}

pub(super) fn read_message(reader: &mut impl BufRead) -> std::io::Result<Option<Value>> {
    let mut length = None;
    loop {
        let mut header = String::new();
        if reader.read_line(&mut header)? == 0 {
            return Ok(None);
        }
        let header = header.trim_end();
        if header.is_empty() {
            break;
        }
        if let Some(value) = header
            .strip_prefix("Content-Length:")
            .or_else(|| header.strip_prefix("content-length:"))
        {
            length = value.trim().parse::<usize>().ok();
        }
    }

    let Some(length) = length else {
        return Ok(None);
    };
    let mut buffer = vec![0; length];
    reader.read_exact(&mut buffer)?;
    Ok(serde_json::from_slice(&buffer).ok())
}

pub(super) fn write_message(writer: &mut impl Write, message: &Value) -> std::io::Result<()> {
    let body = serde_json::to_vec(message)?;
    write!(writer, "Content-Length: {}\r\n\r\n", body.len())?;
    writer.write_all(&body)?;
    writer.flush()
}
