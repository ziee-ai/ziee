//! Forwarding LFS transfer progress into the download record.
//!
//! ## Why this module exists
//!
//! The owner watched a healthy 5.68 GB download sit at 0% for its entire
//! duration: *"it stopped at step 2, showing downloading with 0% ... I'm not
//! sure if it is running or not"*. The transfer was fine; nothing was reporting
//! it. That is its own defect — with no resume, a user who concludes the app has
//! hung and kills it loses the whole download.
//!
//! The cause was in `uploads.rs`:
//!
//! ```ignore
//! let (lfs_progress_tx, _lfs_progress_rx) = mpsc::unbounded_channel::<GitProgress>();
//! ```
//!
//! The sender went to the LFS pull, which faithfully emits progress per chunk.
//! Nothing ever read the other end, so the record stayed frozen at the
//! `"Checking for LFS files..."` write that preceded it.
//!
//! **A correction to the diagnosis this was reported under**, because it changes
//! what else was wrong: `_lfs_progress_rx` is an underscore-PREFIXED binding, not
//! the bare `_` pattern. It therefore keeps the receiver ALIVE to the end of
//! scope rather than dropping it immediately. So the sends did not fail silently
//! — they SUCCEEDED, and every one of them queued in an unbounded channel that
//! nothing drained, for the whole multi-GB download. Alongside the frozen bar
//! there was steady memory growth proportional to chunk count (order 10^5–10^6
//! messages, each carrying a heap-allocated `String`), and the converter task
//! inside `pull_lfs_files_with_cancellation` never took its `is_err()` break.
//! Consuming the receiver fixes both.
//!
//! ## Shape
//!
//! [`spawn_forwarder`] owns BOTH ends of the channel and hands the caller only
//! the sender. The original bug — binding a receiver and never reading it — is
//! not expressible against this API, which is a stronger guarantee than a test
//! that the call site got it right.
//!
//! [`LfsProgressThrottle`] holds the decision logic and is pure: it takes the
//! current byte counts and a caller-supplied `Instant`, and answers with the
//! `DownloadProgressData` to write, or `None`. That keeps the rate limiting and
//! the speed/ETA arithmetic testable without a database, a clock, or a socket.

use crate::core::Repos;
use crate::modules::llm_model::models::{DownloadPhase, DownloadProgressData};
use crate::modules::llm_model::types;
use crate::utils::git::GitProgress;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use uuid::Uuid;

/// Minimum wall-clock gap between two progress writes.
///
/// `LfsProgress` fires per HTTP chunk — order 10^5–10^6 times for a 5.68 GB
/// object — so writing per chunk would put hundreds of thousands of UPDATEs on
/// the database for one download. One write per second is far below what a human
/// can read and still makes the bar visibly live, which is the whole point: the
/// user must be able to tell "slow" from "hung".
const MIN_WRITE_INTERVAL: Duration = Duration::from_secs(1);

/// Window over which the transfer rate is averaged.
///
/// Instantaneous chunk-to-chunk rates are far too noisy to display. Averaging
/// over the gap between writes gives a figure that moves smoothly.
const RATE_WINDOW: Duration = Duration::from_secs(1);

/// Decides WHEN to write progress and WHAT to write.
///
/// Pure and clock-injected: every method takes `now`, so tests drive it with a
/// synthetic timeline instead of sleeping.
pub struct LfsProgressThrottle {
    last_write: Option<Instant>,
    rate_anchor: Instant,
    rate_anchor_bytes: u64,
    speed_bps: u64,
}

impl LfsProgressThrottle {
    pub fn new(started: Instant) -> Self {
        Self {
            last_write: None,
            rate_anchor: started,
            rate_anchor_bytes: 0,
            speed_bps: 0,
        }
    }

