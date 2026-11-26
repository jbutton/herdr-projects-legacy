//! A stop holds the whole agent: Claude Code's helpers in the background (its subagents) work on after the interrupt,
//! and each one that comes back would wake it. A helper's conversation (`<conversation>/subagents/`) still written to a
//! minute after the stop ends the agent's program, after your stop and the board's for extra usage alike; with nothing
//! more written, the agent is kept. Woken by itself, a stopped task is never re-armed (no review, no leave for extra
//! usage, nothing carried on at the window's reset); retry carries on a task you stopped, in its own conversation, and
//! the board's carrying on at the reset says that the stop was the board's.
use crate::common;
use crate::usage::{at, at_work, claude_window, limits, line, tick, told, HOUR};
use common::*;
use serde_json::{json, Value};
use std::path::PathBuf;
use sw::json::*;

/// The agent's saved conversation, where Claude Code keeps the task's session.
fn conversation(t: &Obj) -> PathBuf {
    let real = std::fs::canonicalize(sw::store::repos::cwd_of(t)).unwrap();
    let dashed: String = real.display().to_string().chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect();
    PathBuf::from(std::env::var("CLAUDE_CONFIG_DIR").unwrap()).join("projects").join(dashed).join(format!("{}.jsonl", t.s("session")))
}
/// The task's prompt in its conversation at `at`, and the agent at work on it since. `helper`: it sent a helper to the
/// background, which has not come back.
fn prompted(t: &Obj, at: i64, helper: bool) {
    let f = conversation(t);
    line(&f, json!({ "type": "user", "uuid": "u1", "timestamp": sw::clock::iso(at), "message": { "role": "user", "content": "Do the task." } }));
    if helper { line(&f, json!({ "type": "user", "uuid": "u2", "timestamp": sw::clock::iso(at + 1000), "message": { "role": "user", "content": [{ "type": "tool_result", "tool_use_id": "t1", "content": [{ "type": "text", "text": "Async agent launched successfully.\nagentId: a1 (internal ID)" }] }] } })); }
}
/// Claude Code's own lines at the interrupt.
fn interrupted(t: &Obj) { line(&conversation(t), json!({ "type": "user", "uuid": "u3", "timestamp": sw::clock::iso(sw::clock::now()), "message": { "role": "user", "content": [{ "type": "text", "text": "[Request interrupted by user]" }] } })); }
/// The helper writes on in its own conversation.
fn helper_writes(t: &Obj) {
    let f = conversation(t).with_extension("").join("subagents").join("agent-a1.jsonl");
    line(&f, json!({ "type": "assistant", "isSidechain": true, "agentId": "a1", "timestamp": sw::clock::iso(sw::clock::now()), "message": { "role": "assistant", "content": [{ "type": "text", "text": "Still checking." }] } }));
}
/// Its notification comes back to the agent, and wakes it.
fn helper_back(t: &Obj, at: i64) { line(&conversation(t), json!({ "type": "user", "uuid": "u4", "timestamp": sw::clock::iso(at), "message": { "role": "user", "content": "<task-notification>\n<task-id>a1</task-id>\n<status>completed</status>\n</task-notification>" } })); }
/// The stop's watch, at `now`.
fn pass(w: &World, now: i64) { at(now); sw::stop_hold::hold_tick(&sw::flow::live::board_for(&w.socket), now); }
fn pane(id: &str) -> String { task(id).at("link").t("pane") }
/// The agent's own turn-end report, as the hook the board gave Claude Code sends it.
fn turn_end(t: &Obj) {
    let tokens: Value = serde_json::from_str(&read(&sw::store::files::dir_of(t.s("id")).join("tokens.json"))).unwrap();
    sw::turn::turn_ended(t.s("id"), "agent", tokens.s("agent"), 0, &[json!({ "session_id": t.s("session"), "transcript_path": conversation(t).display().to_string() }).to_string()]);
}
/// The agent's program was started again in its own conversation: the last start resumes the task's session.
fn resumed(w: &World, t: &Obj) -> bool { calls(w).iter().filter(|c| c.starts_with("agent start")).last().is_some_and(|c| c.contains(&format!("--resume {}", t.s("session")))) }

#[test]
fn your_stop_ends_the_agents_program_when_a_helper_still_writes_a_minute_on_and_retry_carries_it_on_in_its_conversation() {
    let w = world();
    let id = at_work(&w, "claude", json!({}));
    let t0 = sw::clock::now();
    prompted(&task(&id), t0 - 60_000, true);
    let agent = agent_of(&w, &pane(&id));
    sw::actions::talk::stop(&id).unwrap();
    assert_eq!(task(&id).at("hold").i("at"), t0, "watched from the stop");
    interrupted(&task(&id));
    helper_writes(&task(&id));
    pass(&w, t0 + 20_000);
    assert!(!task(&id).at("hold").b("seen"), "not before the interrupt's own lines are written");
    pass(&w, t0 + 30_000);
    helper_writes(&task(&id));
    pass(&w, t0 + 50_000);
    assert!(agent_of(&w, &pane(&id)).is_object(), "not before a minute");
    pass(&w, t0 + 61_000);
    let t = task(&id);
    assert!(agent_of(&w, &pane(&id)).is_null() && w.state().arr("killed").contains(&agent.at("pid").clone()), "its program ended, its helpers with it");
    assert_eq!((t.s("stage"), t.s("msg"), t.at("hold").i("ended")), ("stopped", "stopped: its helpers worked on after the stop, so its program was ended", t0 + 61_000));
    // Retry carries it on: started again in its own conversation, told that you stopped it and that its helpers are gone.
    let said = sw::actions::finish_line::send_error_noted(&id, "").unwrap();
    assert_eq!(said.msg, format!("{id}: told the agent to continue"));
    let t = settle(&id);
    assert_eq!(t.s("stage"), "working");
    assert!(resumed(&w, &t), "{:?}", calls(&w).iter().filter(|c| c.starts_with("agent start")).collect::<Vec<_>>());
    assert!(told(&w, &id).last().unwrap().starts_with("The human stopped you, and now says to carry on. Your program was ended after the stop, as helpers you had started in the background worked on: they are gone, so start again any you still need. Continue the task where you left off."), "{:?}", told(&w, &id));
    assert!(!t.b("hold"));
}

