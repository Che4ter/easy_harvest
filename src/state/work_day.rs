use chrono::{Duration, NaiveDate, NaiveTime};
use serde::{Deserialize, Serialize};

/// Operational phase of a work day, derived from which time fields are set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum WorkPhase {
    #[default]
    NotStarted,
    Working,
    /// At least one break is in progress (last break has no end time).
    OnBreak,
    Ended,
}

/// A single break period within a work day.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Break {
    pub start: NaiveTime,
    pub end: Option<NaiveTime>,
}

/// A single work day's time-tracking record.
///
/// Supports an arbitrary number of breaks.  Transition methods (`start`,
/// `start_break`, etc.) are pure state mutations — all I/O is handled by
/// [`super::persistence::WorkDayStore`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkDay {
    pub date: NaiveDate,
    pub start_time: Option<NaiveTime>,
    pub breaks: Vec<Break>,
    pub end_time: Option<NaiveTime>,
}

impl WorkDay {
    pub fn new(date: NaiveDate) -> Self {
        Self {
            date,
            start_time: None,
            breaks: Vec::new(),
            end_time: None,
        }
    }

    /// Current operational phase, derived from the time fields.
    pub fn phase(&self) -> WorkPhase {
        if self.start_time.is_none() {
            return WorkPhase::NotStarted;
        }
        if self.end_time.is_some() {
            return WorkPhase::Ended;
        }
        if let Some(last) = self.breaks.last()
            && last.end.is_none() {
                return WorkPhase::OnBreak;
            }
        WorkPhase::Working
    }

    // -----------------------------------------------------------------------
    // Transitions
    // -----------------------------------------------------------------------

    /// Begin the work day.
    pub fn start(&mut self, time: NaiveTime) {
        self.start_time = Some(time);
    }

    /// Begin a new break.  Multiple breaks are tracked independently.
    pub fn start_break(&mut self, time: NaiveTime) {
        self.breaks.push(Break { start: time, end: None });
    }

    /// End the current (last) open break.
    pub fn end_break(&mut self, time: NaiveTime) {
        if let Some(last) = self.breaks.last_mut()
            && last.end.is_none() {
                last.end = Some(time);
            }
    }

    /// End the work day.  Automatically closes an open break.
    pub fn end(&mut self, time: NaiveTime) {
        if let Some(last) = self.breaks.last_mut()
            && last.end.is_none() {
                last.end = Some(time);
            }
        self.end_time = Some(time);
    }

    // -----------------------------------------------------------------------
    // Duration helpers
    // -----------------------------------------------------------------------

    /// Total duration of all completed breaks.
    pub fn break_duration(&self) -> Duration {
        self.breaks
            .iter()
            .filter_map(|b| {
                b.end.map(|e| {
                    if e > b.start {
                        e.signed_duration_since(b.start)
                    } else {
                        Duration::zero()
                    }
                })
            })
            .fold(Duration::zero(), |acc, d| acc + d)
    }

    /// Net worked duration, excluding all breaks, up to `now` (or `end_time`
    /// if set).
    ///
    /// While on break, time stops accumulating at the current break's start
    /// so the break does not inflate the count.  Returns zero if the day has
    /// not started yet.
    pub fn worked_duration(&self, now: NaiveTime) -> Duration {
        let start = match self.start_time {
            Some(t) => t,
            None => return Duration::zero(),
        };

        // Choose the reference upper bound.
        let upper = match self.end_time {
            Some(t) => t,
            None => {
                // If currently on break, cap at that break's start.
                if let Some(last) = self.breaks.last() {
                    if last.end.is_none() {
                        last.start
                    } else {
                        now
                    }
                } else {
                    now
                }
            }
        };

        let gross = if upper > start {
            upper.signed_duration_since(start)
        } else {
            Duration::zero()
        };

        (gross - self.break_duration()).max(Duration::zero())
    }

    /// Worked hours as a decimal fraction.
    pub fn worked_hours(&self, now: NaiveTime) -> f64 {
        self.worked_duration(now).num_seconds() as f64 / 3600.0
    }

