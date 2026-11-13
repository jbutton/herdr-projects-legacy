//! The extra usage pass, run by whoever ticks (the keeper, or an open board), beside the limits' own: each working
//! agent's reading (at most every few seconds, and only what its files gained since: agents/read_kept.rs), kept as its
//! program's window (store/usage.rs); the task marked while its agent is on extra usage (`extra`: since when, until
//! when); and, under `ask` and `defer`, the agent stopped there and then (stop.rs). The newest Codex conversation of the
//! machine is read once a minute, so Codex's window is known before a task runs it.
use super::gate::extra_ok;
use super::stop::{fallback_for, stop_now};
use super::words::{setting_of, untold};
use crate::agents::read_window::{codex_newest, newest, status_file, status_reading, Reading};
use crate::flow::task::is_agent_task;
use crate::json::*;
use crate::store::files::{dir_of, read_json};
use crate::store::repos::cwd_of;
use crate::store::tasks::{update_wait, Change};
use crate::store::usage::{fold, Now, READ};
use std::collections::HashMap;
use std::path::Path;
use std::sync::{LazyLock, Mutex};

const LOOK: i64 = 5000;
static LOOKED: LazyLock<Mutex<HashMap<String, i64>>> = LazyLock::new(Default::default);

/// A working agent's reading: its conversation's last lines, and for Claude Code the status line input the board kept.
pub fn reading_of(t: &Obj, kind: &str, role: &str) -> Option<Reading> {
    let kept = || read_json(&status_file(&dir_of(t.s("id")), role)).and_then(|v| status_reading(&v));
    if role == "reviewer" { return if kind == "claude" { kept() } else { None }; }
    let rows = crate::agents::installed::read_turn_kept_of(kind, t.so("transcript").map(Path::new), t.s("session"), &cwd_of(t)).and_then(|x| x.window);
    if kind == "claude" { newest(rows, kept()) } else { rows }
}
/// `rows`: the board's list as this herdr session shows it, attempts among them (flow/live.rs).
pub fn usage_tick(rows: &[Obj], now: i64) {
    if crate::store::marks::due("usage-codex", 60_000) { if let Some(r) = codex_newest() { fold("codex", &r, now); } }
    for r in rows {
        let live = r.at("live");
        if r.b("archivedAt") || !is_agent_task(r) || !live.is_object() || live.b("away") || live.b("down") { continue; }
        let stage = r.s("stage");
        if stage != "working" && stage != "reviewing" { continue; }
        let key = format!("{}|{}", r.at("link").s("socket"), r.s("id"));
        { let mut l = LOOKED.lock().unwrap_or_else(|e| e.into_inner()); if l.get(&key).is_some_and(|&at| now >= at && now - at < LOOK) { continue; } l.insert(key, now); }
        if stage == "reviewing" { let k = r.at("link").at("reviewer").t("kind"); if let Some(x) = reading_of(r, &k, "reviewer") { fold(&k, &x, now); } continue; }
        let kind = r.t("agent");
        // Another agent took over from the one marked: its mark goes once board.notify was told of it.
        if r.at("extra").is_object() && r.at("extra").s("agent") != kind && !untold(r) { clear(&r.t("id")); }
        if !READ.contains(&kind.as_str()) { continue; }
        let Some(x) = reading_of(r, &kind, "agent") else { continue };
        let n = fold(&kind, &x, now);
        mark(r, &kind, &x.at_now(now), &n, now);
    }
}
fn clear(id: &str) { let _ = update_wait(id, 300, |t| { t.put("extra", Value::Null); Ok(Change::Write) }); }
/// The task's mark, and what its setting says to do.
fn mark(r: &Obj, kind: &str, x: &Reading, n: &Now, now: i64) {
    let id = r.t("id");
    if !(x.extra && n.extra) {
        if r.at("extra").is_object() && !untold(r) { clear(&id); } // told first, then it goes
        return;
    }
    let was = r.at("extra");
    let since = if was.s("agent") == kind && was.i("window") == n.since && was.i("since") > 0 { was.i("since") } else { x.at.max(n.since).min(now) };
    let m = json!({ "agent": kind, "since": since, "resets": n.resets, "window": n.since });
    if tidy(m.clone()) != *was { let _ = update_wait(&id, 300, |t| { if t.s("stage") != "working" { return Ok(Change::Keep); } t.put("extra", m.clone()); Ok(Change::Write) }); }
    if crate::chat_only::is_chat(r) || extra_ok(r, kind, n, now) { return; }
    match setting_of(r).as_str() {
        "ask" => { stop_now(&id, kind, n, since, "ask"); }
        "defer" if fallback_for(r, now).is_some() => { crate::runner::spawn_claimed("usage", &id, &[]); }
        "defer" => { stop_now(&id, kind, n, since, "defer"); }
        _ => {}
    }
}
