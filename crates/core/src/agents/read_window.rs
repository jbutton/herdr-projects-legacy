//! A reading of an agent program's usage window, from what it saved (docs/extra-usage.md): Codex's last `token_count`
//! (its `rate_limits`: the 5-hour and weekly windows and its credits), Claude Code's status line input as the board's
//! own status line kept it (agents/statusline.rs), and a request Claude Code saw refused (`quotaLimits`, its hard stop).
//! On extra usage: a window at 100% or more while answers are still served. Read from files only: no model call.
use super::read_turn::{tail_lines, when};
use crate::json::*;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// One window: its name (`5h`, `week`), how much of it is used (percent) and when it resets (ms; 0: not known).
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Window { pub name: String, pub pct: f64, pub resets: i64 }
/// `at`: when it was read (ms). `extra`: on extra usage. `hard`: refused, the window used up with nothing to pay on.
/// `resets`: when the full windows reset (ms; the later one when both are full; 0: not known).
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Reading { pub at: i64, pub windows: Vec<Window>, pub extra: bool, pub hard: bool, pub resets: i64 }
/// A window used up: at or past this percent.
pub const FULL: f64 = 100.0;

impl Reading {
    /// Any window used up.
    pub fn full(&self) -> bool { self.windows.iter().any(|w| w.pct >= FULL) }
    /// When the full windows reset: the later one; 0 when one of them does not say.
    fn full_resets(&self) -> i64 {
        let full: Vec<&Window> = self.windows.iter().filter(|w| w.pct >= FULL).collect();
        if full.is_empty() || full.iter().any(|w| w.resets <= 0) { 0 } else { full.iter().map(|w| w.resets).max().unwrap_or(0) }
    }
    /// As it stands at `now`: a window whose reset has passed is a fresh one and goes, and so does what it said.
    pub fn at_now(&self, now: i64) -> Reading {
        let mut r = self.clone();
        r.windows.retain(|w| w.resets <= 0 || now < w.resets);
        let over = self.resets > 0 && now >= self.resets;
        r.extra = self.extra && r.full() && !over;
        r.hard = self.hard && !over;
        if over { r.resets = 0; }
        r
    }
}
fn name_of(minutes: i64) -> String {
    match minutes { 300 => "5h".into(), 10080 => "week".into(), m if m > 0 && m % 1440 == 0 => format!("{}d", m / 1440), m if m > 0 && m % 60 == 0 => format!("{}h", m / 60), m => format!("{m}m") }
}
/// Seconds since 1970, as all three sources say a reset, in milliseconds (0: none).
fn ms_of(v: &Value) -> i64 { (number(v) * 1000.0) as i64 }
/// Do these words say the allowance is used up? (The board's own reading of a provider's words: store/handovers.rs.)
fn used_up(words: &str) -> bool { !words.is_empty() && crate::store::handovers::why_of(words, false, false, "", 0, 0).starts_with("out of allowance") }

