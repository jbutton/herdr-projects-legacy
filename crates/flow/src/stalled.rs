//! A stalled task: its agent works, but its screen showed nothing new for `stall.after` (20 minutes by default), or it
//! ran past the Watcher's estimate by as much again (at least `stall.after`). Read off what the task keeps, so the
//! keeper and an open board judge alike: its screen's last change (watcher/output.rs) and the Watcher's guess with when
//! it last moved (`watch.movedAt`). Judged only while the board looks at that screen: a look older than FRESH_MS says
//! nothing. Each stall is one mark, `stalled:quiet:<since>:<at>` or `stalled:late:<guess>:<at>`, that automations
//! (automations/events.rs) and autopilot (autopilot/events.rs) fire once; new output, or a guess that moved, makes the
//! next stall a new mark. A hint that something may hang, never proof of it: nothing is stopped.
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
/// A stall's mark in a few words, after the task's name: `has shown nothing new for 25m`, `ran well past its estimate`.
pub fn say(mark: &str, now: i64) -> String {
    let mut p = mark.split(':').skip(1);
    match (p.next(), p.next().and_then(|x| x.parse::<i64>().ok())) {
        (Some("quiet"), Some(since)) => format!("has shown nothing new for {}", age(now - since)),
        _ => "ran well past its estimate".into(),
    }
}
