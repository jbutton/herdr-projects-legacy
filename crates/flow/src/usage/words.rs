//! Extra usage in words: the board's header (`Claude: extra usage since 14:10, resets 17:00`), a working task's row (`On
//! extra usage since 14:10`), a stop's line and a wait's, what a stopped agent is told when it carries on, and the
//! `usage` entry of `details`. Also which of them waits by itself and which waits for you, and the setting each program
//! goes by: its own (`usage.extra.codex`) where the nearest place says one, else `usage.extra`.
use crate::board::keys::key_name;
use crate::json::*;
use crate::store::settings::lookup::{lookup, Scope};
use crate::store::settings::places::yours;
use crate::store::usage::{now_of, Now, READ};
use chrono::{Local, TimeZone};

/// The setting, and the event `board.notify` is told (`HERDR_PROJECTS_EVENT`).
pub const SETTING: &str = "usage.extra";
pub const EVENT: &str = "extra usage";
/// The setting one program goes by: its own where the board can read its window (`usage.extra.codex`), else `usage.extra`.
pub fn name_for(kind: &str) -> String { if READ.contains(&kind) { format!("{SETTING}.{kind}") } else { SETTING.into() } }
/// What this program goes by on the task: `allow`, `ask` or `defer`, the setting that said it (its own, or `usage.extra`
/// where the nearest place says only that) and where it was said ("you (usage.extra)").
pub fn said_for(t: &Obj, kind: &str) -> (String, String, String) {
    let name = name_for(kind);
    match lookup(&name, &Scope::task(t), &yours()) {
        Ok(f) => (text(&f.value), if f.via.is_empty() { name } else { f.via }, f.from),
        Err(_) => ("allow".into(), SETTING.into(), "built in".into()),
    }
}
/// `allow`, `ask` or `defer` for this program, as the task reads it: the worker's, the reviewer's or a fallback's.
pub fn setting_for(t: &Obj, kind: &str) -> String { said_for(t, kind).0 }
/// The same from the task's settings as read at once (store/settings/lookup.rs: `work`).
pub fn in_work(w: &Obj, kind: &str) -> String { w.t(&name_for(kind)) }
/// "Claude", "Codex".
pub fn name(kind: &str) -> String { let mut c = kind.chars(); c.next().map(|f| f.to_uppercase().chain(c).collect()).unwrap_or_default() }
/// A moment as the clock on the wall says it: "17:00", with its day when it is not within the next day ("Tue 17:00").
pub fn hm(ms: i64, now: i64) -> String {
    let Some(d) = Local.timestamp_millis_opt(ms).single() else { return String::new() };
    if (ms - now).abs() < 20 * 3_600_000 { d.format("%H:%M").to_string() } else { d.format("%a %H:%M").to_string() }
}
fn until(resets: i64, now: i64) -> String { if resets > 0 { format!(" until {}", hm(resets, now)) } else { String::new() } }
/// One program in the header, where its window is known.
fn program_words(kind: &str, n: &Now, now: i64) -> String {
    let who = name(kind);
    if n.extra { return format!("{who}: extra usage since {}{}", hm(n.since, now), if n.resets > 0 { format!(", resets {}", hm(n.resets, now)) } else { String::new() }); }
    if n.hard { return format!("{who}: out of allowance{}", until(n.resets, now)); }
    if n.windows.is_empty() || now - n.at > 6 * 3_600_000 { return String::new(); }
    format!("{who}: {}", n.windows.iter().map(|w| format!("{} {}%", w.name, w.pct.round() as i64)).collect::<Vec<_>>().join(", "))
}
/// The header's words: each program whose window the board knows, "" when it knows none.
pub fn header(now: i64) -> String {
    READ.iter().map(|k| program_words(k, &now_of(k, now), now)).filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" · ")
}
/// Its agent now is the one seen on extra usage (a mark kept through a hand-over until `board.notify` was told is not).
pub fn on_it(t: &Obj) -> bool { t.at("extra").is_object() && t.at("extra").s("agent") == t.s("agent") }
/// A working task whose agent is on extra usage: what its row says before what the agent does.
pub fn row_line(t: &Obj) -> String { format!("On extra usage since {}", hm(t.at("extra").i("since"), crate::clock::now())) }
/// How a stop for extra usage holds now: as its program's setting reads now while it still stops (`ask`, `defer`:
/// changed while it waits, the stop follows), else as it stopped.
pub fn stop_how(t: &Obj) -> String { let s = setting_for(t, t.at("limitStop").s("limit")); if s == "ask" || s == "defer" { s } else { t.at("limitStop").t("how") } }
/// A stop for extra usage (limitStop `what: usage`) that waits for its window by itself, not for you: `defer` as the
/// setting reads now, its reset known.
pub fn waits_window(t: &Obj) -> bool {
    let ls = t.at("limitStop");
    t.s("stage") == "stopped" && ls.s("what") == "usage" && ls.i("resets") > 0 && !t.b("archivedAt") && stop_how(t) == "defer"
}
/// A queued task held back for extra usage that waits for you (`usageWait.you`), not for its window.
pub fn start_asks(t: &Obj) -> bool { t.s("stage") == "queued" && t.at("usageWait").b("you") && !t.b("archivedAt") }
/// Its agent was seen on extra usage in this episode, and `board.notify` has not been told yet.
pub fn untold(t: &Obj) -> bool { t.at("extra").is_object() && t.at("extra").at("window") != t.at("extraTold") && !t.b("archivedAt") }
/// The line of a task stopped for extra usage; a review's says so first ("Reviewing: waiting for the usage window").
pub fn stop_line(ls: &Obj, now: i64) -> String {
    let (kind, resets) = (ls.t("limit"), ls.i("resets"));
    let line = if ls.s("how") == "defer" && resets > 0 { format!("waiting for the usage window: resets {}", hm(resets, now)) }
        else if ls.s("how") == "defer" { format!("stopped: {kind} is on extra usage, and when its window resets is not known") }
        else { format!("stopped: {kind} moved onto extra usage at {}{}", hm(ls.i("used"), now), if resets > 0 { format!(" (resets {})", hm(resets, now)) } else { String::new() }) };
    if ls.s("role") == "reviewer" { format!("Reviewing: {line}") } else { line }
}
/// What it had come to when it stopped, for the task panel: "claude on extra usage since 14:10, its window resets 17:00".
pub fn used_say(ls: &Value) -> String {
    let now = crate::clock::now();
    let resets = if ls.i("resets") > 0 { format!(", its window resets {}", hm(ls.i("resets"), now)) } else { String::new() };
    if ls.b("hard") { format!("{} out of allowance{resets}", ls.t("limit")) } else { format!("{} on extra usage since {}{resets}", ls.t("limit"), hm(ls.i("used"), now)) }
}
/// What the stopped agent is told when it carries on: that the board stopped it, not the human, and why; that its
/// program was ended to hold it, where it was (stop_hold.rs); and why it goes on now.
pub fn carry_words(ls: &Value, raised: bool, t: &Obj) -> String {
    let why = if ls.b("hard") { "your program's usage window was used up" } else { "your program moved onto paid extra usage" };
    let go = if !raised { "The human said to carry on" } else if setting_for(t, ls.s("limit")) == "allow" { "The human allowed extra usage for this task" } else { "The usage window has reset" };
    format!("The board stopped you, not the human, because {why}: not because of anything you did.{} {go}: continue the task where you left off.", crate::stop_hold::yours::ended_words(t))
}
/// A queued task held back for extra usage: what it waits for. `by`: the setting that said `setting`.
pub fn start_line(kind: &str, resets: i64, you: bool, setting: &str, by: &str, now: i64) -> String {
    if !you { return format!("waiting for the usage window: resets {}", hm(resets, now)); }
    let why = if setting == "defer" { format!("{kind} is on extra usage, and when its window resets is not known") } else { format!("{kind} is on extra usage{}", until(resets, now)) };
    format!("waits for you: {why} ({by} {setting}); {} starts it anyway", key_name("sendBack"))
}
/// `details`: for a task whose program's window the board can read, where it stands: the program of its stop for extra
/// usage (its reviewer's, for a review), else its agent's; the setting it goes by, and what each program goes by.
pub fn detail(t: &Obj, now: i64) -> Value {
    let ls = t.at("limitStop");
    let kind = if ls.s("what") == "usage" && ls.b("limit") { ls.t("limit") } else if t.b("on") { crate::store::settings::values::agent_of(t.s("on")).kind } else { t.t("agent") };
    if !READ.contains(&kind.as_str()) { return Value::Null; }
    let n = now_of(&kind, now);
    let at = |ms: i64| if ms > 0 { json!(crate::clock::iso(ms)) } else { Value::Null };
    let (setting, by, from) = said_for(t, &kind);
    let each: Obj = READ.iter().map(|k| { let (v, by, from) = said_for(t, k); (k.to_string(), json!({ "value": v, "setting": by, "from": from })) }).collect();
    let mut out = obj(json!({ "agent": kind, "setting": setting, "settingName": by, "settingFrom": from, "settings": each, "known": n.known, "windows": n.windows, "onExtraUsage": n.extra, "outOfAllowance": n.hard, "resets": if n.resets > 0 { json!(hm(n.resets, now)) } else { Value::Null }, "resetsAt": at(n.resets) }));
    let x = t.at("extra");
    if x.is_object() { out.put("extraAgent", x.at("agent").clone()); out.put("extraSince", hm(x.i("since"), now)); out.put("extraSinceAt", at(x.i("since"))); out.put("words", row_line(t)); }
    if t.at("limitStop").s("what") == "usage" { out.put("stopped", t.at("limitStop").clone()); }
    if t.at("usageWait").is_object() { out.put("waiting", t.at("usageWait").clone()); }
    if t.at("extraOk").is_object() { out.put("allowedFor", t.at("extraOk").clone()); }
    Value::Object(out)
}
