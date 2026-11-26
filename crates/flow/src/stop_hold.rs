//! A stop holds the whole agent (docs/stopping.md): the interrupt ends the agent's turn, not the helpers it runs in the
//! background (Claude Code's subagents), and each one that comes back wakes it again. So every stop of a live agent, by
//! you or by the board, is watched (`hold` on the task): its saved conversation, or a helper's beside it
//! (`<conversation>/subagents/`), still growing a minute after the interrupt ends the agent's program. A stopped agent
//! that wakes by itself is watched the same way, and never re-armed (woke.rs). Run by whoever ticks, beside the limits'.
pub mod woke;
pub mod yours;

use crate::agents::installed::file_for;
use crate::json::*;
use crate::store::repos::cwd_of;
use crate::store::tasks::{update_wait, Change, Task};
use std::path::Path;

/// The first look this long after the stop takes the size of its conversation (the interrupt's own lines are written
/// by then); a look this long after that one says whether it still grows: a minute after the interrupt.
pub const SETTLE_MS: i64 = 30_000;

/// The agent in the task's pane is watched from `now` (its stop, or its waking by itself after one).
pub fn begin(t: &mut Task, now: i64) { t.put("hold", json!({ "at": now })); }
/// The same, unless it is watched already.
pub fn watch(t: &mut Task, now: i64) { if !t.at("hold").is_object() { begin(t, now); } }
/// The files of the agent's conversation, each with its size: its own, and its helpers' under `<conversation>/subagents/`.
/// None: no conversation the board can find.
pub fn sizes(t: &Obj) -> Option<Obj> {
    let f = file_for(t.s("agent"), t.so("transcript").map(Path::new), t.s("session"), &cwd_of(t))?;
    let mut out = Obj::new();
    let mut add = |p: &Path| { if let Ok(m) = std::fs::metadata(p) { out.put(&p.display().to_string(), m.len()); } };
    add(&f);
    let helpers = std::fs::read_dir(f.with_extension("").join("subagents")).into_iter().flatten().flatten().map(|e| e.path());
    for p in helpers.filter(|p| p.extension().is_some_and(|x| x == "jsonl")) { add(&p); }
    Some(out)
}
/// Has any of them grown since `before` (or is one there that was not)?
fn grown(before: &Value, now: &Obj) -> bool { now.iter().any(|(f, n)| !before.has(f) || n.as_i64().unwrap_or(0) > before.i(f)) }
/// A helper it started is still out, as its conversation says (agents/helpers.rs): it may wait on a long command
/// without a word, and its answer would wake the agent.
fn helpers_out(t: &Obj) -> bool {
    let Some(f) = file_for(t.s("agent"), t.so("transcript").map(Path::new), t.s("session"), &cwd_of(t)) else { return false };
    crate::agents::helpers::running(t.s("agent"), &f, t.i("startedAgentAt")) > 0
}
/// Change the task's watch, while it is still the one that began at `at`.
fn change(id: &str, at: i64, f: impl FnOnce(&mut Task)) { let _ = update_wait(id, 2000, |t| { if t.at("hold").i("at") != at { return Ok(Change::Keep); } f(t); Ok(Change::Write) }); }

/// `rows`: the board's list as this herdr session shows it, attempts among them (flow/live.rs).
pub fn hold_tick(rows: &[Obj], now: i64) {
    for r in rows.iter().filter(|r| r.at("hold").is_object() && !r.at("hold").b("ended")) {
        let (id, h, live) = (r.t("id"), r.at("hold"), r.at("live"));
        let at = h.i("at");
        if live.b("away") || live.b("down") { continue; } // another herdr session's, or herdr does not answer: not now
        // Carried on, archived, or no agent left in its pane to hold: nothing to watch.
        if r.s("stage") != "stopped" || r.b("archivedAt") || live.at("status").is_null() { change(&id, at, |t| { t.del("hold"); }); continue; }
        if now < at + SETTLE_MS { continue; }
        let Some(files) = sizes(r) else { change(&id, at, |t| { t.del("hold"); }); continue };
        if !h.b("seen") { change(&id, at, |t| { let o = t.obj_mut("hold"); o.put("sizes", files); o.put("seen", now); }); continue; }
        if now < h.i("seen") + SETTLE_MS { continue; }
        // Still written to, and by nobody's prompt (a reply typed in its terminal re-arms it: woke.rs): its program ends.
        if grown(h.at("sizes"), &files) && woke::typed_since(r, r.i("stoppedAt")) != Some(true) { end(r, at, now); continue; }
        if helpers_out(r) { change(&id, at, |t| { let o = t.obj_mut("hold"); o.put("sizes", files); o.put("seen", now); }); }
        else { change(&id, at, |t| { t.del("hold"); }); } // nothing more written, and nothing of its out: it is held
    }
}
/// End the agent's program, as a hand-over does (its helpers in it go with it): a reply, retry or the board's carrying on
/// start it again in its own conversation. Said on a stop of yours; one at a limit keeps its own line.
fn end(r: &Obj, at: i64, now: i64) {
    let id = r.t("id");
    if !crate::store::tasks::get(&id).is_some_and(|t| t.s("stage") == "stopped" && t.at("hold").i("at") == at) { return; } // carried on since the rows were read
    let gone = crate::flow::task::h(r).pane_quit(r.at("link").s("pane"), 6000);
    crate::hook::log(&format!("stop hold: {id}: still at work a minute after its stop, its program {}", if gone { "was ended" } else { "would not end" }));
    change(&id, at, |t| {
        t.obj_mut("hold").put(if gone { "ended" } else { "stuck" }, now);
        if !gone { t.obj_mut("hold").put("seen", now); } // tried again half a minute on, not at every look
        if t.at("limitStop").is_object() || t.s("stage") != "stopped" { return; }
        t.put("msg", if gone { "stopped: its helpers worked on after the stop, so its program was ended" } else { "stopped, but its helpers work on and its program would not end: close its terminal" });
    });
    if let Some(t) = crate::store::tasks::get(&id) { crate::flow::sidebar::tell_sidebar(&t); }
}
