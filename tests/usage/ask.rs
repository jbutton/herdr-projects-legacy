//! `ask`: the moment its agent is seen on extra usage the agent is stopped (its work kept, nothing committed or merged)
//! and the task waits for you with the reason; your carry-on is leave for the rest of that episode, and the next one
//! asks again; changing the setting to `allow` carries it on by itself; and a task does not start while its program is
//! known to be on extra usage, until your retry.
use super::*;
use sw::usage::words::hm;

#[test]
fn usage_ask_stops_at_once_waits_for_you_and_your_carry_on_is_leave_for_the_window() {
    let w = world();
    let id = at_work(&w, "claude", json!({ "usage.extra": "ask" }));
    let t0 = sw::clock::now();
    std::fs::write(Path::new(task(&id).s("worktree")).join("f.txt"), "half done\n").unwrap();
    // On its plan: it works on.
    claude_window(&id, t0, 80.0, t0 + HOUR);
    tick(&w, t0 + 10_000);
    assert_eq!(task(&id).s("stage"), "working");
    // Past its 5-hour window: stopped there and then, its work as it was, waiting for you with the reason.
    claude_window(&id, t0 + 20_000, 100.0, t0 + HOUR);
    tick(&w, t0 + 30_000);
    let t = task(&id);
    let ls = t.at("limitStop");
    assert_eq!((t.s("stage"), ls.s("what"), ls.s("limit"), ls.s("how"), ls.i("resets")), ("stopped", "usage", "claude", "ask", t0 / 1000 * 1000 + HOUR));
    assert_eq!(t.s("msg"), format!("stopped: claude moved onto extra usage at {} (resets {})", hm(t0 + 20_000, t0), hm(t0 + HOUR, t0)));
    assert_eq!(read(&Path::new(t.s("worktree")).join("f.txt")), "half done\n");
    assert_eq!(row(&id).s("state"), "blocked");
    assert_eq!(sw::notify::event_of(&t), "needs you", "the stop is what tells you, once");
    let d = sw::actions::finish_line::details(&id).unwrap();
    assert_eq!(d.at("usage").at("stopped").s("how"), "ask");
    // It does not carry on by itself, nor at the window's reset.
    limits(&w, t0 + 2 * HOUR);
    assert_eq!(task(&id).s("stage"), "stopped");
    // Your carry-on: it goes on, told why it was stopped, with leave for this episode.
    sw::clock::set(Some(t0 + 40_000));
    sw::actions::finish_line::retry(&id).unwrap();
    let t = settle(&id);
    assert_eq!(t.s("stage"), "working");
    assert!(told(&w, &id).last().unwrap().contains("You were stopped because its program moved onto paid extra usage, not because of anything you did. The human said to carry on"), "{:?}", told(&w, &id));
    assert_eq!(t.at("extraOk").s("agent"), "claude");
    claude_window(&id, t0 + 50_000, 100.0, t0 + HOUR);
    tick(&w, t0 + 60_000);
    assert_eq!(task(&id).s("stage"), "working", "not stopped again in the same episode");
    // The next window, on it again: it asks again.
    let next = t0 + 6 * HOUR;
    claude_window(&id, t0 + HOUR + 60_000, 100.0, next);
    tick(&w, t0 + HOUR + 70_000);
    assert_eq!(task(&id).s("stage"), "stopped");
}

#[test]
fn usage_ask_changed_to_allow_carries_on_by_itself() {
    use sw::store::settings::{change::set_value, lookup::Scope};
    let w = world();
    let id = at_work(&w, "claude", json!({ "usage.extra": "ask" }));
    let t0 = sw::clock::now();
    claude_window(&id, t0, 100.0, t0 + HOUR);
    tick(&w, t0 + 10_000);
    assert_eq!(task(&id).s("stage"), "stopped");
    set_value("usage.extra", &json!("allow"), &Scope::task(&task(&id))).unwrap();
    limits(&w, t0 + 20_000);
    let t = settle(&id);
    assert_eq!(t.s("stage"), "working");
    assert!(told(&w, &id).last().unwrap().contains("The human allowed extra usage for this task"), "{:?}", told(&w, &id));
}

