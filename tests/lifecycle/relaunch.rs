//! A working task's agent whose terminal went (herdr or the Mac restarted) or whose program exited is started again by
//! the board itself, in its own conversation: three tries at most, further apart each time, then the task waits for you
//! with why. Never a task stopped on purpose, stopped at a limit, done or archived.
use crate::common;
use common::*;
use serde_json::{json, Value};
use sw::housekeeping::{housekeeping, Force};
use sw::json::*;
use sw::store::tasks::patch;

fn keep(w: &World) { housekeeping(&w.socket, Force::Yes); }
/// The worker's starts so far (not a reviewer's), as herdr was asked for them.
fn starts(w: &World) -> Vec<String> { calls(w).into_iter().filter(|c| c.starts_with("agent start") && !c.contains("-r ")).collect() }
/// Its conversation, saved where Claude Code keeps it: what it resumes.
fn saved(t: &Obj) {
    let dir = std::path::Path::new(&std::env::var("CLAUDE_CONFIG_DIR").unwrap()).join("projects").join("-work");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join(format!("{}.jsonl", t.s("session"))), format!("{}\n", json!({ "type": "user", "message": { "role": "user", "content": "Work board task" } }))).unwrap();
}
/// A task whose agent is at work, its conversation saved.
fn working(w: &World, title: &str) -> String {
    let id = w.add(title);
    delegate(&id);
    let t = settle(&id);
    assert_eq!((t.s("stage"), row(&id).t("state")), ("working", "working".into()), "{}", t.at("failure"));
    saved(&t);
    id
}
/// Its program ends, to the pane's shell.
fn exits(w: &World, id: &str) {
    let pane = task(id).at("link").t("pane");
    w.patch(|s| { for p in s["panes"].as_array_mut().unwrap() { if p.s("pane_id") == pane { p["agent"] = Value::Null; } } });
}
/// Its terminal closes, and with it its space, as herdr leaves no empty one.
fn closes(w: &World, id: &str) {
    let pane = task(id).at("link").t("pane");
    w.patch(|s| { s["panes"].as_array_mut().unwrap().retain(|p| p.s("pane_id") != pane); common::herdr::FakeHerdr::drop_empty(s); });
}
/// Time goes by, and the board looks after its tasks.
fn later(w: &World, id: &str, ms: i64) -> Obj { sw::clock::advance(ms); keep(w); settle(id) }

#[test]
fn a_working_agent_whose_terminal_went_comes_back_by_itself_in_its_own_conversation() {
    let w = world_with(Opts { repo: true, settings: json!({ "review.when": "never" }), ..Default::default() });
    w.script(json!({ "worker": [{ "status": "working" }, { "status": "working" }] }));
    let id = working(&w, "Through a reboot");
    let session = task(&id).t("session");
    // herdr (or the Mac) restarted: the agent's terminal is gone
    closes(&w, &id);
    keep(&w);
    assert_eq!((row(&id).t("state"), row(&id).t("note")), ("error".into(), "The agent's terminal is gone".into()));
    // not at once: herdr may still be on its way back
    later(&w, &id, 10_000);
    assert_eq!(starts(&w).len(), 1);
    let t = later(&w, &id, 11_000);
    // started again, in a new terminal of its space, resuming its conversation, and told why
    let s = starts(&w);
    assert_eq!(s.len(), 2, "{s:?}");
    assert!(s[1].contains(&format!("--resume {session}")), "{}", s[1]);
    assert_eq!((t.s("stage"), t.t("session"), row(&id).t("state")), ("working", session.clone(), "working".into()), "{}", t.at("failure"));
    let said = prompts(&w, t.at("link").s("pane"));
    assert!(said.last().unwrap().contains("the board started it again. Continue the task where you left off."), "{said:?}");
    assert_eq!(t.at("relaunched").i("tries"), 1);
    // nothing more while it runs
    later(&w, &id, 120_000);
    assert_eq!(starts(&w).len(), 2);
    // ten minutes of it running without a break: its tries count from none again
    let t = later(&w, &id, 600_000);
    assert!(!t.b("relaunched"), "{}", t.at("relaunched"));
}

