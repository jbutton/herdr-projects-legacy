//! A finished turn's end that is not a turn: Claude Code compacting a session left idle (#423) leaves a task waiting
//! for approval where it was; and a turn that stopped without a summary says the agent's words, never the program's
//! input box, a draft typed into it, or the status lines under it (#406).
use crate::common;
use common::*;
use serde_json::{json, Value};
use std::path::Path;
use sw::json::*;

fn claude() -> World { world_with(Opts { repo: true, settings: json!({ "worker.agent": "claude" }), ..Default::default() }) }
/// herdr's count of finished turns moves on, with no reading of the agent at work before it.
fn counted(w: &World, pane: &str) {
    w.patch(|s| { let seq = s.i("seq") + 1; s["seq"] = json!(seq); for p in s["panes"].as_array_mut().unwrap() { if p.s("pane_id") == pane { p["agent"]["status"] = json!("done"); p["agent"]["seq"] = json!(seq); p["agent"]["completion"] = json!(seq); } } });
    on_pane(w, pane);
}

#[test]
fn claude_compacting_a_finished_tasks_idle_session_leaves_it_waiting_for_approval() {
    let w = claude();
    w.script(json!({ "worker": [{ "files": { "f.txt": "v1\n" }, "commit": "v1", "note": "Did v1.", "ends": "claude-finished-made", "hook": true }] }));
    let id = to_approval(&w, "Idle compaction");
    let t = task(&id);
    let (msg, pane) = (t.t("msg"), t.at("link").t("pane"));
    // An hour later, nobody touching it: Claude Code compacts the session, and writes no prompt and no reply.
    sw::clock::advance(3_600_000);
    let at = sw::clock::now();
    let lines: String = endings::fixture("claude-idle-compaction").into_iter().enumerate().map(|(i, mut l)| { if l.b("timestamp") { l["timestamp"] = json!(sw::clock::iso(at + i as i64)); } format!("{l}\n") }).collect();
    use std::io::Write;
    std::fs::OpenOptions::new().append(true).open(t.s("transcript")).unwrap().write_all(lines.as_bytes()).unwrap();
    counted(&w, &pane);
    let t = settle(&id);
    assert_eq!((t.s("stage"), t.t("msg")), ("approval", msg), "{}", t.at("failure"));
}

#[test]
fn a_claude_agent_is_started_with_idle_compaction_off() {
    let w = world(); // a test world starts agents without their hooks: the launch settings are asked for here
    let h = sw::agents::adapter::Hook { shim: "/bin/board".into(), id: "repo/1".into(), role: "agent".into(), token: "t".into(), round: 0, dir: w.base.clone() };
    let args = sw::agents::adapter::hook_args("claude", &h).unwrap().unwrap();
    let file = &args[args.iter().position(|a| a == "--settings").unwrap() + 1];
    let s: Value = serde_json::from_str(&read(Path::new(file))).unwrap();
    assert_eq!(s.at("idleCompaction"), &json!(false));
    assert!(s.at("hooks").at("Stop").is_array());
}

const CLAUDE_SCREEN: &str = "● I looked at the failing test and stopped: the fixture is missing.\n\n────────────────────────────────────────\n❯ \n────────────────────────────────────────\n  ~/repo · main · Opus · 42% context\n  ⏵⏵ bypass permissions on (shift+tab to cycle)\n";

#[test]
fn a_turn_that_stopped_without_a_summary_says_the_agents_words_not_its_status_lines() {
    let w = world_with(Opts { repo: true, settings: json!({ "review.when": "never" }), ..Default::default() });
    w.script(json!({ "worker": [{ "files": { "x.txt": "x\n" }, "note": null, "screen": CLAUDE_SCREEN }] }));
    let id = w.add("No summary");
    delegate(&id);
    let t = settle(&id);
    assert_eq!(t.s("stage"), "nosummary");
    assert_eq!(t.s("msg"), "● I looked at the failing test and stopped: the fixture is missing.");
    for junk in ["bypass permissions", "42% context", "❯"] { assert!(!t.s("screen").contains(junk), "{}", t.s("screen")); }
}

