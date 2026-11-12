//! While an agent works, whoever ticks (the keeper, an open board) refreshes its last words, tokens and model from its
//! saved conversation every five seconds (keeper.rs: `refresh_said`). That file is read again only when its size or
//! time changed, and then only what was written since: these files grow to many megabytes, and every working task's was
//! read, up to its last 768 KB, at every pass.
use crate::common;
use common::*;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use sw::json::*;
use sw::store::tasks::{create, update, Change};

fn line(file: &Path, o: Value) { use std::io::Write; let mut f = std::fs::OpenOptions::new().create(true).append(true).open(file).unwrap(); writeln!(f, "{o}").unwrap(); }
fn iso(at: i64) -> String { sw::clock::iso(at) }
fn prompt(file: &Path, text: &str) { let at = sw::clock::now(); line(file, json!({ "type": "user", "uuid": format!("u{at}"), "timestamp": iso(at), "message": { "role": "user", "content": text } })); }
/// The agent's words; with `tool`, a tool call follows them in the same message.
fn says(file: &Path, n: usize, text: &str, tool: bool) {
    let at = sw::clock::now();
    let mut content = vec![json!({ "type": "text", "text": text })];
    if tool { content.push(json!({ "type": "tool_use", "id": format!("t{n}"), "name": "Bash", "input": {} })); }
    line(file, json!({ "type": "assistant", "uuid": format!("a{n}"), "timestamp": iso(at), "message": { "id": format!("msg_{n}"), "role": "assistant", "model": "claude-opus-4-1", "stop_reason": if tool { "tool_use" } else { "end_turn" }, "content": content, "usage": { "input_tokens": 1000 * n, "output_tokens": 10 * n } } }));
}
fn result(file: &Path, n: usize, bytes: usize) { let at = sw::clock::now(); line(file, json!({ "type": "user", "uuid": format!("r{n}"), "timestamp": iso(at), "message": { "role": "user", "content": [{ "type": "tool_result", "tool_use_id": format!("t{n}"), "content": "x".repeat(bytes) }] } })); }

/// A task at work in a repo of its own (its id is its own too: the keeper's five seconds are counted by task id, in this
/// process that all tests share), and its saved conversation: a prompt and a tool result larger than the part read.
fn working(w: &World, repo: &str) -> (String, PathBuf) {
    let root = w.base.join(repo);
    std::fs::create_dir_all(&root).unwrap();
    git(&root, &["init", "-q", "-b", "main"]);
    let id = create(&root, "Work on it", "", None).unwrap().t("id");
    let file = w.base.join(format!("{repo}.jsonl"));
    update(&id, |t| { t.put("stage", "working"); t.put("agent", "claude"); t.put("transcript", file.display().to_string()); t.put("link", json!({ "pane": "p1" })); Ok(Change::Write) }).unwrap();
    prompt(&file, "Go on.");
    says(&file, 1, "Reading the files.", true);
    result(&file, 1, 900 * 1024);
    (id, file)
}
/// One pass of the keeper's, five seconds after the last: the task as it then is.
fn pass(id: &str) -> Obj {
    sw::clock::advance(5000);
    let mut row = task(id);
    row.put("live", json!({ "status": "working" }));
    sw::keeper::refresh_said(&[row], sw::clock::now());
    task(id)
}
/// The file rewritten in place to the same size: what no agent program does (they only add to its end), so it shows
/// whether a pass read again what it had read before. `keep_time`: its time is set back as it was too.
fn rewrite(file: &Path, from: &str, to: &str, keep_time: bool) {
    assert_eq!(from.len(), to.len());
    let at = std::fs::metadata(file).unwrap().modified().unwrap();
    let text = std::fs::read_to_string(file).unwrap();
    assert!(text.contains(from), "{from}");
    std::fs::write(file, text.replacen(from, to, 1)).unwrap();
    if keep_time { std::fs::File::options().write(true).open(file).unwrap().set_modified(at).unwrap(); }
}

#[test]
fn a_working_tasks_saved_conversation_is_not_read_again_while_its_size_and_time_stay_the_same() {
    let w = world();
    sw::clock::set(Some(sw::clock::now()));
    let (id, file) = working(&w, "said-same");
    says(&file, 2, "First words.", false);
    let t = pass(&id);
    assert_eq!((t.s("said"), t.at("usage").i("context"), t.s("model")), ("First words.", 2020, "claude-opus-4-1"));
    rewrite(&file, "First words.", "Other words.", true);
    assert_eq!(pass(&id).s("said"), "First words.", "the file was read again, with nothing added to it");
    says(&file, 3, "Second words.", false);
    let t = pass(&id);
    assert_eq!((t.s("said"), t.at("usage").i("context")), ("Second words.", 3030), "what is added is read");
}

#[test]
fn a_working_tasks_saved_conversation_that_grew_is_read_only_from_where_the_last_pass_ended() {
    let w = world();
    sw::clock::set(Some(sw::clock::now()));
    let (id, file) = working(&w, "said-grown");
    says(&file, 2, "Looked at A.", true);
    assert_eq!(pass(&id).s("said"), "Looked at A.");
    // The old part changed in place, and a tool result added: only that result is read, and the agent's last words
    // are still those read before.
    rewrite(&file, "Looked at A.", "Looked at B.", false);
    result(&file, 2, 100);
    let t = pass(&id);
    assert_eq!((t.s("said"), t.at("usage").i("context")), ("Looked at A.", 2020), "the old part was read again");
    says(&file, 3, "Done.", false);
    assert_eq!(pass(&id).s("said"), "Done.");
}
