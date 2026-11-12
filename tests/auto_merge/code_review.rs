//! `merge.by none`, a repo whose work lands through code review on GitHub: a task whose review passed waits as "Review
//! passed: ready for code review" on its row, in its message (`list`, `details`) and on the web board; approve is
//! refused with the reason, `--anyway` too, and the yes key says it at once; a failed review and no review say what
//! they are; turned on or off while a task waits, it says so again, and a merge you approved before is called off; a
//! wrong value is refused; off, all is as it was.
use crate::common;
use common::*;
use common::board::web;
use serde_json::{json, Value};
use sw::actions::finish_line;
use sw::board::keys::key_name;
use sw::json::*;
use sw::store::settings::change::{set_value, unset_value};
use sw::store::settings::lookup::{lookup, Scope};
use sw::store::settings::places::yours;

const LABEL: &str = "Review passed: ready for code review";
const WHY: &str = "this repo is landed through code review on GitHub, not by the board (merge.by none)";
fn has(text: &str, re: &str) -> bool { regex::Regex::new(re).unwrap().is_match(text) }
fn verdict(severity: &str) -> Value { json!({ "verdict": { "meets": true, "text": "Looked at it.", "findings": [{ "severity": severity, "subject": "A name", "where": "f.txt", "proof": "read it" }] } }) }
fn merged(w: &World) -> bool { w.root.join("f.txt").exists() }
fn says(id: &str) -> String { sw::shown::row_says::sentence(&sw::shown::states::as_row(&row(id))).text() }
fn pass(w: &World) { sw::housekeeping::housekeeping(&w.socket, sw::housekeeping::Force::Yes); }
fn refused(id: &str) -> String { format!("{id} was not merged: {WHY}.") }
fn web_row(w: &World, id: &str) -> Value { web(w, "board", json!({})).at("list").arr("groups").iter().flat_map(|g| g.arr("rows").iter().cloned()).find(|x| x.s("id") == id).unwrap() }

#[test]
fn merge_by_none_for_a_repo_a_passed_review_waits_ready_for_code_review_and_approve_is_refused_anyway_too() {
    let w = world();
    let id = w.add("Greeting");
    assert_eq!(set_value("merge.by", &json!("none"), &Scope::repo("proj")).unwrap().value, json!("none"));
    assert_eq!(set_value("merge.by", &json!("nobody"), &Scope::repo("proj")).unwrap_err().msg, "merge.by must be you, itself, none, not \"nobody\".");
    w.script(json!({ "worker": [{ "files": { "f.txt": "v1\n" }, "commit": "v1", "note": "Did v1." }], "reviewer": [verdict("minor")] }));
    delegate(&id);
    assert_eq!(settle(&id).s("stage"), "approval");
    let f = lookup("merge.by", &Scope::task(&task(&id)), &yours()).unwrap();
    assert_eq!((f.value, f.from), (json!("none"), "your entry for proj".to_string()));
    assert!(has(&sw::store::settings::show::show_text(&Scope::task(&task(&id))), r"merge\.by +none +your entry for proj"));
    // the label: its message (what `list` and `details` say, the reason it waits for you), its row
    let msg = format!("{LABEL} · 1 minor. Did v1.");
    assert_eq!(task(&id).t("msg"), msg);
    let r = row(&id);
    assert_eq!((r.t("note"), r.t("state"), r.t("kind"), r.b("codeReview")), (msg.clone(), "blocked".into(), "permission".into(), true), "`list` prints the row's note");
    assert_eq!(finish_line::details(&id).unwrap().at("task").t("msg"), msg);
    assert_eq!(says(&id), format!("{LABEL} · 1 minor"));
    // approve is refused with the reason, --anyway too, from the command line too, and a retry is one; nothing lands
    for anyway in [false, true] { assert_eq!(finish_line::approve(&id, None, anyway).unwrap_err().msg, refused(&id)); }
    assert_eq!(sw::cli::main(vec!["approve".into(), id.clone(), "--anyway".into()]), 1);
    assert_eq!(finish_line::retry(&id).unwrap_err().msg, refused(&id));
    let t = settle(&id);
    assert_eq!((t.t("stage"), t.at("mergeIntent").clone(), t.t("msg")), ("approval".into(), Value::Null, msg.clone())); assert!(!merged(&w));
    // the board offers no yes and its panel says how it lands; the yes key says why at once, asking nothing
    let mut b = common::board::open(&w.root, 40);
    b.s.sel = id.clone(); b.load();
    let text = b.text();
    assert!(has(&text, &format!(r"\n[ ▌]          {} · 1 minor ", regex::escape(LABEL))), "{text}");
    assert!(text.contains("send the review to this task's agent") && !text.contains("approve and merge"), "{text}");
    assert!(text.contains("Merge        by code review on GitHub"), "{text}");
    b.press(&[&key_name("yes")]);
    assert_eq!(b.s.msg, refused("PRJ-1")); assert!(b.s.ask.is_none());
    // the web board: its row says it, and it has no yes button
    assert_eq!(web_row(&w, &id).arr("says").iter().map(|p| p.t("t")).collect::<String>(), format!("{LABEL} · 1 minor"));
    let page = web(&w, "pick", json!({ "id": id, "open": true }));
    assert!(!page.arr("actions").iter().any(|a| a.s("id") == "yes") && page.arr("actions").iter().any(|a| a.s("id") == "sendBack"), "{}", page.at("actions"));
    let t = settle(&id); assert_eq!(t.s("stage"), "approval"); assert!(!merged(&w));
    // a coordinator on autopilot that may say yes has none to say here
    assert_eq!(sw::autopilot::care::need_of(&task(&id), true), None);
}