/// A screen recorded from a real Claude Code (tests/fixtures/screens).
fn recorded(name: &str) -> String { read(&Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/screens").join(format!("{name}.txt"))) }
const LAST_WORDS: &str = "⏺ The spec still fails on the missing fixture.\n  I stopped here; it needs the fixture file from you.";

#[test]
fn a_draft_of_more_than_eight_lines_in_claudes_input_box_is_not_taken_for_its_words_nor_is_its_footer() {
    let w = world_with(Opts { repo: true, settings: json!({ "review.when": "never" }), ..Default::default() });
    w.script(json!({ "worker": [{ "files": { "x.txt": "x\n" }, "note": null, "screen": recorded("claude-draft-in-box") }] }));
    let id = w.add("Draft in the box");
    delegate(&id);
    let t = settle(&id);
    assert_eq!(t.s("stage"), "nosummary");
    assert_eq!(t.s("msg"), "⏺ The spec still fails on the missing fixture.   I stopped here; it needs the fixture file from you.");
    assert!(t.s("screen").ends_with(LAST_WORDS), "{}", t.s("screen"));
    for junk in ["bypass permissions", "% left", "Draft reply", "❯ Draft"] { assert!(!t.s("screen").contains(junk), "{}", t.s("screen")); }
}

#[test]
fn claude_codes_line_after_a_turn_is_not_the_agents_words() {
    let words = sw::agents::screen_text::agent_words(&recorded("claude-at-rest"));
    assert!(words.ends_with(LAST_WORDS), "{words}");
    for junk in ["Cooked for", "Cogitated for", "bypass permissions"] { assert!(!words.contains(junk), "{words}"); }
    assert!(words.contains("⏺ I looked at the failing test and stopped, the fixture is missing."), "{words}"); // the turn before stays
}

#[test]
fn claude_codes_notice_over_its_input_box_is_not_the_agents_words() {
    let words = sw::agents::screen_text::agent_words(&recorded("claude-notice-over-box"));
    assert!(words.ends_with("  The report now notes that they're posted. The closing note still calls them drafts; I didn't change it."), "{words}");
    for junk in ["new task?", "Crunched for", "bypass permissions"] { assert!(!words.contains(junk), "{words}"); }
    // a line of the agent's own right over the box, indented, in a pane of any width, is its words, not a notice
    for (width, indent) in [(80, 8), (40, 8), (40, 16), (24, 8), (40, 34)] {
        let words = command_over_box(width, indent);
        assert!(words.ends_with(&format!("\n\n{}{CURL}", " ".repeat(indent))), "width {width}, indent {indent}: {words:?}");
    }
    // while the notice of a narrow pane is left out all the same
    let narrow = format!("⏺ Done.\n{}new task? /clear to save 599.8k tokens\n{}\n❯\n{}\n  ⏵⏵ bypass permissions on", " ".repeat(10), "─".repeat(50), "─".repeat(50));
    assert_eq!(sw::agents::screen_text::agent_words(&narrow), "⏺ Done.");
}
const CURL: &str = "curl --retry 3 --connect-timeout 20 https://service.example.test/health";
fn command_over_box(width: usize, indent: usize) -> String {
    sw::agents::screen_text::agent_words(&format!("⏺ The command failed:\n\n{}{CURL}\n{}\n❯\n{}\n  Opus · 42% left\n  ⏵⏵ bypass permissions on", " ".repeat(indent), "─".repeat(width), "─".repeat(width)))
}

#[test]
fn an_indented_command_over_the_input_box_of_a_narrow_pane_stays_in_the_message() {
    let screen = format!("⏺ The command failed:\n\n{}{CURL}\n{}\n❯\n{}\n  Opus · 42% left\n  ⏵⏵ bypass permissions on", " ".repeat(16), "─".repeat(40), "─".repeat(40));
    let w = world_with(Opts { repo: true, settings: json!({ "review.when": "never" }), ..Default::default() });
    w.script(json!({ "worker": [{ "files": { "x.txt": "x\n" }, "note": null, "screen": screen }] }));
    let id = w.add("Narrow pane");
    delegate(&id);
    let t = settle(&id);
    assert_eq!(t.s("stage"), "nosummary");
    assert!(t.s("msg").contains(CURL), "{:?}", t.s("msg"));
}

#[test]
fn a_screen_with_no_input_box_is_kept_whole() {
    let words = sw::agents::screen_text::agent_words;
    assert_eq!(words("I ran out of context\n> "), sw::agents::screen_text::screen_text("I ran out of context\n> "));
    // a rule the agent wrote itself, far from the end, is not taken for a box
    let ruled = format!("Part one\n{}\nPart two\n{}\nline\nline\nline\nline\nline\nline\nline\nline\nline", "─".repeat(20), "─".repeat(20));
    assert!(words(&ruled).contains("Part two"), "{}", words(&ruled));
    // two rules the agent wrote itself near the end hold no prompt: kept
    let parts = format!("Intro\n{}\nDetails\n{}\nConclusion", "─".repeat(20), "─".repeat(20));
    assert!(words(&parts).contains("Conclusion"), "{}", words(&parts));
    // a prompt with no rules round it
    assert_eq!(words("Stopped\n❯ \n  ⏵⏵ bypass permissions on\n"), "Stopped");
    // an older Claude Code's input box, with edges, and a line typed into it
    assert_eq!(words("Done here.\n╭──────────────╮\n│ > half typed │\n╰──────────────╯\n  ? for shortcuts"), "Done here.");
    // the same box with ten lines typed into it
    let typed: String = (2..=10).map(|n| format!("│   line {n:<6} │\n")).collect();
    assert_eq!(words(&format!("Done here.\n╭──────────────╮\n│ > line 1     │\n{typed}╰──────────────╯\n  ? for shortcuts")), "Done here.");
    // the agent's own rule near the end, after more than eight lines under a rule and a quote of its own: kept
    let quoted = format!("Intro\n{}\n> a quote\n{}{}\nConclusion", "─".repeat(20), "line\n".repeat(9), "─".repeat(20));
    assert!(words(&quoted).contains("Conclusion"), "{}", words(&quoted));
    // what is not the line after a turn is kept
    assert_eq!(words("✻ Note for the reviewer\nStopped\n❯ \n"), "✻ Note for the reviewer\nStopped");
}
