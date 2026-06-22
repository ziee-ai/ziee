//! `progress.v1` — the typed vocabulary a `kind: sandbox` step's code writes to
//! `$ZIEE_PROGRESS` (plan P2.2). Parsed **leniently, line by line, on the host**:
//! a malformed / unknown / missing-required line is **dropped (counted), never
//! failing the step** — progress is best-effort UX, the durable record is the
//! step output. A buggy `echo` must never crash a workflow.
//!
//! Author writes FLAT JSON (`{ "type":"bar", "id":"x", "fraction":0.42 }`); this
//! maps it into the nested [`ProgressTrack`]/[`ProgressKind`] (defined in
//! `events.rs`) the SSE event + persisted `step_progress_json` use.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;

use serde_json::Value;
use sqlx::PgPool;
use tokio::sync::mpsc::UnboundedReceiver;
use uuid::Uuid;

use crate::modules::workflow::events::{
    ProgressEmitter, ProgressKind, ProgressTrack, SSEStepProgressData, SSEWorkflowRunEvent,
};
use crate::modules::workflow::repository;

/// Max concurrent tracks surfaced per step. Beyond this, NEW track ids are
/// dropped (counted) so a runaway script can't unbound the SSE/UI. (P2.5)
pub const MAX_TRACKS_PER_STEP: usize = 50;
/// Coalesce/throttle window — flush changed tracks at most this often. (P2.5)
pub const PROGRESS_FLUSH_MS: u64 = 150;

/// Per-field plaintext caps (untrusted input). Over-length → truncated (not
/// dropped — truncation is friendlier than dropping a slightly-too-long label).
pub const MAX_MESSAGE_CHARS: usize = 500;
pub const MAX_LABEL_CHARS: usize = 120;
pub const MAX_ID_CHARS: usize = 64;
pub const MAX_UNIT_CHARS: usize = 24;

/// Outcome of parsing one raw `$ZIEE_PROGRESS` line.
#[derive(Debug, Clone, PartialEq)]
pub enum ParsedLine {
    /// A valid track update (clamped / truncated as needed).
    Track(ProgressTrack),
    /// Structured-but-invalid (unknown `type`, missing a required field,
    /// `total <= 0`) — the caller increments the per-step `dropped` counter.
    Dropped,
    /// Whitespace-only — silently ignored (not counted).
    Empty,
}

fn trunc(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        s.chars().take(max).collect()
    }
}

/// Parse one line per the P2.2 policy.
pub fn parse_progress_line(line: &str) -> ParsedLine {
    let line = line.trim();
    if line.is_empty() {
        return ParsedLine::Empty;
    }
    // A JSON OBJECT → the structured path; anything else (plain text, or a
    // non-object JSON value) → the ergonomic bare-string `status` default.
    match serde_json::from_str::<serde_json::Map<String, Value>>(line) {
        Ok(map) => parse_object(&map),
        Err(_) => ParsedLine::Track(ProgressTrack {
            id: String::new(),
            label: None,
            done: false,
            kind: ProgressKind::Status {
                message: trunc(line, MAX_MESSAGE_CHARS),
            },
        }),
    }
}

fn parse_object(map: &serde_json::Map<String, Value>) -> ParsedLine {
    let ty = match map.get("type").and_then(Value::as_str) {
        Some(t) => t,
        None => return ParsedLine::Dropped, // structured attempt with no type
    };
    let id = map
        .get("id")
        .and_then(Value::as_str)
        .map(|s| trunc(s, MAX_ID_CHARS))
        .unwrap_or_default();
    let label = map
        .get("label")
        .and_then(Value::as_str)
        .map(|s| trunc(s, MAX_LABEL_CHARS));
    let done = map.get("done").and_then(Value::as_bool).unwrap_or(false);

    let kind = match ty {
        "status" => match map.get("message").and_then(Value::as_str) {
            Some(m) => ProgressKind::Status {
                message: trunc(m, MAX_MESSAGE_CHARS),
            },
            None => return ParsedLine::Dropped,
        },
        "bar" => match map.get("fraction").and_then(Value::as_f64) {
            Some(f) => ProgressKind::Bar {
                fraction: f.clamp(0.0, 1.0),
            },
            None => return ParsedLine::Dropped,
        },
        "counter" => {
            let current = match map.get("current").and_then(Value::as_f64) {
                Some(c) => c.max(0.0),
                None => return ParsedLine::Dropped,
            };
            let total = match map.get("total").and_then(Value::as_f64) {
                Some(t) if t > 0.0 => t,
                _ => return ParsedLine::Dropped, // missing or non-positive
            };
            let unit = map
                .get("unit")
                .and_then(Value::as_str)
                .map(|s| trunc(s, MAX_UNIT_CHARS));
            ProgressKind::Counter {
                current,
                total,
                unit,
            }
        }
        "log" => match map.get("line").and_then(Value::as_str) {
            Some(l) => ProgressKind::Log {
                line: trunc(l, MAX_MESSAGE_CHARS),
            },
            None => return ParsedLine::Dropped,
        },
        "phase" => match map.get("name").and_then(Value::as_str) {
            Some(n) => ProgressKind::Phase {
                name: trunc(n, MAX_LABEL_CHARS),
                index: map.get("index").and_then(Value::as_u64).map(|v| v as u32),
                total: map.get("total").and_then(Value::as_u64).map(|v| v as u32),
            },
            None => return ParsedLine::Dropped,
        },
        _ => return ParsedLine::Dropped, // unknown type → forward-compat drop
    };

    ParsedLine::Track(ProgressTrack {
        id,
        label,
        done,
        kind,
    })
}

