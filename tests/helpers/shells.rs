//! A command Claude Code runs in the background (a Bash with `run_in_background`, or one moved there when its timeout
//! ran out) is the agent's own work still going: a turn that ends while one runs is not the end of the task's work. The
//! check, the review and "ready" start once it has ended, on the turn its end starts. Before, they started while it
//! still ran, on what the agent said before it ended (the fork's report on Shepherdr, row 4; upstream SHEP-148, #865,
//! 54a65b23).
use crate::common;
use common::*;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use sw::json::*;

fn reviewer_starts(w: &World) -> usize { calls(w).iter().filter(|c| regex::Regex::new(r"^agent start \S+-r ").unwrap().is_match(c)).count() }
fn checks(w: &World) -> usize { read(&w.base.join("checks.log")).lines().filter(|l| !l.is_empty()).count() }
fn write(file: &Path, lines: &[Value]) { std::fs::create_dir_all(file.parent().unwrap()).unwrap(); std::fs::write(file, lines.iter().map(|l| format!("{l}\n")).collect::<String>()).unwrap(); }
fn result(at: &str, text: &str, extra: Value) -> Value {
    let mut r = json!({ "stdout": "", "stderr": "", "interrupted": false, "isImage": false, "noOutputExpected": false });
    for (k, v) in extra.as_object().cloned().unwrap_or_default() { r[k] = v; }
    json!({ "timestamp": at, "type": "user", "isSidechain": false, "message": { "role": "user", "content": [{ "tool_use_id": "toolu_x", "type": "tool_result", "content": text }] }, "toolUseResult": r })
}
/// What Claude Code 2.1.314 writes for a Bash started in the background (recorded; its output file's folder cut).
fn started(at: &str, id: &str) -> Value { result(at, &format!("Command running in background with ID: {id}. Output is being written to: /tmp/tasks/{id}.output. You will be notified when it completes."), json!({ "backgroundTaskId": id })) }
/// And for one moved there when its timeout ran out (2.1.312).
fn moved(at: &str, id: &str) -> Value { result(at, &format!("Command did not complete within its 120s timeout and was moved to the background (ID: {id}). Output is being written to: /tmp/tasks/{id}.output. You will be notified when it completes."), json!({ "backgroundTaskId": id, "timedOutAfterMs": 120000 })) }
/// Its end: a `<task-notification>`, as a message of its own (`user`) or queued into a turn still going.
fn ended(at: &str, kind: &str, id: &str, status: &str) -> Value {
    let text = format!("<task-notification>\n<task-id>{id}</task-id>\n<tool-use-id>toolu_x</tool-use-id>\n<output-file>/tmp/tasks/{id}.output</output-file>\n<status>{status}</status>\n<summary>Background command \"npm test\" {status}</summary>\n</task-notification>");
    if kind == "user" { json!({ "timestamp": at, "type": "user", "isSidechain": false, "message": { "role": "user", "content": text } }) } else { json!({ "timestamp": at, "type": kind, "operation": "enqueue", "content": text }) }
}
/// TaskStop's result: a stopped shell gets no notification (seen in every recorded stop, 2.1.308 to 2.1.322).
fn stopped(at: &str, id: &str) -> Value { result(at, &json!({ "message": format!("Successfully stopped task: {id} (npm run dev)"), "task_id": id }).to_string(), json!({ "message": format!("Successfully stopped task: {id} (npm run dev)"), "task_id": id, "task_type": "local_bash" })) }

