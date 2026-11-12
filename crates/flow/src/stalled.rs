//! A stalled task: its agent works, but its screen showed nothing new for `stall.after` (20 minutes by default), or it
//! ran past the Watcher's estimate by as much again (at least `stall.after`). Read off what the task keeps, so the
//! keeper and an open board judge alike: its screen's last change (watcher/output.rs) and the Watcher's guess with when
//! it last moved (`watch.movedAt`). Judged only while the board looks at that screen: a look older than FRESH_MS says
//! nothing. Each stall is one mark, `stalled:quiet:<since>:<at>` or `stalled:late:<guess>:<at>`, that automations
//! (automations/events.rs) and autopilot (autopilot/events.rs) fire once. A stall seen is remembered until something
//! re-arms it (new output, a new run, a guess that moved), also while the task does not stall for a moment (a dialog,
//! a status that flapped), so the same stall is never heard of twice. A hint that something may hang, never proof of
//! it: nothing is stopped.
use crate::flow::task::is_agent_task;
use crate::json::*;
use crate::shown::words::age;
use crate::state::{began, eta_of};
use crate::store::{retries::waiting_of, settings::{lookup::setting_text, values::ms}};
use crate::watcher::output::FRESH_MS;

/// How it stalled (`quiet`: nothing new on its screen; `late`: past the estimate), since when, and from when it counts.
#[derive(Debug, PartialEq)]
pub struct Stall { pub how: &'static str, pub since: i64, pub at: i64 }
/// Its stalls now: none, or one of each kind.
pub fn stalls(t: &Obj, now: i64) -> Vec<Stall> {
    // An agent at work: not a task of another machine, not one waiting for you or for the board to try again.
    if t.b("archivedAt") || t.b("host") || t.s("stage") != "working" || !is_agent_task(t) || t.at("last").s("status") != "working" || waiting_of(t).is_some() { return vec![]; }
    let o = t.at("output");
    if !o.b("seen") || now - o.i("seen") > FRESH_MS { return vec![]; }
    let after = ms(&setting_text("stall.after", Some(t)));
    if after <= 0 { return vec![]; }
    let run = began(t, "agent");
    let mut out = vec![];
    let quiet = run.max(o.i("at"));
    if now - quiet >= after { out.push(Stall { how: "quiet", since: quiet, at: quiet + after }); }
    // The Watcher's estimate when its guess last moved, counted from when the looks began if that was later.
    let w = t.at("watch");
    if w.s("role") == "agent" && w.b("movedAt") {
        if let Some(left) = eta_of(w.at("progress"), run, w.i("movedAt")) {
            let left = left * 60_000;
            let due = w.i("movedAt").max(o.i("from")) + left + left.max(after);
            if now >= due { out.push(Stall { how: "late", since: w.i("movedAt"), at: due }); }
        }
    }
    out
}
/// Its stalls as marks: `stalled:quiet:<since>:<at>`.
pub fn marks(t: &Obj, now: i64) -> Vec<String> { stalls(t, now).iter().map(|s| format!("stalled:{}:{}:{}", s.how, s.since, s.at)).collect() }
/// A mark as it is told apart: a stall by its kind and since, whenever it counts from (`stall.after` changed, the looks
/// began again); any other mark as it is.
pub fn key(mark: &str) -> &str { if mark.starts_with("stalled:") { mark.rsplit_once(':').map(|(k, _)| k).unwrap_or(mark) } else { mark } }
/// Is this mark new against those seen before?
pub fn is_new(mark: &str, had: &[String]) -> bool { !had.iter().any(|h| key(h) == key(mark)) }
/// The marks of a look, with the stalls seen before that still stand though the task does not stall right now.
pub fn with_kept(t: &Obj, mut marks: Vec<String>, had: &[String]) -> Vec<String> {
    for h in had.iter().filter(|h| h.starts_with("stalled:")) { if is_new(h, &marks) && stands(t, h) { marks.push(h.clone()); } }
    marks
}
/// A stall stands while its task still works and nothing re-armed it: for a quiet one no new output and no new run since,
/// for a late one the same guess.
fn stands(t: &Obj, mark: &str) -> bool {
    if t.b("archivedAt") || t.s("stage") != "working" { return false; }
    let mut p = mark.split(':').skip(1);
    let (how, since) = (p.next().unwrap_or(""), p.next().and_then(|x| x.parse::<i64>().ok()).unwrap_or(-1));
    match how {
        "quiet" => began(t, "agent").max(t.at("output").i("at")) <= since,
        "late" => t.at("watch").s("role") == "agent" && t.at("watch").i("movedAt") == since,
        _ => false,
    }
}
/// A stall's mark in a few words, after the task's name: `has shown nothing new for 25m`, `ran well past its estimate`.
pub fn say(mark: &str, now: i64) -> String {
    let mut p = mark.split(':').skip(1);
    match (p.next(), p.next().and_then(|x| x.parse::<i64>().ok())) {
        (Some("quiet"), Some(since)) => format!("has shown nothing new for {}", age(now - since)),
        _ => "ran well past its estimate".into(),
    }
}
