//! Scriptable HTTP/1.1 upstream on an ephemeral loopback port. It counts
//! every request whose body it fully read — the oracle for "did the
//! provider execute this" — and records when a client closed a stream.

#![allow(dead_code)]

use std::collections::VecDeque;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

/// One request as received.
#[derive(Debug, Clone)]
pub struct Seen {
    pub method: String,
    pub path: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Seen {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }
}

/// What to do with a request.
#[derive(Debug, Clone)]
pub enum Act {
    /// Respond. `chunks` are sent with chunked encoding, each after its delay.
    Respond {
        status: u16,
        headers: Vec<(String, String)>,
        chunks: Vec<(Duration, Vec<u8>)>,
    },
    /// Read the whole request, then drop the connection without a response.
    Reset,
    /// Wait before acting (headers are delayed too).
    Delay(Duration, Box<Act>),
}

impl Act {
    pub fn json(status: u16, body: &str) -> Act {
        Act::Respond {
            status,
            headers: vec![("content-type".into(), "application/json".into())],
            chunks: vec![(Duration::ZERO, body.as_bytes().to_vec())],
        }
    }

    pub fn sse(chunks: Vec<&[u8]>) -> Act {
        Act::Respond {
            status: 200,
            headers: vec![("content-type".into(), "text/event-stream".into())],
            chunks: chunks
                .into_iter()
                .map(|c| (Duration::ZERO, c.to_vec()))
                .collect(),
        }
    }

    pub fn with_header(mut self, k: &str, v: &str) -> Act {
        if let Act::Respond { headers, .. } = &mut self {
            headers.push((k.into(), v.into()));
        }
        self
    }
}

#[derive(Default)]
struct State {
    seen: Vec<Seen>,
    script: VecDeque<Act>,
    default: Option<Act>,
    closed_at: Vec<Instant>,
}

/// Handle to a running mock.
#[derive(Clone)]
pub struct Mock {
    pub addr: SocketAddr,
    state: Arc<Mutex<State>>,
}

impl Mock {
    pub async fn start(default: Act) -> Mock {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let state = Arc::new(Mutex::new(State {
            default: Some(default),
            ..Default::default()
        }));
        let st = state.clone();
        tokio::spawn(async move {
            loop {
                let Ok((s, _)) = listener.accept().await else {
                    return;
                };
                let st = st.clone();
                tokio::spawn(async move { conn(s, st).await });
            }
        });
        Mock { addr, state }
    }

    pub fn url(&self) -> String {
        format!("http://{}", self.addr)
    }

    /// Queues acts used before the default.
    pub fn push(&self, act: Act) {
        self.state.lock().unwrap().script.push_back(act);
    }

    /// Requests executed (body fully received).
    pub fn count(&self) -> usize {
        self.state.lock().unwrap().seen.len()
    }

    pub fn seen(&self) -> Vec<Seen> {
        self.state.lock().unwrap().seen.clone()
    }

    pub fn closed_at(&self) -> Vec<Instant> {
        self.state.lock().unwrap().closed_at.clone()
    }
}

fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

async fn conn(mut s: TcpStream, st: Arc<Mutex<State>>) {
    let mut buf: Vec<u8> = Vec::new();
    loop {
        // Headers.
        let head_end = loop {
            if let Some(i) = find(&buf, b"\r\n\r\n") {
                break i;
            }
            let mut tmp = [0u8; 8192];
            match s.read(&mut tmp).await {
                Ok(0) | Err(_) => return,
                Ok(n) => buf.extend_from_slice(&tmp[..n]),
            }
        };
        let head = String::from_utf8_lossy(&buf[..head_end]).into_owned();
        let mut lines = head.split("\r\n");
        let first: Vec<&str> = lines.next().unwrap_or("").split(' ').collect();
        let headers: Vec<(String, String)> = lines
            .filter_map(|l| l.split_once(':'))
            .map(|(k, v)| (k.trim().to_ascii_lowercase(), v.trim().to_string()))
            .collect();
        let len: usize = headers
            .iter()
            .find(|(k, _)| k == "content-length")
            .and_then(|(_, v)| v.parse().ok())
            .unwrap_or(0);
        buf.drain(..head_end + 4);
        while buf.len() < len {
            let mut tmp = [0u8; 65536];
            match s.read(&mut tmp).await {
                Ok(0) | Err(_) => return,
                Ok(n) => buf.extend_from_slice(&tmp[..n]),
            }
        }
        let body: Vec<u8> = buf.drain(..len).collect();
        let act = {
            let mut g = st.lock().unwrap();
            g.seen.push(Seen {
                method: first.first().unwrap_or(&"").to_string(),
                path: first.get(1).unwrap_or(&"").to_string(),
                headers,
                body,
            });
            g.script.pop_front().or_else(|| g.default.clone()).unwrap()
        };
        let act = match act {
            Act::Delay(d, inner) => {
                tokio::time::sleep(d).await;
                *inner
            }
            other => other,
        };
        match act {
            Act::Reset | Act::Delay(..) => return,
            Act::Respond {
                status,
                headers,
                chunks,
            } => {
                let mut head = format!("HTTP/1.1 {status} X\r\ntransfer-encoding: chunked\r\n");
                for (k, v) in &headers {
                    head.push_str(&format!("{k}: {v}\r\n"));
                }
                head.push_str("\r\n");
                if s.write_all(head.as_bytes()).await.is_err() {
                    st.lock().unwrap().closed_at.push(Instant::now());
                    return;
                }
                for (delay, c) in chunks {
                    if !delay.is_zero() {
                        tokio::time::sleep(delay).await;
                    }
                    let mut frame = format!("{:x}\r\n", c.len()).into_bytes();
                    frame.extend_from_slice(&c);
                    frame.extend_from_slice(b"\r\n");
                    if s.write_all(&frame).await.is_err() || s.flush().await.is_err() {
                        st.lock().unwrap().closed_at.push(Instant::now());
                        return;
                    }
                }
                if s.write_all(b"0\r\n\r\n").await.is_err() {
                    st.lock().unwrap().closed_at.push(Instant::now());
                    return;
                }
            }
        }
    }
}