#[test]
fn merge_by_you_is_as_it_was_approve_to_merge_and_approve_merges() {
    let w = world();
    w.script(json!({ "reviewer": [verdict("minor")] }));
    let id = to_approval(&w, "Greeting");
    let msg = "Approve to merge? Review: Pass · 1 minor. Did v1.";
    assert_eq!((task(&id).t("msg"), row(&id).t("note"), row(&id).b("codeReview")), (msg.into(), msg.into(), false));
    assert_eq!(finish_line::details(&id).unwrap().at("task").t("msg"), msg);
    assert_eq!(says(&id), "Pass · 1 minor");
    assert_eq!(sw::autopilot::care::need_of(&task(&id), true), Some("yes"));
    let mut b = common::board::open(&w.root, 40);
    b.s.sel = id.clone(); b.load();
    let text = b.text();
    assert!(text.contains(&format!("{} approve and merge this task", key_name("yes"))) && text.contains("Merge        after your yes · squash") && !text.contains("code review"), "{text}");
    finish_line::approve(&id, None, false).unwrap();
    let t = settle(&id);
    assert_eq!(t.s("stage"), "done", "{}", t.at("failure")); assert!(merged(&w));
}

#[test]
fn merge_by_none_turned_on_while_a_task_waits_says_it_at_the_next_look_and_turned_off_asks_again() {
    let w = world();
    w.script(json!({ "reviewer": [verdict("minor")] }));
    let id = to_approval(&w, "Greeting");
    set_value("merge.by", &json!("none"), &Scope::task(&task(&id))).unwrap();
    assert!(row(&id).b("codeReview"), "its row says it at once");
    pass(&w);
    assert_eq!(task(&id).t("msg"), format!("{LABEL} · 1 minor. Did v1."));
    assert_eq!(finish_line::approve(&id, None, true).unwrap_err().msg, refused(&id));
    unset_value("merge.by", &Scope::task(&task(&id))).unwrap();
    pass(&w);
    assert_eq!((task(&id).t("msg"), row(&id).b("codeReview")), ("Approve to merge? Review: Pass · 1 minor. Did v1.".into(), false));
    finish_line::approve(&id, None, false).unwrap();
    assert_eq!(settle(&id).s("stage"), "done"); assert!(merged(&w));
}

#[test]
fn merge_by_none_a_failed_review_offers_no_merge_anyway_and_no_review_is_only_ready_for_code_review() {
    let w = world_with(Opts { repo: true, settings: json!({ "merge.by": "none", "review.rounds": 0 }), ..Default::default() });
    w.script(json!({ "reviewer": [verdict("major")] }));
    let id = to_approval(&w, "Fails");
    assert_eq!(task(&id).t("msg"), "Review: Fail · 1 major. Did v1.");
    assert_eq!(says(&id), "Review failed · 1 major");
    assert_eq!(finish_line::approve(&id, None, true).unwrap_err().msg, refused(&id));
    let mut b = common::board::open(&w.root, 40);
    b.s.sel = id.clone(); b.load();
    let text = b.text();
    assert!(text.contains("It found 1 major.") && !text.contains("anyway"), "{text}");
    // not reviewed at all: ready for code review, and no more
    set_value("review.when", &json!("never"), &Scope::you()).unwrap();
    w.script(json!({ "worker": [{ "files": { "g.txt": "v1\n" }, "commit": "v1", "note": "Did g." }] }));
    let quiet = w.add("Unreviewed");
    delegate(&quiet);
    let t = settle(&quiet);
    assert_eq!((t.t("stage"), t.t("msg")), ("approval".into(), "Ready for code review. Did g.".into()), "{}", t.at("failure"));
    assert_eq!(says(&quiet), "Ready for code review");
    // the label is the board's own review words: a screen that shows the review elsewhere takes it off
    assert_eq!(sw::gate::strip_sentence(&format!("{LABEL} · 2 minor. Did it.")), "Did it.");
    assert_eq!(sw::shown::review_words::review_note(&obj(json!({ "agent": "claude", "stage": "approval", "msg": format!("{LABEL} · 2 minor. Did it.") }))), "Ready for code review. Did it.");
}

