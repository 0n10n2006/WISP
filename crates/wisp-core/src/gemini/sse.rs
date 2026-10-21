//! Minimal incremental Server-Sent Events parser for `alt=sse` streams.

#[derive(Default)]
pub struct SseParser {
    buf: String,
}

impl SseParser {
    pub fn new() -> Self {
        Self::default()
    }

    /// Feed raw bytes; returns the `data` payload of every complete event.
    pub fn push(&mut self, chunk: &[u8]) -> Vec<String> {
        self.buf.push_str(&String::from_utf8_lossy(chunk));
        if self.buf.contains('\r') {
            self.buf = self.buf.replace("\r\n", "\n");
        }
        let mut out = Vec::new();
        while let Some(idx) = self.buf.find("\n\n") {
            let event: String = self.buf.drain(..idx + 2).collect();
            if let Some(data) = parse_event(&event) {
                out.push(data);
            }
        }
        out
    }

    /// Flush a trailing event that was not terminated by a blank line.
    pub fn finish(&mut self) -> Option<String> {
        let rest = std::mem::take(&mut self.buf);
        parse_event(&rest)
    }
}

fn parse_event(event: &str) -> Option<String> {
    let data: Vec<&str> = event
        .lines()
        .filter_map(|l| l.strip_prefix("data:"))
        .map(|d| d.strip_prefix(' ').unwrap_or(d))
        .collect();
    if data.is_empty() {
        None
    } else {
        Some(data.join("\n"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_events_split_across_chunks() {
        let mut p = SseParser::new();
        assert!(p.push(b"data: {\"a\":").is_empty());
        let out = p.push(b"1}\n\ndata: {\"b\":2}\r\n\r\ndata: {\"c\"");
        assert_eq!(out, vec!["{\"a\":1}", "{\"b\":2}"]);
        assert!(p.push(b":3}").is_empty());
        assert_eq!(p.finish().as_deref(), Some("{\"c\":3}"));
    }

    #[test]
    fn ignores_comments_and_multiline_data() {
        let mut p = SseParser::new();
        let out = p.push(b": keepalive\n\nevent: x\ndata: line1\ndata: line2\n\n");
        assert_eq!(out, vec!["line1\nline2"]);
    }
}
