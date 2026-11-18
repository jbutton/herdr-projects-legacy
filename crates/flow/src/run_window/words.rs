//! Run windows in words: a start held (`waiting for its window: starts 22:00`), a task paused at its window's end
//! (`waiting for its window: continues 22:00`), the row of one let go or carried on late (`Started late: its window
//! opened at 22:00`), and the `window` entry of `details`. The clock times are said as extra usage says them
//! (usage/words.rs: `hm`, with the day when it is not within the next day).
use crate::json::*;
use crate::store::settings::lookup::setting_text;
use crate::usage::words::hm;
use crate::window_times::{next_open, open, stretch};

/// The settings, as their names are written.
pub const SETTING: &str = "run.window";
pub const END: &str = "run.windowEnd";
/// What a task held or paused waits for, in a word or two (herdr's sidebar).
pub const WAITS: &str = "waiting for its window";

/// "run.window nights": the setting as the task reads it, for a line that says why.
pub fn named(t: &Obj) -> String { format!("{SETTING} {}", setting_text(SETTING, Some(t))) }
/// A queued task held for its window.
pub fn start_line(opens: i64, now: i64) -> String { format!("{WAITS}: starts {}", hm(opens, now)) }
/// A task paused at its window's end (`limitStop.what: window`), or one waiting for it to carry on (pause.rs).
pub fn stop_line(opens: i64, now: i64) -> String { format!("{WAITS}: continues {}", hm(opens, now)) }
/// What it had come to when it paused, for the task panel and `finish`: "paused when its window (nights) had closed".
pub fn used_say(ls: &Value) -> String { format!("paused when its window ({}) had closed", ls.t("limit")) }
/// Stopped at its window's end, to carry on by itself when the window opens again (pause.rs).
pub fn paused(t: &Obj) -> bool { t.s("stage") == "stopped" && t.at("limitStop").s("what") == "window" && !t.b("archivedAt") }
/// Let go or carried on well after its window opened, and no turn of its has ended since: its row says so.
pub fn late(t: &Obj) -> bool { let l = t.at("windowLate"); l.is_object() && t.at("handled").i("at") < l.i("at") && !t.b("archivedAt") }
/// "Started late: its window opened at 22:00", before what the agent is doing.
pub fn late_line(t: &Obj) -> String {
    let l = t.at("windowLate");
    format!("{} late: its window opened at {}", if l.s("how") == "continued" { "Continued" } else { "Started" }, hm(l.i("opened"), crate::clock::now()))
}
/// `details`: the task's window as it stands, and what it waits for. Nothing for a task with no window and no mark.
pub fn detail(t: &Obj, now: i64) -> Value {
    let win = setting_text(SETTING, Some(t));
    let held = super::gate::start_waits(t);
    let marked = held || ["windowLate", "windowOk"].iter().any(|k| t.at(k).is_object()) || paused(t);
    if win == "any" && !marked { return Value::Null; }
    let at = |ms: i64| if ms > 0 { json!(crate::clock::iso(ms)) } else { Value::Null };
    let say = |ms: i64| if ms > 0 { json!(hm(ms, now)) } else { Value::Null };
    let around = stretch(&win, now);
    let opens = if around.is_some() { 0 } else { next_open(&win, now) };
    let closes = around.map(|s| s.1).unwrap_or(0);
    let words = if paused(t) || held { t.t("msg") } else if late(t) { late_line(t) } else if closes > 0 { format!("its window is open until {}", hm(closes, now)) } else if opens > 0 { format!("its window opens {}", hm(opens, now)) } else { String::new() };
    let mut out = obj(json!({ "setting": win, "end": setting_text(END, Some(t)), "open": open(&win, now), "opens": say(opens), "opensAt": at(opens), "closes": say(closes), "closesAt": at(closes), "words": words }));
    if held { out.put("waiting", t.at("windowWait").clone()); }
    if paused(t) { out.put("paused", t.at("limitStop").clone()); }
    if t.at("windowLate").is_object() { let mut l = t.o("windowLate").cloned().unwrap_or_default(); l.put("words", late_line(t)); out.put("late", l); }
    if t.at("windowOk").is_object() { out.put("leave", t.at("windowOk").clone()); }
    Value::Object(out)
}
