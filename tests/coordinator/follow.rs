//! The Coordinator's conversation followed past Claude Code's continued-in records: a chat carried on in a new session
//! is resumed there, along a line of two or three, never past a missing file nor away from an old session talked in
//! after its record; one herdr brought back after a restart in an older session is started again on the newest.
use crate::common;
use common::*;
use serde_json::{json, Value};
use std::io::Write;
use std::path::PathBuf;
use sw::agents::continued::newest;
use sw::coordinator::{pump::tick, requests::send, state};
use sw::json::*;
use super::coord_world;

const B: &str = "bbbbbbbb-0000-4000-8000-000000000002";
const C: &str = "cccccccc-0000-4000-8000-000000000003";
const D: &str = "dddddddd-0000-4000-8000-000000000004";

fn drain() { flush(); sw::runner::drain_jobs(&|job, id, extra| { sw::jobs::run_job(job, id, extra); flush(); }); }
fn tell(w: &World, t: &str) { send(&w.root, t, false, "", &w.socket).unwrap(); drain(); }
fn get(w: &World) -> Obj { state::get(&w.socket).unwrap_or_default() }
fn agents(w: &World) -> Vec<Value> { w.state().arr("panes").iter().filter(|p| p.at("agent").is_object()).cloned().collect() }
fn resumed(w: &World) -> String { let a: Vec<String> = agents(w)[0].at("agent").arr("args").iter().map(text).collect(); a.iter().position(|x| x == "--resume").map(|i| a[i + 1].clone()).unwrap_or_default() }
/// Where Claude Code saves the Coordinator's conversations.
fn dir(w: &World) -> PathBuf {
    let real = std::fs::canonicalize(state::home(&w.socket)).unwrap().display().to_string();
    PathBuf::from(std::env::var("CLAUDE_CONFIG_DIR").unwrap()).join("projects").join(real.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect::<String>())
}
fn at(ms: i64) -> String { sw::clock::iso(sw::clock::now() + ms) }
fn said(text: &str, ms: i64) -> Vec<Value> {
    vec![json!({ "type": "user", "message": { "role": "user", "content": "go on" }, "timestamp": at(ms) }),
        json!({ "type": "assistant", "uuid": format!("u{ms}"), "message": { "id": format!("msg_{ms}"), "role": "assistant", "content": [{ "type": "text", "text": text }], "stop_reason": "end_turn" }, "timestamp": at(ms) })]
}
/// Claude Code carrying the chat on in a new session: the record, then its own bookkeeping.
fn link(from: &str, to: &str, ms: i64) -> Vec<Value> {
    vec![json!({ "type": "continued-in", "timestamp": at(ms), "sessionId": from, "continuedInSessionId": to }), json!({ "type": "cost-state", "sessionId": from, "totalCostUSD": 1.5 })]
}
/// Lines added to a saved conversation, which is then as new as `ms` from now.
fn save(w: &World, id: &str, rows: Vec<Value>, ms: i64) {
    let f = dir(w).join(format!("{id}.jsonl"));
    std::fs::create_dir_all(f.parent().unwrap()).unwrap();
    let mut fh = std::fs::OpenOptions::new().create(true).append(true).open(&f).unwrap();
    for r in rows { writeln!(fh, "{r}").unwrap(); }
    fh.set_modified(std::time::UNIX_EPOCH + std::time::Duration::from_millis((sw::clock::now() + ms) as u64)).unwrap();
}
/// herdr restarted: the Coordinator's pane is back in another terminal, its program resumed in `session`.
fn herdr_back(w: &World, session: &str, status: &str) {
    w.patch(|st| { for p in st["panes"].as_array_mut().unwrap() { if p.at("agent").is_object() { p["terminal_id"] = json!("term_back"); p["agent"]["session"] = json!(session); p["agent"]["status"] = json!(status); } } });
}
fn tick_later(w: &World) { sw::clock::advance(11_000); tick(&w.socket); drain(); }

