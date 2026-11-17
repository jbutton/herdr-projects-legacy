//! A review and extra usage: under `defer` a review does not start while its reviewer's program is on extra usage (it
//! waits for the window as an agent does, "Reviewing: waiting for the usage window", and is reviewed at the reset), or
//! the next on `reviewer.fallback` with room reviews instead, passing over one on extra usage too; under `ask` it waits
//! for you, and your carrying on reviews it on extra usage; under `allow` nothing changes; a review under way is
//! stopped the moment its program is seen on extra usage; and a plan's review is held back the same way.
use super::*;
use sw::agents::read_window::{Reading, Window};
use sw::store::handovers::last_handover;
use sw::usage::words::hm;

/// A program's window as another agent of it left it at `at`: its 5-hour window at `five` percent until `resets`, the
/// week at 60%; at 100% answers are still served (extra usage).
pub fn window(kind: &str, at: i64, five: f64, resets: i64) {
    let windows = vec![Window { name: "5h".into(), pct: five, resets }, Window { name: "week".into(), pct: 60.0, resets: resets + 100 * HOUR }];
    let full = five >= 100.0;
    sw::store::usage::fold(kind, &Reading { at, extra: full, resets: if full { resets } else { 0 }, windows, ..Default::default() }, at);
}
fn pass() -> Value { json!({ "verdict": { "meets": true, "findings": [], "text": "Looks right." } }) }
/// The reviewers started so far, by program.
pub fn reviewers(w: &World) -> Vec<String> { calls(w).into_iter().filter(|c| regex::Regex::new(r"^agent start \S+-r ").unwrap().is_match(c)).map(|c| c.split(' ').nth(4).unwrap_or("").to_string()).collect() }
/// A task whose agent did its work: its review comes next. The clock stands still at `t0` from here.
pub fn finished(w: &World, t0: i64) -> Obj {
    w.script(json!({ "worker": [step(1)], "reviewer": [pass(), pass()] }));
    sw::clock::set(Some(t0));
    let id = w.add("Reviewed");
    delegate(&id);
    settle(&id)
}
fn defer_world(more: Value) -> World {
    let mut s = json!({ "usage.extra": "defer" });
    for (k, v) in more.as_object().unwrap() { s[k] = v.clone(); }
    world_with(Opts { repo: true, settings: s, ..Default::default() })
}

#[test]
fn usage_defer_holds_a_review_while_its_reviewer_is_on_extra_usage_and_the_window_reset_reviews_it() {
    let w = defer_world(json!({}));
    let t0 = sw::clock::now();
    let resets = t0 + HOUR;
    window("codex", t0 - 60_000, 100.0, resets);
    let t = finished(&w, t0);
    let id = t.t("id");
    let ls = t.at("limitStop");
    assert_eq!((t.s("stage"), ls.s("what"), ls.s("role"), ls.s("limit"), ls.s("how"), ls.i("resets")), ("stopped", "usage", "reviewer", "codex", "defer", resets), "{} {}", t.s("msg"), t.at("failure"));
    let line = format!("Reviewing: waiting for the usage window: resets {}", hm(resets, t0));
    assert_eq!(t.s("msg"), line);
    assert!(reviewers(&w).is_empty(), "no review ran on extra usage");
    // It waits by itself, as an agent stopped at work does: not for you.
    let r = row(&id);
    assert_eq!((r.s("state"), sw::notify::event_of(&t)), ("working", ""));
    assert!(sw::shown::row_says::sentence(&sw::shown::states::as_row(&r)).text().contains(&line));
    let u = sw::actions::finish_line::details(&id).unwrap().at("usage").clone();
    assert_eq!((u.s("agent"), u.at("stopped").s("role"), u.b("onExtraUsage")), ("codex", "reviewer", true));
    // Before the reset it stays; at the reset it is reviewed by itself.
    limits(&w, resets - 5000);
    assert_eq!(settle(&id).s("stage"), "stopped");
    limits(&w, resets + 1000);
    let t = settle(&id);
    assert_eq!(t.s("stage"), "approval", "{} {}", t.s("msg"), t.at("failure"));
    assert_eq!((reviewers(&w), t.at("limitStop").is_null()), (vec!["codex".to_string()], true));
}

