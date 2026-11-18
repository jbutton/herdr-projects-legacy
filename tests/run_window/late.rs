//! The machine asleep, or the board not running, when the window opened: the task starts at the next pass inside the
//! window and its row says it started late; if the window has closed again by then, it waits for the next one.
use super::*;

#[test]
fn run_window_starts_late_after_the_machine_slept_and_its_row_says_so() {
    let w = afternoon(json!({}));
    let id = w.add("Tonight");
    delegate_with(&id, json!({ "run.window": "nights" }));
    assert_eq!(settle(&id).s("stage"), "queued");
    // Nothing looked from 14:00 until 23:30: the window opened at 22:00 with nobody there.
    housekeep(&w, local(0, 23, 30));
    let t = settle(&id);
    assert_eq!(t.s("stage"), "working");
    assert_eq!((t.at("windowLate").i("opened"), t.at("windowLate").s("how")), (local(0, 22, 0), "started"));
    assert!(line(&id).starts_with("Started late: its window opened at 22:00"), "{}", line(&id));
    assert_eq!(window(&id).at("late").s("words"), "Started late: its window opened at 22:00");
}

#[test]
fn run_window_closed_again_by_the_next_pass_waits_for_the_next_one() {
    let w = afternoon(json!({}));
    let id = w.add("Tonight");
    delegate_with(&id, json!({ "run.window": "nights" }));
    assert_eq!(settle(&id).s("stage"), "queued");
    // Asleep through the whole night: at 07:00 the window has closed again.
    housekeep(&w, local(1, 7, 0));
    let t = settle(&id);
    assert_eq!((t.s("stage"), t.s("msg")), ("queued", "waiting for its window: starts 22:00"));
    assert_eq!(t.at("windowWait").i("opens"), local(1, 22, 0));
    assert_eq!(starts(&w).len(), 0);
}