    /// Observe a progress report; return the record to write, or `None` to skip.
    ///
    /// The FIRST observation always writes. The user's complaint was a bar that
    /// never moved off zero, so the first sign of life must not wait a full
    /// interval behind the throttle.
    pub fn observe(
        &mut self,
        current: u64,
        total: u64,
        now: Instant,
    ) -> Option<DownloadProgressData> {
        let due = match self.last_write {
            None => true,
            Some(last) => now.duration_since(last) >= MIN_WRITE_INTERVAL,
        };
        if !due {
            return None;
        }

        if self.last_write.is_none() {
            // ANCHOR the rate window on the first observation rather than
            // measuring the opening interval's gain from zero. Without this a
            // transfer whose first report is already non-zero (a resumed or
            // fast-starting object) reports a wildly inflated opening rate, and
            // a counter that moves BACKWARDS reports a positive one. Caught by
            // `a_stalled_transfer_does_not_produce_a_negative_or_wrapping_eta`,
            // which failed against the first version of this function.
            self.rate_anchor = now;
            self.rate_anchor_bytes = current;
        } else {
            // Refresh the rate over the elapsed window. `saturating_sub` holds a
            // backwards counter at a gain of zero instead of wrapping.
            let window = now.duration_since(self.rate_anchor);
            if window >= RATE_WINDOW {
                let gained = current.saturating_sub(self.rate_anchor_bytes);
                let secs = window.as_secs_f64();
                if secs > 0.0 {
                    self.speed_bps = (gained as f64 / secs) as u64;
                }
                self.rate_anchor = now;
                self.rate_anchor_bytes = current;
            }
        }

        self.last_write = Some(now);

        // ETA only when it is meaningful: a rate of zero, or a total we do not
        // know, would produce either a division by zero or a confident lie.
        let eta_seconds = if self.speed_bps > 0 && total > current {
            (total - current) / self.speed_bps
        } else {
            0
        };

        Some(DownloadProgressData {
            phase: DownloadPhase::Downloading,
            // BYTES, not a 0-100 scale: `DownloadItem.tsx` renders these through
            // `formatBytes(current) / formatBytes(total)`, so the pre-existing
            // `current: 20, total: 100` writes were being shown to the user as
            // the literal string "20 B / 100 B".
            current: clamp_i64(current),
            total: clamp_i64(total),
            message: format!(
                "Downloading model weights — {} of {}",
                human_bytes(current),
                human_bytes(total)
            ),
            speed_bps: clamp_i64(self.speed_bps),
            eta_seconds: clamp_i64(eta_seconds),
        })
    }
}

/// The record's numeric columns are `i64`; transfer counters are `u64`.
///
/// Saturating rather than `unwrap()`: these are runtime values, and a panic
/// inside a progress task would be a spectacular way to fail at the one job of
/// not disturbing the download.
fn clamp_i64(v: u64) -> i64 {
    i64::try_from(v).unwrap_or(i64::MAX)
}

/// Format bytes for the user-facing message (the UI formats the numeric fields
/// itself; this is for the sentence beside them).
fn human_bytes(bytes: u64) -> String {
    const GB: f64 = 1_000_000_000.0;
    const MB: f64 = 1_000_000.0;
    let b = bytes as f64;
    if b >= GB {
        format!("{:.2} GB", b / GB)
    } else if b >= MB {
        format!("{:.0} MB", b / MB)
    } else {
        format!("{bytes} B")
    }
}

