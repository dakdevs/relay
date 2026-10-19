//! Response bodies: small local bodies, and the pass-through body that taps
//! usage and records the attempt when it ends, is cancelled or fails.

use crate::state::{AccountRt, Shared, now_ms};
use bytes::Bytes;
use http_body::{Body, Frame, SizeHint};
use hyper::body::Incoming;
use relay_core::attempt::Outcome;
use relay_providers::tap::{Decisive, Tap};
use relay_usage::UsageEvent;
use std::collections::VecDeque;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::task::{Context, Poll};
use std::time::Instant;
use tokio::sync::OwnedSemaphorePermit;

/// Boxed error type for response bodies.
pub type BoxError = Box<dyn std::error::Error + Send + Sync>;

/// Counts a request in flight on an account for as long as it lives.
pub struct InFlight {
    pub(crate) shared: Arc<Shared>,
    pub(crate) account: Arc<AccountRt>,
    _permit: Option<OwnedSemaphorePermit>,
}

impl InFlight {
    /// Starts counting.
    pub fn new(
        shared: Arc<Shared>,
        account: Arc<AccountRt>,
        permit: Option<OwnedSemaphorePermit>,
    ) -> InFlight {
        account.in_flight.fetch_add(1, Ordering::Relaxed);
        shared.changed();
        InFlight {
            shared,
            account,
            _permit: permit,
        }
    }
}

impl Drop for InFlight {
    fn drop(&mut self) {
        self.account.in_flight.fetch_sub(1, Ordering::Relaxed);
        self.shared.changed();
    }
}

/// What to record when the body ends.
pub struct Record {
    pub(crate) flight: InFlight,
    pub(crate) status: u16,
    pub(crate) started: Instant,
    pub(crate) attempt: u8,
    pub(crate) model: Option<String>,
    /// Record a usage event (generating endpoints only).
    pub(crate) record: bool,
    /// Endpoint produces a terminal event (Messages / Responses).
    pub(crate) generating: bool,
    pub(crate) streaming: bool,
}

impl Record {
    /// Records one attempt that never reached the client.
    pub fn emit(self, outcome: Outcome, tap: Option<&Tap>) {
        let Record {
            flight,
            status,
            started,
            attempt,
            model,
            record,
            ..
        } = self;
        if outcome == Outcome::Ok {
            flight.account.succeeded();
        }
        if record {
            let (usage, known) =
                tap.map_or((Default::default(), false), |t| (t.usage, t.usage_seen));
            flight.shared.usage.record(UsageEvent {
                at_ms: now_ms(),
                account_id: flight.account.cfg.id.clone(),
                provider: flight.account.cfg.provider,
                model,
                mode: flight.account.cfg.mode,
                outcome,
                status,
                attempt,
                usage,
                usage_known: known,
                latency_ms: started.elapsed().as_millis().min(u32::MAX as u128) as u32,
                ttft_ms: None,
            });
        }
    }
}

/// Upstream body passed to the client unchanged, with a usage tap.
pub struct TapBody {
    inner: Option<Incoming>,
    held: VecDeque<Frame<Bytes>>,
    tap: Tap,
    rec: Option<Record>,
    forwarded: bool,
    ttft_ms: Option<u32>,
    drop_outcome: Outcome,
    cooled: bool,
}

impl TapBody {
    /// Wraps `inner`, replaying `held` frames (already fed to `tap`) first.
    /// `drop_outcome` is recorded if the body is dropped before its end:
    /// `Cancelled` for a committed response, or the attempt's own outcome
    /// for an error response Relay may discard to fail over.
    pub fn new(
        inner: Option<Incoming>,
        held: VecDeque<Frame<Bytes>>,
        tap: Tap,
        rec: Record,
        drop_outcome: Outcome,
    ) -> TapBody {
        TapBody {
            inner,
            held,
            tap,
            rec: Some(rec),
            forwarded: false,
            ttft_ms: None,
            drop_outcome,
            cooled: false,
        }
    }

    /// Marks that the attempt's cooldown was already applied, so ending the
    /// body doesn't apply it a second time.
    pub fn cooled(self) -> TapBody {
        self.cooled_if(true)
    }

    /// [`TapBody::cooled`] when `applied`.
    pub fn cooled_if(mut self, applied: bool) -> TapBody {
        self.cooled |= applied;
        self
    }