#[test]
fn newest_follows_the_records_along_a_line_of_three_and_stops_at_a_missing_file_or_an_old_session_talked_in_after_its_record() {
    let w = coord_world();
    let h = state::home(&w.socket);
    let a = "aaaaaaaa-0000-4000-8000-000000000001";
    save(&w, a, said("Hello.", 0), 0);
    assert_eq!(newest("claude", a, &h), a, "no record: the session itself");
    save(&w, a, link(a, B, 100), 100);
    assert_eq!(newest("claude", a, &h), a, "the session it names is not saved: the line ends where it is");
    save(&w, B, said("Where were we?", 200), 200);
    assert_eq!(newest("claude", a, &h), B, "two linked");
    save(&w, B, link(B, C, 300), 300);
    save(&w, C, said("On it.", 400), 400);
    assert_eq!(newest("claude", a, &h), C, "a line of three, from its start"); assert_eq!(newest("claude", B, &h), C);
    save(&w, C, link(C, D, 500), 500);
    assert_eq!(newest("claude", a, &h), C, "a missing file ends the line");
    // The second one was resumed and talked in after its record: it went on there, so the line ends at it.
    save(&w, B, said("Back in the old one.", 600), 600);
    assert_eq!(newest("claude", a, &h), B); assert_eq!(newest("claude", B, &h), B);
    assert_eq!(newest("codex", a, &h), a, "only Claude Code writes the records");
}

#[test]
fn a_coordinator_whose_chat_went_on_in_a_new_session_is_resumed_in_that_one_and_its_state_keeps_that_id() {
    let w = coord_world();
    w.script(json!({ "coordinator": [{ "said": "Hello." }, { "said": "Still here." }] }));
    tell(&w, "hello");
    let first = get(&w).t("session");
    // Claude Code went on in a new session; as it quit, it wrote its bookkeeping into the old one, now the newest file.
    save(&w, B, said("Where were we?", 1000), 1000);
    save(&w, &first, link(&first, B, 500), 2000);
    w.patch(|st| { for p in st["panes"].as_array_mut().unwrap() { if p.at("agent").is_object() { p["agent"] = Value::Null; } } });
    tell(&w, "still there?");
    assert_eq!(resumed(&w), B); assert_eq!(get(&w).s("session"), B);
    assert_eq!(sw::coordinator::look::look(&w.socket).said, "Still here.");
}

#[test]
fn one_herdr_brought_back_in_an_older_session_is_restarted_at_rest_on_the_newest_of_a_line_of_three() {
    let w = coord_world();
    w.script(json!({ "coordinator": [{ "said": "Hello." }] }));
    tell(&w, "hello");
    let (first, tab) = (get(&w).t("session"), get(&w).t("tab"));
    // Claude Code went on in a second session, then a third.
    save(&w, B, said("Where were we?", 1000), 1000);
    save(&w, &first, link(&first, B, 500), 1500);
    save(&w, C, said("On it.", 2000), 2000);
    save(&w, B, link(B, C, 1800), 2500);
    // herdr restarted and resumed the first one, whose file it wrote last. Not while it works.
    herdr_back(&w, &first, "working");
    save(&w, &first, vec![json!({ "type": "cost-state", "sessionId": first })], 3000);
    tick_later(&w);
    assert_eq!(get(&w).s("tab"), tab); assert!(agents(&w)[0].at("agent").arr("args").iter().all(|a| text(a) != C));
    // At rest: a new terminal, in the newest one, and its state keeps that id.
    herdr_back(&w, &first, "idle");
    tick_later(&w);
    assert_ne!(get(&w).s("tab"), tab); assert_eq!(agents(&w).len(), 1);
    assert_eq!(resumed(&w), C); assert_eq!(get(&w).s("session"), C);
    // The board started this one itself: an older id herdr still names for it does not restart it again.
    let tab = get(&w).t("tab");
    w.patch(|st| { for p in st["panes"].as_array_mut().unwrap() { if p.at("agent").is_object() { p["agent"]["session"] = json!(first); } } });
    tick_later(&w);
    assert_eq!(get(&w).s("tab"), tab);
    // herdr restarted again and resumed the first one, and you talked in it there: that conversation went on, and stays.
    herdr_back(&w, &first, "idle");
    save(&w, &first, said("Back in the old one.", 5000), 5000);
    tick_later(&w);
    assert_eq!(get(&w).s("tab"), tab);
}

#[test]
fn a_message_to_one_herdr_brought_back_in_an_older_session_restarts_it_on_the_newest_first_and_goes_in_there() {
    let w = coord_world();
    w.script(json!({ "coordinator": [{ "said": "Hello." }, { "said": "Right here." }] }));
    tell(&w, "hello");
    let (first, tab) = (get(&w).t("session"), get(&w).t("tab"));
    save(&w, B, said("Where were we?", 1000), 1000);
    save(&w, &first, link(&first, B, 500), 2000);
    herdr_back(&w, &first, "idle");
    tell(&w, "still there?");
    assert_ne!(get(&w).s("tab"), tab); assert_eq!(agents(&w).len(), 1);
    assert_eq!(resumed(&w), B); assert_eq!(get(&w).s("session"), B);
    assert_eq!(sw::coordinator::look::look(&w.socket).said, "Right here.");
}
