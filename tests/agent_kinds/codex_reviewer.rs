//! A Codex reviewer and Codex's own start-up screens, which herdr reads as idle, so the board would type the brief into
//! them (upstream #798, #745): it starts with Codex's hooks off, so "Hooks need review" never holds it, and a program
//! that says it is signed out here is not started to review at all: the review goes to `reviewer.fallback`, or fails
//! as "Needs a login", saying what to run.
use crate::common;
use common::*;
use serde_json::{json, Value};
use sw::json::*;
use sw::store::handovers::{handover_line, handovers};

fn pass() -> Value { json!({ "verdict": { "meets": true, "findings": [], "text": "Looks right." } }) }
fn starts_of(w: &World, kind: &str) -> Vec<String> { calls(w).into_iter().filter(|c| c.starts_with("agent start") && c.contains(&format!("--kind {kind} "))).collect() }
/// Codex is signed out here: what `codex login status` said last (kept by the board in signins.json), and the program
/// itself, which comes up at its sign-in page.
fn codex_signed_out(w: &World) {
    let f = sw::store::files::data_dir().join("signins.json");
    let mut all = sw::store::files::read_json(&f).unwrap_or(json!({}));
    all["codex"] = json!({ "in": false, "until": i64::MAX });
    sw::store::files::write_json(&f, &all).unwrap();
    w.patch(|s| s["signedOut"] = json!(["codex"]));
}
/// The review's limit is short: a reviewer held by a screen of its own times out in a second instead of in 20 minutes.
fn reviewed_by(more: Value) -> World {
    let mut settings = json!({ "reviewer.agent": "codex", "reviewer.fallback": "none", "review.rounds": 0, "review.timeout": "1s" });
    for (k, v) in more.as_object().cloned().unwrap_or_default() { settings[k] = v; }
    world_with(Opts { repo: true, settings, ..Default::default() })
}

#[test]
fn a_codex_reviewer_starts_with_its_hooks_off_so_a_new_hook_never_holds_the_review() {
    let w = reviewed_by(json!({}));
    w.patch(|s| s["hooksToReview"] = json!(["codex"])); // a hook new or changed: Codex asks to review it before its prompt
    w.script(json!({ "worker": [step(1)], "reviewer": [pass()] }));
    let id = w.add("Past a new hook");
    delegate(&id);
    let t = settle(&id);
    assert_eq!(t.s("stage"), "approval", "{} {}", t.s("msg"), t.at("failure"));
    let starts = starts_of(&w, "codex");
    assert_eq!(starts.len(), 1, "{starts:?}");
    assert!(starts[0].contains(" -c features.hooks=false"), "{starts:?}");
}

#[test]
fn a_signed_out_codex_is_not_started_to_review_and_the_task_says_to_sign_in() {
    let w = reviewed_by(json!({}));
    codex_signed_out(&w);
    w.script(json!({ "worker": [step(1)], "reviewer": [pass()] }));
    let id = w.add("Signed out");
    delegate(&id);
    let t = settle(&id);
    assert_eq!(t.s("stage"), "failed", "{} {}", t.s("msg"), t.at("failure"));
    let f = t.at("failure");
    assert_eq!((f.s("title"), f.b("auth")), ("Needs a login", true), "{f}");
    assert_eq!(f.s("detail"), "Review failed: codex is not signed in here (`codex login status` says so): run `codex login` in a terminal here, then retry the review", "{f}");
    assert_eq!(t.s("msg"), "The reviewer needs a login");
    assert!(starts_of(&w, "codex").is_empty(), "a signed-out reviewer is never started: {:?}", starts_of(&w, "codex"));
    assert_eq!(t.i("reviewRound"), 0, "no review was done, so none is counted");
}

#[test]
fn a_signed_out_codex_hands_the_review_to_the_fallback_as_logged_out() {
    let w = reviewed_by(json!({ "reviewer.fallback": "claude" }));
    codex_signed_out(&w);
    w.script(json!({ "worker": [step(1)], "reviewer": [pass()] }));
    let id = w.add("Signed out, with a fallback");
    delegate(&id);
    let t = settle(&id);
    assert_eq!(t.s("stage"), "approval", "{} {}", t.s("msg"), t.at("failure"));
    assert!(starts_of(&w, "codex").is_empty(), "{:?}", starts_of(&w, "codex"));
    assert_eq!(starts_of(&w, "claude").len(), 2, "the worker, and claude reviewing in codex's place");
    assert_eq!(handovers(&t).iter().map(handover_line).collect::<Vec<_>>(), vec!["claude took over from codex: logged out"]);
}