/// Codex: its last `token_count` with `rate_limits`, and an error after it that says the allowance is used up.
pub fn codex_reading(rows: &[Value]) -> Option<Reading> {
    let mut last: Option<Reading> = None;
    for e in rows.iter().filter(|e| e.s("type") == "event_msg") {
        let p = e.at("payload");
        if p.s("type") == "token_count" && p.at("rate_limits").is_object() {
            let r = p.at("rate_limits");
            let windows = ["primary", "secondary"].iter().map(|k| r.at(k)).filter(|w| w.is_object())
                .map(|w| Window { name: name_of(w.i("window_minutes")), pct: w.f("used_percent"), resets: ms_of(w.at("resets_at")) }).collect();
            let pays = r.at("credits").b("has_credits") || r.at("credits").b("unlimited");
            let mut x = Reading { at: when(e.at("timestamp")), windows, hard: r.b("rate_limit_reached_type"), ..Default::default() };
            x.resets = x.full_resets();
            x.extra = x.full() && pays && !x.hard;
            last = Some(x);
            continue;
        }
        let err = match p.s("type") {
            "task_complete" if p.b("error") => { let x = p.at("error"); if x.is_object() { x.t("message") } else { text(x) } }
            "error" | "stream_error" => if p.b("message") { p.t("message") } else { p.t("error") },
            _ => continue,
        };
        if !used_up(&err) { continue; }
        let mut x = last.take().unwrap_or_default();
        x.at = when(e.at("timestamp")).max(x.at);
        x.hard = true; x.extra = false;
        last = Some(x);
    }
    last
}
/// Claude Code: a request it saw refused for the allowance (`quotaLimits`), when no answer was served after it.
pub fn claude_reading(rows: &[Value]) -> Option<Reading> {
    let mut last: Option<Reading> = None;
    for e in rows.iter().filter(|e| e.s("type") == "assistant") {
        let q = if e.at("quotaLimits").is_object() { e.at("quotaLimits") } else { e.at("apiErrorParams").at("rate_limit_info") };
        let refused = e.b("isApiErrorMessage") || e.at("message").s("model") == "<synthetic>";
        if refused && q.is_object() && (e.s("apiError") == "usage_limit_reached" || q.s("status") == "rejected") {
            let resets = ms_of(q.at("resetsAt"));
            let name = match q.s("rateLimitType") { "five_hour" => "5h".to_string(), "seven_day" => "week".into(), "" => "limit".into(), o => o.into() };
            last = Some(Reading { at: when(e.at("timestamp")), windows: vec![Window { name, pct: FULL, resets }], hard: true, resets, ..Default::default() });
        } else if !refused { last = None; } // an answer was served after it: that stop is over
    }
    last
}
/// Claude Code's status line input as the board kept it (`{at, rate_limits}`): its 5-hour and weekly windows. It comes
/// after an answer was served, so a window at 100% here is extra usage.
pub fn status_reading(kept: &Value) -> Option<Reading> {
    let r = kept.at("rate_limits");
    if !r.is_object() { return None; }
    let windows: Vec<Window> = [("five_hour", "5h"), ("seven_day", "week")].iter().filter(|(k, _)| r.at(k).is_object())
        .map(|(k, n)| Window { name: n.to_string(), pct: r.at(k).f("used_percentage"), resets: ms_of(r.at(k).at("resets_at")) }).collect();
    let mut x = Reading { at: kept.i("at"), windows, ..Default::default() };
    x.resets = x.full_resets();
    x.extra = x.full();
    Some(x)
}
/// A program's reading from a conversation's last lines.
pub fn window_in(kind: &str, rows: &[Value]) -> Option<Reading> {
    match kind { "codex" => codex_reading(rows), "claude" => claude_reading(rows), _ => None }
}
/// The newer of two readings.
pub fn newest(a: Option<Reading>, b: Option<Reading>) -> Option<Reading> {
    match (a, b) { (Some(a), Some(b)) => Some(if b.at > a.at { b } else { a }), (a, b) => a.or(b) }
}
/// Where the board keeps a Claude agent's status line input: beside the task, one file per role.
pub fn status_file(dir: &Path, role: &str) -> PathBuf { dir.join(format!("window-{role}.json")) }
/// The newest Codex conversation of this machine (today's or yesterday's), yours or a task's: Codex's window before any
/// task of the board ran it.
pub fn codex_newest() -> Option<Reading> {
    let root = super::homes::codex_home().join("sessions");
    use chrono::TimeZone;
    let today = chrono::Local.timestamp_millis_opt(crate::clock::now()).single()?.date_naive();
    let days = [Some(today), today.pred_opt()];
    let files = days.iter().flatten().flat_map(|d| super::read_turn::find_file(&root.join(d.format("%Y/%m/%d").to_string()), &|n| n.ends_with(".jsonl"), 0));
    let newest = files.max_by_key(|f| std::fs::metadata(f).and_then(|m| m.modified()).ok())?;
    codex_reading(&tail_lines(&newest, 256 * 1024).ok()?)
}