/// Emit the changed tracks as one batched `StepProgress` frame, then evict any
/// `done` tracks from the live map (they were delivered once). Clears `changed`.
/// Returns `true` if a frame was emitted (→ the caller persists the new map).
fn flush(
    emit: &Arc<dyn ProgressEmitter>,
    run_id: Uuid,
    step_id: &str,
    tracks: &mut HashMap<String, ProgressTrack>,
    changed: &mut HashSet<String>,
) -> bool {
    if changed.is_empty() {
        return false;
    }
    let batch: Vec<ProgressTrack> = changed
        .iter()
        .filter_map(|id| tracks.get(id).cloned())
        .collect();
    let emitted = !batch.is_empty();
    if emitted {
        emit.emit(SSEWorkflowRunEvent::StepProgress(SSEStepProgressData {
            run_id,
            step_id: step_id.to_string(),
            tracks: batch,
        }));
    }
    for id in changed.drain() {
        if tracks.get(&id).map(|t| t.done).unwrap_or(false) {
            tracks.remove(&id);
        }
    }
    emitted
}

/// Persist the running step's current track map onto the run row (P2.6) so a
/// reconnect/refresh Snapshot rehydrates the in-flight bars. Best-effort: a DB
/// hiccup must never disturb the live exec (progress is decorative).
async fn persist_tracks(pool: &PgPool, run_id: Uuid, tracks: &HashMap<String, ProgressTrack>) {
    if let Ok(json) = serde_json::to_value(tracks) {
        let _ = repository::set_step_progress(pool, run_id, &json).await;
    }
}