    /// Hours worked but not yet logged in Harvest (`worked − booked`).
    /// Returns 0.0 when the day is over-logged.
    pub fn unbooked_hours(&self, booked_hours: f64, now: NaiveTime) -> f64 {
        (self.worked_hours(now) - booked_hours).max(0.0)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn date() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 4, 10).unwrap()
    }

    fn t(h: u32, m: u32) -> NaiveTime {
        NaiveTime::from_hms_opt(h, m, 0).unwrap()
    }

    // --- phase transitions ---

    #[test]
    fn test_phase_not_started() {
        let day = WorkDay::new(date());
        assert_eq!(day.phase(), WorkPhase::NotStarted);
    }

    #[test]
    fn test_phase_working_after_start() {
        let mut day = WorkDay::new(date());
        day.start(t(9, 0));
        assert_eq!(day.phase(), WorkPhase::Working);
    }

    #[test]
    fn test_phase_on_break() {
        let mut day = WorkDay::new(date());
        day.start(t(9, 0));
        day.start_break(t(12, 0));
        assert_eq!(day.phase(), WorkPhase::OnBreak);
    }

    #[test]
    fn test_phase_working_after_break() {
        let mut day = WorkDay::new(date());
        day.start(t(9, 0));
        day.start_break(t(12, 0));
        day.end_break(t(13, 0));
        assert_eq!(day.phase(), WorkPhase::Working);
    }

    #[test]
    fn test_phase_ended() {
        let mut day = WorkDay::new(date());
        day.start(t(9, 0));
        day.start_break(t(12, 0));
        day.end_break(t(13, 0));
        day.end(t(17, 0));
        assert_eq!(day.phase(), WorkPhase::Ended);
    }

    // --- worked_duration ---

    #[test]
    fn test_worked_not_started() {
        let day = WorkDay::new(date());
        assert_eq!(day.worked_duration(t(10, 0)), Duration::zero());
    }

    #[test]
    fn test_worked_no_break() {
        let mut day = WorkDay::new(date());
        day.start(t(9, 0));
        assert_eq!(day.worked_hours(t(11, 0)), 2.0);
    }

    #[test]
    fn test_worked_with_completed_break() {
        // Start 9, break 12–13, end 17 → 8h gross − 1h break = 7h
        let mut day = WorkDay::new(date());
        day.start(t(9, 0));
        day.start_break(t(12, 0));
        day.end_break(t(13, 0));
        day.end(t(17, 0));
        assert_eq!(day.worked_hours(t(17, 0)), 7.0);
    }

    #[test]
    fn test_worked_during_active_break() {
        // Start 9, break starts 12, now 13 (still on break) → should count only 3h
        let mut day = WorkDay::new(date());
        day.start(t(9, 0));
        day.start_break(t(12, 0));
        assert_eq!(day.worked_hours(t(13, 0)), 3.0);
    }

    #[test]
    fn test_worked_partial_day_no_end() {
        let mut day = WorkDay::new(date());
        day.start(t(9, 0));
        assert!((day.worked_hours(t(14, 30)) - 5.5).abs() < 1e-9);
    }

    // --- multiple breaks ---

    #[test]
    fn test_multiple_breaks() {
        // Start 8, coffee 10:00-10:15, lunch 12:00-13:00, end 17:00
        // Gross = 9h, breaks = 0:15 + 1:00 = 1:15h → worked = 7:45 = 7.75h
        let mut day = WorkDay::new(date());
        day.start(t(8, 0));
        day.start_break(t(10, 0));
        day.end_break(t(10, 15));
        day.start_break(t(12, 0));
        day.end_break(t(13, 0));
        day.end(t(17, 0));
        assert_eq!(day.worked_hours(t(17, 0)), 7.75);
        assert_eq!(day.breaks.len(), 2);
    }

    #[test]
    fn test_three_breaks() {
        // Start 8, three 15-min breaks, end 17
        // Gross = 9h, breaks = 3×15m = 45m → worked = 8:15 = 8.25h
        let mut day = WorkDay::new(date());
        day.start(t(8, 0));
        day.start_break(t(10, 0));
        day.end_break(t(10, 15));
        day.start_break(t(12, 0));
        day.end_break(t(12, 15));
        day.start_break(t(15, 0));
        day.end_break(t(15, 15));
        day.end(t(17, 0));
        assert_eq!(day.worked_hours(t(17, 0)), 8.25);
    }

    #[test]
    fn test_end_auto_closes_open_break() {
        let mut day = WorkDay::new(date());
        day.start(t(9, 0));
        day.start_break(t(16, 0));
        // Don't call end_break — end() should close it
        day.end(t(17, 0));
        assert_eq!(day.phase(), WorkPhase::Ended);
        assert_eq!(day.breaks[0].end, Some(t(17, 0)));
        // Gross 8h, break 1h → 7h worked
        assert_eq!(day.worked_hours(t(17, 0)), 7.0);
    }

    #[test]
    fn test_worked_during_second_break() {
        // Start 8, break1 10-10:15, break2 starts 12 (still open), now 13
        // Worked = (12:00 - 8:00) - 15min = 3:45 = 3.75h
        let mut day = WorkDay::new(date());
        day.start(t(8, 0));
        day.start_break(t(10, 0));
        day.end_break(t(10, 15));
        day.start_break(t(12, 0));
        assert_eq!(day.phase(), WorkPhase::OnBreak);
        assert_eq!(day.worked_hours(t(13, 0)), 3.75);
    }

    // --- unbooked_hours ---

    #[test]
    fn test_unbooked_positive() {
        let mut day = WorkDay::new(date());
        day.start(t(9, 0));
        day.end(t(17, 0));
        assert_eq!(day.unbooked_hours(6.0, t(17, 0)), 2.0);
    }

    #[test]
    fn test_unbooked_zero_when_over_logged() {
        let mut day = WorkDay::new(date());
        day.start(t(9, 0));
        day.end(t(17, 0));
        assert_eq!(day.unbooked_hours(9.0, t(17, 0)), 0.0);
    }

    #[test]
    fn test_unbooked_not_started() {
        let day = WorkDay::new(date());
        assert_eq!(day.unbooked_hours(0.0, t(9, 0)), 0.0);
    }

    // --- break_duration ---

    #[test]
    fn test_break_duration_incomplete() {
        let mut day = WorkDay::new(date());
        day.start(t(9, 0));
        day.start_break(t(12, 0));
        assert_eq!(day.break_duration(), Duration::zero());
    }

    #[test]
    fn test_break_duration_complete() {
        let mut day = WorkDay::new(date());
        day.start(t(9, 0));
        day.start_break(t(12, 0));
        day.end_break(t(13, 30));
        assert_eq!(day.break_duration(), Duration::minutes(90));
    }

    #[test]
    fn test_break_duration_multiple() {
        let mut day = WorkDay::new(date());
        day.start(t(8, 0));
        day.start_break(t(10, 0));
        day.end_break(t(10, 15)); // 15 min
        day.start_break(t(12, 0));
        day.end_break(t(13, 0)); // 60 min
        assert_eq!(day.break_duration(), Duration::minutes(75));
    }
}
