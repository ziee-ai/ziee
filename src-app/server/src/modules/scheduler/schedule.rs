//! Pure schedule engine — no I/O, fully unit-testable.
//!
//! Computes the next fire instant for a task's schedule and validates a
//! proposed schedule at create/update time. Recurring schedules are 5-field
//! POSIX/Vixie cron (croner) evaluated in the task's IANA timezone; the result
//! is always normalized to UTC for storage. `once` schedules are a single UTC
//! instant.
//!
//! croner weekday numbering is POSIX (0 = Sunday); the frontend preset builder
//! must emit POSIX-numbered expressions (asserted in tests).

use std::str::FromStr as _;

use chrono::{DateTime, Utc};
use chrono_tz::Tz;
use croner::Cron;

/// Which flavor of schedule a task carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScheduleKind {
    Once,
    Recurring,
}

/// Why a proposed schedule was rejected (mapped to a 400 by the handler).
#[derive(Debug, PartialEq, Eq)]
pub enum ScheduleError {
    BadCron(String),
    BadTimezone(String),
    RunAtInPast,
    TooFrequent { min_interval_seconds: i64 },
    /// A recurring schedule has no future occurrence at all (e.g. `0 0 30 2 *`
    /// — Feb 30 never happens).
    NoOccurrence,
    MissingField(&'static str),
}

impl std::fmt::Display for ScheduleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ScheduleError::BadCron(e) => write!(f, "invalid cron expression: {e}"),
            ScheduleError::BadTimezone(tz) => write!(f, "unknown timezone: {tz}"),
            ScheduleError::RunAtInPast => write!(f, "run_at is in the past"),
            ScheduleError::TooFrequent {
                min_interval_seconds,
            } => write!(
                f,
                "schedule fires more often than the minimum interval of {min_interval_seconds}s"
            ),
            ScheduleError::NoOccurrence => {
                write!(f, "cron expression has no upcoming occurrence")
            }
            ScheduleError::MissingField(field) => write!(f, "missing required field: {field}"),
        }
    }
}

/// Parse an IANA timezone string (e.g. `America/New_York`, `UTC`).
fn parse_tz(timezone: &str) -> Result<Tz, ScheduleError> {
    Tz::from_str(timezone).map_err(|_| ScheduleError::BadTimezone(timezone.to_string()))
}

/// Parse a 5-field cron expression.
fn parse_cron(cron_expr: &str) -> Result<Cron, ScheduleError> {
    Cron::from_str(cron_expr).map_err(|e| ScheduleError::BadCron(e.to_string()))
}

/// The next fire instant strictly AFTER `after`, in UTC.
///
/// - `Once`: `Some(run_at)` while `run_at > after`, else `None` (already fired).
/// - `Recurring`: the next cron match in `timezone`, normalized to UTC. `None`
///   only if the cron has no future occurrence.
pub fn next_occurrence(
    kind: ScheduleKind,
    run_at: Option<DateTime<Utc>>,
    cron_expr: Option<&str>,
    timezone: &str,
    after: DateTime<Utc>,
) -> Result<Option<DateTime<Utc>>, ScheduleError> {
    match kind {
        ScheduleKind::Once => {
            let at = run_at.ok_or(ScheduleError::MissingField("run_at"))?;
            Ok((at > after).then_some(at))
        }
        ScheduleKind::Recurring => {
            let expr = cron_expr.ok_or(ScheduleError::MissingField("cron_expr"))?;
            let cron = parse_cron(expr)?;
            let tz = parse_tz(timezone)?;
            let after_local = after.with_timezone(&tz);
            match cron.find_next_occurrence(&after_local, false) {
                Ok(next_local) => Ok(Some(next_local.with_timezone(&Utc))),
                // croner returns an error when the pattern has no reachable next
                // time — treat as "no occurrence" (the task self-disables).
                Err(_) => Ok(None),
            }
        }
    }
}