#[test]
fn a_claude_worker_whose_background_command_still_runs_is_not_finished_until_it_ends() {
    let w = cut_world("30", json!({ "worker.agent": "claude" }));
    // Every run of the check leaves a line outside the repo, so the test can count them.
    std::fs::write(w.root.join(".herdr-projects.json"), json!({ "check.command": "echo run >> \"$HERDR_PROJECTS_REPO/../checks.log\"" }).to_string()).unwrap();
    git(&w.root, &["add", "-A"]); git(&w.root, &["commit", "-qm", "check"]);
    // It commits, starts the full check in the background and ends its turn; the check's end wakes it, and it is done.
    let first = json!({ "files": { "f.txt": "v1\n" }, "commit": "v1", "note": "Did v1.", "ends": "claude-shell-running-made", "hook": true, "after": 0, "then": { "ends": "claude-shell-back-made", "hook": true } });
    w.script(json!({ "worker": [first] }));
    let id = w.add("Background check");
    delegate(&id);
    let t = settle(&id);
    assert_eq!((t.s("stage"), checks(&w), reviewer_starts(&w)), ("working", 0, 0), "{}", t.s("msg"));
    let looks = calls(&w).len();
    let pane = t.at("link").t("pane");
    set_status(&w, &pane, "working"); set_status(&w, &pane, "idle"); // as herdr saw it: at work, then at rest
    assert_eq!(settle(&id).s("stage"), "working"); // looked at again, it still waits for its command
    assert_eq!((row(&id).s("note"), checks(&w), reviewer_starts(&w)), ("Finishing", 0, 0));
    assert!(calls(&w).len() - looks < 20, "{:?}", &calls(&w)[looks..]);
    w.go_on(&t);
    let t = settle(&id);
    assert_eq!(t.s("stage"), "approval", "{} {}", t.s("msg"), t.at("failure"));
    assert_eq!((checks(&w), reviewer_starts(&w)), (1, 1));
    assert!(t.s("said").contains("The full check passed. Done."), "{}", t.s("said"));
}

#[test]
fn a_background_command_runs_until_its_notification_or_its_stop() {
    let _w = world();
    let started_at = sw::clock::now();
    let at = |ms: i64| sw::clock::iso(started_at + ms);
    let file = PathBuf::from(std::env::var("CLAUDE_CONFIG_DIR").unwrap()).join("projects").join("x").join("shells.jsonl");
    let running = |lines: &[Value]| { write(&file, lines); sw::agents::helpers::running("claude", &file, started_at) };
    // Started in the background, however long ago: at work until it ends.
    let mut lines = vec![started(&at(1000), "bk3m9x2qa")];
    sw::clock::advance(3 * 3600000);
    assert_eq!(running(&lines), 1);
    // Its end came back while a turn was still going: queued, taken into that turn.
    lines.push(ended(&at(2000), "queue-operation", "bk3m9x2qa", "completed"));
    assert_eq!(running(&lines), 0);
    // A foreground one moved to the background by its timeout, ending in a failure of its own.
    lines.push(moved(&at(3000), "b5ea5sjiy"));
    assert_eq!(running(&lines), 1);
    lines.push(ended(&at(4000), "user", "b5ea5sjiy", "failed"));
    assert_eq!(running(&lines), 0);
    // Stopped by the agent: no notification, only TaskStop's result.
    lines.push(started(&at(5000), "bumphhsle"));
    assert_eq!(running(&lines), 1);
    lines.push(stopped(&at(6000), "bumphhsle"));
    assert_eq!(running(&lines), 0);
    // A command's own output that only quotes such a line, and Claude Code's copies of its requests, start nothing.
    lines.push(result(&at(7000), "  10 Command running in background with ID: b0po6ap81", json!({})));
    lines.push(json!({ "type": "api-request-blob", "timestamp": at(7000), "message": { "role": "user", "content": started(&at(1), "bq8w3e5rt").at("message").at("content").clone() }, "toolUseResult": { "backgroundTaskId": "bq8w3e5rt" } }));
    assert_eq!(running(&lines), 0);
    // A shell lives in the agent's program: one left running by the program before this one is gone with it.
    lines.push(started(&at(8000), "b7h2kq0zx"));
    assert_eq!(running(&lines), 1);
    assert_eq!(sw::agents::helpers::running("claude", &file, sw::clock::now()), 0);
}

#[test]
fn a_command_that_never_ends_holds_the_task_at_finishing_and_the_finish_key_takes_the_work_as_it_is() {
    let w = cut_world("30", json!({ "worker.agent": "claude" }));
    w.script(json!({ "worker": [{ "files": { "f.txt": "v1\n" }, "commit": "v1", "note": "Did v1.", "ends": "claude-shell-running-made", "hook": true }] }));
    let id = w.add("Left running");
    delegate(&id);
    let t = settle(&id);
    let pane = t.at("link").t("pane");
    set_status(&w, &pane, "working"); set_status(&w, &pane, "idle");
    assert_eq!((settle(&id).s("stage"), row(&id).s("note"), reviewer_starts(&w)), ("working", "Finishing", 0));
    assert!(sw::shown::states::stuck_finishing(&row(&id)));
    sw::actions::finish_line::finish_anyway(&id).unwrap();
    let t = settle(&id);
    assert_eq!(t.s("stage"), "approval", "{} {}", t.s("msg"), t.at("failure"));
    assert_eq!(reviewer_starts(&w), 1);
}
