// SPDX-License-Identifier: GPL-3.0-or-later
//! When the desktop asks AirVPN for a new server list. Pure; the caller owns the clock.
use std::time::{Duration, Instant};

/// No automatic refresh more often than this, whatever the options say.
pub const MIN_AUTOMATIC: Duration = Duration::from_secs(5 * 60);
/// A manual refresh is ignored within this time of the previous attempt.
pub const MIN_MANUAL: Duration = Duration::from_secs(60);
/// First retry after a failed refresh; doubles per consecutive failure.
pub const FAILURE_BASE: Duration = Duration::from_secs(5 * 60);
/// Failed refreshes never back off further than this (or the interval, if longer).
pub const FAILURE_CAP: Duration = Duration::from_secs(60 * 60);
/// Automatic refreshes fetch only the manifest; the account document (devices,
/// subscription) is added at most this often. A manual refresh always has it.
pub const ACCOUNT_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);

/// Whether an automatic refresh should also fetch the account document.
pub fn account_due(checked: Instant, now: Instant) -> bool {
    now.saturating_duration_since(checked) >= ACCOUNT_INTERVAL
}

/// The automatic interval: the user option, the manifest's `next_update`
/// (minutes) and the floor, whichever is longest. All three are strict
/// minimums; jitter only ever adds to the result (see `jitter_above`).
pub fn interval(option_seconds: u64, manifest_minutes: Option<u32>) -> Duration {
    let manifest = Duration::from_secs(u64::from(manifest_minutes.unwrap_or(0)) * 60);
    Duration::from_secs(option_seconds)
        .max(manifest)
        .max(MIN_AUTOMATIC)
}

fn unit(unit: f64) -> f64 {
    if unit.is_finite() {
        unit.clamp(0., 1.)
    } else {
        0.5
    }
}
/// Spread reconnects by ±20 %, so clients that lost a server together do not
/// return in the same second. `unit` is a uniform random number in [0, 1].
/// Not for server-list refreshes, which have a strict minimum.
pub fn jitter(delay: Duration, unit: f64) -> Duration {
    delay.mul_f64(0.8 + 0.4 * self::unit(unit))
}
/// Spread refreshes over [delay, delay + 20 %]: never earlier than `delay`, so
/// the five-minute floor and the manifest's `next_update` hold for every client.
pub fn jitter_above(delay: Duration, unit: f64) -> Duration {
    delay.mul_f64(1. + 0.2 * self::unit(unit))
}

/// Delay before retrying after `failures` consecutive failures (at least 1).
/// The first retry waits the five-minute floor; jitter only lengthens it. A
/// retry follows an attempt that was itself scheduled no earlier than the
/// manifest's `next_update`, so the hint is honoured before backoff starts.
pub fn failure_delay(failures: u32, interval: Duration) -> Duration {
    let cap = FAILURE_CAP.max(interval);
    FAILURE_BASE
        .saturating_mul(1u32 << failures.saturating_sub(1).min(16))
        .min(cap)
        .max(MIN_AUTOMATIC)
}

#[derive(Debug)]
pub struct RefreshSchedule {
    next: Instant,
    last_attempt: Option<Instant>,
    failures: u32,
    running: bool,
}
impl RefreshSchedule {
    pub fn new(now: Instant) -> Self {
        Self {
            next: now + MIN_AUTOMATIC,
            last_attempt: None,
            failures: 0,
            running: false,
        }
    }
    pub fn due(&self, now: Instant) -> bool {
        !self.running && now >= self.next
    }
    pub fn manual_allowed(&self, now: Instant) -> bool {
        !self.running
            && self
                .last_attempt
                .is_none_or(|t| now.saturating_duration_since(t) >= MIN_MANUAL)
    }
    pub fn running(&self) -> bool {
        self.running
    }
    pub fn failures(&self) -> u32 {
        self.failures
    }
    pub fn next(&self) -> Instant {
        self.next
    }
    pub fn start(&mut self, now: Instant) {
        self.running = true;
        self.last_attempt = Some(now);
    }
    /// A sign-in or refresh succeeded at `now`.
    pub fn succeeded(&mut self, now: Instant, interval: Duration, unit: f64) {
        self.running = false;
        self.failures = 0;
        self.last_attempt = Some(now);
        self.next = now + jitter_above(interval.max(MIN_AUTOMATIC), unit);
    }
    pub fn failed(&mut self, now: Instant, interval: Duration, unit: f64) {
        self.running = false;
        self.failures = self.failures.saturating_add(1);
        // A failed manual refresh never pulls the automatic schedule earlier.
        self.next =
            (now + jitter_above(failure_delay(self.failures, interval), unit)).max(self.next);
    }
    /// Sign-out: forget the in-flight request; its result is discarded by epoch.
    pub fn reset(&mut self, now: Instant) {
        *self = Self::new(now);
    }
}
