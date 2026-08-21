use serde_json::{Value, json};

use crate::ast::Span;

/// Maps byte offsets to LSP positions (zero-based line, UTF-16 column).
pub struct LineIndex {
    /// Byte offset of the start of each line.
    starts: Vec<usize>,
    text: String,
}

impl LineIndex {
    pub fn new(text: &str) -> LineIndex {
        let mut starts = vec![0];
        for (i, b) in text.bytes().enumerate() {
            if b == b'\n' {
                starts.push(i + 1);
            }
        }
        LineIndex {
            starts,
            text: text.to_string(),
        }
    }

    pub fn line_col(&self, offset: usize) -> (usize, usize) {
        let offset = offset.min(self.text.len());
        let line = match self.starts.binary_search(&offset) {
            Ok(exact) => exact,
            Err(next) => next - 1,
        };
        let col = self.text[self.starts[line]..offset]
            .chars()
            .map(char::len_utf16)
            .sum();
        (line, col)
    }

    pub fn offset(&self, line: usize, col: usize) -> usize {
        let Some(&start) = self.starts.get(line) else {
            return self.text.len();
        };
        let end = self
            .starts
            .get(line + 1)
            .copied()
            .unwrap_or(self.text.len());

        let mut utf16 = 0;
        for (i, c) in self.text[start..end].char_indices() {
            if utf16 >= col {
                return start + i;
            }
            utf16 += c.len_utf16();
        }
        end
    }

    pub(super) fn range(&self, span: Span) -> Value {
        let (sl, sc) = self.line_col(span.start);
        let (el, ec) = self.line_col(span.end);
        json!({
            "start": { "line": sl, "character": sc },
            "end":   { "line": el, "character": ec },
        })
    }
}
