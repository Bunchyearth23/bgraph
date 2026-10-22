use serde_json::{Value, json};

pub(crate) use super::file_uri::path_of;
use super::{file_uri, positions::LineIndex};
use crate::check::{self, Severity};

#[cfg(test)]
pub(crate) fn diagnostics_for(text: &str, path: Option<&std::path::Path>) -> Vec<Value> {
    diagnostics_with_sources(text, path, &std::collections::HashMap::new())
}

pub(super) fn diagnostics_with_sources(
    text: &str,
    path: Option<&std::path::Path>,
    sources: &std::collections::HashMap<std::path::PathBuf, String>,
) -> Vec<Value> {
    let Some(path) = path else {
        return diagnostics(text);
    };
    let Ok(composed) = crate::compose::compose_with_sources(path, Some(text), sources)
        .and_then(|composed| crate::analyze_composed(composed, None))
    else {
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
            let message = format!(
                "{}:{}:{}: {}",
                owner.display(),
                line + 1,
                column + 1,
                diag.message
            );
            let related = json!([{
                "location": { "uri": file_uri::uri_of(owner), "range": child.range(local) },
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
