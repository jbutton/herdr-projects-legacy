//! While an agent works, whoever ticks (the keeper, an open board) refreshes its last words, tokens and model from its
//! saved conversation every five seconds (keeper.rs: `refresh_said`). That file is read again only when its size or
//! time changed, and then only what was written since: these files grow to many megabytes, and every working task's was
//! read, up to its last 768 KB, at every pass. What it reads is what a read of the file's last bytes reads, however the
//! file grows or changes (agents/read_kept.rs).
use crate::common;
use common::*;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use sw::agents::read_kept::{bytes_read, read_turn_kept};
use sw::agents::read_turn::{read_turn, TAIL};
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

/// A small source of numbers, the same at every run.
struct Dice(u64);
impl Dice { fn below(&mut self, n: u64) -> u64 { self.0 ^= self.0 << 13; self.0 ^= self.0 >> 7; self.0 ^= self.0 << 17; self.0 % n.max(1) } }
fn size_of(file: &Path) -> u64 { std::fs::metadata(file).map(|m| m.len()).unwrap_or(0) }

#[test]
fn a_pass_reads_only_what_was_written_since_and_nothing_when_nothing_was() {
    let w = world();
    let file = w.base.join("long.jsonl");
    prompt(&file, "Go on.");
    says(&file, 1, "Reading.", true);
    result(&file, 1, 900 * 1024);
    says(&file, 2, "Read it.", true);
    let read = |said: &str| {
        let n = bytes_read();
        let kept = read_turn_kept("claude", &file, TAIL);
        assert_eq!(kept, read_turn("claude", &file, TAIL));
        assert_eq!(kept.map(|t| t.said).as_deref(), Some(said));
        bytes_read() - n
    };
    assert_eq!(read("Read it."), TAIL, "the first time: its last 768 KB");
    assert_eq!(read("Read it."), 0, "nothing changed: nothing read");
    let size = size_of(&file);
    result(&file, 2, 100);
    assert_eq!(read("Read it."), size_of(&file) - size + 1, "what was added, and the line's end before it");
    let size = size_of(&file);
    says(&file, 3, "Done.", false);
    assert_eq!(read("Done."), size_of(&file) - size + 1, "again only what was added");
}

#[test]
fn what_is_kept_of_a_saved_conversation_reads_as_its_last_bytes_however_it_grows_or_changes() {
    let w = world();
    let mut dice = Dice(0x9e37_79b9_7f4a_7c15);
    let window = 2048;
    for kind in ["claude", "codex", "pi"] {
        // The program's own lines (tests/fixtures/endings), a line longer than the part read, letters of more than one
        // byte, a line that is no JSON and an empty one: written in pieces of any size, cut anywhere.
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/endings");
        let mut pool: Vec<String> = std::fs::read_dir(&dir).unwrap().flatten().map(|e| e.path()).filter(|p| p.file_name().unwrap().to_string_lossy().starts_with(&format!("{kind}-")))
            .flat_map(|p| std::fs::read_to_string(p).unwrap().lines().map(String::from).collect::<Vec<_>>()).filter(|l| !l.is_empty()).collect();
        pool.sort();
        assert!(pool.len() >= 6, "{kind}: {}", pool.len());
        pool.push(json!({ "type": "note", "text": "żółć ".repeat(400) }).to_string());
        pool.push("{ not json".into());
        pool.push(String::new());
        let file = w.base.join(format!("{kind}.jsonl"));
        let (mut stream, mut turns) = (Vec::<u8>::new(), 0);
        for step in 0..800 {
            let same = match dice.below(40) {
                0 if size_of(&file) > 0 => { std::fs::File::options().write(true).open(&file).unwrap().set_len(dice.below(size_of(&file))).unwrap(); false }
                1 => {
                    let new = w.base.join("new.jsonl");
                    std::fs::write(&new, (0..=dice.below(4)).map(|_| format!("{}\n", pool[dice.below(pool.len() as u64) as usize])).collect::<String>()).unwrap();
                    std::fs::rename(&new, &file).unwrap();
                    false
                }
                2 if size_of(&file) > 0 => {
                    let at = std::fs::metadata(&file).unwrap().modified().unwrap();
                    let mut b = std::fs::read(&file).unwrap();
                    let i = dice.below(b.len() as u64) as usize;
                    if b[i] != b'\n' { b[i] = b'q'; }
                    std::fs::write(&file, &b).unwrap();
                    std::fs::File::options().write(true).open(&file).unwrap().set_modified(at + std::time::Duration::from_secs(1)).unwrap();
                    false
                }
                3..=5 => true,
                _ => {
                    let n = 1 + if dice.below(10) == 0 { dice.below(3 * window) } else { dice.below(window / 3) } as usize;
                    while stream.len() < n { stream.extend(pool[dice.below(pool.len() as u64) as usize].bytes()); stream.push(b'\n'); }
                    use std::io::Write;
                    std::fs::OpenOptions::new().create(true).append(true).open(&file).unwrap().write_all(&stream.drain(..n).collect::<Vec<u8>>()).unwrap();
                    false
                }
            };
            let n = bytes_read();
            let kept = read_turn_kept(kind, &file, window);
            assert_eq!(kept, read_turn(kind, &file, window), "{kind}, step {step}");
            if same { assert_eq!(bytes_read(), n, "{kind}, step {step}: nothing changed, yet it was read"); }
            turns += kept.is_some() as usize;
        }
        assert!(turns > 400, "{kind}: only {turns} reads found a turn");
    }
}
