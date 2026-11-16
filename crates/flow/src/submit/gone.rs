//! A task's agent whose program is not running where the board was about to type, or had just typed: nothing more goes
//! into its pane (a shell runs what is typed into it). Under `usage.extra defer`, with its program's window used up or
//! on extra usage, the task waits for that window as one stopped mid-run does (usage/stop.rs) and carries on by itself
//! at the reset; else it fails as not started. Either way its text is kept, for the start that follows.
use super::give_up::screen;
use crate::agents::read_window::codex_newest;
use crate::board::keys::key_name;
use crate::clock::now;
use crate::finish::shared::last_lines;
use crate::herdr::{cli::HerdrError, Herdr};
use crate::json::*;
use crate::limits::reached::{stop_at, StopHow};
use crate::store::failures::failure_patch;
use crate::store::tasks::{get, update_wait, Change, Task};
use crate::store::usage::{fold, now_of, READ};
use crate::usage::words::setting_of;

/// herdr's answer for it, as `hand` gives it: nothing was typed.
pub const GONE: &str = "agent_gone";
pub const ENDED: &str = "Not started: its program ended before it was given its instructions";
pub fn error() -> HerdrError { HerdrError { code: GONE.into(), message: "the agent's program is not running in its pane: nothing was typed".into() } }

/// Under `defer`, is its program's window used up or on extra usage, as read afresh now? The stop's own marks, if so.
pub fn window_used(id: &str) -> Option<Obj> {
    let t = get(id)?;
    let kind = t.t("agent");
    if setting_of(&t) != "defer" || crate::chat_only::is_chat(&t) || !READ.contains(&kind.as_str()) { return None; }
    let at = now();
    let fresh = if kind == "codex" { codex_newest() } else { crate::usage::tick::reading_of(&t, &kind, "agent") };
    if let Some(r) = fresh { fold(&kind, &r, at); }
    let n = now_of(&kind, at);
    if !(n.extra || n.hard) || n.resets <= at { return None; }
    Some(obj(json!({ "how": "defer", "resets": n.resets, "window": n.since, "hard": n.hard })))
}
/// The task stopped to wait for its program's window, as one stopped mid-run is: it carries on by itself at the reset.
pub fn wait(id: &str, extra: Obj) -> bool {
    let Some(kind) = get(id).map(|t| t.t("agent")) else { return false };
    stop_at(id, "usage", &kind, now() as f64, StopHow { role: "agent".into(), extra, ..Default::default() }).is_some()
}
/// Text typed into the agent's box that it never started on (`m`, its mark): kept for the start that follows.
pub fn keep_typed(id: &str, m: &Value) {
    let _ = update_wait(id, 10000, |c| {
        if c.at("sent").at("at") != m.at("at") { return Ok(Change::Keep); }
        c.put("unsent", json!({ "text": m.at("text"), "first": m.b("first"), "at": if m.b("typedAt") { m.at("typedAt").clone() } else { m.at("at").clone() }, "typed": true }));
        c.put("sent", Value::Null);
        Ok(Change::Write)
    });
}

/// Nothing was typed: its program had ended. The text is kept as not typed, before what waited unsent already, and the
/// task waits for its usage window or fails as not started.
pub fn before_typed(t: &Task, text: &str, first: bool, api: &Herdr) {
    let id = t.t("id");
    let shown = screen(api, t.at("link").s("pane"), None);
    let _ = update_wait(&id, 10000, |c| {
        let old = c.at("unsent").clone();
        let all = if old.is_object() && !old.s("text").is_empty() { format!("{}\n\n{text}", old.s("text")) } else { text.to_string() };
        c.put("unsent", json!({ "text": all, "first": first || old.b("first"), "at": now() }));
        c.put("sent", Value::Null);
        Ok(Change::Write)
    });
    if let Some(x) = window_used(&id) { if wait(&id, x) { return; } }
    let tail = last_lines(&shown, 8);
    let detail = format!("The board was about to type the text into the agent's box, but its program had already ended: nothing was typed into its pane.{}", if tail.is_empty() { String::new() } else { format!("\n\nThe last lines of its terminal:\n{tail}") });
    let next = format!("Retry ({}) starts the agent's program again and hands it the text.", key_name("yes"));
    let _ = update_wait(&id, 10000, |c| {
        if c.s("stage") != "working" { return Ok(Change::Keep); }
        c.put("stage", "failed"); c.put("msg", ENDED); c.put("turnOpen", false); c.put("watch", Value::Null);
        for (k, v) in failure_patch(&obj(json!({ "stage": "send", "title": "The agent's program ended as it started", "auth": false, "agentCanFix": false, "detail": detail, "next": next }))) { c.insert(k, v); }
        Ok(Change::Write)
    });
}
