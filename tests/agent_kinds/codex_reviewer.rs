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
    assert_eq!(handovers(&t).iter().map(handover_line).collect::<Vec<_>>(), vec!["claude took over the review from codex: logged out"]);
}

#[test]
fn the_attempts_picker_on_codex_also_starts_with_its_hooks_off() {
    let w = reviewed_by(json!({ "merge.by": "itself", "merge.pr": false }));
    w.patch(|s| s["hooksToReview"] = json!(["codex"]));
    w.script(json!({ "worker": [step(1), step(2)], "picker": [{ "comparison": { "pick": "b", "why": "B is the smallest change." } }] }));
    let id = w.add("Picked past a new hook");
    delegate_with(&id, json!({ "attempts.by": "claude, pi", "attempts.pick": "reviewer" }));
    let t = settle_all(&id);
    assert_eq!(t.at("picked").s("letter"), "B", "{} {} {}", t.s("stage"), t.at("comparison"), t.at("failure"));
    let starts = starts_of(&w, "codex");
    assert!(!starts.is_empty() && starts.iter().all(|c| c.contains(" -c features.hooks=false")), "{starts:?}");
}

#[test]
fn a_signed_out_codex_is_not_started_to_pick_and_the_comparison_says_to_sign_in() {
    let w = reviewed_by(json!({ "merge.by": "itself", "merge.pr": false }));
    codex_signed_out(&w);
    w.script(json!({ "worker": [step(1), step(2)], "picker": [{ "comparison": { "pick": "b", "why": "B is the smallest change." } }] }));
    let id = w.add("Pick while signed out");
    delegate_with(&id, json!({ "attempts.by": "claude, pi", "attempts.pick": "reviewer" }));
    let t = settle_all(&id);
    assert!(starts_of(&w, "codex").is_empty(), "{:?}", starts_of(&w, "codex"));
    assert!(t.at("picked").is_null(), "nothing was picked: {}", t.at("picked"));
    assert_eq!(t.at("comparison").s("error"), "codex is not signed in here (`codex login status` says so): run `codex login` in a terminal here, then pick again", "{} {}", t.s("stage"), t.at("comparison"));
}

#[test]
fn only_a_role_that_looks_is_started_with_its_hooks_off() {
    let _w = world();
    let args = |k: &str, no_hooks: bool| sw::agents::start::start_args(k, &sw::agents::start::StartOpts { no_hooks, ..Default::default() }).unwrap();
    assert!(args("codex", true).windows(2).any(|a| a == ["-c", "features.hooks=false"]), "{:?}", args("codex", true));
    // A worker keeps your hooks (herdr's own integration is one), and a program with no such switch starts as it is.
    assert!(!args("codex", false).iter().any(|a| a.contains("hooks")), "{:?}", args("codex", false));
    assert_eq!(args("claude", true), args("claude", false));
}

/// A `codex` first on PATH that answers as Codex 0.167.2 did here with no sign-in at all: `login status` says "Not
/// logged in" (exit 1) whatever its provider, and its app-server's `account/read` says whether that provider needs
/// one (`requiresOpenaiAuth`: false for a `model_provider` of your own with `requires_openai_auth = false`). Every
/// call is noted in `asked`. PATH is put back when it is dropped, also when the test fails.
struct FakeCodex { path: String, asked: std::path::PathBuf }
impl Drop for FakeCodex { fn drop(&mut self) { unsafe { std::env::set_var("PATH", &self.path) } } }
fn fake_codex(w: &World, needs_sign_in: bool) -> FakeCodex {
    let bin = w.base.join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    let asked = w.base.join("asked");
    let account = format!(r#"{{"id":1,"result":{{"account":null,"requiresOpenaiAuth":{needs_sign_in},"workspaceRouting":null}}}}"#);
    std::fs::write(bin.join("codex"), format!("#!/bin/sh\necho \"$*\" >> {}\nif [ \"$1 $2\" = 'login status' ]; then echo 'Not logged in'; exit 1; fi\nif [ \"$1\" = app-server ]; then\n  while read l; do case \"$l\" in *'\"account/read\"'*) printf '%s\\n' '{account}';; esac; done\n  exit 0\nfi\nexit 2\n", asked.display())).unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(bin.join("codex"), std::fs::Permissions::from_mode(0o755)).unwrap();
    let path = std::env::var("PATH").unwrap_or_default();
    unsafe { std::env::set_var("PATH", format!("{}:{path}", bin.display())) };
    // Nothing kept from before: the board asks the program.
    let f = sw::store::files::data_dir().join("signins.json");
    let mut all = sw::store::files::read_json(&f).unwrap_or(json!({}));
    all.as_object_mut().unwrap().remove("codex");
    sw::store::files::write_json(&f, &all).unwrap();
    FakeCodex { path, asked }
}

#[test]
fn real_programs_a_codex_on_a_provider_of_its_own_reviews_although_it_says_it_is_not_logged_in() {
    let w = reviewed_by(json!({}));
    let codex = fake_codex(&w, false);
    w.script(json!({ "worker": [step(1)], "reviewer": [pass()] }));
    let id = w.add("Own provider");
    delegate(&id);
    let t = settle(&id);
    assert_eq!(t.s("stage"), "approval", "{} {}", t.s("msg"), t.at("failure"));
    assert_eq!(starts_of(&w, "codex").len(), 1, "{:?}", starts_of(&w, "codex"));
    assert_eq!(std::fs::read_to_string(&codex.asked).unwrap_or_default(), "login status\napp-server\n", "its app-server says whether the sign-in is needed at all");
}

#[test]
fn real_programs_a_codex_that_needs_its_sign_in_and_has_none_is_not_started() {
    let w = reviewed_by(json!({}));
    let codex = fake_codex(&w, true);
    w.patch(|s| s["signedOut"] = json!(["codex"]));
    w.script(json!({ "worker": [step(1)], "reviewer": [pass()] }));
    let id = w.add("Needs a sign-in");
    delegate(&id);
    let t = settle(&id);
    assert_eq!(t.s("stage"), "failed", "{} {}", t.s("msg"), t.at("failure"));
    assert_eq!(t.at("failure").s("title"), "Needs a login", "{}", t.at("failure"));
    assert!(starts_of(&w, "codex").is_empty(), "{:?}", starts_of(&w, "codex"));
    assert!(std::fs::read_to_string(&codex.asked).unwrap_or_default().starts_with("login status\n"));
}
