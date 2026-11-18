//! A window's opening and `board.agents`: a task paused at its window's end held no slot while it waited, so it goes on
//! only into a free one, and first: a task held for the same window waits for the slot it keeps.
use super::*;

fn turn() -> Value { json!({ "files": { "f.txt": "v1\n" }, "note": "Did v1." }) }
fn passes() -> Value { json!({ "verdict": { "meets": true, "findings": [], "text": "Looks right." } }) }

#[test]
fn run_window_a_paused_task_goes_on_first_when_its_window_opens_and_never_past_board_agents() {
    let w = world_with(Opts { settings: json!({ "board.agents": 1 }), ..Default::default() });
    w.script(json!({ "worker": [{ "status": "working", "after": 0, "then": turn() }, { "status": "working" }], "reviewer": [passes()] }));
    sw::clock::set(Some(local(0, 5, 50)));
    let id = w.add("Night work");
    delegate_with(&id, json!({ "run.window": "22:00-06:00", "run.windowEnd": "pause" }));
    assert_eq!(settle(&id).s("stage"), "working");
    sw::clock::set(Some(local(0, 6, 10)));
    w.go_on(&task(&id));
    assert_eq!(settle(&id).s("stage"), "stopped");
    // In the day another task takes the slot it freed; a third waits for the same window as it.
    let day = w.add("Day work");
    sw::store::tasks::set(&day, json!({ "stage": "working" })).unwrap();
    let held = w.add("Tonight");
    delegate_with(&held, json!({ "run.window": "22:00-06:00" }));
    assert_eq!(settle(&held).s("stage"), "queued");
    // The window opens with the slot still taken: the paused task waits for it, and says so.
    limits(&w, local(0, 22, 0) + 30_000);
    let t = task(&id);
    assert_eq!((t.s("stage"), t.s("msg")), ("stopped", "waiting for a free slot"));
    // The slot freed, the paused task keeps it for itself: the held one does not start before it.
    sw::store::tasks::set(&day, json!({ "stage": "done" })).unwrap();
    housekeep(&w, local(0, 22, 1));
    assert_eq!(task(&held).s("stage"), "queued", "the paused task's slot");
    limits(&w, local(0, 22, 2));
    let t = settle(&id);
    assert!(["approval", "done"].contains(&t.s("stage")), "{} {}", t.s("stage"), t.s("msg"));
    housekeep(&w, local(0, 22, 3));
    assert_ne!(task(&held).s("stage"), "queued", "its slot is free now");
}
