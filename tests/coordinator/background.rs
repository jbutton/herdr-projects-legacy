//! The Coordinator's conversation kept running in Claude Code's background (its terminal closed while it worked): a
//! resume of it is refused, and the board waits until its transcript stops growing, stops it there (`claude stop`) and
//! resumes it in its own pane, each step in the start log; when that fails, the error names the session and the two
//! commands to run.
use super::coord_world;
use super::follow::{add, agents, at, file, get, resumed, said, tell};
use crate::common;
use common::*;
use serde_json::{json, Value};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use sw::agents::claude_running::set_fake;
use sw::coordinator::state;
use sw::json::*;

/// `claude stop` as Claude Code answers it, every call noted with the time it came: the session it names leaves the
/// background. `refuses`: it fails with these words instead.
struct Stop { calls: Rc<RefCell<Vec<(String, i64)>>> }
impl Drop for Stop { fn drop(&mut self) { set_fake(None); } }
fn claude_stop(w: &World, refuses: Option<&str>) -> Stop {
    let calls = Rc::new(RefCell::new(vec![]));
    let (c, f, refuses) = (calls.clone(), w.fake.clone(), refuses.map(String::from));
    set_fake(Some(Rc::new(move |a: &[String]| {
        c.borrow_mut().push((a.join(" "), sw::clock::now()));
        if let Some(e) = &refuses { return Err(e.clone()); }
        let short = a[1].clone();
        if let Some(l) = f.st.borrow_mut()["background"].as_array_mut() { l.retain(|x| !text(x).starts_with(&short)); }
        Ok(format!("Stopped {short}."))
    })));
    Stop { calls }
}
fn start_log(w: &World) -> String { std::fs::read_to_string(state::dir_of(&w.socket).join("start.log")).unwrap_or_default() }
/// Its terminal was closed while it worked: the program is gone from it, and the turn goes on in Claude Code's
/// background. The clock stands still from here: the board's waiting moves it.
fn closed_mid_turn(w: &World, session: &str) {
    w.patch(|st| { for p in st["panes"].as_array_mut().unwrap() { if p.at("agent").is_object() { p["agent"] = Value::Null; } } st["background"] = json!([session]); });
    sw::clock::advance(0);
}
/// A step of a turn still at work: it called a tool.
fn working() -> Vec<Value> { vec![json!({ "type": "assistant", "message": { "role": "assistant", "content": [{ "type": "text", "text": "Running the tests." }], "stop_reason": "tool_use" }, "timestamp": at(0) })] }

#[test]
fn a_resume_refused_for_a_session_in_claude_codes_background_waits_until_it_is_still_then_stops_it_and_resumes_it_in_the_boards_pane() {
    let w = coord_world();
    w.script(json!({ "coordinator": [{ "said": "Hello." }, { "said": "Back with you." }] }));
    tell(&w, "hello");
    let (s, tab) = (get(&w).t("session"), get(&w).t("tab"));
    closed_mid_turn(&w, &s);
    let stop = claude_stop(&w, None);
    // In the background it works on for a while, then ends its turn.
    let (f, looks, last) = (file(&w, &s), Rc::new(Cell::new(0)), Rc::new(Cell::new(0i64)));
    let (n, l) = (looks.clone(), last.clone());
    sw::clock::meanwhile(move || {
        n.set(n.get() + 1);
        match n.get() { 1..=6 => add(&f, working(), 0), 7 => add(&f, vec![json!({ "type": "assistant", "uuid": "bg", "message": { "role": "assistant", "content": [{ "type": "text", "text": "Done in the background." }], "stop_reason": "end_turn" }, "timestamp": at(0) })], 0), _ => return }
        l.set(sw::clock::now());
    });
    tell(&w, "still there?");
    // Stopped once, by the id Claude Code gave, only after its transcript had stopped growing with its turn ended.
    let calls = stop.calls.borrow().clone();
    assert_eq!(calls.len(), 1, "{calls:?}"); assert_eq!(calls[0].0, format!("stop {}", &s[..8]));
    assert!(looks.get() > 7 && calls[0].1 - last.get() >= 30_000, "stopped {} ms after its last line", calls[0].1 - last.get());
    // Resumed in the board's own pane, in the same conversation, and the message went in there.
    assert_eq!(get(&w).s("tab"), tab); assert_eq!(agents(&w).len(), 1);
    assert_eq!(resumed(&w), s); assert_eq!(get(&w).s("session"), s);
    assert_eq!(sw::coordinator::look::look(&w.socket).said, "Back with you.");
    assert!(!get(&w).b("error"), "{}", get(&w).s("error"));
    // Each step is in the start log, in order.
    let log = start_log(&w);
    let steps = ["is running in its background", "Waiting until its transcript stops growing", "Its transcript stopped growing", "Running `claude stop", "answered: Stopped", "Resuming session", "Resumed."];
    let at: Vec<usize> = steps.iter().map(|x| log.find(x).unwrap_or_else(|| panic!("{x:?} not in:\n{log}"))).collect();
    assert!(at.windows(2).all(|p| p[0] < p[1]), "{log}");
}