    fn finish(&mut self, outcome: Option<Outcome>) {
        let Some(rec) = self.rec.take() else {
            return;
        };
        let success = (200..300).contains(&rec.status);
        self.tap.finish(success);
        let outcome = outcome.unwrap_or_else(|| {
            if !success {
                Outcome::from_status(rec.status)
            } else if let Some(Decisive::Rejection { .. }) = self.tap.decisive {
                Outcome::Rejected
            } else if let Some(Decisive::Error { .. }) = self.tap.decisive {
                // e.g. `response.failed` before any output.
                Outcome::ServerError
            } else if self.tap.late_error.is_some() {
                Outcome::MidStream
            } else if rec.generating && rec.streaming && !self.tap.terminal {
                // EOF before message_stop / response.completed.
                Outcome::Ambiguous
            } else {
                Outcome::Ok
            }
        });
        let now = now_ms();
        let acc = &rec.flight.account;
        if !self.cooled
            && let Some(Decisive::Rejection { retry_after_ms, .. }) = &self.tap.decisive
        {
            acc.cool_down(*retry_after_ms, now);
        }
        if outcome == Outcome::Ok {
            acc.succeeded();
        }
        if rec.record {
            rec.flight.shared.usage.record(UsageEvent {
                at_ms: now,
                account_id: acc.cfg.id.clone(),
                provider: acc.cfg.provider,
                model: rec.model.clone(),
                mode: acc.cfg.mode,
                outcome,
                status: rec.status,
                attempt: rec.attempt,
                usage: self.tap.usage,
                usage_known: self.tap.usage_seen,
                latency_ms: rec.started.elapsed().as_millis().min(u32::MAX as u128) as u32,
                ttft_ms: self.ttft_ms,
            });
        }
        // Dropping `rec` releases the slot and wakes the publisher.
    }

    fn on_forward(&mut self) {
        if !self.forwarded {
            self.forwarded = true;
            if let Some(rec) = &self.rec {
                self.ttft_ms = Some(rec.started.elapsed().as_millis().min(u32::MAX as u128) as u32);
            }
        }
    }
}

impl Body for TapBody {
    type Data = Bytes;
    type Error = BoxError;

    fn poll_frame(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Bytes>, BoxError>>> {
        let this = &mut *self;
        if let Some(f) = this.held.pop_front() {
            this.on_forward();
            return Poll::Ready(Some(Ok(f)));
        }
        let Some(inner) = this.inner.as_mut() else {
            this.finish(None);
            return Poll::Ready(None);
        };
        match Pin::new(inner).poll_frame(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(Some(Ok(frame))) => {
                if let Some(d) = frame.data_ref() {
                    this.tap.feed(d);
                }
                this.on_forward();
                Poll::Ready(Some(Ok(frame)))
            }
            Poll::Ready(Some(Err(e))) => {
                let outcome = if this.forwarded {
                    Outcome::MidStream
                } else {
                    Outcome::Ambiguous
                };
                this.inner = None;
                this.finish(Some(outcome));
                Poll::Ready(Some(Err(Box::new(e))))
            }
            Poll::Ready(None) => {
                this.inner = None;
                this.finish(None);
                Poll::Ready(None)
            }
        }
    }

    fn is_end_stream(&self) -> bool {
        self.held.is_empty() && self.inner.as_ref().is_none_or(|i| i.is_end_stream())
    }

    fn size_hint(&self) -> SizeHint {
        let held: u64 = self
            .held
            .iter()
            .filter_map(|f| f.data_ref().map(|d| d.len() as u64))
            .sum();
        match &self.inner {
            Some(i) => {
                let h = i.size_hint();
                let mut s = SizeHint::new();
                s.set_lower(h.lower() + held);
                if let Some(u) = h.upper() {
                    s.set_upper(u + held);
                }
                s
            }
            None => SizeHint::with_exact(held),
        }
    }
}

impl Drop for TapBody {
    fn drop(&mut self) {
        // hyper may drop a body without a final poll once `is_end_stream`
        // is true, so a fully delivered body ends normally here.
        let delivered =
            self.held.is_empty() && self.inner.as_ref().is_none_or(|i| i.is_end_stream());
        let outcome = if delivered {
            None
        } else if self.forwarded {
            // The client went away: dropping `inner` closes the upstream
            // stream, so generation stops and the slot frees now.
            Some(Outcome::Cancelled)
        } else {
            Some(self.drop_outcome)
        };
        self.finish(outcome);
    }
}

/// Response body: a small local body or a tapped upstream body.
pub enum RespBody {
    /// Local body (errors, health, held error bodies).
    Full(Option<Bytes>),
    /// Upstream pass-through.
    Tap(Box<TapBody>),
}

impl RespBody {
    /// Local body from bytes.
    pub fn full(b: impl Into<Bytes>) -> RespBody {
        RespBody::Full(Some(b.into()))
    }

    /// Empty body.
    pub fn empty() -> RespBody {
        RespBody::Full(None)
    }
}

impl Body for RespBody {
    type Data = Bytes;
    type Error = BoxError;

    fn poll_frame(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Bytes>, BoxError>>> {
        match self.get_mut() {
            RespBody::Full(b) => Poll::Ready(
                b.take()
                    .filter(|b| !b.is_empty())
                    .map(|b| Ok(Frame::data(b))),
            ),
            RespBody::Tap(t) => Pin::new(t.as_mut()).poll_frame(cx),
        }
    }

    fn is_end_stream(&self) -> bool {
        match self {
            RespBody::Full(b) => b.as_ref().is_none_or(Bytes::is_empty),
            RespBody::Tap(t) => t.is_end_stream(),
        }
    }

    fn size_hint(&self) -> SizeHint {
        match self {
            RespBody::Full(b) => SizeHint::with_exact(b.as_ref().map_or(0, |b| b.len() as u64)),
            RespBody::Tap(t) => t.size_hint(),
        }
    }
}
