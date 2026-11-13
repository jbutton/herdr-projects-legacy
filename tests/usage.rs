//! Extra usage (`usage.extra`, docs/extra-usage.md): what Claude Code and Codex say of their usage window and how the
//! board reads it (read.rs), and what a task does under each value: `allow` warns (allow.rs), `ask` stops and waits for
//! you (ask.rs), `defer` hands over, holds back, stops and carries on at the window's reset (defer.rs). What they share
//! is here: the files the programs write, a task at work, one pass of the ticks.
use crate::common;
use common::*;
use serde_json::{json, Value};
use std::path::Path;
use sw::json::*;

mod allow;
mod ask;
mod defer;
mod read;

pub const HOUR: i64 = 3_600_000;
/// A line added to a saved conversation.
pub fn line(file: &Path, o: Value) {
    use std::io::Write;
    if let Some(d) = file.parent() { std::fs::create_dir_all(d).unwrap(); }
    std::fs::OpenOptions::new().create(true).append(true).open(file).unwrap().write_all(format!("{o}\n").as_bytes()).unwrap();
}
/// Codex's `token_count` as 0.168.0 writes it: the 5-hour window at `pct` (resetting at `resets`, ms), the week at 78%,
/// and its credits there or not.
pub fn codex_count(at: i64, pct: f64, resets: i64, credits: bool) -> Value {
    json!({ "timestamp": sw::clock::iso(at), "type": "event_msg", "payload": { "type": "token_count", "info": { "last_token_usage": { "input_tokens": 100, "output_tokens": 10 } },
        "rate_limits": { "limit_id": "codex", "limit_name": null,
            "primary": { "used_percent": pct, "window_minutes": 300, "resets_at": resets / 1000 },
            "secondary": { "used_percent": 78.0, "window_minutes": 10080, "resets_at": (resets + 100 * HOUR) / 1000 },
            "credits": { "has_credits": credits, "unlimited": false, "balance": null }, "individual_limit": null, "spend_control_reached": null, "plan_type": "team", "rate_limit_reached_type": null } } })
}
/// A Codex turn under way: its start, then its token count.
pub fn codex_turn(file: &Path, at: i64, pct: f64, resets: i64, credits: bool) {
    line(file, json!({ "timestamp": sw::clock::iso(at - 1000), "type": "event_msg", "payload": { "type": "task_started", "turn_id": format!("t{at}") } }));
    line(file, codex_count(at, pct, resets, credits));
}
/// Claude Code's status line input (its `rate_limits`, as 2.1.323 gives it), kept beside the task by the board's own
/// status line at `at`. `resets` 0: the window does not say.
pub fn claude_window(id: &str, at: i64, five: f64, resets: i64) {
    let mut fh = json!({ "used_percentage": five });
    if resets > 0 { fh["resets_at"] = json!(resets / 1000); }
    let input = json!({ "session_id": "s1", "rate_limits": { "five_hour": fh, "seven_day": { "used_percentage": 63, "resets_at": (sw::clock::now() + 90 * HOUR) / 1000 } } });
    assert!(sw::agents::statusline::keep(id, "agent", &input, at), "kept");
}
/// A task's own field, set.
pub fn put(id: &str, k: &str, v: Value) { sw::store::tasks::patch(id, |t| { t.put(k, v); }).unwrap(); }
/// A task at work: its agent `agent`, with these settings; the clock stands still from here.
pub fn at_work(w: &World, agent: &str, settings: Value) -> String {
    w.script(json!({ "worker": vec![json!({ "status": "working" }); 8], "reviewer": [{ "verdict": { "meets": true, "text": "Looked at it.", "findings": [] } }] }));
    let id = w.add("Long work");
    let mut s = settings.as_object().cloned().unwrap_or_default();
    s.insert("worker.agent".into(), json!(agent));
    delegate_with(&id, Value::Object(s));
    assert_eq!(settle(&id).s("stage"), "working");
    sw::clock::set(Some(sw::clock::now()));
    id
}
/// Each reading of extra usage's own pass, the clock moved to `now` first (a look waits a few seconds after the last).
pub fn tick(w: &World, now: i64) { sw::clock::set(Some(now)); sw::usage::tick::usage_tick(&sw::flow::live::board_for(&w.socket), now); }
/// The limits' own pass, which carries on a task whose stop no longer holds.
pub fn limits(w: &World, now: i64) {
    sw::clock::set(Some(now));
    let c = |id: &str| { let _ = sw::actions::limits::carry_on(id, true); };
    sw::limits::tick::limits_tick(&sw::flow::live::board_for(&w.socket), now, Some(&c));
}
/// What the task's agent was told.
pub fn told(w: &World, id: &str) -> Vec<String> { prompts(w, task(id).at("link").s("pane")) }
