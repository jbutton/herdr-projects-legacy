//! `defer`: a task does not start while its program is on extra usage (it waits in the queue, holding no slot, and the
//! window's reset starts it by itself); a working agent seen on extra usage is stopped at once, its work kept, and the
//! reset carries it on by itself; its reset not known, it waits for you; `worker.fallback` takes it first, at the start
//! and at work, passing over one that is on extra usage too; a turn out of allowance waits for its window too; a task
//! queued after others meets the same start; and a task stopped at its token limit is left at that.
use super::*;
use sw::agents::read_window::{Reading, Window};
use sw::usage::words::hm;

/// A program's window as another task's agent left it: on extra usage until `resets` (0: not said).
fn on_extra(kind: &str, at: i64, resets: i64) {
    let x = Reading { at, extra: true, resets, windows: vec![Window { name: "5h".into(), pct: 100.0, resets }], ..Default::default() };
    sw::store::usage::fold(kind, &x, at);
}
fn housekeep(w: &World) { sw::housekeeping::housekeeping(&w.socket, sw::housekeeping::Force::Yes); }
fn run(a: &[&str]) -> Result<(Value, String), String> { sw::cli::tasks::command(a[0], &sw::cli::flags::parse(&a.iter().map(|s| s.to_string()).collect::<Vec<_>>())).map_err(|e| e.msg) }

#[test]
fn usage_defer_holds_a_start_in_the_queue_without_a_slot_and_the_window_reset_starts_it_by_itself() {
    let w = world_with(Opts { repo: true, settings: json!({ "usage.extra": "defer", "board.agents": 1 }), ..Default::default() });
    w.script(json!({ "worker": [{ "status": "working" }, { "status": "working" }] }));
    let t0 = sw::clock::now();
    sw::clock::set(Some(t0));
    let resets = t0 + HOUR;
    on_extra("claude", t0 - 60_000, resets);
    let held = w.add("Held");
    delegate(&held);
    let t = settle(&held);
    assert_eq!((t.s("stage"), t.at("usageWait").b("you"), t.at("usageWait").s("agent")), ("queued", false, "claude"));
    assert_eq!(t.s("msg"), format!("waiting for the usage window: resets {}", hm(resets, t0)));
    // It waits as a queued task does, not for you, and takes no slot: a Codex task after it starts at once.
    let r = row(&held);
    assert_eq!((r.s("state"), sw::notify::event_of(&t)), ("working", ""));
    assert_eq!(sw::shown::queue::words(&r), format!("waiting for the usage window: resets {}", hm(resets, t0)));
    let other = w.add("Other");
    sw::actions::start::delegate(&other, vec![("worker.agent".into(), json!("codex"))]).unwrap();
    assert_eq!(settle(&other).s("stage"), "working");
    assert_eq!(starts(&w).len(), 1);
    sw::store::tasks::set(&other, json!({ "stage": "done" })).unwrap();
    // Before the reset it stays; at the reset it starts by itself.
    sw::clock::set(Some(resets - 1000)); housekeep(&w);
    assert_eq!(settle(&held).s("stage"), "queued");
    sw::clock::set(Some(resets + 1000)); housekeep(&w);
    let t = settle(&held);
    assert_eq!(t.s("stage"), "working");
    assert!(!t.b("usageWait") && t.s("agent") == "claude");
    assert_eq!(starts(&w).len(), 2);
}

#[test]
fn usage_defer_stops_a_working_agent_at_once_and_the_window_reset_carries_it_on_by_itself() {
    let w = world();
    let id = at_work(&w, "claude", json!({ "usage.extra": "defer" }));
    let t0 = sw::clock::now();
    let resets = (t0 + HOUR) / 1000 * 1000;
    std::fs::write(Path::new(task(&id).s("worktree")).join("f.txt"), "half done\n").unwrap();
    claude_window(&id, t0, 100.0, resets);
    tick(&w, t0 + 10_000);
    let t = task(&id);
    let ls = t.at("limitStop");
    assert_eq!((t.s("stage"), ls.s("what"), ls.s("how"), ls.i("resets")), ("stopped", "usage", "defer", resets));
    assert_eq!(t.s("msg"), format!("waiting for the usage window: resets {}", hm(resets, t0)));
    assert_eq!(read(&Path::new(t.s("worktree")).join("f.txt")), "half done\n", "its work kept");
    assert!(git(Path::new(t.s("worktree")), &["status", "--porcelain"]).contains("f.txt"), "nothing committed");
    // It waits by itself: not for you, and board.notify hears "extra usage", once.
    assert_eq!(row(&id).s("state"), "working");
    assert_eq!(sw::notify::event_of(&t), "extra usage");
    // Before the reset it stays stopped; at the reset it carries on, told why.
    limits(&w, resets - 1000);
    assert_eq!(task(&id).s("stage"), "stopped");
    limits(&w, resets + 1000);
    let t = settle(&id);
    assert_eq!(t.s("stage"), "working");
    assert!(told(&w, &id).last().unwrap().contains("its program moved onto paid extra usage, not because of anything you did. The usage window has reset: continue the task where you left off."), "{:?}", told(&w, &id));
    // The window is a fresh one: the mark goes at the next look, once board.notify was told of it.
    tick(&w, resets + 20_000);
    assert!(task(&id).b("extra"), "not before it was told");
    sw::notify::notify_pass(&sw::store::tasks::list(Default::default()));
    tick(&w, resets + 30_000);
    assert!(!task(&id).b("extra"));
}