#[test]
fn usage_ask_changed_to_defer_waits_for_the_window_and_carries_on_at_its_reset() {
    use sw::store::settings::{change::set_value, lookup::Scope};
    let w = world();
    let id = at_work(&w, "claude", json!({ "usage.extra": "ask" }));
    let t0 = sw::clock::now();
    let resets = (t0 + HOUR) / 1000 * 1000;
    claude_window(&id, t0, 100.0, resets);
    tick(&w, t0 + 10_000);
    assert_eq!(sw::notify::event_of(&task(&id)), "needs you");
    // Set to defer while it waits: it waits for the window, not for you, and its line says so.
    set_value("usage.extra", &json!("defer"), &Scope::task(&task(&id))).unwrap();
    let t = task(&id);
    assert!(sw::usage::words::waits_window(&t));
    assert_eq!((row(&id).s("state"), sw::notify::event_of(&t)), ("working", ""));
    limits(&w, t0 + 20_000);
    let t = task(&id);
    assert_eq!((t.s("stage"), t.at("limitStop").s("how"), t.s("msg")), ("stopped", "defer", format!("waiting for the usage window: resets {}", hm(resets, t0)).as_str()));
    // And back to ask: it waits for you again.
    set_value("usage.extra", &json!("ask"), &Scope::task(&task(&id))).unwrap();
    limits(&w, t0 + 30_000);
    let t = task(&id);
    assert!(!sw::usage::words::waits_window(&t));
    assert!(t.s("msg").starts_with("stopped: claude moved onto extra usage"), "{}", t.s("msg"));
    // Defer again, and its window's reset carries it on by itself.
    set_value("usage.extra", &json!("defer"), &Scope::task(&task(&id))).unwrap();
    limits(&w, resets + 1000);
    assert_eq!(settle(&id).s("stage"), "working");
}

#[test]
fn usage_ask_holds_a_start_while_its_program_is_on_extra_usage_and_retry_starts_it_anyway() {
    let w = world_with(Opts { repo: true, settings: json!({ "usage.extra": "ask" }), ..Default::default() });
    w.script(json!({ "worker": [{ "status": "working" }] }));
    let t0 = sw::clock::now();
    sw::clock::set(Some(t0));
    // Claude's window, as another task's agent left it: on extra usage until an hour from now.
    let x = sw::agents::read_window::Reading { at: t0 - 60_000, extra: true, resets: t0 + HOUR, windows: vec![sw::agents::read_window::Window { name: "5h".into(), pct: 100.0, resets: t0 + HOUR }], ..Default::default() };
    sw::store::usage::fold("claude", &x, t0);
    let id = w.add("Wait for me");
    delegate(&id);
    let t = settle(&id);
    assert_eq!((t.s("stage"), t.at("usageWait").b("you")), ("queued", true));
    assert_eq!(t.s("msg"), format!("waits for you: claude is on extra usage until {} (usage.extra ask); {} starts it anyway", hm(t0 + HOUR, t0), sw::board::keys::key_name("sendBack")));
    assert!(starts(&w).is_empty());
    assert_eq!((row(&id).s("state"), sw::notify::event_of(&t)), ("blocked", "needs you"));
    assert!(sw::limits::run::carries_on(&t));
    // Your retry starts it, on extra usage.
    let said = sw::actions::finish_line::retry(&id).unwrap();
    assert!(said.msg.contains("on extra usage, as you said"), "{}", said.msg);
    let t = settle(&id);
    assert_eq!(t.s("stage"), "working");
    assert_eq!(starts(&w).len(), 1);
    assert!(!t.b("usageWait"));
}