#[test]
fn when_the_stop_fails_the_error_names_the_session_and_the_two_commands_to_run() {
    let w = coord_world();
    w.script(json!({ "coordinator": [{ "said": "Hello." }, { "said": "Back with you." }] }));
    tell(&w, "hello");
    let s = get(&w).t("session");
    closed_mid_turn(&w, &s);
    let _stop = claude_stop(&w, Some("no such background session"));
    tell(&w, "still there?");
    let e = get(&w).t("error");
    assert!(e.contains(&s), "{e}");
    assert!(e.contains(&format!("Run `claude stop {}`, then `herdr-projects coordinator restart`", &s[..8])), "{e}");
    assert!(e.contains("no such background session"), "{e}");
    // Nothing runs in its place, its tab is closed, and the message still waits.
    assert!(agents(&w).is_empty()); assert!(!get(&w).b("pane"));
    assert_eq!(get(&w).arr("queue").len(), 1);
    let log = start_log(&w);
    assert!(log.contains("`claude stop") && log.contains("Not taken back: `claude stop"), "{log}");
}

#[test]
fn a_session_whose_transcript_cannot_be_found_is_never_stopped_and_one_named_by_its_short_id_is_waited_for_first() {
    use sw::coordinator::background::{take_back, Refused};
    let w = coord_world();
    sw::clock::advance(0);
    let stop = claude_stop(&w, None);
    let (api, co) = (sw::herdr::Herdr::new(Some(w.socket.clone())), sw::coordinator::co::Co::top(&w.socket));
    let r = Refused { api: &api, co: &co, kind: "claude", pane: "nowhere", name: "sw-coordinator", args: &[], session: "" };
    // Nothing saved under the id Claude Code gave: whether it still works cannot be told, so it is left running and the
    // two commands are named.
    let e = take_back(&r, "eeeeeeee").unwrap_err();
    assert!(stop.calls.borrow().is_empty(), "{:?}", stop.calls.borrow());
    assert!(e.contains("Run `claude stop eeeeeeee`, then `herdr-projects coordinator restart`") && e.contains("transcript was not found"), "{e}");
    assert!(start_log(&w).contains("Not taken back: its transcript was not found"), "{}", start_log(&w));
    // Saved under the full id its short one begins: that transcript is found and watched until it is still, then stopped.
    let full = "eeeeeeee-0000-4000-8000-000000000005";
    add(&file(&w, full), said("Done in the background.", 0), 0);
    let e = take_back(&r, "eeeeeeee").unwrap_err(); // its resume has no pane here
    let calls = stop.calls.borrow().clone();
    assert_eq!(calls.len(), 1, "{calls:?}"); assert_eq!(calls[0].0, "stop eeeeeeee");
    assert!(e.contains(full) && e.contains("the resume failed"), "{e}");
    let log = start_log(&w);
    let (waits, stops) = (log.rfind("Waiting until its transcript stops growing").unwrap(), log.rfind("Running `claude stop eeeeeeee`").unwrap());
    assert!(log.contains("Its transcript stopped growing.") && waits < stops, "{log}");
}
