use serde_json::{Value, json};

use super::positions::LineIndex;
use crate::check::{self, Severity};

/// A `file://` URI as a path, when it is one.
pub(crate) fn path_of(uri: &str) -> Option<std::path::PathBuf> {
    let rest = uri.strip_prefix("file://")?;
    let mut out = String::with_capacity(rest.len());
    let mut chars = rest.chars();
    while let Some(c) = chars.next() {
        if c != '%' {
            out.push(c);
            continue;
        }
        let hex: String = chars.by_ref().take(2).collect();
        match u8::from_str_radix(&hex, 16) {
            Ok(byte) => out.push(byte as char),
            Err(_) => {
                out.push('%');
                out.push_str(&hex);
            }
        }
    }
    Some(std::path::PathBuf::from(out))
}

pub(crate) fn diagnostics_for(text: &str, path: Option<&std::path::Path>) -> Vec<Value> {
    let composed = path
        .filter(|_| text.contains("include"))
        .and_then(|path| crate::compose::load_text(path, text).ok());

    let Some(composed) = composed else {
        return diagnostics(text);
    };

    let index = LineIndex::new(text);
    let diags = crate::analyze_diagram(&composed.diagram).1;

    let mut out = Vec::new();
    for diag in diags {
        let (owner, _, _) = composed.map.locate(diag.span.start);
        if owner != path.expect("a path was needed to compose") {
            continue;
        }
        let base = composed.map.base_of(diag.span.start);
        let local = crate::ast::Span {
            start: diag.span.start.saturating_sub(base),
            end: diag.span.end.saturating_sub(base),
        };
        out.push(json!({
            "range": index.range(local),
            "severity": if diag.severity == Severity::Error { 1 } else { 2 },
            "source": "bgraph",
            "message": diag.message,
        }));
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
