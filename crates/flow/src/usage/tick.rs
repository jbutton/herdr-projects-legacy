//! The extra usage pass, run by whoever ticks (the keeper, or an open board), beside the limits' own: each working
//! agent's reading (at most every few seconds, and only what its files gained since: agents/read_kept.rs), kept as its
//! program's window (store/usage.rs); the task marked while its agent is on extra usage (`extra`: since when, until
//! when); and, under `ask` and `defer`, the agent stopped there and then (stop.rs), a review under way too (review.rs).
//! The newest Codex conversation of the machine is read once a minute, so Codex's window is known before a task runs
//! it; while a task waits for a program's window, that program is read afresh at each pass, so a reset bought early is
//! seen (stop.rs: `lifted`).
use super::gate::extra_ok;
use super::review::{held_now, stop_review};
use super::stop::{fallback_for, stop_now};
use super::words::{setting_for, stop_how, untold};
use crate::agents::read_window::{codex_newest, newest, status_file, status_reading, Reading};
use crate::flow::task::is_agent_task;
use crate::json::*;
use crate::store::files::{dir_of, read_json};
use crate::store::repos::cwd_of;
use crate::store::tasks::{update_wait, Change};
use crate::store::usage::{fold, now_of, Now, READ};
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
    let waits: Vec<(&Obj, String, String)> = rows.iter().filter_map(|r| waits_on(r).map(|(k, role)| (r, k, role))).collect();
    let minute = crate::store::marks::due("usage-codex", 60_000);
    let waited = waits.iter().any(|w| w.1 == "codex") && crate::store::marks::due("usage-codex-wait", LOOK);
    if (minute || waited) && let Some(r) = codex_newest() { fold("codex", &r, now); }
    for (r, kind, role) in &waits {
        if kind == "claude" && looked(&format!("wait|{}|{}", r.at("link").s("socket"), r.s("id")), now) && let Some(x) = read_json(&status_file(&dir_of(r.s("id")), role)).and_then(|v| status_reading(&v)) { fold(kind, &x, now); }
    }
    for r in rows {
        let live = r.at("live");
        if r.b("archivedAt") || !is_agent_task(r) || !live.is_object() || live.b("away") || live.b("down") { continue; }
        let stage = r.s("stage");
        if stage != "working" && stage != "reviewing" { continue; }
        if !looked(&format!("{}|{}", r.at("link").s("socket"), r.s("id")), now) { continue; }
        if stage == "reviewing" { review_look(r, now); continue; }
        let kind = r.t("agent");
        // Another agent took over from the one marked: its mark goes once board.notify was told of it.
        if r.at("extra").is_object() && r.at("extra").s("agent") != kind && !untold(r) { clear(&r.t("id")); }
        if !READ.contains(&kind.as_str()) { continue; }
        let Some(x) = reading_of(r, &kind, "agent") else { continue };
        let n = fold(&kind, &x, now);
        mark(r, &kind, &x.at_now(now), &n, now);
    }
}
/// Is it time for another look at this key (at most every few seconds)? Marks it looked at now when it is.
fn looked(key: &str, now: i64) -> bool {
    let mut l = LOOKED.lock().unwrap_or_else(|e| e.into_inner());
    if l.get(key).is_some_and(|&at| now >= at && now - at < LOOK) { return false; }
    l.insert(key.to_string(), now);
    true
}
/// A task stopped to wait for its program's window by itself (`defer`): that program, and whose reading it was.
fn waits_on(r: &Obj) -> Option<(String, String)> {
    let ls = r.at("limitStop");
    if r.s("stage") != "stopped" || ls.s("what") != "usage" || r.b("archivedAt") || stop_how(r) != "defer" { return None; }
    Some((ls.t("limit"), if ls.s("role") == "reviewer" { "reviewer".into() } else { "agent".into() }))
}
/// A review under way: its reviewer's reading kept as its program's window; under `ask` and `defer` the review is
/// stopped the moment its program is seen on extra usage, as an agent is (review.rs). Not the comparison of attempts.
fn review_look(r: &Obj, now: i64) {
    let k = r.at("link").at("reviewer").t("kind");
    let own = reading_of(r, &k, "reviewer");
    let n = match &own { Some(x) => fold(&k, x, now), None => now_of(&k, now) };
    // Its own reading, where there is one (Claude Code's status line), says it too.
    if r.b("comparing") || !n.extra || own.is_some_and(|x| !x.at_now(now).extra) { return; }
    let s = setting_for(r, &k);
    if held_now(r, &s, &k, &n, now) { stop_review(r.s("id"), &k, &n, &s, now); }
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
    match setting_for(r, kind).as_str() {
        "ask" => { stop_now(&id, kind, n, since, "ask"); }
        "defer" if fallback_for(r, now).is_some() => { crate::runner::spawn_claimed("usage", &id, &[]); }
        "defer" => { stop_now(&id, kind, n, since, "defer"); }
        _ => {}
    }
}