/// Spawn the forwarder and return the sender to hand to the LFS pull.
///
/// The caller never sees the receiver, so it cannot forget to read it.
///
/// Await the returned handle BEFORE writing a terminal status: the channel may
/// still hold queued updates when the pull returns, and a progress write landing
/// after the completion write would un-finish the record.
pub fn spawn_forwarder(
    download_id: Uuid,
) -> (mpsc::UnboundedSender<GitProgress>, tokio::task::JoinHandle<()>) {
    let (tx, mut rx) = mpsc::unbounded_channel::<GitProgress>();
    let handle = tokio::spawn(async move {
        let mut throttle = LfsProgressThrottle::new(Instant::now());
        while let Some(progress) = rx.recv().await {
            if let Some(data) = throttle.observe(progress.current, progress.total, Instant::now()) {
                // Progress reporting must NEVER break the transfer it is
                // reporting on. A failed write is dropped deliberately; the next
                // tick will carry a newer figure anyway.
                let _ = Repos
                    .download_instance
                    .update_progress(
                        download_id,
                        types::UpdateDownloadProgressRequest {
                            progress_data: data,
                            status: None,
                        },
                    )
                    .await;
            }
        }
    });
    (tx, handle)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 5.68 GB — the default model's Q4_K_M blob, the transfer that was stuck.
    const BLOB: u64 = 5_680_000_000;

    #[test]
    fn progress_advances_across_the_transfer() {
        // The defect this module exists to fix: the record never moved. Assert
        // the observed values ADVANCE, not merely that something was emitted.
        let start = Instant::now();
        let mut throttle = LfsProgressThrottle::new(start);
        let mut seen = Vec::new();

        for tick in 0..10u64 {
            let now = start + Duration::from_secs(tick);
            let current = tick * (BLOB / 10);
            if let Some(data) = throttle.observe(current, BLOB, now) {
                seen.push(data);
            }
        }

        assert!(seen.len() >= 5, "expected repeated updates, got {}", seen.len());
        for pair in seen.windows(2) {
            assert!(
                pair[1].current > pair[0].current,
                "progress must advance: {} then {}",
                pair[0].current,
                pair[1].current,
            );
        }
        assert_eq!(seen[0].total, BLOB as i64, "total must be the real object size");
    }

    #[test]
    fn the_first_observation_is_never_withheld() {
        // The user's complaint was a bar pinned at 0%. The first sign of life
        // must not sit behind the throttle interval.
        let start = Instant::now();
        let mut throttle = LfsProgressThrottle::new(start);
        assert!(
            throttle.observe(1_024, BLOB, start).is_some(),
            "the first progress report must be written immediately",
        );
    }

    #[test]
    fn writes_are_throttled_far_below_the_chunk_rate() {
        // Without this bound a 5.68 GB download issues one UPDATE per HTTP
        // chunk — order 10^5-10^6 database writes for a single download.
        let start = Instant::now();
        let mut throttle = LfsProgressThrottle::new(start);
        let mut writes = 0;

        // 10,000 chunk reports spread over 5 simulated seconds.
        for i in 0..10_000u64 {
            let now = start + Duration::from_millis(i / 2);
            if throttle.observe(i * 1024, BLOB, now).is_some() {
                writes += 1;
            }
        }

        assert!(
            writes <= 8,
            "10k chunk reports over ~5s must coalesce to a handful of writes, got {writes}",
        );
        assert!(writes >= 2, "but progress must still be reported, got {writes}");
    }

    #[test]
    fn speed_and_eta_are_reported_once_a_rate_is_known() {
        // After the absolute timeout became a stall timeout (DEC-19), a visible
        // rate is what distinguishes "slow but alive" from "hung" — which is
        // exactly the judgement the owner could not make.
        let start = Instant::now();
        let mut throttle = LfsProgressThrottle::new(start);

        throttle.observe(0, BLOB, start).expect("first write");
        let second = throttle
            .observe(10_000_000, BLOB, start + Duration::from_secs(1))
            .expect("second write");

        assert_eq!(second.speed_bps, 10_000_000, "1 s at 10 MB gives 10 MB/s");
        assert!(second.eta_seconds > 0, "a known rate must produce an ETA");
        assert_eq!(
            second.eta_seconds,
            ((BLOB - 10_000_000) / 10_000_000) as i64,
            "ETA is remaining bytes over the measured rate",
        );
    }

    #[test]
    fn an_unknown_rate_reports_no_eta_rather_than_a_wrong_one() {
        let start = Instant::now();
        let mut throttle = LfsProgressThrottle::new(start);
        let first = throttle.observe(0, BLOB, start).expect("first write");
        assert_eq!(first.speed_bps, 0);
        assert_eq!(first.eta_seconds, 0, "no rate yet ⇒ no ETA, not a fabricated one");
    }

    #[test]
    fn a_stalled_transfer_does_not_produce_a_negative_or_wrapping_eta() {
        // Byte counters that go backwards (a retried object) must not wrap.
        let start = Instant::now();
        let mut throttle = LfsProgressThrottle::new(start);
        throttle.observe(5_000_000, BLOB, start).expect("first");
        let back = throttle
            .observe(1_000, BLOB, start + Duration::from_secs(2))
            .expect("second");
        assert_eq!(back.speed_bps, 0, "a backwards counter must not fabricate a rate");
        assert_eq!(back.eta_seconds, 0);
    }

    #[test]
    fn totals_are_bytes_so_the_ui_formats_them_correctly() {
        // `DownloadItem.tsx` renders these through `formatBytes`, so the old
        // `current: 20, total: 100` was displayed as "20 B / 100 B".
        let start = Instant::now();
        let mut throttle = LfsProgressThrottle::new(start);
        let data = throttle.observe(1_200_000_000, BLOB, start).expect("write");
        assert!(data.message.contains("1.20 GB"), "message was {:?}", data.message);
        assert!(data.message.contains("5.68 GB"), "message was {:?}", data.message);
    }
}
