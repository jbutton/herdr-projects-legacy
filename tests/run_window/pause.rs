//! At the window's end: under `finish` a task runs on; under `pause` the step under way ends and the next one waits
//! for the window (the finish line after the agent's turn, findings going back, a review not yet started), the work
//! kept, and the window's opening carries it on by itself; carrying on now, or `finish` set while it waits, goes on.
use super::*;
use std::path::Path;

fn turn() -> Value { json!({ "files": { "f.txt": "v1\n" }, "note": "Did v1." }) }
fn passes() -> Value { json!({ "verdict": { "meets": true, "findings": [], "text": "Looks right." } }) }
fn reviewer_starts(w: &World) -> usize { calls(w).iter().filter(|c| regex::Regex::new(r"^agent start \S+-r ").unwrap().is_match(c)).count() }
/// A task at work at 05:50 in a night window (22:00-06:00), its turn to end when the test says.
fn night_work(w: &World, end: &str) -> String {
    sw::clock::set(Some(local(0, 5, 50)));
    let id = w.add("Night work");
    delegate_with(&id, json!({ "run.window": "22:00-06:00", "run.windowEnd": end }));
    assert_eq!(settle(&id).s("stage"), "working");
    id
}
fn end_turn(w: &World, id: &str, at: i64) -> Obj { sw::clock::set(Some(at)); w.go_on(&task(id)); settle(id) }

#[test]
fn run_window_pause_stops_at_the_end_of_the_turn_once_the_window_closed_and_carries_on_in_the_next() {
    let w = passing_world();
    let id = night_work(&w, "pause");
    // The window closed at 06:00; the turn under way ends at 06:10, and the finish line waits for the window.
    let t = end_turn(&w, &id, local(0, 6, 10));
    let ls = t.at("limitStop");
    assert_eq!((t.s("stage"), ls.s("what"), ls.s("then"), ls.i("opens")), ("stopped", "window", "finish", local(0, 22, 0)));
    assert_eq!(t.s("msg"), "waiting for its window: continues 22:00");
    let tree = t.t("worktree");
    assert!(git(Path::new(&tree), &["status", "--porcelain"]).contains("f.txt"), "its work kept, nothing committed");
    assert_eq!(reviewer_starts(&w), 0, "no review");
    // It waits by itself: not for you, nothing for board.notify; details and the panel say what it waits for.
    assert_eq!((row(&id).s("state"), sw::notify::event_of(&t)), ("working", ""));
    assert_eq!(window(&id).at("paused").s("then"), "finish");
    // Before the window it stays; at the opening it carries on by itself: checked, reviewed, up for approval.
    limits(&w, local(0, 21, 59));
    assert_eq!(task(&id).s("stage"), "stopped");
    limits(&w, local(0, 22, 0) + 30_000);
    let t = settle(&id);
    assert_eq!(t.s("stage"), "approval", "{} {}", t.s("msg"), t.at("failure"));
    assert_eq!(reviewer_starts(&w), 1);
    assert!(!t.b("windowLate"), "on time");
}

/// A world whose agent works on until the test ends its turn, then passes its review.
fn passing_world() -> World {
    let w = world();
    w.script(json!({ "worker": [{ "status": "working", "after": 0, "then": turn() }], "reviewer": [passes()] }));
    w
}

#[test]
fn run_window_finish_the_built_in_value_runs_on_past_the_window() {
    let w = passing_world();
    let id = night_work(&w, "finish");
    assert_eq!(end_turn(&w, &id, local(0, 6, 10)).s("stage"), "approval");
}

#[test]
fn run_window_a_paused_task_goes_on_when_run_window_end_is_set_to_finish() {
    let w = passing_world();
    let id = night_work(&w, "pause");
    assert_eq!(end_turn(&w, &id, local(0, 6, 10)).s("stage"), "stopped");
    run(&["set", "run.windowEnd", "finish", "--task", &id]).unwrap();
    limits(&w, local(0, 6, 30));
    assert_eq!(settle(&id).s("stage"), "approval");
}

#[test]
fn run_window_a_paused_task_carried_on_now_goes_on_with_leave_until_the_window_opens() {
    let w = passing_world();
    let id = night_work(&w, "pause");
    assert_eq!(end_turn(&w, &id, local(0, 6, 10)).s("stage"), "stopped");
    let said = sw::actions::limits::carry_on(&id, false).unwrap().msg;
    assert_eq!(said, format!("{id}: carrying on, its work goes through the check and the review"));
    let t = settle(&id);
    assert_eq!(t.s("stage"), "approval");
    assert_eq!(t.at("windowOk").i("until"), local(0, 22, 0));
}

