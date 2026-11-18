//! At the window's end, under `run.windowEnd pause` (docs/run-window.md): once the task's window has closed, the board
//! starts no new step of its run by itself. The step under way ends (its agent's turn, the check, a review); then the
//! task stops with the limits' own stop (limits/reached.rs, `limitStop.what: window`), its work kept as it is, as
//! "waiting for its window: continues 22:00". It stops at the end of its agent's turn, before the finish line (`then:
//! finish`: finish/turn_ended.rs); at a send-back the board starts itself, the words kept (`then: send`: flow/send.rs);
//! before a review (`role: reviewer`: finish/review.rs). The limits' tick carries it on by itself once the window is
//! open again (`lifted`, `carry_on`), as it carries on a stop for extra usage at its reset (PRJ-26). Never held: an
//! approved merge on its way, a chat-only task, a task with your leave (gate.rs: `outside`).
use super::gate::{outside, window_of, LATE};
use super::words::{stop_line, END};
use crate::actions::shared::{say, Said};
use crate::clock::now;
use crate::err::Res;
use crate::json::*;
use crate::limits::reached::{stop_at, StopHow};
use crate::limits::run::{granted, stopped_at_limit};
use crate::runner::{runner_alive, spawn_runner};
use crate::store::settings::lookup::setting_text;
use crate::store::tasks::{get, update, update_wait, Change, Task};
use crate::window_times::{next_open, stretch};

/// The send-backs of a run (flow/send.rs: send_own's reasons) that wait for the window. A repair of finished work (a
/// conflict with main, GitHub's checks) and a turn a provider cut off (`retry`) go on.
pub const HELD: [&str; 4] = ["check", "review", "research", "plan"];

