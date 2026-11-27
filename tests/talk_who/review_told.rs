//! What people told a task after its description reaches its reviewers (finish/told_later.rs): the work's brief and
//! the plan's carry each `tell` or reply, newest last and saying that it wins over the details; the board's own
//! messages never; the newest kept within the cap; and a task nobody told anything has the brief it always had.
use crate::common;
use common::*;
use serde_json::json;
use sw::finish::review_brief::{review_brief, Brief};
use sw::finish::told_later::{CHARS, HEAD, MOST};
use sw::json::*;
use sw::store::said_by::note_told;
use sw::store::settings::lookup::work;

const SPLIT: &str = "Split the change into three pull requests: A (the store), B (the brief) and C (the docs).";
fn brief(t: &Obj) -> String { review_brief(t, &Brief { text: "Did A.", change: None, to_land: false, ch: &work(Some(t)), earlier: None, check: "" }) }
fn plan_brief(t: &Obj) -> String { sw::plan::review::plan_brief(t, std::path::Path::new("/nowhere")) }
/// The section, as the brief has it: from its first line to the next blank line.
fn section(b: &str) -> String { b.split("\n\n").find(|p| p.starts_with(HEAD)).unwrap_or("").to_string() }
/// A task described with A, B and C, its agent at work.
fn split_task(w: &World) -> String {
    w.script(json!({ "worker": [{ "status": "working" }, { "status": "working" }, { "status": "working" }, { "status": "working" }] }));
    let id = sw::actions::filing::add(&w.root, "Split the change", sw::actions::filing::AddOpts { description: SPLIT.into(), ..Default::default() }).unwrap().t("id");
    delegate(&id); settle(&id);
    id
}

#[test]
fn a_task_told_to_keep_only_part_a_has_that_in_its_review_brief_and_the_brief_says_it_wins() {
    let w = world_with(Opts { repo: true, settings: json!({ "review.when": "never" }), ..Default::default() });
    let id = split_task(&w);
    let before = brief(&task(&id));
    assert!(!before.contains(HEAD), "{before}");
    sw::actions::talk::reply(&id, "Keep only A in this one.\nB and C come later, in tasks of their own.", "you").unwrap(); settle(&id);
    sw::actions::talk::reply(&id, "And name the store file after what it keeps.", "coordinator").unwrap(); settle(&id);
    let t = task(&id);
    for b in [brief(&t), plan_brief(&t)] {
        // after the details, in the order said, under who said it, multi-line words kept together
        let s = section(&b);
        assert!(b.find(&format!("Details:\n{SPLIT}")).unwrap() < b.find(HEAD).unwrap(), "{b}");
        assert!(s.contains("- the human (") && s.contains("): Keep only A in this one.\n  B and C come later, in tasks of their own."), "{s}");
        assert!(s.find("Keep only A").unwrap() < s.find("- the coordinator (").unwrap(), "{s}");
        assert!(HEAD.contains("newest last") && HEAD.contains("where they differ from the details, they win"));
    }
}

#[test]
fn the_boards_own_messages_never_appear_among_the_later_instructions() {
    let w = world_with(Opts { repo: true, settings: json!({ "review.when": "never" }), ..Default::default() });
    let id = split_task(&w);
    // the board, on its own: a failed check sent back, a carry on after a stop, a retry
    sw::flow::send::send_own(&id, "The check failed: fix it.", "check", false, Obj::new(), 0).unwrap(); settle(&id);
    sw::actions::talk::tell(&id, "Continue the task where you left off.", false).unwrap(); settle(&id);
    assert!(!brief(&task(&id)).contains(HEAD), "{}", brief(&task(&id)));
    sw::actions::talk::reply(&id, "Keep only A.", "you").unwrap(); settle(&id);
    sw::flow::send::send_own(&id, "The review found faults: fix them.", "review", false, Obj::new(), 0).unwrap(); settle(&id);
    let s = section(&brief(&task(&id)));
    assert!(s.contains("Keep only A."), "{s}");
    for board in ["check failed", "left off", "review found"] { assert!(!s.contains(board), "{board} in {s}"); }
    // nor anything noted as the board's
    note_told(&id, "board", "The board's own words.");
    assert!(!section(&brief(&task(&id))).contains("board's own"));
}

#[test]
fn past_the_cap_the_newest_are_kept() {
    let w = world_with(Opts { repo: true, ..Default::default() });
    let id = w.add("Make f");
    // more of them than are read: the oldest go, and the brief says how many
    for n in 1..=MOST + 3 { note_told(&id, "you", &format!("Instruction {n:02}.")); }
    let s = section(&brief(&task(&id)));
    assert!(s.contains("(3 older instructions are left out.)"), "{s}");
    for n in 1..=3 { assert!(!s.contains(&format!("Instruction {n:02}.")), "{s}"); }
    let kept: Vec<usize> = (4..=MOST + 3).map(|n| s.find(&format!("Instruction {n:02}.")).unwrap_or_else(|| panic!("{n} not in {s}"))).collect();
    assert!(kept.windows(2).all(|p| p[0] < p[1]), "newest last: {s}");
    // more words than are read: the newest whole, the older ones that do not fit left out
    let id = w.add("Make g");
    note_told(&id, "you", &format!("OLD {}", "o".repeat(CHARS / 2)));
    note_told(&id, "you", &format!("MID {}", "m".repeat(CHARS / 2)));
    note_told(&id, "you", "NEW keep only A.");
    let s = section(&brief(&task(&id)));
    assert!(s.contains("NEW keep only A.") && s.contains("MID ") && !s.contains("OLD "), "{s}");
    assert!(s.contains("(One older instruction is left out.)"), "{s}");
    // one alone longer than the cap: its start, cut short
    let id = w.add("Make h");
    note_told(&id, "you", &"x".repeat(CHARS * 2));
    let s = section(&brief(&task(&id)));
    assert!(s.contains(&format!("{} …", "x".repeat(CHARS))) && !s.contains(&"x".repeat(CHARS + 1)), "{} chars", s.len());
}

#[test]
fn a_task_nobody_told_anything_has_the_brief_it_always_had() {
    let w = world_with(Opts { repo: true, ..Default::default() });
    let id = sw::actions::filing::add(&w.root, "Split the change", sw::actions::filing::AddOpts { description: SPLIT.into(), ..Default::default() }).unwrap().t("id");
    let t = task(&id);
    for b in [brief(&t), plan_brief(&t)] {
        assert!(!b.contains("Later instructions"), "{b}");
        let after = &b[b.find(&format!("Details:\n{SPLIT}")).unwrap() + "Details:\n".len() + SPLIT.len()..];
        assert!(after.starts_with("\n\nWhat the agent says it did:") || after.starts_with("\n\nFirst answer one fixed question"), "{after}");
    }
}
