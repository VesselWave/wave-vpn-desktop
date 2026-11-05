// SPDX-License-Identifier: GPL-3.0-or-later
//! Bounded retry and suspend detection shared by the desktop and tests.
use std::time::Duration;
pub fn retry_delay(attempt: u32) -> Duration {
    Duration::from_secs(3u64.saturating_mul(1u64 << attempt.min(7)).min(120))
}
/// `retry_delay` spread by ±20 % (`unit` uniform in [0, 1]), so clients that
/// lost a server together do not all reconnect in the same second.
pub fn retry_delay_jittered(attempt: u32, unit: f64) -> Duration {
    crate::schedule::jitter(retry_delay(attempt), unit)
}
/// Wall time includes suspend on systems where the monotonic clock does not.
pub fn resumed(wall_gap: Duration, monotonic_gap: Duration) -> bool {
    wall_gap > Duration::from_secs(15) || monotonic_gap > Duration::from_secs(15)
}
