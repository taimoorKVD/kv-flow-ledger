//! Task work time = union of developer-active and agent-execution intervals.
//! Combined duration is never the arithmetic sum of overlapping segments.

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IntervalKind {
    Developer,
    Agent,
    IdleExcluded,
    Unattributed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TimeInterval {
    pub kind: IntervalKind,
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct TimeBreakdown {
    pub combined_ms: i64,
    pub developer_ms: i64,
    pub agent_ms: i64,
    pub overlap_ms: i64,
    pub idle_excluded_ms: i64,
    pub unattributed_ms: i64,
}

impl TimeInterval {
    pub fn duration_ms(&self) -> i64 {
        (self.end - self.start).num_milliseconds().max(0)
    }
}

/// Merge overlapping intervals of the same kind into a disjoint cover.
pub fn merge_same_kind(mut intervals: Vec<TimeInterval>) -> Vec<TimeInterval> {
    if intervals.is_empty() {
        return intervals;
    }
    intervals.sort_by_key(|i| (i.kind as u8, i.start));
    let mut out: Vec<TimeInterval> = Vec::new();
    for iv in intervals {
        if let Some(last) = out.last_mut() {
            if last.kind == iv.kind && iv.start <= last.end {
                if iv.end > last.end {
                    last.end = iv.end;
                }
                continue;
            }
        }
        out.push(iv);
    }
    out
}

/// Union duration of intervals in milliseconds (overlap counted once).
pub fn union_duration_ms(intervals: &[TimeInterval]) -> i64 {
    if intervals.is_empty() {
        return 0;
    }
    let mut pts: Vec<(DateTime<Utc>, DateTime<Utc>)> =
        intervals.iter().map(|i| (i.start, i.end)).collect();
    pts.sort_by_key(|(s, _)| *s);
    let mut total = Duration::zero();
    let mut cur_start = pts[0].0;
    let mut cur_end = pts[0].1;
    for (s, e) in pts.into_iter().skip(1) {
        if s <= cur_end {
            if e > cur_end {
                cur_end = e;
            }
        } else {
            total = total + (cur_end - cur_start);
            cur_start = s;
            cur_end = e;
        }
    }
    total = total + (cur_end - cur_start);
    total.num_milliseconds().max(0)
}

pub fn overlap_duration_ms(a: &[TimeInterval], b: &[TimeInterval]) -> i64 {
    let mut total = 0i64;
    for x in a {
        for y in b {
            let start = x.start.max(y.start);
            let end = x.end.min(y.end);
            if end > start {
                total += (end - start).num_milliseconds();
            }
        }
    }
    total
}

pub fn compute_breakdown(intervals: &[TimeInterval]) -> TimeBreakdown {
    let developer: Vec<_> = intervals
        .iter()
        .filter(|i| i.kind == IntervalKind::Developer)
        .cloned()
        .collect();
    let agent: Vec<_> = intervals
        .iter()
        .filter(|i| i.kind == IntervalKind::Agent)
        .cloned()
        .collect();
    let idle: Vec<_> = intervals
        .iter()
        .filter(|i| i.kind == IntervalKind::IdleExcluded)
        .cloned()
        .collect();
    let unattributed: Vec<_> = intervals
        .iter()
        .filter(|i| i.kind == IntervalKind::Unattributed)
        .cloned()
        .collect();

    let developer_ms = union_duration_ms(&developer);
    let agent_ms = union_duration_ms(&agent);
    let overlap_ms = overlap_duration_ms(&developer, &agent);
    let mut work = developer;
    work.extend(agent);
    let combined_ms = union_duration_ms(&work);

    TimeBreakdown {
        combined_ms,
        developer_ms,
        agent_ms,
        overlap_ms,
        idle_excluded_ms: union_duration_ms(&idle),
        unattributed_ms: union_duration_ms(&unattributed),
    }
}

/// Close an open interval at `now`, clamping if wake gap exceeds idle threshold.
pub fn reconcile_after_sleep(
    open_start: DateTime<Utc>,
    last_heartbeat: DateTime<Utc>,
    now: DateTime<Utc>,
    idle_threshold: Duration,
) -> (DateTime<Utc>, bool) {
    let gap = now - last_heartbeat;
    if gap > idle_threshold {
        // Treat as closed at last heartbeat; remainder is excluded idle.
        (last_heartbeat.max(open_start), true)
    } else {
        (now, false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ts(min: i64) -> DateTime<Utc> {
        DateTime::from_timestamp(min * 60, 0).unwrap()
    }

    #[test]
    fn union_example_developer_10_agent_15_overlap_5_equals_20() {
        // Dev 0-10, Agent 5-20 → union 20, overlap 5
        let intervals = vec![
            TimeInterval {
                kind: IntervalKind::Developer,
                start: ts(0),
                end: ts(10),
            },
            TimeInterval {
                kind: IntervalKind::Agent,
                start: ts(5),
                end: ts(20),
            },
        ];
        let b = compute_breakdown(&intervals);
        assert_eq!(b.developer_ms, 10 * 60_000);
        assert_eq!(b.agent_ms, 15 * 60_000);
        assert_eq!(b.overlap_ms, 5 * 60_000);
        assert_eq!(b.combined_ms, 20 * 60_000);
    }

    #[test]
    fn rejects_double_counting_on_identical_ranges() {
        let intervals = vec![
            TimeInterval {
                kind: IntervalKind::Developer,
                start: ts(0),
                end: ts(10),
            },
            TimeInterval {
                kind: IntervalKind::Agent,
                start: ts(0),
                end: ts(10),
            },
        ];
        let b = compute_breakdown(&intervals);
        assert_eq!(b.combined_ms, 10 * 60_000);
        assert_eq!(b.overlap_ms, 10 * 60_000);
    }

    #[test]
    fn sleep_reconciliation_cuts_at_heartbeat() {
        let start = ts(0);
        let hb = ts(5);
        let now = ts(40);
        let (end, cut) = reconcile_after_sleep(start, hb, now, Duration::minutes(5));
        assert!(cut);
        assert_eq!(end, hb);
    }

    #[test]
    fn merge_overlapping_same_kind() {
        let merged = merge_same_kind(vec![
            TimeInterval {
                kind: IntervalKind::Agent,
                start: ts(0),
                end: ts(10),
            },
            TimeInterval {
                kind: IntervalKind::Agent,
                start: ts(8),
                end: ts(15),
            },
        ]);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].duration_ms(), 15 * 60_000);
    }
}
