//! Classifies each upstream attempt and decides whether another attempt is
//! safe. The rule: never re-execute a request the provider may already have
//! run. See `research/routing-architecture.md` §2.2.

use serde::{Deserialize, Serialize};

/// What happened to one upstream attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    /// Completed normally (terminal event seen, or a 2xx non-stream body).
    Ok,
    /// Failure before the request could reach the provider: DNS, TCP connect
    /// or TLS handshake.
    NotSent,
    /// Provider explicitly refused to generate: 429, 529, `overloaded_error`,
    /// unified rate-limit rejection, `usage_limit_reached`. This says no
    /// output was produced; it does not prove the provider spent nothing
    /// (input may have been read or counted), so a retry elsewhere can still
    /// cost something on both accounts.
    Rejected,
    /// 5xx or 408 before any output. The provider may have processed the
    /// request (a gateway error can follow execution), so this is never
    /// retried. Only an explicit refusal ([`Outcome::Rejected`]) allows a
    /// move, and even that doesn't prove nothing was counted or billed.
    ServerError,
    /// 401, or a 403 whose body says `authentication_error`: the credential
    /// itself was refused.
    AuthError,
    /// 400, 404, 413 and other client errors; forwarded untouched.
    ClientError,
    /// The provider may have executed the request: reset or timeout after the
    /// body was written, or EOF before the terminal event.
    Ambiguous,
    /// The downstream client went away.
    Cancelled,
    /// An upstream error after bytes were forwarded to the client.
    MidStream,
}

impl Outcome {
    /// Stable storage key.
    pub fn key(self) -> &'static str {
        match self {
            Outcome::Ok => "ok",
            Outcome::NotSent => "not_sent",
            Outcome::Rejected => "rejected",
            Outcome::ServerError => "server_error",
            Outcome::AuthError => "auth_error",
            Outcome::ClientError => "client_error",
            Outcome::Ambiguous => "ambiguous",
            Outcome::Cancelled => "cancelled",
            Outcome::MidStream => "mid_stream",
        }
    }

    /// Counts toward the error rate.
    pub fn is_error(self) -> bool {
        !matches!(self, Outcome::Ok | Outcome::Cancelled)
    }

    /// Classifies an HTTP status received before any byte reached the client.
    /// A 403 is a permission or model denial (`ClientError`); callers
    /// upgrade it to `AuthError` only on provider proof in the body.
    pub fn from_status(status: u16) -> Outcome {
        match status {
            200..=299 => Outcome::Ok,
            429 | 529 => Outcome::Rejected,
            401 => Outcome::AuthError,
            408 | 500..=599 => Outcome::ServerError,
            _ => Outcome::ClientError,
        }
    }
}

/// Facts about the request that decide whether it may move between accounts.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RequestTraits {
    /// No continuation state bound to an account (`previous_response_id`,
    /// reasoning/encrypted content, compaction capsules, signed thinking,
    /// containers, account-scoped file or conversation references).
    pub stateless: bool,
}

/// Next step after an attempt that did not succeed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    /// Try the same account again (only for proven-not-sent).
    RetrySame,
    /// Try another account.
    Failover,
    /// Return the upstream response or error to the client unchanged.
    Return,
}

/// Decides what to do after `outcome`.
///
/// `router_owned` is true when Relay chose the account (pooled or key mode);
/// pass-through requests are never retried on another account, because the
/// client's own credential fixes the account.
pub fn decide(
    outcome: Outcome,
    traits: RequestTraits,
    router_owned: bool,
    attempt: u8,
) -> Decision {
    let may_move = router_owned && traits.stateless;
    match outcome {
        Outcome::NotSent if attempt == 0 => Decision::RetrySame,
        Outcome::NotSent if may_move => Decision::Failover,
        Outcome::Rejected if may_move => Decision::Failover,
        _ => Decision::Return,
    }
}

/// Cooldown after a rejection: `retry-after` when given, otherwise
/// exponential from 1 s, capped at 5 minutes.
pub fn cooldown_ms(retry_after_ms: Option<i64>, level: u32) -> i64 {
    const CAP: i64 = 5 * 60 * 1000;
    match retry_after_ms {
        Some(ms) if ms > 0 => ms.min(24 * 60 * 60 * 1000),
        _ => (1000_i64 << level.min(9)).min(CAP),
    }
}

/// Maximum accounts tried per request.
pub const MAX_ACCOUNTS_PER_REQUEST: usize = 3;
/// Total time budget for retries.
pub const RETRY_BUDGET_MS: u64 = 10_000;

#[cfg(test)]
mod tests {
    use super::*;

    const STATELESS: RequestTraits = RequestTraits { stateless: true };
    const STATEFUL: RequestTraits = RequestTraits { stateless: false };

    #[test]
    fn ambiguous_is_never_retried() {
        for owned in [true, false] {
            for attempt in 0..3 {
                assert_eq!(
                    decide(Outcome::Ambiguous, STATELESS, owned, attempt),
                    Decision::Return
                );
                assert_eq!(
                    decide(Outcome::MidStream, STATELESS, owned, attempt),
                    Decision::Return
                );
            }
        }
    }

    #[test]
    fn not_sent_retries_same_once_then_moves_only_if_allowed() {
        assert_eq!(
            decide(Outcome::NotSent, STATEFUL, false, 0),
            Decision::RetrySame
        );
        assert_eq!(
            decide(Outcome::NotSent, STATEFUL, true, 1),
            Decision::Return
        );
        assert_eq!(
            decide(Outcome::NotSent, STATELESS, true, 1),
            Decision::Failover
        );
    }

    #[test]
    fn rejection_moves_only_for_router_owned_stateless() {
        assert_eq!(
            decide(Outcome::Rejected, STATELESS, true, 0),
            Decision::Failover
        );
        assert_eq!(
            decide(Outcome::Rejected, STATELESS, false, 0),
            Decision::Return
        );
        assert_eq!(
            decide(Outcome::Rejected, STATEFUL, true, 0),
            Decision::Return
        );
    }

    #[test]
    fn server_errors_are_never_retried() {
        for traits in [STATELESS, STATEFUL] {
            for attempt in 0..3 {
                assert_eq!(
                    decide(Outcome::ServerError, traits, true, attempt),
                    Decision::Return
                );
            }
        }
    }

    #[test]
    fn client_and_auth_errors_return() {
        assert_eq!(
            decide(Outcome::ClientError, STATELESS, true, 0),
            Decision::Return
        );
        assert_eq!(
            decide(Outcome::AuthError, STATELESS, true, 0),
            Decision::Return
        );
    }

    #[test]
    fn status_classes() {
        assert_eq!(Outcome::from_status(529), Outcome::Rejected);
        assert_eq!(Outcome::from_status(502), Outcome::ServerError);
        assert_eq!(Outcome::from_status(408), Outcome::ServerError);
        assert_eq!(Outcome::from_status(413), Outcome::ClientError);
        assert_eq!(Outcome::from_status(403), Outcome::ClientError);
        assert_eq!(Outcome::from_status(401), Outcome::AuthError);
    }

    #[test]
    fn cooldown_prefers_retry_after_and_caps_backoff() {
        assert_eq!(cooldown_ms(Some(7_000), 5), 7_000);
        assert_eq!(cooldown_ms(None, 0), 1_000);
        assert_eq!(cooldown_ms(None, 1), 2_000);
        assert_eq!(cooldown_ms(None, 30), 300_000);
    }
}