#[test]
fn merge_by_none_set_after_your_yes_calls_off_the_merge_still_to_come() {
    let w = world();
    w.script(json!({ "reviewer": [verdict("minor")] }));
    let id = to_approval(&w, "Approved first");
    finish_line::approve(&id, None, false).unwrap();
    assert_eq!(task(&id).s("stage"), "merging", "its land runner is queued, not run");
    set_value("merge.by", &json!("none"), &Scope::repo("proj")).unwrap();
    let t = settle(&id);
    assert_eq!((t.t("stage"), t.at("mergeIntent").clone(), t.t("msg")), ("approval".into(), Value::Null, format!("{LABEL} · 1 minor. Did v1.")), "{}", t.at("failure"));
    assert!(!merged(&w), "your yes came before merge.by none: nothing lands all the same");
    pass(&w); assert_eq!(settle(&id).s("stage"), "approval"); assert!(!merged(&w));
}

#[test]
fn merge_by_none_set_while_your_approved_merge_waits_for_githubs_checks_calls_it_off() {
    let w = world_with(Opts { repo: true, origin: true, settings: json!({ "review.when": "never" }), ..Default::default() });
    w.gh_says(json!({ "mergeFail": "checks pending", "view": { "state": "OPEN", "statusCheckRollup": [{ "name": "build", "state": "PENDING" }], "comments": [], "reviews": [], "author": { "login": "me" } } }));
    w.script(json!({ "worker": [{ "files": { "f.txt": "v1\n" }, "commit": "v1", "note": "Did v1." }] }));
    let id = w.add("With a PR"); delegate(&id); assert_eq!(settle(&id).s("stage"), "approval");
    finish_line::approve(&id, None, false).unwrap();
    let t = settle(&id);
    assert_eq!((t.t("stage"), t.b("autoMerge"), t.at("mergeIntent").b("auto")), ("merging".into(), true, false), "{}", t.at("failure"));
    set_value("merge.by", &json!("none"), &Scope::task(&task(&id))).unwrap();
    pass(&w);
    let t = settle(&id);
    assert_eq!((t.t("stage"), t.b("autoMerge"), t.at("mergeIntent").clone(), t.t("msg")), ("approval".into(), false, Value::Null, "Ready for code review. Did v1.".into()));
    assert_eq!(gh_log(&w).lines().filter(|l| l.starts_with("pr merge ") && !l.contains("--disable-auto")).count(), 0); assert!(!merged(&w));
}

#[test]
fn merge_by_none_takes_your_approved_pull_request_out_of_githubs_merge_queue_and_says_so_while_it_cannot() {
    let w = world_with(Opts { repo: true, origin: true, settings: json!({ "review.when": "never" }), ..Default::default() });
    w.gh_says(json!({ "queue": true, "dequeueFail": "GraphQL: Could not dequeue the pull request" }));
    w.script(json!({ "worker": [{ "files": { "f.txt": "v1\n" }, "commit": "v1", "note": "Did v1." }] }));
    let id = w.add("Queued on GitHub"); delegate(&id); assert_eq!(settle(&id).s("stage"), "approval");
    finish_line::approve(&id, None, false).unwrap();
    let t = settle(&id);
    assert_eq!((t.t("stage"), t.b("autoMerge"), t.b("autoMergeLocal")), ("merging".into(), true, false), "lined up in GitHub's merge queue: {}", t.t("msg"));
    set_value("merge.by", &json!("none"), &Scope::task(&task(&id))).unwrap();
    // switching GitHub's auto-merge off leaves a queued one where it is: it is taken out, and while GitHub refuses that,
    // the task stays as it is and says so, with nothing claimed
    pass(&w);
    let t = settle(&id);
    assert_eq!((t.t("stage"), t.b("autoMerge"), t.at("failure").t("title")), ("merging".into(), true, "Still in GitHub's merge queue".into()));
    assert!(t.at("failure").s("detail").contains("Could not dequeue the pull request"), "{}", t.at("failure"));
    let log = gh_log(&w);
    assert!(log.contains("pr merge 7 --disable-auto") && log.contains("dequeuePullRequest"), "{log}");
    // the queue is asked of GitHub's GraphQL API: `gh pr view --json` has no field for it, and refuses one
    assert!(log.lines().any(|l| l.starts_with("api graphql -f owner=o -f name=r -F number=7 ") && l.contains("isInMergeQueue")) && !log.lines().any(|l| l.starts_with("pr view") && l.contains("isInMergeQueue")), "{log}");
    // GitHub takes it out: the task waits for code review, its failure gone
    w.gh_says(json!({ "queue": true }));
    pass(&w);
    let t = settle(&id);
    assert_eq!((t.t("stage"), t.b("autoMerge"), t.at("mergeIntent").clone(), t.at("failure").clone(), t.t("msg")), ("approval".into(), false, Value::Null, Value::Null, "Ready for code review. Did v1.".into()));
    assert!(!merged(&w));
}