/// Drain the sandbox step's `$ZIEE_PROGRESS` lines (delivered as raw `Vec<u8>`
/// from the transport seam), parse each leniently, coalesce per track id, and
/// flush batched `StepProgress` events on a throttle. Ends when the sender is
/// dropped (the exec finished/was cancelled). A final flush delivers the last
/// changes; a one-shot note surfaces the dropped-line count for debuggability.
///
/// Persists the running step's track map into `step_progress_json` on each flush
/// (P2.6) so a page refresh rehydrates in-flight bars; clears it when the step
/// ends. On CANCEL the dispatcher aborts this task, so the dispatcher clears the
/// column itself (this task's end-clear won't run).
pub async fn run_progress_consumer(
    mut rx: UnboundedReceiver<Vec<u8>>,
    emit: Arc<dyn ProgressEmitter>,
    pool: PgPool,
    run_id: Uuid,
    step_id: String,
) {
    let mut tracks: HashMap<String, ProgressTrack> = HashMap::new();
    let mut changed: HashSet<String> = HashSet::new();
    let mut dropped: u64 = 0;
    let mut tick = tokio::time::interval(Duration::from_millis(PROGRESS_FLUSH_MS));
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

    loop {
        tokio::select! {
            line = rx.recv() => match line {
                Some(bytes) => {
                    let s = String::from_utf8_lossy(&bytes);
                    match parse_progress_line(&s) {
                        ParsedLine::Track(t) => {
                            let id = t.id.clone();
                            // Track cap: drop NEW ids beyond the cap (existing
                            // ids keep updating).
                            if !tracks.contains_key(&id) && tracks.len() >= MAX_TRACKS_PER_STEP {
                                dropped += 1;
                            } else {
                                tracks.insert(id.clone(), t);
                                changed.insert(id);
                            }
                        }
                        ParsedLine::Dropped => dropped += 1,
                        ParsedLine::Empty => {}
                    }
                }
                None => break, // sender dropped → exec done
            },
            _ = tick.tick() => {
                if flush(&emit, run_id, &step_id, &mut tracks, &mut changed) {
                    persist_tracks(&pool, run_id, &tracks).await;
                }
            }
        }
    }

    if flush(&emit, run_id, &step_id, &mut tracks, &mut changed) {
        persist_tracks(&pool, run_id, &tracks).await;
    }
    // Step ended normally (completed/failed) → clear the live-progress slot.
    let _ = repository::clear_step_progress(&pool, run_id).await;

    if dropped > 0 {
        emit.emit(SSEWorkflowRunEvent::StepProgress(SSEStepProgressData {
            run_id,
            step_id: step_id.clone(),
            tracks: vec![ProgressTrack {
                id: "_dropped".into(),
                label: None,
                done: false,
                kind: ProgressKind::Status {
                    message: format!("{dropped} progress line(s) dropped (malformed or over cap)"),
                },
            }],
        }));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn track(line: &str) -> ProgressTrack {
        match parse_progress_line(line) {
            ParsedLine::Track(t) => t,
            other => panic!("expected Track, got {other:?}"),
        }
    }

    #[test]
    fn bare_string_becomes_status() {
        let t = track("Processing 420/1000");
        assert_eq!(t.id, "");
        assert_eq!(
            t.kind,
            ProgressKind::Status {
                message: "Processing 420/1000".into()
            }
        );
    }

    #[test]
    fn non_object_json_is_bare_status() {
        // A JSON array/number is NOT an object → bare status of the raw text.
        assert!(matches!(
            track("[1,2,3]").kind,
            ProgressKind::Status { .. }
        ));
    }

    #[test]
    fn each_kind_parses() {
        assert_eq!(
            track(r#"{"type":"status","message":"hi"}"#).kind,
            ProgressKind::Status { message: "hi".into() }
        );
        assert_eq!(
            track(r#"{"type":"bar","fraction":0.42}"#).kind,
            ProgressKind::Bar { fraction: 0.42 }
        );
        assert_eq!(
            track(r#"{"type":"counter","current":420,"total":1000,"unit":"files"}"#).kind,
            ProgressKind::Counter { current: 420.0, total: 1000.0, unit: Some("files".into()) }
        );
        assert_eq!(
            track(r#"{"type":"log","line":"epoch 3"}"#).kind,
            ProgressKind::Log { line: "epoch 3".into() }
        );
        assert_eq!(
            track(r#"{"type":"phase","name":"Indexing","index":2,"total":4}"#).kind,
            ProgressKind::Phase { name: "Indexing".into(), index: Some(2), total: Some(4) }
        );
    }

    #[test]
    fn id_and_done_carry() {
        let t = track(r#"{"type":"bar","id":"download:A","fraction":1.0,"done":true}"#);
        assert_eq!(t.id, "download:A");
        assert!(t.done);
    }

    #[test]
    fn fraction_is_clamped() {
        assert_eq!(
            track(r#"{"type":"bar","fraction":1.5}"#).kind,
            ProgressKind::Bar { fraction: 1.0 }
        );
        assert_eq!(
            track(r#"{"type":"bar","fraction":-0.3}"#).kind,
            ProgressKind::Bar { fraction: 0.0 }
        );
    }

    #[test]
    fn negative_current_clamped_total_must_be_positive() {
        assert_eq!(
            track(r#"{"type":"counter","current":-5,"total":10}"#).kind,
            ProgressKind::Counter { current: 0.0, total: 10.0, unit: None }
        );
        // total <= 0 → dropped.
        assert_eq!(
            parse_progress_line(r#"{"type":"counter","current":1,"total":0}"#),
            ParsedLine::Dropped
        );
    }

    #[test]
    fn unknown_type_and_missing_required_are_dropped() {
        assert_eq!(
            parse_progress_line(r#"{"type":"typo","x":1}"#),
            ParsedLine::Dropped
        );
        assert_eq!(
            parse_progress_line(r#"{"type":"bar"}"#),
            ParsedLine::Dropped
        );
        // object with no `type` → dropped (structured attempt gone wrong).
        assert_eq!(
            parse_progress_line(r#"{"message":"hi"}"#),
            ParsedLine::Dropped
        );
    }

    #[test]
    fn empty_line_ignored() {
        assert_eq!(parse_progress_line("   "), ParsedLine::Empty);
    }

    #[test]
    fn overlong_strings_truncate() {
        let long = "x".repeat(MAX_MESSAGE_CHARS + 50);
        let t = track(&format!(r#"{{"type":"status","message":"{long}"}}"#));
        if let ProgressKind::Status { message } = t.kind {
            assert_eq!(message.chars().count(), MAX_MESSAGE_CHARS);
        } else {
            panic!("expected status");
        }
    }
}
