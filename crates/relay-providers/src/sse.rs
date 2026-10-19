//! Incremental, bounded Server-Sent Events scanner. It only reads; the bytes
//! it sees are forwarded unchanged by the caller.
//!
//! Lines longer than the cap are skipped (and counted) rather than buffered
//! without bound. Data for events the caller does not want is not copied.

use memchr::memchr;

/// Largest line kept for parsing.
pub const MAX_LINE: usize = 1024 * 1024;

/// Scanner state.
#[derive(Debug, Default)]
pub struct Scanner {
    partial: Vec<u8>,
    skipping: bool,
    event: String,
    data: Vec<u8>,
    keep_data: bool,
    has_data: bool,
    /// Lines dropped for exceeding [`MAX_LINE`].
    pub overflowed: u32,
}

impl Scanner {
    /// Feeds a chunk. `wants(event)` says whether the event's data is
    /// needed (`event` is empty when the stream sends no `event:` line).
    /// `on_event(event, data)` runs once per complete event.
    pub fn feed(
        &mut self,
        mut chunk: &[u8],
        wants: &impl Fn(&str) -> bool,
        on_event: &mut impl FnMut(&str, &[u8]),
    ) {
        while !chunk.is_empty() {
            match memchr(b'\n', chunk) {
                Some(i) => {
                    let (line, rest) = (&chunk[..i], &chunk[i + 1..]);
                    chunk = rest;
                    if self.skipping {
                        self.skipping = false;
                        self.partial.clear();
                        continue;
                    }
                    if self.partial.is_empty() {
                        self.line(line, wants, on_event);
                    } else {
                        let mut buf = std::mem::take(&mut self.partial);
                        buf.extend_from_slice(line);
                        self.line(&buf, wants, on_event);
                        buf.clear();
                        self.partial = buf;
                    }
                }
                None => {
                    if !self.skipping {
                        if self.partial.len() + chunk.len() > MAX_LINE {
                            self.skipping = true;
                            self.overflowed += 1;
                            self.partial.clear();
                        } else {
                            self.partial.extend_from_slice(chunk);
                        }
                    }
                    return;
                }
            }
        }
    }

    /// Flushes a final event not followed by a blank line.
    pub fn finish(&mut self, on_event: &mut impl FnMut(&str, &[u8])) {
        if !self.skipping && !self.partial.is_empty() {
            let buf = std::mem::take(&mut self.partial);
            self.line(&buf, &|_: &str| true, on_event);
        }
        if self.has_data {
            on_event(&self.event, &self.data);
        }
        self.reset_event();
    }

    fn reset_event(&mut self) {
        self.event.clear();
        self.data.clear();
        self.keep_data = false;
        self.has_data = false;
    }

    fn line(
        &mut self,
        line: &[u8],
        wants: &impl Fn(&str) -> bool,
        on_event: &mut impl FnMut(&str, &[u8]),
    ) {
        let line = line.strip_suffix(b"\r").unwrap_or(line);
        if line.is_empty() {
            if self.has_data {
                on_event(&self.event, &self.data);
            }
            self.reset_event();
            return;
        }
        if let Some(v) = line.strip_prefix(b"event:") {
            self.event.clear();
            self.event
                .push_str(std::str::from_utf8(v).unwrap_or("").trim());
            return;
        }
        if let Some(v) = line.strip_prefix(b"data:") {
            if !self.has_data {
                self.keep_data = wants(&self.event);
            }
            self.has_data = true;
            if self.keep_data {
                if !self.data.is_empty() {
                    self.data.push(b'\n');
                }
                self.data
                    .extend_from_slice(v.strip_prefix(b" ").unwrap_or(v));
            }
        }
        // Comments (`:`), `id:` and `retry:` are ignored.
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn collect(chunks: &[&[u8]]) -> Vec<(String, String)> {
        let mut s = Scanner::default();
        let mut out = Vec::new();
        for c in chunks {
            s.feed(c, &|e: &str| e != "skip", &mut |e: &str, d: &[u8]| {
                out.push((e.to_string(), String::from_utf8_lossy(d).into_owned()))
            });
        }
        s.finish(&mut |e: &str, d: &[u8]| {
            out.push((e.to_string(), String::from_utf8_lossy(d).into_owned()))
        });
        out
    }

    #[test]
    fn events_split_across_arbitrary_chunks() {
        let stream =
            b"event: a\ndata: {\"x\":1}\n\n: comment\nevent: b\r\ndata: one\r\ndata: two\r\n\r\n";
        let whole = collect(&[stream]);
        assert_eq!(
            whole,
            vec![
                ("a".into(), "{\"x\":1}".into()),
                ("b".into(), "one\ntwo".into())
            ]
        );
        // Every split point gives the same events.
        for i in 0..stream.len() {
            assert_eq!(
                collect(&[&stream[..i], &stream[i..]]),
                whole,
                "split at {i}"
            );
        }
    }

    #[test]
    fn unwanted_data_is_not_kept_but_event_still_fires() {
        let out = collect(&[b"event: skip\ndata: big\n\n"]);
        assert_eq!(out, vec![("skip".into(), String::new())]);
    }

    #[test]
    fn overlong_lines_are_skipped_and_counted() {
        let mut s = Scanner::default();
        let big = vec![b'x'; MAX_LINE + 10];
        let mut events = 0;
        s.feed(
            b"event: e\ndata: ",
            &|_: &str| true,
            &mut |_: &str, _: &[u8]| events += 1,
        );
        s.feed(&big, &|_: &str| true, &mut |_: &str, _: &[u8]| events += 1);
        s.feed(
            b"\n\nevent: f\ndata: ok\n\n",
            &|_: &str| true,
            &mut |_: &str, _: &[u8]| events += 1,
        );
        assert_eq!(s.overflowed, 1);
        assert_eq!(events, 1);
    }

    #[test]
    fn trailing_event_without_blank_line_flushes_on_finish() {
        assert_eq!(
            collect(&[b"data: tail"]),
            vec![(String::new(), "tail".into())]
        );
    }
}