#[test]
fn one_that_keeps_stopping_is_tried_three_times_further_apart_then_waits_for_you_with_why() {
    let w = world_with(Opts { repo: true, settings: json!({ "review.when": "never" }), ..Default::default() });
    w.script(json!({ "worker": [{ "status": "working" }, { "status": "working" }, { "status": "working" }, { "status": "working" }] }));
    let id = working(&w, "Keeps crashing");
    // its program exits to its shell
    exits(&w, &id);
    keep(&w);
    assert_eq!((row(&id).t("state"), row(&id).t("note")), ("error".into(), "The agent has exited".into()));
    // the first try, in the same terminal, once it has stayed down a while
    later(&w, &id, 21_000);
    assert_eq!(starts(&w).len(), 2);
    exits(&w, &id);
    // the second a minute after the first
    keep(&w);
    later(&w, &id, 30_000);
    assert_eq!(starts(&w).len(), 2);
    later(&w, &id, 31_000);
    assert_eq!(starts(&w).len(), 3);
    exits(&w, &id);
    // the third five minutes after the second
    keep(&w);
    later(&w, &id, 120_000);
    assert_eq!(starts(&w).len(), 3);
    let t = later(&w, &id, 181_000);
    assert_eq!(starts(&w).len(), 4);
    assert_eq!(t.at("relaunched").i("tries"), 3);
    exits(&w, &id);
    // down once more: the board gives up, and the task waits for you with why
    keep(&w);
    let t = later(&w, &id, 21_000);
    assert_eq!(t.s("stage"), "failed");
    assert_eq!(t.at("failure").s("title"), "The agent keeps stopping");
    assert!(t.at("failure").s("detail").contains("started it again 3 times"), "{}", t.at("failure").s("detail"));
    assert_eq!((row(&id).t("state"), row(&id).t("note")), ("error".into(), "The agent keeps stopping: started again 3 times".into()));
    assert_eq!(sw::notify::event_of(&t), "failed");
    // and that is all: no more tries
    later(&w, &id, 3_600_000);
    assert_eq!(starts(&w).len(), 4);
    // a reply starts it again, and its tries count from none
    w.script(json!({ "worker": [{ "status": "working" }] }));
    sw::actions::talk::tell(&id, "Try once more.", false).unwrap();
    let t = settle(&id);
    assert_eq!(starts(&w).len(), 5);
    assert_eq!(t.s("stage"), "working");
    assert!(!t.b("relaunched"), "{}", t.at("relaunched"));
}

#[test]
fn never_one_stopped_on_purpose_stopped_at_a_limit_done_or_archived() {
    let w = world_with(Opts { repo: true, settings: json!({ "review.when": "never" }), ..Default::default() });
    w.script(json!({ "worker": [{ "status": "working" }, { "status": "working" }, { "status": "working" }, { "status": "working" }] }));
    // stopped by you, then its terminal closed
    let stopped = working(&w, "Stopped by you");
    sw::actions::talk::stop(&stopped).unwrap();
    closes(&w, &stopped);
    // stopped at its time limit, then its program exited
    let limit = working(&w, "At its limit");
    patch(&limit, |t| { t.put("limitStop", json!({ "what": "time", "limit": "2h", "role": "agent", "used": 7_200_000, "at": sw::clock::now() })); t.put("stage", "stopped"); }).unwrap();
    exits(&w, &limit);
    // done, its program still in its terminal, which then closed
    let done = working(&w, "Done");
    patch(&done, |t| { t.put("stage", "done"); }).unwrap();
    closes(&w, &done);
    // archived while its agent worked, then its terminal closed
    let archived = working(&w, "Archived");
    patch(&archived, |t| { t.put("archivedAt", sw::clock::now()); }).unwrap();
    closes(&w, &archived);
    let before = starts(&w).len();
    for ms in [21_000, 61_000, 301_000, 21_000] { sw::clock::advance(ms); keep(&w); for id in [&stopped, &limit, &done, &archived] { settle(id); } }
    assert_eq!(starts(&w).len(), before);
    for id in [&stopped, &limit, &done, &archived] { assert!(!task(id).b("relaunched"), "{id}"); }
    assert_eq!((task(&stopped).t("stage"), task(&limit).t("stage"), task(&done).t("stage")), ("stopped".into(), "stopped".into(), "done".into()));
}

#[test]
fn nor_an_attempt_whose_agent_went_it_is_out_as_before() {
    let w = world_with(Opts { repo: true, settings: json!({ "review.when": "never" }), ..Default::default() });
    w.script(json!({ "worker": [{ "status": "working" }, { "status": "working" }] }));
    let id = w.add("Two ways");
    delegate_with(&id, json!({ "attempts.by": "claude, codex" }));
    sw::runner::drain_jobs(&|job, id, extra| { sw::jobs::run_job(job, id, extra); flush(); });
    let a = kid(&id, "A");
    assert_eq!(a.s("stage"), "working");
    closes(&w, a.s("id"));
    let before = starts(&w).len();
    for ms in [21_000, 61_000, 301_000] { sw::clock::advance(ms); keep(&w); }
    assert_eq!(starts(&w).len(), before);
    assert!(!kid(&id, "A").b("relaunched"));
    assert_eq!(sw::attempts::words::standing_of(&kid(&id, "A"), sw::clock::now()), "out");
}

