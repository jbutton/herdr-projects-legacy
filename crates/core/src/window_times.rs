//! When a `run.window` is open, in this machine's time zone (docs/run-window.md): at a moment or not, when it next
//! opens, and the open stretch around a moment. Each window is laid out on the days around the moment, and windows that
//! touch or overlap join up: `mon-fri 22:00-06:00, weekends` is open from Friday 22:00 to Monday 00:00 in one stretch. A
//! time the clock skips (the night summer time starts) is taken as the first moment after it; one it repeats, first.
use crate::store::settings::run_window::{windows_of, Window};
use chrono::{Datelike, Duration, Local, NaiveDate, TimeZone};

/// Days laid out before and after the moment: a window open for days in a row may have begun a week back (`thu-sun`, or
/// every day but one), and every window opens again within a week.
const BACK: i64 = 8;
const AHEAD: i64 = 8;

/// A day's minute as a moment here, in ms: 1440 is the next day's midnight.
fn moment(d: NaiveDate, min: u32) -> i64 {
    let at = d.and_hms_opt(0, 0, 0).unwrap_or_default() + Duration::minutes(min as i64);
    Local.from_local_datetime(&at).earliest().or_else(|| (1..=180).find_map(|m| Local.from_local_datetime(&(at + Duration::minutes(m))).earliest())).map(|t| t.timestamp_millis()).unwrap_or(0)
}
/// The open stretches around `at` (from, to: to not included), in order and joined where they touch.
fn stretches(ws: &[Window], at: i64) -> Vec<(i64, i64)> {
    let Some(today) = Local.timestamp_millis_opt(at).single().map(|t| t.date_naive()) else { return vec![] };
    let mut all: Vec<(i64, i64)> = vec![];
    for n in -BACK..=AHEAD {
        let d = today + Duration::days(n);
        let wd = d.weekday().num_days_from_monday();
        for w in ws.iter().filter(|w| w.days.contains(&wd)) {
            let to = if w.to > w.from { moment(d, w.to) } else { moment(d + Duration::days(1), w.to) };
            all.push((moment(d, w.from), to));
        }
    }
    all.sort();
    let mut out: Vec<(i64, i64)> = vec![];
    for (a, b) in all {
        match out.last_mut() { Some(last) if a <= last.1 => last.1 = last.1.max(b), _ => out.push((a, b)) }
    }
    out
}
/// A stretch open a whole week runs into its own next week: the value is open around the clock (`weekdays, weekends`),
/// as `any` is, and never closes.
const WEEK: i64 = 7 * 24 * 3_600_000;
fn around(value: &str, at: i64) -> Option<(i64, i64)> { stretches(&windows_of(value), at).into_iter().find(|(a, b)| *a <= at && at < *b) }
fn always(value: &str, at: i64) -> bool { windows_of(value).is_empty() || around(value, at).is_some_and(|(a, b)| b - a >= WEEK) }
/// Is the window open at `at`? `any`, and a value open around the clock, always is.
pub fn open(value: &str, at: i64) -> bool { around(value, at).is_some() || windows_of(value).is_empty() }
/// The open stretch around `at`: when it opened and when it closes. None while it is closed, for `any`, and for a value
/// open around the clock.
pub fn stretch(value: &str, at: i64) -> Option<(i64, i64)> { around(value, at).filter(|(a, b)| b - a < WEEK) }
/// When it next opens after `at` (while it is closed); 0 for `any`, and for a value open around the clock.
pub fn next_open(value: &str, at: i64) -> i64 { if always(value, at) { return 0; } stretches(&windows_of(value), at).into_iter().map(|(a, _)| a).find(|a| *a > at).unwrap_or(0) }

#[cfg(test)]
mod tests {
    use super::*;
    /// A moment here: `days` after Monday 2026-11-16, at h:m.
    fn at(days: i64, h: u32, m: u32) -> i64 { moment(NaiveDate::from_ymd_opt(2026, 11, 16).unwrap() + Duration::days(days), h * 60 + m) }
    #[test]
    fn a_window_across_midnight_belongs_to_the_day_it_began_on() {
        let v = "mon-fri 22:00-06:00";
        assert!(!open(v, at(0, 21, 59)) && open(v, at(0, 22, 0)) && open(v, at(1, 5, 59)) && !open(v, at(1, 6, 0)));
        assert_eq!(next_open(v, at(0, 14, 0)), at(0, 22, 0));
        // Friday night runs into Saturday morning; Saturday night is not one.
        assert!(open(v, at(5, 3, 0)) && !open(v, at(5, 23, 0)));
        assert_eq!(next_open(v, at(5, 7, 0)), at(7, 22, 0));
        assert_eq!(stretch(v, at(2, 23, 0)), Some((at(2, 22, 0), at(3, 6, 0))));
    }
    #[test]
    fn windows_that_touch_are_one_stretch() {
        let v = "mon-fri 22:00-06:00, weekends";
        // Sunday night is no weekday's: the stretch ends with the weekend, at Monday 00:00.
        assert_eq!(stretch(v, at(5, 12, 0)), Some((at(4, 22, 0), at(7, 0, 0))));
        assert_eq!(next_open("weekends", at(2, 9, 0)), at(5, 0, 0));
        assert_eq!(stretch("sat 00:00-24:00", at(5, 10, 0)), Some((at(5, 0, 0), at(6, 0, 0))));
        // Open four whole days: let go on Sunday, it opened on Thursday.
        assert_eq!(stretch("thu-sun", at(6, 12, 0)), Some((at(3, 0, 0), at(7, 0, 0))));
        assert!(open("any", at(0, 3, 0)) && next_open("any", at(0, 3, 0)) == 0 && stretch("any", at(0, 3, 0)).is_none());
        assert!(open("nights", at(3, 23, 30)) && !open("nights", at(3, 12, 0)));
    }
    #[test]
    fn a_value_open_around_the_clock_never_closes() {
        for v in ["weekdays, weekends", "mon-sun", "nights, 06:00-22:00"] {
            assert!(open(v, at(2, 9, 0)) && stretch(v, at(2, 9, 0)).is_none() && next_open(v, at(2, 9, 0)) == 0, "{v}");
        }
    }
    #[test]
    fn a_time_the_clock_skips_is_the_first_moment_after_the_gap() {
        // The night summer time starts, where this machine's time zone has one at 02:00 (as Europe's, on 2027-03-28).
        let day = NaiveDate::from_ymd_opt(2027, 3, 28).unwrap();
        if Local.from_local_datetime(&day.and_hms_opt(2, 30, 0).unwrap()).earliest().is_some() { return; }
        let three = Local.from_local_datetime(&day.and_hms_opt(3, 0, 0).unwrap()).earliest().unwrap().timestamp_millis();
        assert_eq!(moment(day, 150), three);
        assert!(open("02:30-05:00", three + 10 * 60_000));
    }
}
