//! Bounded exponential backoff. Never loops forever, never retries quota exhaustion.

use crate::error::{ErrorCode, WispError};
use std::time::Duration;

#[derive(Debug, Clone, Copy)]
pub struct RetryPolicy {
    pub max_attempts: u32,
    pub base: Duration,
    pub cap: Duration,
    /// Provider-suggested waits longer than this are surfaced to the user
    /// instead of being waited out silently.
    pub max_auto_wait: Duration,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            base: Duration::from_millis(500),
            cap: Duration::from_secs(8),
            max_auto_wait: Duration::from_secs(10),
        }
    }
}

impl RetryPolicy {
    pub fn none() -> Self {
        Self {
            max_attempts: 1,
            ..Self::default()
        }
    }

    /// Delay before attempt number `attempt + 1`, or `None` to stop retrying.
    /// `attempt` is 1-based (the attempt that just failed).
    pub fn next_delay(&self, attempt: u32, err: &WispError) -> Option<Duration> {
        if attempt >= self.max_attempts || !err.retryable {
            return None;
        }
        if matches!(
            err.code,
            ErrorCode::QuotaExhausted | ErrorCode::InvalidKey | ErrorCode::Cancelled
        ) {
            return None;
        }
        let exp = self
            .base
            .saturating_mul(2u32.saturating_pow(attempt - 1))
            .min(self.cap);
        match err.retry_after_secs.map(Duration::from_secs) {
            Some(hint) if hint > self.max_auto_wait => None,
            Some(hint) => Some(hint.max(exp)),
            None => Some(exp),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn err(code: ErrorCode) -> WispError {
        WispError::new(code, "x")
    }

    #[test]
    fn exponential_and_bounded() {
        let p = RetryPolicy::default();
        let e = err(ErrorCode::ServerError);
        assert_eq!(p.next_delay(1, &e), Some(Duration::from_millis(500)));
        assert_eq!(p.next_delay(2, &e), Some(Duration::from_millis(1000)));
        assert_eq!(p.next_delay(3, &e), None);
        let long = RetryPolicy {
            max_attempts: 10,
            ..p
        };
        assert_eq!(long.next_delay(9, &e), Some(Duration::from_secs(8)));
    }

    #[test]
    fn never_retries_terminal_errors() {
        let p = RetryPolicy::default();
        assert_eq!(p.next_delay(1, &err(ErrorCode::QuotaExhausted)), None);
        assert_eq!(p.next_delay(1, &err(ErrorCode::InvalidKey)), None);
        assert_eq!(p.next_delay(1, &err(ErrorCode::BadRequest)), None);
        assert_eq!(p.next_delay(1, &err(ErrorCode::Cancelled)), None);
    }

    #[test]
    fn honours_retry_after_within_limit() {
        let p = RetryPolicy::default();
        let short = err(ErrorCode::RateLimited).with_retry_after(Some(3));
        assert_eq!(p.next_delay(1, &short), Some(Duration::from_secs(3)));
        let long = err(ErrorCode::RateLimited).with_retry_after(Some(60));
        assert_eq!(p.next_delay(1, &long), None);
    }
}