#[test]
fn usage_defer_hands_a_review_to_the_next_on_reviewer_fallback_with_room() {
    let w = defer_world(json!({ "reviewer.fallback": "claude" }));
    let t0 = sw::clock::now();
    window("codex", t0 - 60_000, 100.0, t0 + HOUR);
    let t = finished(&w, t0);
    assert_eq!(t.s("stage"), "approval", "{} {}", t.s("msg"), t.at("failure"));
    assert_eq!(reviewers(&w), vec!["claude"]);
    let h = last_handover(&t, "reviewer").unwrap();
    assert_eq!((h.s("from"), h.s("to"), h.t("why")), ("codex", "claude", format!("codex is on extra usage until {}", hm(t0 + HOUR, t0))));
    assert_eq!((t.s("reviewerOn"), sw::store::reviews::review_history(&t).last().unwrap().s("by")), ("claude", "claude"));
}

#[test]
fn usage_defer_passes_over_a_fallback_reviewer_on_extra_usage_too_and_the_first_with_room_reviews() {
    let w = defer_world(json!({ "reviewer.fallback": "claude" }));
    let t0 = sw::clock::now();
    sw::clock::set(Some(t0));
    // Its agent (Claude) started before Claude moved onto extra usage, and ends its work after.
    w.script(json!({ "worker": [{ "status": "working", "after": 0, "then": step(1) }], "reviewer": [pass()] }));
    let id = w.add("Reviewed");
    delegate(&id);
    assert_eq!(settle(&id).s("stage"), "working");
    window("codex", t0 - 60_000, 100.0, t0 + 3 * HOUR);
    window("claude", t0 - 60_000, 100.0, t0 + HOUR);
    w.go_on(&task(&id));
    let t = settle(&id);
    assert_eq!((t.s("stage"), t.at("limitStop").i("resets")), ("stopped", t0 + 3 * HOUR), "{}", t.s("msg"));
    assert!(reviewers(&w).is_empty() && sw::store::handovers::handovers(&t).is_empty());
    // Claude's window resets first: its room is enough, Codex's reset is not waited for.
    limits(&w, t0 + HOUR - 5000);
    assert_eq!(settle(&id).s("stage"), "stopped");
    limits(&w, t0 + HOUR + 1000);
    let t = settle(&id);
    assert_eq!(t.s("stage"), "approval", "{} {}", t.s("msg"), t.at("failure"));
    assert_eq!((reviewers(&w), t.s("reviewerOn")), (vec!["claude".to_string()], "claude"));
}

#[test]
fn usage_ask_holds_a_review_for_you_and_your_carrying_on_reviews_it_on_extra_usage() {
    let w = world_with(Opts { repo: true, settings: json!({ "usage.extra": "ask", "reviewer.fallback": "claude" }), ..Default::default() });
    let t0 = sw::clock::now();
    let resets = t0 + HOUR;
    window("codex", t0 - 60_000, 100.0, resets);
    let t = finished(&w, t0);
    let id = t.t("id");
    let ls = t.at("limitStop");
    assert_eq!((t.s("stage"), ls.s("role"), ls.s("how")), ("stopped", "reviewer", "ask"), "{}", t.s("msg"));
    assert_eq!(t.s("msg"), format!("Reviewing: stopped: codex moved onto extra usage at {} (resets {})", hm(t0 - 60_000, t0), hm(resets, t0)));
    assert_eq!((row(&id).s("state"), sw::notify::event_of(&t)), ("blocked", "needs you"));
    assert!(reviewers(&w).is_empty() && sw::store::handovers::handovers(&t).is_empty(), "under ask nothing is handed over: you decide");
    // Not by itself; your carrying on reviews it, on extra usage, as you said.
    limits(&w, t0 + 30 * 60_000);
    assert_eq!(settle(&id).s("stage"), "stopped");
    sw::actions::finish_line::retry(&id).unwrap();
    let t = settle(&id);
    assert_eq!(t.s("stage"), "approval", "{} {}", t.s("msg"), t.at("failure"));
    assert_eq!((reviewers(&w), t.at("extraOk").s("agent")), (vec!["codex".to_string()], "codex"));
}

