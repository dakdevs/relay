//! Proxy overhead against a local SSE upstream: the same load direct and
//! through Relay, reporting latency percentiles and the difference.
//!
//! Run: `cargo bench -p relay-proxy --bench overhead`. Loopback only; it
//! touches no user configuration (temp dir, ephemeral ports).

use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use hyper::service::service_fn;
use hyper_util::client::legacy::Client;
use hyper_util::client::legacy::connect::HttpConnector;
use hyper_util::rt::{TokioExecutor, TokioIo};
use relay_core::config::{AccountConfig, Config};
use relay_core::model::{AccountId, AccountMode, Provider};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

const SSE: &[u8] = b"event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"usage\":{\"input_tokens\":10,\"output_tokens\":1}}}\n\n\
event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"delta\":{\"type\":\"text_delta\",\"text\":\"hello\"}}\n\n\
event: message_delta\ndata: {\"type\":\"message_delta\",\"usage\":{\"output_tokens\":5}}\n\n\
event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n";

async fn upstream() -> SocketAddr {
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = l.local_addr().unwrap();
    tokio::spawn(async move {
        loop {
            let Ok((s, _)) = l.accept().await else { return };
            let _ = s.set_nodelay(true);
            tokio::spawn(async move {
                let svc = service_fn(|req: hyper::Request<hyper::body::Incoming>| async move {
                    // Read the whole body like a real upstream.
                    let _ = req.into_body().collect().await;
                    let mut r = hyper::Response::new(Full::new(Bytes::from_static(SSE)));
                    r.headers_mut()
                        .insert("content-type", "text/event-stream".parse().unwrap());
                    r.headers_mut().insert(
                        "anthropic-ratelimit-unified-5h-utilization",
                        "0.3".parse().unwrap(),
                    );
                    Ok::<_, std::convert::Infallible>(r)
                });
                let _ = hyper::server::conn::http1::Builder::new()
                    .serve_connection(TokioIo::new(s), svc)
                    .await;
            });
        }
    });
    addr
}

struct Stats {
    ttfb: Vec<Duration>,
    total: Vec<Duration>,
}

fn pct(v: &mut [Duration], p: f64) -> Duration {
    v.sort();
    v[((v.len() as f64 - 1.0) * p).round() as usize]
}

async fn load(
    c: &Client<HttpConnector, Full<Bytes>>,
    url: &str,
    headers: &[(String, String)],
    body: Bytes,
    concurrency: usize,
    per_worker: usize,
) -> Stats {
    let mut tasks = Vec::new();
    for _ in 0..concurrency {
        let c = c.clone();
        let url = url.to_string();
        let headers = headers.to_vec();
        let body = body.clone();
        tasks.push(tokio::spawn(async move {
            let mut ttfb = Vec::new();
            let mut total = Vec::new();
            for _ in 0..per_worker {
                let mut b = hyper::Request::post(&url);
                for (k, v) in &headers {
                    b = b.header(k, v);
                }
                let t = Instant::now();
                let r = c
                    .request(b.body(Full::new(body.clone())).unwrap())
                    .await
                    .unwrap();
                ttfb.push(t.elapsed());
                let bytes = r.into_body().collect().await.unwrap().to_bytes();
                total.push(t.elapsed());
                assert_eq!(&bytes[..], SSE, "byte-identical stream");
            }
            (ttfb, total)
        }));
    }
    let mut s = Stats {
        ttfb: vec![],
        total: vec![],
    };
    for t in tasks {
        let (a, b) = t.await.unwrap();
        s.ttfb.extend(a);
        s.total.extend(b);
    }
    s
}

fn row(name: &str, direct: &mut Stats, proxied: &mut Stats) {
    let us = |d: Duration| d.as_secs_f64() * 1e6;
    for (metric, d, p) in [
        ("first byte", &mut direct.ttfb, &mut proxied.ttfb),
        ("complete", &mut direct.total, &mut proxied.total),
    ] {
        let (d50, d99) = (pct(d, 0.5), pct(d, 0.99));
        let (p50, p99) = (pct(p, 0.5), pct(p, 0.99));
        println!(
            "| {name} | {metric} | {:.0} | {:.0} | {:.0} | {:.0} | {:+.0} | {:+.0} |",
            us(d50),
            us(d99),
            us(p50),
            us(p99),
            us(p50) - us(d50),
            us(p99) - us(d99)
        );
    }
}

fn main() {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .enable_all()
        .build()
        .unwrap();
    rt.block_on(async {
        let up = upstream().await;
        let dir = tempfile::tempdir().unwrap();
        let paths = relay_proxy::paths::Paths::in_dir(dir.path());
        let mut cfg = Config::default();
        cfg.server.listen = "127.0.0.1:0".parse().unwrap();
        cfg.upstreams.allowed.push(format!("http://{up}"));
        cfg.accounts.push(AccountConfig {
            id: AccountId("bench".into()),
            provider: Provider::Anthropic,
            vendor: None,
            device_id: None,
            label: "Bench".into(),
            mode: AccountMode::Observed,
            plan: None,
            identifier: None,
            credential_kind: None,
            upstream: Some(format!("http://{up}")),
            credential: None,
            paused: false,
            max_in_flight: 1024,
            models: vec![],
        });
        let handle = Arc::new(relay_proxy::start(&paths, cfg).unwrap());
        let key = handle.reveal_local_key();
        let c: Client<HttpConnector, Full<Bytes>> = Client::builder(TokioExecutor::new()).build_http();
        let headers = vec![
            ("authorization".to_string(), "Bearer client-token".to_string()),
            ("x-relay-key".to_string(), key),
            ("x-relay-account".to_string(), "bench".to_string()),
            ("content-type".to_string(), "application/json".to_string()),
        ];
        let direct_url = format!("http://{up}/v1/messages");
        let proxy_url = format!("http://{}/v1/messages", handle.addr());

        let mk = |size: usize| {
            let mut s = String::from(r#"{"model":"claude-sonnet-5-5","stream":true,"messages":[{"role":"user","content":""#);
            s.push_str(&"x".repeat(size));
            s.push_str(r#""}]}"#);
            Bytes::from(s)
        };
        println!("| Load | Metric | direct p50 µs | direct p99 µs | relay p50 µs | relay p99 µs | added p50 µs | added p99 µs |");
        println!("| --- | --- | --- | --- | --- | --- | --- | --- |");
        // Warm-up (connection pools, allocator).
        load(&c, &direct_url, &headers, mk(1024), 8, 50).await;
        load(&c, &proxy_url, &headers, mk(1024), 8, 50).await;

        let body = mk(256);
        let mut d = load(&c, &direct_url, &headers, body.clone(), 1, 500).await;
        let mut p = load(&c, &proxy_url, &headers, body, 1, 500).await;
        row("sequential, 256 B body", &mut d, &mut p);

        let body = mk(32 * 1024);
        let mut d = load(&c, &direct_url, &headers, body.clone(), 64, 60).await;
        let mut p = load(&c, &proxy_url, &headers, body, 64, 60).await;
        row("64 concurrent, 32 KB body", &mut d, &mut p);

        let body = mk(2 * 1024 * 1024);
        let mut d = load(&c, &direct_url, &headers, body.clone(), 1, 150).await;
        let mut p = load(&c, &proxy_url, &headers, body, 1, 150).await;
        row("sequential, 2 MB body", &mut d, &mut p);

        let snap = handle.snapshot();
        println!("\ntelemetry_dropped = {}", snap.telemetry_dropped);
    });
}