#[test]
fn usage_defer_with_its_window_reset_not_known_waits_for_you() {
    let w = world();
    let id = at_work(&w, "claude", json!({ "usage.extra": "defer" }));
    let t0 = sw::clock::now();
    claude_window(&id, t0, 100.0, 0);
    tick(&w, t0 + 10_000);
    let t = task(&id);
    assert_eq!((t.s("stage"), t.at("limitStop").i("resets")), ("stopped", 0));
    assert_eq!(t.s("msg"), "stopped: claude is on extra usage, and when its window resets is not known");
    assert_eq!((row(&id).s("state"), sw::notify::event_of(&t)), ("blocked", "needs you"));
    limits(&w, t0 + 10 * HOUR);
    assert_eq!(task(&id).s("stage"), "stopped", "only you carry it on");
}

#[test]
fn usage_defer_hands_a_working_task_to_the_fallback_and_passes_over_one_on_extra_usage_too() {
    let w = world();
    w.patch(|s| s["pids"] = json!(true)); // the agent's own process in its pane, which the hand-over ends
    let id = at_work(&w, "claude", json!({ "usage.extra": "defer", "worker.fallback": "codex" }));
    let t0 = sw::clock::now();
    let resets = (t0 + HOUR) / 1000 * 1000;
    claude_window(&id, t0, 100.0, resets);
    tick(&w, t0 + 10_000);
    let t = settle(&id);
    assert_eq!((t.s("stage"), t.s("agent"), t.s("on")), ("working", "codex", "codex"), "{} {}", t.at("fallbacks"), t.s("msg"));
    let h = sw::store::handovers::last_handover(&t, "worker").unwrap();
    assert_eq!((h.s("from"), h.s("to"), h.t("why")), ("claude", "codex", format!("claude is on extra usage until {}", hm(resets, t0))));
    assert!(!t.b("limitStop"));
    // The mark is kept through the hand-over, so board.notify is told, once, though Codex is on it now: the
    // housekeeping after the hand-over's job told it (`extraTold`), and nothing is pending.
    assert_eq!((t.at("extra").s("agent"), t.at("extraTold")), ("claude", t.at("extra").at("window")));
    assert_eq!(sw::notify::event_of(&t), "");
    let says = sw::shown::row_says::sentence(&sw::shown::states::as_row(&row(&id))).text();
    assert!(!says.contains("On extra usage"), "the row does not say Codex is on it: {says}");
    // Told: the mark of the agent before it goes at the next look.
    tick(&w, t0 + 30_000);
    assert!(!task(&id).b("extra"));
}

#[test]
fn usage_defer_passes_over_a_fallback_on_extra_usage_too_and_waits_for_the_window() {
    let w = world();
    let id = at_work(&w, "claude", json!({ "usage.extra": "defer", "worker.fallback": "codex" }));
    let t0 = sw::clock::now();
    on_extra("codex", t0, t0 + 2 * HOUR);
    claude_window(&id, t0, 100.0, t0 + HOUR);
    tick(&w, t0 + 10_000);
    let t = settle(&id);
    assert_eq!((t.s("stage"), t.at("limitStop").s("how"), t.s("agent")), ("stopped", "defer", "claude"));
    assert!(sw::store::handovers::handovers(&t).is_empty());
}

#[test]
fn usage_defer_starts_a_task_on_the_fallback_when_its_program_is_on_extra_usage() {
    let w = world_with(Opts { repo: true, settings: json!({ "usage.extra": "defer", "worker.fallback": "codex" }), ..Default::default() });
    w.script(json!({ "worker": [{ "status": "working" }] }));
    let t0 = sw::clock::now();
    sw::clock::set(Some(t0));
    on_extra("claude", t0 - 60_000, t0 + HOUR);
    let id = w.add("On codex");
    delegate(&id);
    let t = settle(&id);
    assert_eq!((t.s("stage"), t.s("agent"), t.s("on")), ("working", "codex", "codex"));
    assert!(starts(&w).iter().any(|s| s.contains("codex")), "{:?}", starts(&w));
    let h = sw::store::handovers::last_handover(&t, "worker").unwrap();
    assert_eq!((h.s("from"), h.s("to"), h.t("why")), ("claude", "codex", format!("claude is on extra usage until {}", hm(t0 + HOUR, t0))));
}