#[test]
fn usage_allow_reviews_on_extra_usage_as_before() {
    let w = world();
    let t0 = sw::clock::now();
    window("codex", t0 - 60_000, 100.0, t0 + HOUR);
    let t = finished(&w, t0);
    assert_eq!(t.s("stage"), "approval", "{} {}", t.s("msg"), t.at("failure"));
    assert_eq!(reviewers(&w), vec!["codex"]);
}

#[test]
fn usage_defer_stops_a_review_under_way_the_moment_its_program_moves_onto_extra_usage() {
    let w = defer_world(json!({ "reviewer.agent": "claude" }));
    let t0 = sw::clock::now();
    let t = finished(&w, t0);
    let id = t.t("id");
    assert_eq!((t.s("stage"), reviewers(&w)), ("approval", vec!["claude".to_string()]));
    // Another round under way, its reviewer's status line at 100% of its 5-hour window, answers still served.
    sw::store::tasks::patch(&id, |t| { t.put("stage", "reviewing"); t.put("reviewAt", t0); t.obj_mut("link").put("reviewer", json!({ "kind": "claude", "agent": "r" })); }).unwrap();
    let resets = (t0 + HOUR) / 1000 * 1000;
    let input = json!({ "rate_limits": { "five_hour": { "used_percentage": 100, "resets_at": resets / 1000 }, "seven_day": { "used_percentage": 63, "resets_at": (t0 + 90 * HOUR) / 1000 } } });
    assert!(sw::agents::statusline::keep(&id, "reviewer", &input, t0 + 5000));
    tick(&w, t0 + 10_000);
    let t = task(&id);
    let ls = t.at("limitStop");
    assert_eq!((t.s("stage"), ls.s("role"), ls.s("how"), ls.i("resets")), ("stopped", "reviewer", "defer", resets), "{}", t.s("msg"));
    assert_eq!(t.s("msg"), format!("Reviewing: waiting for the usage window: resets {}", hm(resets, t0)));
    // At the reset it is reviewed again by itself.
    limits(&w, resets + 1000);
    assert_eq!(settle(&id).s("stage"), "approval");
}

#[test]
fn usage_defer_holds_a_plans_review_too_and_the_window_reset_reviews_it() {
    let w = defer_world(json!({ "plan.approve": "reviewer", "worker.agent": "codex", "reviewer.agent": "claude", "review.when": "never" }));
    unsafe { std::env::set_var("HERDR_PROJECTS_SANDBOX", "bwrap") };
    w.patch(|s| s["pids"] = json!(true));
    w.script(json!({ "worker": [{ "plan": "# Plan\n\n1. Add f.txt.\n", "note": "Written." }, { "status": "working" }], "reviewer": [pass()] }));
    let t0 = sw::clock::now();
    sw::clock::set(Some(t0));
    let resets = t0 + HOUR;
    window("claude", t0 - 60_000, 100.0, resets);
    let id = w.add("Planned");
    delegate(&id);
    let mut t = settle(&id);
    for _ in 0..2 { t = settle(&id); }
    assert_eq!((t.s("stage"), t.at("limitStop").s("role")), ("stopped", "reviewer"), "{} {}", t.s("msg"), t.at("failure"));
    assert_eq!(t.s("msg"), format!("Reviewing: waiting for the usage window: resets {}", hm(resets, t0)));
    assert!(!t.at("plan").b("review"), "no review of the plan ran on extra usage");
    limits(&w, resets + 1000);
    let mut t = settle(&id);
    for _ in 0..2 { if t.s("stage") != "working" { t = settle(&id); } }
    assert_eq!((t.s("stage"), t.at("plan").s("approvedBy"), t.at("plan").at("review").s("by")), ("working", "reviewer", "claude"), "{} {}", t.s("msg"), t.at("failure"));
}