#[test]
fn a_stop_after_which_nothing_more_is_written_keeps_the_agent() {
    let w = world();
    let id = at_work(&w, "claude", json!({}));
    let t0 = sw::clock::now();
    prompted(&task(&id), t0 - 60_000, false);
    sw::actions::talk::stop(&id).unwrap();
    interrupted(&task(&id));
    pass(&w, t0 + 30_000);
    pass(&w, t0 + 61_000);
    let t = task(&id);
    assert!(agent_of(&w, &pane(&id)).is_object(), "kept for a reply");
    assert!(w.state().arr("killed").is_empty() && !t.b("hold") && t.s("msg").is_empty(), "{}", t.at("hold"));
}

#[test]
fn a_stopped_agent_a_helper_woke_is_never_rearmed_nor_reviewed_and_a_prompt_typed_in_its_terminal_rearms_it() {
    let w = world();
    let id = at_work(&w, "claude", json!({}));
    let t0 = sw::clock::now();
    prompted(&task(&id), t0 - 60_000, true);
    sw::actions::talk::stop(&id).unwrap();
    on_pane(&w, &pane(&id)); // at rest after the interrupt
    // The helper comes back, the agent works on its answer and ends that turn: the task stays stopped, and nothing is reviewed.
    helper_back(&task(&id), t0 + 5_000);
    set_status(&w, &pane(&id), "working");
    assert_eq!((task(&id).s("stage"), task(&id).b("turnOpen")), ("stopped", false));
    set_status(&w, &pane(&id), "done");
    turn_end(&task(&id));
    let t = settle(&id);
    assert_eq!(t.s("stage"), "stopped");
    assert!(!t.b("review") && !calls(&w).iter().any(|c| regex::Regex::new(r"^agent start \S+-r ").unwrap().is_match(c)), "no review round");
    // A prompt you type in its terminal after the stop is your turn: it re-arms the task, as before.
    line(&conversation(&t), json!({ "type": "user", "uuid": "u5", "timestamp": sw::clock::iso(t0 + 9_000), "message": { "role": "user", "content": "Use the other table." } }));
    set_status(&w, &pane(&id), "working");
    let t = task(&id);
    assert_eq!((t.s("stage"), t.s("turnBy")), ("working", "typed"));
}

#[test]
fn a_stop_for_extra_usage_ends_the_agents_program_when_a_helper_still_writes_and_the_reset_resumes_it_saying_the_board_stopped_it() {
    let w = world();
    let id = at_work(&w, "claude", json!({ "usage.extra": "defer" }));
    let t0 = sw::clock::now();
    let resets = (t0 + HOUR) / 1000 * 1000;
    prompted(&task(&id), t0 - 60_000, true);
    claude_window(&id, t0, 100.0, resets);
    tick(&w, t0 + 10_000);
    let t = task(&id);
    assert_eq!((t.s("stage"), t.at("limitStop").s("what"), t.at("hold").i("at")), ("stopped", "usage", t0 + 10_000));
    let line_said = t.s("msg").to_string();
    on_pane(&w, &pane(&id));
    // Woken by its helper: no leave for extra usage, and not re-armed.
    helper_back(&t, t0 + 12_000);
    set_status(&w, &pane(&id), "working");
    let t = task(&id);
    assert_eq!((t.s("stage"), t.b("extraOk")), ("stopped", false));
    helper_writes(&t);
    pass(&w, t0 + 40_000);
    helper_writes(&t);
    pass(&w, t0 + 71_000);
    let t = task(&id);
    assert!(agent_of(&w, &pane(&id)).is_null(), "its program ended");
    assert_eq!((t.s("msg"), t.at("hold").i("ended")), (line_said.as_str(), t0 + 71_000), "the stop's own line stays");
    // At the reset it carries on by itself, in its own conversation, told the stop was the board's.
    limits(&w, resets + 1000);
    let t = settle(&id);
    assert_eq!(t.s("stage"), "working");
    assert!(resumed(&w, &t), "{:?}", calls(&w).iter().filter(|c| c.starts_with("agent start")).collect::<Vec<_>>());
    assert!(told(&w, &id).last().unwrap().starts_with("The board stopped you, not the human, because your program moved onto paid extra usage: not because of anything you did. Your program was ended after the stop, as helpers you had started in the background worked on: they are gone, so start again any you still need. The usage window has reset: continue the task where you left off."), "{:?}", told(&w, &id));
}

#[test]
fn your_stop_of_a_task_waiting_for_its_window_is_never_carried_on_at_the_reset() {
    let w = world();
    let id = at_work(&w, "claude", json!({ "usage.extra": "defer" }));
    let t0 = sw::clock::now();
    let resets = (t0 + HOUR) / 1000 * 1000;
    claude_window(&id, t0, 100.0, resets);
    tick(&w, t0 + 10_000);
    assert_eq!(task(&id).at("limitStop").s("what"), "usage");
    sw::actions::talk::stop(&id).unwrap();
    limits(&w, resets + 1000);
    let t = settle(&id);
    assert_eq!((t.s("stage"), t.b("limitStop")), ("stopped", false), "yours to carry on");
}
