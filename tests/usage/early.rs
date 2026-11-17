//! A reset bought early (`defer`): a task waiting for its program's window does not wait for the reset it was told. Its
//! program is read afresh at each pass, and a fresh reading with room in both the 5-hour and the weekly window carries
//! it on at once: an agent stopped at work, a review held back, a start held in the queue. A fresh reading with one of
//! them still full changes nothing.
use super::review::{finished, reviewers};
use super::*;
use sw::usage::words::hm;

/// Claude Code's status line input kept beside the task at `at`: its 5-hour window at `five`, its week at `week`.
fn claude_both(id: &str, at: i64, five: f64, week: f64, resets: i64) {
    let input = json!({ "rate_limits": { "five_hour": { "used_percentage": five, "resets_at": resets / 1000 }, "seven_day": { "used_percentage": week, "resets_at": (resets + 100 * HOUR) / 1000 } } });
    assert!(sw::agents::statusline::keep(id, "agent", &input, at), "kept");
}
/// A Codex turn of yours today, its 5-hour window at `pct` (read by the board's `codex_newest`).
fn codex_yours(w: &World, at: i64, pct: f64, resets: i64) {
    use chrono::TimeZone;
    let day = chrono::Local.timestamp_millis_opt(sw::clock::now()).single().unwrap().format("%Y/%m/%d").to_string();
    codex_turn(&w.base.join("codex/sessions").join(day).join("rollout-yours.jsonl"), at, pct, resets, pct >= 100.0);
}
fn housekeep(w: &World) { sw::housekeeping::housekeeping(&w.socket, sw::housekeeping::Force::Yes); }

#[test]
fn usage_defer_carries_on_an_agent_as_soon_as_a_fresh_reading_shows_room_before_the_reset_it_was_told() {
    let w = world();
    let id = at_work(&w, "claude", json!({ "usage.extra": "defer" }));
    let t0 = sw::clock::now();
    let resets = (t0 + 18 * HOUR) / 1000 * 1000;
    claude_both(&id, t0, 100.0, 63.0, resets);
    tick(&w, t0 + 10_000);
    let t = task(&id);
    assert_eq!((t.s("stage"), t.at("limitStop").i("resets")), ("stopped", resets));
    assert_eq!(t.s("msg"), format!("waiting for the usage window: resets {}", hm(resets, t0)));
    limits(&w, t0 + HOUR);
    assert_eq!(task(&id).s("stage"), "stopped", "nothing new was read");
    // A fresh reading: the 5-hour window has room again, the week is full. Still no room.
    claude_both(&id, t0 + 2 * HOUR, 1.0, 100.0, t0 + 7 * HOUR);
    tick(&w, t0 + 2 * HOUR + 10_000);
    limits(&w, t0 + 2 * HOUR + 20_000);
    assert_eq!(task(&id).s("stage"), "stopped", "both windows must have room");
    // The reset bought: a fresh reading with room in both. It carries on at once, a day before the reset it was told.
    claude_both(&id, t0 + 3 * HOUR, 1.0, 5.0, t0 + 8 * HOUR);
    tick(&w, t0 + 3 * HOUR + 10_000);
    limits(&w, t0 + 3 * HOUR + 20_000);
    let t = settle(&id);
    assert_eq!(t.s("stage"), "working", "{}", t.s("msg"));
    assert!(told(&w, &id).last().unwrap().contains("The usage window has reset: continue the task where you left off."), "{:?}", told(&w, &id));
}

#[test]
fn usage_defer_reviews_as_soon_as_codex_is_read_with_room_again_before_the_reset_it_was_told() {
    let w = world_with(Opts { repo: true, settings: json!({ "usage.extra": "defer" }), ..Default::default() });
    let t0 = sw::clock::now();
    sw::clock::set(Some(t0));
    let resets = (t0 + 18 * HOUR) / 1000 * 1000;
    codex_yours(&w, t0 - 60_000, 100.0, resets);
    tick(&w, t0); // the once-a-minute read: Codex is on extra usage
    let t = finished(&w, t0);
    let id = t.t("id");
    assert_eq!((t.s("stage"), t.at("limitStop").s("role"), t.at("limitStop").i("resets")), ("stopped", "reviewer", resets), "{}", t.s("msg"));
    limits(&w, t0 + 5000);
    assert_eq!(settle(&id).s("stage"), "stopped");
    // You bought a reset and used Codex: its conversation says so. Within the minute: the pass reads Codex afresh
    // because the review waits for it, not once a minute.
    sw::clock::set(Some(t0 + 20_000));
    codex_yours(&w, t0 + 20_000, 2.0, t0 + 5 * HOUR);
    tick(&w, t0 + 30_000);
    assert!(sw::store::usage::now_of("codex", t0 + 30_000).room());
    limits(&w, t0 + 40_000);
    let t = settle(&id);
    assert_eq!(t.s("stage"), "approval", "{} {}", t.s("msg"), t.at("failure"));
    assert_eq!(reviewers(&w), vec!["codex"]);
}

#[test]
fn usage_defer_starts_a_held_task_as_soon_as_codex_is_read_with_room_again() {
    let w = world_with(Opts { repo: true, settings: json!({ "usage.extra": "defer", "worker.agent": "codex" }), ..Default::default() });
    w.script(json!({ "worker": [{ "status": "working" }] }));
    let t0 = sw::clock::now();
    sw::clock::set(Some(t0));
    let resets = (t0 + 18 * HOUR) / 1000 * 1000;
    codex_yours(&w, t0 - 60_000, 100.0, resets);
    let id = w.add("Held");
    delegate(&id);
    let t = settle(&id);
    assert_eq!((t.s("stage"), t.s("msg")), ("queued", format!("waiting for the usage window: resets {}", hm(resets, t0)).as_str()));
    sw::clock::set(Some(t0 + 2 * HOUR));
    codex_yours(&w, t0 + 2 * HOUR, 2.0, t0 + 7 * HOUR);
    housekeep(&w);
    let t = settle(&id);
    assert_eq!((t.s("stage"), t.b("usageWait")), ("working", false), "{}", t.s("msg"));
}
