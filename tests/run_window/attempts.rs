//! A task done several times at once meets its window whole: under `pause` its attempts wait for the window and its
//! row waits with them, then they carry on and the task goes on; `--now` and carrying on start its attempts too. And a
//! task held for its window has no agent: delegated again it is queued afresh, and stopped it is no longer held.
use super::*;

fn turn() -> Value { json!({ "status": "working", "after": 0, "then": { "files": { "f.txt": "v1\n" }, "note": "Did v1." } }) }

#[test]
fn run_window_pause_attempts_wait_for_the_window_and_the_task_goes_on_when_they_carry_on() {
    let w = world();
    w.script(json!({ "worker": [turn(), turn()] }));
    sw::clock::set(Some(local(0, 5, 50)));
    let id = w.add("Twice at night");
    delegate_with(&id, json!({ "run.window": "22:00-06:00", "run.windowEnd": "pause", "attempts.by": 2, "review.when": "never" }));
    settle_all(&id);
    assert_eq!(kids(&id).len(), 2);
    // Both turns end after the window closed: each attempt waits for the window, and so does the task, not failed.
    sw::clock::set(Some(local(0, 6, 10)));
    for c in kids(&id) { w.go_on(&c); }
    let t = settle_all(&id);
    for c in kids(&id) { assert_eq!((c.s("stage"), c.s("msg")), ("stopped", "waiting for its window: continues 22:00"), "{}", c.s("letter")); }
    assert_eq!(t.s("stage"), "attempts", "{}", t.s("msg"));
    assert_eq!((row(&id).s("state"), sw::notify::event_of(&t)), ("working", ""));
    assert!(line(&id).contains("A claude waiting for its window: continues 22:00"), "{}", line(&id));
    // At the opening they carry on by themselves, pass the check, and the task waits for your pick.
    limits(&w, local(0, 22, 0) + 30_000);
    let t = settle_all(&id);
    assert_eq!(t.s("stage"), "pick", "{} {} {:?}", t.s("msg"), t.at("failure"), kids(&id).iter().map(|c| (c.t("stage"), c.t("msg"))).collect::<Vec<_>>());
}

#[test]
fn run_window_now_and_carrying_on_start_the_attempts_of_a_task_done_several_times_at_once() {
    let w = afternoon(json!({}));
    let id = w.add("Twice now");
    let (_, said) = run(&["delegate", &id, "--run.window", "nights", "--attempts.by", "2", "--now"]).unwrap();
    assert!(said.contains("started, 2 attempts at once") && said.ends_with(", outside its window (run.window nights), as you said"), "{said}");
    settle_all(&id);
    for c in kids(&id) { assert_eq!(c.s("stage"), "working", "{} {}", c.s("letter"), c.s("msg")); }
    // Held whole until you carry it on: then its attempts start.
    let later = w.add("Twice later");
    delegate_with(&later, json!({ "run.window": "nights", "attempts.by": 2 }));
    let t = settle_all(&later);
    assert_eq!((t.s("stage"), kids(&later).len()), ("queued", 0));
    let said = sw::actions::limits::carry_on(&later, false).unwrap().msg;
    assert_eq!(said, format!("{later}: started, outside its window (run.window nights), as you said"));
    settle_all(&later);
    assert_eq!(kids(&later).len(), 2);
    for c in kids(&later) { assert_eq!(c.s("stage"), "working", "{} {}", c.s("letter"), c.s("msg")); }
}

#[test]
fn run_window_a_held_task_delegated_again_is_queued_afresh_and_stopped_it_is_no_longer_held() {
    let w = afternoon(json!({}));
    let id = w.add("Held");
    delegate_with(&id, json!({ "run.window": "nights" }));
    assert_eq!(settle(&id).s("stage"), "queued");
    // It has no agent yet: delegated again it is queued again, still held; with --run.window any it starts.
    assert_eq!(run(&["delegate", &id]).unwrap().1, format!("{id}: delegated, waiting for its window: starts 22:00"));
    run(&["delegate", &id, "--now", "--run.window", "any"]).unwrap();
    assert_eq!(settle(&id).s("stage"), "working");
    // Stopped while held: no longer held, and details says nothing of a wait.
    let other = w.add("Stopped");
    delegate_with(&other, json!({ "run.window": "nights" }));
    assert_eq!(settle(&other).s("stage"), "queued");
    run(&["stop", &other]).unwrap();
    let t = task(&other);
    assert!(!t.b("windowWait") && t.s("stage") != "queued", "{t:?}");
    assert!(window(&other).at("waiting").is_null(), "{:?}", window(&other));
}