/// Does `pause` hold this task's next step now? Not for a repair of work that waited for you (`windowRepair`).
pub fn holds(t: &Obj, at: i64) -> bool { setting_text(END, Some(t)) == "pause" && !t.b("mergeIntent") && !t.b("windowRepair") && outside(t, at) }
/// The turn that ended repaired work that had waited for you: the board sent it a conflict with main or GitHub's failed
/// checks (`reason`, its wake), or the pull request's feedback or a failure to fix (sent for this very turn).
fn repair(t: &Obj, reason: &str) -> bool { ["conflict", "checks"].contains(&reason) || ["failure", "prFailure"].iter().any(|k| t.at(k).i("sentAt") >= t.i("armedAt").max(1)) }
fn holds_now(id: &str) -> bool { get(id).is_some_and(|t| t.s("stage") != "stopped" && holds(&t, now())) }
/// Stops the task to wait for its window: `role` whose step waits, `extra` what carrying on does.
fn stop(id: &str, role: &str, mut extra: Obj) -> bool {
    let Some(t) = get(id) else { return false };
    let (win, at) = (window_of(&t), now());
    extra.put("opens", next_open(&win, at));
    stop_at(id, "window", &win, 0.0, StopHow { role: role.into(), extra, ..Default::default() }).is_some()
}
/// At the end of the agent's turn, before the finish line: true when the task stopped there. `reason`: the board's own
/// wake that started the turn. A repair of work that waited for you goes on, and so does all that follows it (its
/// check, its review, what that sends back) until the task is delegated again.
pub fn at_turn_end(id: &str, reason: &str) -> bool {
    if !holds_now(id) { return false; }
    if get(id).is_some_and(|t| repair(&t, reason)) { let _ = update_wait(id, 2000, |t| { t.put("windowRepair", true); Ok(Change::Write) }); return false; }
    stop(id, "agent", obj(json!({ "then": "finish" })))
}
/// Before a review starts: true when the task stopped to wait for its window (carrying on checks and reviews it).
pub fn before_review(id: &str) -> bool { holds_now(id) && stop(id, "reviewer", Obj::new()) }
/// A send-back the board starts itself: true when the words were kept for the window instead.
pub fn hold_send(id: &str, text: &str, reason: &str, keep_intent: bool, patch: &Obj, tries: i64) -> bool {
    if !HELD.contains(&reason) || !holds_now(id) { return false; }
    stop(id, "agent", obj(json!({ "then": "send", "send": { "text": text, "reason": reason, "keepIntent": keep_intent, "patch": patch, "tries": tries } })))
}
/// Is a task stopped at its window's end free to carry on by itself: its window open, or the settings no longer pause it?
pub fn lifted(t: &Obj) -> bool { !holds(t, now()) }
/// A task stopped to wait for its window (its own stop, or a stop for extra usage whose reset came while its window is
/// closed): its line says until when, as the window stands now. Seen by the limits' tick.
pub fn follow(t: &Obj) {
    let (ls, at) = (t.at("limitStop"), now());
    if t.s("stage") != "stopped" || t.b("archivedAt") { return; }
    let waits = match ls.s("what") { "window" => !lifted(t), "usage" => crate::usage::stop::lifted(t) && holds(t, at), _ => false };
    let opens = next_open(&window_of(t), at);
    if !waits || opens == 0 || t.s("msg") == stop_line(opens, at) { return; }
    let _ = update_wait(t.s("id"), 2000, |t| {
        if !stopped_at_limit(t) { return Ok(Change::Keep); }
        if t.at("limitStop").s("what") == "window" { t.obj_mut("limitStop").put("opens", opens); }
        t.put("msg", stop_line(opens, at));
        Ok(Change::Write)
    });
}
/// Carried on well after its window opened (the machine slept, the board was not running): its row says so.
pub fn mark_late(t: &Task) {
    let at = now();
    let Some((since, _)) = stretch(&window_of(t), at) else { return };
    if t.at("limitStop").i("at") >= since || at - since <= LATE { return; }
    let _ = update_wait(t.s("id"), 2000, |t| { t.put("windowLate", json!({ "opened": since, "at": at, "how": "continued" })); Ok(Change::Write) });
}
/// Carry on a task stopped at its window's end with what it kept (actions/limits.rs: carry_on): `finish`, its work
/// through the finish line as if its turn had just ended; `send`, the words that were to go back. `raised`: the board
/// does it, the window open again; said by you, it has leave until its window opens (limits/run.rs: `granted`).
pub fn carry_on(cur: &Task, raised: bool) -> Res<Said> {
    let cid = cur.t("id");
    let ls = cur.at("limitStop").clone();
    let lift = |t: &mut Task| -> Res<()> {
        if !stopped_at_limit(t) || runner_alive(t) { crate::fail!("{} is busy. Try again in a moment.", t.s("id")); }
        if raised { t.put("limitStop", Value::Null); t.put("runCarry", Value::Null); } else { granted(t); }
        Ok(())
    };
    if ls.s("then") == "send" {
        let s = ls.at("send");
        update(&cid, |t| { lift(t)?; Ok(Change::Write) })?;
        let sent = crate::flow::send::send_own(&cid, s.s("text"), s.s("reason"), s.b("keepIntent"), s.o("patch").cloned().unwrap_or_default(), s.i("tries"));
        // Nothing reached the agent: the stop is put back, for the next pass (or you) to try again.
        if let Err(e) = sent { let _ = update_wait(&cid, 10000, |t| { if t.s("stage") != "stopped" { return Ok(Change::Keep); } t.put("limitStop", ls.clone()); Ok(Change::Write) }); return Err(e); }
        return Ok(say(get(&cid).unwrap_or_else(|| cur.clone()), format!("{cid}: carrying on, what waited for its window went to the agent")));
    }
    crate::flow::live::assert_live(cur)?;
    let t = update(&cid, |t| { lift(t)?; t.put("stage", "checking"); t.put("msg", ""); t.put("since", now()); Ok(Change::Write) })?;
    spawn_runner("pipeline", &cid, &[]);
    Ok(say(t, format!("{cid}: carrying on, its work goes through the check and the review")))
}
