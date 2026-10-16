use serde_json::{Value, json};

use super::positions::LineIndex;
use crate::check::{self, Severity};

/// A `file://` URI as a path, when it is one.
pub(crate) fn path_of(uri: &str) -> Option<std::path::PathBuf> {
    let rest = uri.strip_prefix("file://")?;
    let mut out = Vec::with_capacity(rest.len());
    let mut chars = rest.chars();
    while let Some(c) = chars.next() {
        if c != '%' {
            let mut encoded = [0; 4];
            out.extend_from_slice(c.encode_utf8(&mut encoded).as_bytes());
            continue;
        }
        let hex: String = chars.by_ref().take(2).collect();
        match u8::from_str_radix(&hex, 16) {
            Ok(byte) => out.push(byte),
            Err(_) => {
                out.push(b'%');
                out.extend_from_slice(hex.as_bytes());
            }
        }
    }
    String::from_utf8(out).ok().map(std::path::PathBuf::from)
}

pub(crate) fn diagnostics_for(text: &str, path: Option<&std::path::Path>) -> Vec<Value> {
    let Some(path) = path else {
        return diagnostics(text);
    };
    let Ok(composed) = crate::analyze_file_source(path, text) else {
        return diagnostics(text);
    };
    let entry = path
        .canonicalize()
        .or_else(|_| std::path::absolute(path))
        .unwrap_or_else(|_| path.to_path_buf());
    let index = LineIndex::new(text);
    let mut out = Vec::new();
    for diag in composed.analysis.diagnostics {
        let (owner, line, column) = composed.map.locate(diag.span.start);
        let base = composed.map.base_of(diag.span.start);
        let local = crate::ast::Span {
            start: diag.span.start.saturating_sub(base),
            end: diag.span.end.saturating_sub(base),
        };
        let (range, message, related) = if owner == entry {
            (index.range(local), diag.message, None)
        } else if let Some(include) = composed.map.include_span(&entry, diag.span.start) {
            let child = LineIndex::new(composed.map.source_of(diag.span.start));
            let uri: String = owner
                .to_string_lossy()
                .bytes()
                .map(|byte| {
                    if byte.is_ascii_alphanumeric() || b"/-._~:".contains(&byte) {
                        (byte as char).to_string()
                    } else {
                        format!("%{byte:02X}")
                    }
                })
                .collect();
            let message = format!(
                "{}:{}:{}: {}",
                owner.display(),
                line + 1,
                column + 1,
                diag.message
            );
            let related = json!([{
                "location": { "uri": format!("file://{uri}"), "range": child.range(local) },
                "message": diag.message,
            }]);
            (index.range(include), message, Some(related))
        } else {
            continue;
        };
        let mut diagnostic = json!({
            "range": range,
            "severity": if diag.severity == Severity::Error { 1 } else { 2 },
            "source": "bgraph",
            "message": message,
        });
        if let Some(related) = related {
            diagnostic["relatedInformation"] = related;
        }
        out.push(diagnostic);
    }

    for (span, message) in crate::template::lint(text) {
        out.push(json!({
            "range": index.range(span),
            "severity": 2,
            "source": "bgraph",
            "message": message,
        }));
    }
    out.sort_by_key(|diagnostic| {
        (
            diagnostic["range"]["start"]["line"]
                .as_u64()
                .unwrap_or_default(),
            diagnostic["range"]["start"]["character"]
                .as_u64()
                .unwrap_or_default(),
            diagnostic["severity"].as_u64().unwrap_or_default(),
        )
    });
    out
}

pub(crate) fn diagnostics(text: &str) -> Vec<Value> {
    let index = LineIndex::new(text);
    let mut diags = crate::analyze_source(text).diagnostics;

    for (span, message) in crate::template::lint(text) {
        diags.push(check::Diag {
            severity: Severity::Warning,
            message,
            span,
        });
    }
    diags.sort_by_key(|d| (d.span.start, d.span.end));
    diags
        .into_iter()
        .map(|d| {
            json!({
                "range": index.range(d.span),
                "severity": match d.severity { Severity::Error => 1, Severity::Warning => 2 },
                "source": "bgraph",
                "message": d.message,
            })
        })
        .collect()
}
