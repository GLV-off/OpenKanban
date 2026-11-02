use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

/// Login attempts are counted per normalized email inside a sliding window.
/// The limiter is per process; for multiple instances a shared store is required.
pub const LOGIN_WINDOW: Duration = Duration::from_secs(15 * 60);

#[derive(Debug, Clone, Default)]
pub struct LoginThrottle {
    attempts: Arc<Mutex<HashMap<String, Vec<Instant>>>>,
}

#[derive(Debug, Clone, Copy)]
pub struct ThrottleStatus {
    pub allowed: bool,
    pub retry_after_seconds: u64,
}

impl LoginThrottle {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn check(&self, key: &str, max_attempts: u32) -> ThrottleStatus {
        let now = Instant::now();
        let mut attempts = self.lock();
        let attempts = attempts.entry(key.to_owned()).or_default();
        attempts.retain(|attempt| now.duration_since(*attempt) < LOGIN_WINDOW);

        if attempts.len() < max_attempts as usize {
            return ThrottleStatus {
                allowed: true,
                retry_after_seconds: 0,
            };
        }

        let oldest = attempts.first().copied().unwrap_or(now);
        let remaining = LOGIN_WINDOW.saturating_sub(now.duration_since(oldest));

        ThrottleStatus {
            allowed: false,
            retry_after_seconds: remaining.as_secs().max(1),
        }
    }

    pub fn record_failure(&self, key: &str) {
        let now = Instant::now();
        let mut attempts = self.lock();
        let attempts = attempts.entry(key.to_owned()).or_default();
        attempts.retain(|attempt| now.duration_since(*attempt) < LOGIN_WINDOW);
        attempts.push(now);
    }

    pub fn reset(&self, key: &str) {
        self.lock().remove(key);
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<String, Vec<Instant>>> {
        self.attempts
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_after_max_attempts_and_resets() {
        let throttle = LoginThrottle::new();
        let key = "user@example.com";

        assert!(throttle.check(key, 2).allowed);
        throttle.record_failure(key);
        assert!(throttle.check(key, 2).allowed);
        throttle.record_failure(key);

        let blocked = throttle.check(key, 2);
        assert!(!blocked.allowed);
        assert!(blocked.retry_after_seconds > 0);

        throttle.reset(key);
        assert!(throttle.check(key, 2).allowed);
    }
}