#[test]
fn a_restart_asked_for_and_not_run_yet_leaves_a_task_you_stopped_meanwhile_alone() {
    let w = world_with(Opts { repo: true, settings: json!({ "review.when": "never" }), ..Default::default() });
    w.script(json!({ "worker": [{ "status": "working" }, { "status": "working" }] }));
    let id = working(&w, "Stopped before its restart ran");
    exits(&w, &id);
    keep(&w);
    sw::clock::advance(21_000);
    // a hook in a plan's sandbox asks the keeper for the restart, which runs it a while later
    unsafe { std::env::set_var("HERDR_PROJECTS_BOXED", "1"); }
    keep(&w);
    unsafe { std::env::remove_var("HERDR_PROJECTS_BOXED"); }
    assert!(sw::runner::outside::asked().iter().any(|j| j.0 == "restart" && j.1 == id));
    assert_eq!(task(&id).at("relaunched").i("tries"), 1);
    // you stop it meanwhile: the restart, when it runs, does nothing
    sw::actions::talk::stop(&id).unwrap();
    sw::runner::outside::start_asked();
    let t = settle(&id);
    assert_eq!((starts(&w).len(), t.t("stage")), (1, "stopped".into()));
    // and its words are not left for a later start: a reply starts it with yours alone
    sw::actions::talk::tell(&id, "Go on with the docs.", false).unwrap();
    let t = settle(&id);
    assert_eq!(starts(&w).len(), 2);
    let said = prompts(&w, t.at("link").s("pane")).join("\n");
    assert!(said.contains("Go on with the docs.") && !said.contains("the board started it again"), "{said}");
}

#[test]
fn a_pane_id_herdr_gave_to_another_agent_is_left_alone_and_the_task_comes_back_in_a_pane_of_its_own() {
    let w = world_with(Opts { repo: true, settings: json!({ "review.when": "never" }), ..Default::default() });
    w.script(json!({ "worker": [{ "status": "working" }, { "status": "working" }] }));
    let id = working(&w, "Its pane id given away");
    let pane = task(&id).at("link").t("pane");
    // herdr restarted and handed the task's pane id to another terminal, another agent in another folder
    let elsewhere = w.base.join("other-project").display().to_string();
    w.patch(|s| { for p in s["panes"].as_array_mut().unwrap() { if p.s("pane_id") == pane { p["terminal_id"] = json!("term-of-another"); p["cwd"] = json!(elsewhere); p["agent"]["name"] = json!("another-agent"); p["agent"]["prompts"] = json!([]); } } });
    keep(&w);
    assert_eq!(row(&id).t("note"), "The agent's terminal is gone");
    let t = later(&w, &id, 21_000);
    assert!(prompts(&w, &pane).is_empty(), "the other agent was told: {:?}", prompts(&w, &pane));
    assert_eq!(starts(&w).len(), 2);
    assert_ne!(t.at("link").t("pane"), pane);
    assert!(prompts(&w, t.at("link").s("pane")).last().unwrap().contains("the board started it again"));
}

#[test]
fn herdr_not_answering_about_the_agent_changes_nothing() {
    let w = world_with(Opts { repo: true, settings: json!({ "review.when": "never" }), ..Default::default() });
    w.script(json!({ "worker": [{ "status": "working" }] }));
    let id = working(&w, "herdr times out");
    let last = task(&id).at("last").clone();
    w.patch(|s| s["agentGetFails"] = json!(true));
    keep(&w);
    let t = later(&w, &id, 21_000);
    later(&w, &id, 400_000);
    assert_eq!((t.t("stage"), starts(&w).len(), t.b("relaunched"), t.b("exitedAt")), ("working".into(), 1, false, false), "{}", t.at("failure"));
    assert_eq!(task(&id).at("last"), &last);
    w.patch(|s| s["agentGetFails"] = json!(false));
    keep(&w);
    assert_eq!((row(&id).t("state"), task(&id).t("stage")), ("working".into(), "working".into()));
}

#[test]
fn a_moment_of_running_between_two_stops_does_not_count_its_tries_from_none_ten_minutes_without_a_break_does() {
    let w = world_with(Opts { repo: true, settings: json!({ "review.when": "never" }), ..Default::default() });
    w.script(json!({ "worker": [{ "status": "working" }, { "status": "working" }] }));
    let id = working(&w, "Up and down");
    exits(&w, &id);
    keep(&w);
    later(&w, &id, 21_000);
    let pane = task(&id).at("link").t("pane");
    let agent = agent_of(&w, &pane);
    // it runs for almost ten minutes, stops, and comes back by itself a moment later
    later(&w, &id, 30_000);
    sw::clock::advance(560_000);
    exits(&w, &id);
    keep(&w);
    sw::clock::advance(11_000);
    w.patch(|s| { for p in s["panes"].as_array_mut().unwrap() { if p.s("pane_id") == pane { p["agent"] = agent.clone(); } } });
    keep(&w);
    assert_eq!(task(&id).at("relaunched").i("tries"), 1, "its tries counted from none after a break");
    // nine minutes more without a break: still its first try
    later(&w, &id, 540_000);
    assert_eq!(task(&id).at("relaunched").i("tries"), 1);
    // ten minutes since it came back: from none again
    let t = later(&w, &id, 61_000);
    assert!(!t.b("relaunched"), "{}", t.at("relaunched"));
}