/// The agent in the task's pane moves to `status`; `complete`: its turn completed, as herdr says it.
fn pane(w: &World, id: &str, status: &str, complete: bool) {
    let p = task(id).at("link").t("pane");
    w.patch(|s| { let seq = s.i("seq") + 1; s["seq"] = json!(seq); for x in s["panes"].as_array_mut().unwrap() { if x.s("pane_id") == p { x["agent"]["status"] = json!(status); x["agent"]["seq"] = json!(seq); if complete { x["agent"]["completion"] = json!(seq); } } } });
    on_pane(w, &p);
}

#[test]
fn usage_defer_waits_for_its_window_after_a_turn_out_of_allowance_and_carries_on_at_the_reset() {
    let w = world();
    let id = at_work(&w, "claude", json!({ "usage.extra": "defer" }));
    sw::store::marks::set_token(&id, "agent", "tok", 0).unwrap();
    let t0 = sw::clock::now();
    let resets = (t0 + HOUR) / 1000 * 1000;
    let file = w.base.join("conversation.jsonl");
    line(&file, json!({ "type": "user", "uuid": "u1", "timestamp": sw::clock::iso(t0 - 5000), "message": { "role": "user", "content": "Go on." } }));
    line(&file, json!({ "type": "assistant", "uuid": "a1", "timestamp": sw::clock::iso(t0), "isApiErrorMessage": true, "error": "rate_limit", "apiError": "usage_limit_reached",
        "message": { "id": "m1", "model": "<synthetic>", "role": "assistant", "stop_reason": "stop_sequence", "content": [{ "type": "text", "text": "You've hit your usage limit · your session limit resets 5pm" }] },
        "quotaLimits": { "status": "rejected", "resetsAt": resets / 1000, "rateLimitType": "five_hour", "overageStatus": "rejected", "overageDisabledReason": "out_of_credits", "isUsingOverage": false } }));
    put(&id, "transcript", json!(file.display().to_string()));
    pane(&w, &id, "done", true);
    sw::turn::turn_ended(&id, "agent", "tok", 0, &[json!({ "session_id": "s1", "transcript_path": file.display().to_string() }).to_string()]);
    let t = settle(&id);
    let ls = t.at("limitStop");
    assert_eq!((t.s("stage"), ls.s("what"), ls.s("how"), ls.b("hard"), ls.i("resets")), ("stopped", "usage", "defer", true, resets), "{}", t.s("msg"));
    assert_eq!(t.s("msg"), format!("waiting for the usage window: resets {}", hm(resets, t0)));
    assert_eq!(sw::store::usage::now_of("claude", t0).hard, true, "the header knows it too");
    limits(&w, resets + 1000);
    let t = settle(&id);
    assert_eq!(t.s("stage"), "working");
    assert!(told(&w, &id).last().unwrap().contains("its program's usage window was used up"), "{:?}", told(&w, &id));
}

#[test]
fn usage_defer_meets_a_task_queued_after_others_once_they_are_done() {
    let w = world_with(Opts { repo: true, settings: json!({ "usage.extra": "defer" }), ..Default::default() });
    w.script(json!({ "worker": [{ "status": "working" }] }));
    let t0 = sw::clock::now();
    sw::clock::set(Some(t0));
    on_extra("claude", t0 - 60_000, t0 + HOUR);
    let dep = w.add("First");
    let id = w.add("Second");
    run(&["delegate", &id, "--after", &dep]).unwrap();
    assert_eq!(task(&id).s("stage"), "after");
    sw::store::tasks::set(&dep, json!({ "stage": "done" })).unwrap();
    housekeep(&w);
    let t = settle(&id);
    assert_eq!((t.s("stage"), t.s("msg")), ("queued", format!("waiting for the usage window: resets {}", hm(t0 + HOUR, t0)).as_str()));
    assert!(starts(&w).is_empty());
}

#[test]
fn usage_defer_leaves_a_task_stopped_at_its_token_limit_as_it_is() {
    let w = world();
    let id = at_work(&w, "claude", json!({ "usage.extra": "defer", "limit.tokens": "1m" }));
    let t0 = sw::clock::now();
    sw::limits::reached::stop_at(&id, "tokens", "1m", 2e6, Default::default()).unwrap();
    claude_window(&id, t0, 100.0, t0 + HOUR);
    tick(&w, t0 + 10_000);
    let t = task(&id);
    assert_eq!((t.s("stage"), t.at("limitStop").s("what"), t.s("msg")), ("stopped", "tokens", "stopped at 1m tokens"));
}