#[test]
fn run_window_pause_keeps_the_findings_that_would_go_back_and_sends_them_when_the_window_opens() {
    let w = world();
    w.script(json!({ "worker": [{ "status": "working", "after": 0, "then": turn() }, { "files": { "f.txt": "v2\n" }, "note": "Fixed." }],
        "reviewer": [{ "verdict": { "meets": false, "findings": [], "text": "Not what was asked." } }, passes()] }));
    let id = night_work(&w, "pause");
    // The turn ends inside the window; the window closes while the review runs.
    let after = local(0, 6, 1);
    w.during("agent start", move || sw::clock::set(Some(after)));
    let t = end_turn(&w, &id, local(0, 5, 55));
    let ls = t.at("limitStop");
    assert_eq!((t.s("stage"), ls.s("then"), ls.at("send").s("reason")), ("stopped", "send", "review"), "{}", t.s("msg"));
    let pane = t.at("link").t("pane");
    assert!(!prompts(&w, &pane).join("\n").contains("Not what was asked."), "the findings wait");
    // At the opening they go back by themselves, and the round goes on.
    limits(&w, local(0, 22, 0) + 30_000);
    let t = settle(&id);
    assert!(prompts(&w, &pane).join("\n").contains("Not what was asked."), "{:?}", prompts(&w, &pane));
    assert_eq!(t.s("stage"), "approval", "{} {}", t.s("msg"), t.at("failure"));
}

#[test]
fn run_window_pause_holds_a_review_not_yet_started_and_reviews_when_the_window_opens() {
    let w = passing_world();
    let id = night_work(&w, "pause");
    assert_eq!(end_turn(&w, &id, local(0, 6, 10)).s("stage"), "stopped");
    // The window opens and carries it on: its finish line runs, and only gets to the review once the window has closed
    // again (a long check): the review waits for the next window.
    limits(&w, local(0, 22, 0) + 30_000);
    sw::clock::set(Some(local(1, 6, 1)));
    let t = settle(&id);
    let ls = t.at("limitStop");
    assert_eq!((t.s("stage"), ls.s("what"), ls.s("role")), ("stopped", "window", "reviewer"), "{}", t.s("msg"));
    assert_eq!(t.s("msg"), "waiting for its window: continues 22:00");
    assert_eq!(reviewer_starts(&w), 0);
    // The next window carries it on: checked and reviewed, up for approval.
    limits(&w, local(1, 22, 0) + 30_000);
    let t = settle(&id);
    assert_eq!(t.s("stage"), "approval", "{} {}", t.s("msg"), t.at("failure"));
    assert_eq!(reviewer_starts(&w), 1);
}

#[test]
fn run_window_pause_a_stop_for_extra_usage_whose_reset_comes_outside_the_window_waits_for_the_window() {
    let w = world();
    w.script(json!({ "worker": vec![json!({ "status": "working" }); 4] }));
    sw::clock::set(Some(local(0, 3, 0)));
    let id = w.add("Night work");
    delegate_with(&id, json!({ "run.window": "22:00-06:00", "run.windowEnd": "pause", "usage.extra": "defer" }));
    assert_eq!(settle(&id).s("stage"), "working");
    // Claude moves onto extra usage at 03:00, its window resetting at 07:00: stopped for it (usage.extra defer).
    crate::usage::claude_window(&id, local(0, 3, 0), 100.0, local(0, 7, 0));
    crate::usage::tick(&w, local(0, 3, 0) + 10_000);
    assert_eq!((task(&id).s("stage"), task(&id).at("limitStop").s("what")), ("stopped", "usage"));
    // The reset comes after its run window closed: it waits for the run window, and says so.
    limits(&w, local(0, 7, 30));
    let t = task(&id);
    assert_eq!((t.s("stage"), t.s("msg")), ("stopped", "waiting for its window: continues 22:00"));
    assert_eq!(row(&id).s("state"), "working", "it waits by itself");
    limits(&w, local(0, 22, 0) + 30_000);
    assert_eq!(settle(&id).s("stage"), "working");
}