/// Validate a proposed schedule at create/update time. `now` is the reference
/// instant (injected for tests). Enforces: valid cron/timezone, a future
/// `run_at` for `once`, and a recurring cadence no tighter than
/// `min_interval_seconds`.
pub fn validate_schedule(
    kind: ScheduleKind,
    run_at: Option<DateTime<Utc>>,
    cron_expr: Option<&str>,
    timezone: &str,
    min_interval_seconds: i64,
    now: DateTime<Utc>,
) -> Result<(), ScheduleError> {
    match kind {
        ScheduleKind::Once => {
            let at = run_at.ok_or(ScheduleError::MissingField("run_at"))?;
            if at <= now {
                return Err(ScheduleError::RunAtInPast);
            }
            Ok(())
        }
        ScheduleKind::Recurring => {
            let expr = cron_expr.ok_or(ScheduleError::MissingField("cron_expr"))?;
            let cron = parse_cron(expr)?;
            let tz = parse_tz(timezone)?;
            // Compute two consecutive occurrences and require their gap to meet
            // the floor. This catches sub-minute / every-minute crons regardless
            // of field shape.
            let start = now.with_timezone(&tz);
            let first = cron
                .find_next_occurrence(&start, false)
                .map_err(|_| ScheduleError::NoOccurrence)?;
            let second = cron
                .find_next_occurrence(&first, false)
                .map_err(|_| ScheduleError::NoOccurrence)?;
            let gap = (second - first).num_seconds();
            if gap < min_interval_seconds {
                return Err(ScheduleError::TooFrequent {
                    min_interval_seconds,
                });
            }
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn utc(y: i32, mo: u32, d: u32, h: u32, mi: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(y, mo, d, h, mi, 0).unwrap()
    }

    // TEST-1: once fires then never again.
    #[test]
    fn once_returns_run_at_then_none() {
        let at = utc(2026, 8, 1, 9, 0);
        let before = utc(2026, 7, 1, 0, 0);
        let after = utc(2026, 9, 1, 0, 0);
        assert_eq!(
            next_occurrence(ScheduleKind::Once, Some(at), None, "UTC", before).unwrap(),
            Some(at)
        );
        assert_eq!(
            next_occurrence(ScheduleKind::Once, Some(at), None, "UTC", after).unwrap(),
            None
        );
    }

    // TEST-1: weekly Monday 09:00 computes the next Monday in-zone.
    #[test]
    fn recurring_weekly_monday_9am() {
        // 2026-07-09 is a Thursday; next Monday 09:00 UTC is 2026-07-13.
        let after = utc(2026, 7, 9, 12, 0);
        let next = next_occurrence(
            ScheduleKind::Recurring,
            None,
            Some("0 9 * * 1"),
            "UTC",
            after,
        )
        .unwrap()
        .unwrap();
        assert_eq!(next, utc(2026, 7, 13, 9, 0));
    }

    // TEST-1: the timezone is honored — 09:00 America/New_York, not UTC.
    #[test]
    fn recurring_respects_timezone() {
        let after = utc(2026, 7, 9, 0, 0);
        let next = next_occurrence(
            ScheduleKind::Recurring,
            None,
            Some("0 9 * * *"), // daily 09:00 local
            "America/New_York",
            after,
        )
        .unwrap()
        .unwrap();
        // 09:00 EDT (UTC-4 in July) == 13:00 UTC same day.
        assert_eq!(next, utc(2026, 7, 9, 13, 0));
    }

    // TEST-1: crossing a spring-forward DST boundary still advances by a day.
    #[test]
    fn recurring_across_dst_boundary() {
        // US spring-forward 2026 is 2026-03-08. A daily 09:00 local job on the
        // 7th should next fire on the 8th at 09:00 local (a real instant).
        let after = utc(2026, 3, 7, 20, 0); // after the 7th's 09:00 EST
        let next = next_occurrence(
            ScheduleKind::Recurring,
            None,
            Some("0 9 * * *"),
            "America/New_York",
            after,
        )
        .unwrap()
        .unwrap();
        // 2026-03-08 09:00 EDT (UTC-4) == 13:00 UTC.
        assert_eq!(next, utc(2026, 3, 8, 13, 0));
    }

    // TEST-2: malformed cron / bad tz / past once / too-frequent are rejected.
    #[test]
    fn validate_rejects_bad_inputs() {
        let now = utc(2026, 7, 9, 12, 0);

        assert!(matches!(
            validate_schedule(ScheduleKind::Recurring, None, Some("not a cron"), "UTC", 300, now),
            Err(ScheduleError::BadCron(_))
        ));
        assert!(matches!(
            validate_schedule(
                ScheduleKind::Recurring,
                None,
                Some("0 9 * * 1"),
                "Mars/Phobos",
                300,
                now
            ),
            Err(ScheduleError::BadTimezone(_))
        ));
        assert_eq!(
            validate_schedule(
                ScheduleKind::Once,
                Some(utc(2026, 1, 1, 0, 0)),
                None,
                "UTC",
                300,
                now
            ),
            Err(ScheduleError::RunAtInPast)
        );
        // every minute (gap 60s) vs a 300s floor → too frequent.
        assert!(matches!(
            validate_schedule(ScheduleKind::Recurring, None, Some("* * * * *"), "UTC", 300, now),
            Err(ScheduleError::TooFrequent { .. })
        ));
        // hourly (gap 3600s) is fine under a 300s floor.
        assert!(validate_schedule(
            ScheduleKind::Recurring,
            None,
            Some("0 * * * *"),
            "UTC",
            300,
            now
        )
        .is_ok());
    }
}
