//! `merge.syncRemote`: a task that lands in your checkout (`merge.pr` off) keeps up with your checkout's branch as origin
//! has it, not with your checkout, which may be behind. Off (built in), your checkout as before; on, origin's commits
//! are merged in and `syncedTo` is origin's commit, your checkout is fast-forwarded to it only when the work lands, and
//! one with commits origin lacks is refused. Origin that cannot be fetched holds the task, with nothing half done.
use crate::common;
use common::*;
use serde_json::json;
use std::path::Path;
use sw::actions::{finish_line, talk};
use sw::json::*;
use sw::store::settings::change::set_value;
use sw::store::settings::lookup::{setting, Scope};
use sw::store::settings::show::show_text;
use sw::store::tasks::{update, Change};

fn holds(dir: &Path, sha: &str) -> bool { common::worktrees::git(dir, &["merge-base", "--is-ancestor", sha, "HEAD"]).is_ok() }
fn tree(t: &Obj) -> &Path { Path::new(t.s("worktree")) }
fn parents(dir: &Path) -> usize { git(dir, &["rev-list", "--parents", "-n1", "HEAD"]).split(' ').count() - 1 }
fn sync_jobs() -> usize { sw::runner::JOBS.with(|j| j.borrow().iter().filter(|(job, _, _)| job == "sync").count()) }
fn clear_jobs() { sw::runner::JOBS.with(|j| j.borrow_mut().clear()); }
fn housekeep(w: &World) { sw::housekeeping::housekeeping(&w.socket, sw::housekeeping::Force::Yes); }
/// Pull requests off: the work lands in your checkout. No review, so a finished turn goes straight to approval.
fn local(more: serde_json::Value) -> World {
    let mut s = json!({ "review.when": "never", "merge.pr": false });
    for (k, v) in more.as_object().unwrap() { s[k] = v.clone(); }
    world_with(Opts { repo: true, origin: true, settings: s, ..Default::default() })
}
/// A task whose agent stopped to ask, so origin can move before its work is finished.
fn asking(w: &World, title: &str) -> String {
    w.script(json!({ "worker": [{ "note": "Which one?" }, { "files": { "f.txt": "v1\n" }, "commit": "v1", "note": "Did v1." }] }));
    let id = w.add(title);
    delegate(&id);
    assert_eq!(settle(&id).s("stage"), "asks");
    id
}
/// Someone else pushes to main at origin, and your checkout does not pull it: what they pushed.
fn they_push(w: &World, file: &str, body: &str) -> String {
    let other = w.base.join("other");
    if !other.exists() { git(&w.base, &["clone", "-q", &w.base.join("origin.git").display().to_string(), &other.display().to_string()]); }
    std::fs::write(other.join(file), body).unwrap(); git(&other, &["add", "-A"]); git(&other, &["commit", "-qm", file]); git(&other, &["push", "-q", "origin", "main"]);
    git(&other, &["rev-parse", "HEAD"])
}
fn you_commit(w: &World, file: &str) -> String { std::fs::write(w.root.join(file), "yours\n").unwrap(); git(&w.root, &["add", "-A"]); git(&w.root, &["commit", "-qm", file]); git(&w.root, &["rev-parse", "HEAD"]) }

#[test]
fn off_by_default_a_switch_set_for_one_repo_and_shown_with_the_tasks_settings() {
    let w = world();
    let id = w.add("Any");
    assert_eq!(setting("merge.syncRemote", Some(&task(&id))), json!(false));
    let e = set_value("merge.syncRemote", &json!("maybe"), &Scope::repo("proj")).unwrap_err();
    assert!(e.msg.starts_with("merge.syncRemote is a switch: true or false"), "{}", e.msg);
    assert_eq!(sw::cli::main(vec!["set".into(), "merge.syncRemote".into(), "true".into(), "--repo".into(), "proj".into()]), 0);
    assert_eq!(setting("merge.syncRemote", Some(&task(&id))), json!(true));
    let shown = show_text(&Scope::task(&task(&id)));
    assert!(regex::Regex::new(r"merge\.syncRemote\s+true").unwrap().is_match(&shown), "{shown}");
}

#[test]
fn off_your_checkout_behind_origin_is_what_the_task_keeps_up_with_as_before() {
    let w = local(json!({}));
    let yours = git(&w.root, &["rev-parse", "HEAD"]);
    let id = asking(&w, "Off");
    let theirs = they_push(&w, "g.txt", "theirs\n");
    talk::tell(&id, "The first one.", false).unwrap();
    let t = settle(&id);
    assert_eq!(t.s("stage"), "approval", "{}", t.at("failure"));
    assert_eq!(t.s("syncedTo"), yours); assert_eq!(t.s("syncSeen"), format!("{yours} main"));
    assert!(!holds(tree(&t), &theirs), "origin's commit is not taken in"); assert_eq!(parents(tree(&t)), 1, "no merge commit");
    assert_eq!(git(&w.root, &["rev-parse", "HEAD"]), yours);
}

#[test]
fn on_for_a_repo_origins_commits_are_merged_in_and_your_checkout_is_brought_up_to_origin_only_when_the_work_lands() {
    let w = local(json!({}));
    let yours = git(&w.root, &["rev-parse", "HEAD"]);
    let id = asking(&w, "On");
    set_value("merge.syncRemote", &json!(true), &Scope::repo("proj")).unwrap(); // read at each sync
    let theirs = they_push(&w, "g.txt", "theirs\n");
    talk::tell(&id, "The first one.", false).unwrap();
    let t = settle(&id);
    assert_eq!(t.s("stage"), "approval", "{}", t.at("failure"));
    assert!(holds(tree(&t), &theirs)); assert_eq!(parents(tree(&t)), 2, "a merge commit from origin");
    assert_eq!(t.s("syncedTo"), theirs); assert_eq!(t.s("syncSeen"), format!("{theirs} main")); assert_eq!(t.s("syncBranch"), "main");
    assert!(t.s("stat").starts_with("1 file changed"), "its change is only its own: {}", t.s("stat"));
    assert_eq!((git(&w.root, &["rev-parse", "HEAD"]), git(&w.root, &["status", "--porcelain"])), (yours.clone(), String::new()), "your checkout is not touched");
    // origin moves again while it waits: the periodic fetch notices, and it is merged in
    let newer = they_push(&w, "h.txt", "newer\n");
    clear_jobs();
    sw::housekeeping::poll_prs();
    housekeep(&w);
    assert_eq!(sync_jobs(), 1);
    let t = settle(&id);
    assert_eq!(t.s("stage"), "approval", "{}", t.at("failure"));
    assert!(holds(tree(&t), &newer)); assert_eq!(t.s("syncedTo"), newer);
    assert_eq!(git(&w.root, &["rev-parse", "HEAD"]), yours);
    // after your yes: your checkout is fast-forwarded to origin's commit, and the work lands on it as one commit
    finish_line::approve(&id, None, false).unwrap();
    let t = settle(&id);
    assert_eq!(t.s("stage"), "done", "{}", t.at("failure"));
    assert_eq!(git(&w.root, &["rev-parse", "HEAD^"]), newer);
    assert_eq!(git(&w.root, &["diff", "--name-only", "HEAD^", "HEAD"]), "f.txt");
    assert_eq!(read(&w.root.join("f.txt")), "v1\n");
}

#[test]
fn on_origin_that_cannot_be_fetched_holds_the_task_with_nothing_merged_or_marked_and_it_goes_on_once_origin_answers() {
    let w = local(json!({ "merge.syncRemote": true }));
    let (bare, gone) = (w.base.join("origin.git").display().to_string(), w.base.join("gone.git").display().to_string());
    let yours = git(&w.root, &["rev-parse", "HEAD"]);
    let id = asking(&w, "Unreachable");
    let theirs = they_push(&w, "g.txt", "theirs\n");
    git(&w.root, &["remote", "set-url", "origin", &gone]);
    talk::tell(&id, "The first one.", false).unwrap();
    let t = settle(&id);
    // a clear failure: the task waits as behind main, with why and what git said, and cannot be approved
    assert_eq!(t.s("stage"), "approval", "{}", t.at("failure")); assert!(t.at("failure").is_null());
    assert!(t.at("syncBlocked").s("why").starts_with("main could not be fetched (origin is not a repository the board can read"), "{}", t.at("syncBlocked"));
    assert!(t.at("syncBlocked").s("detail").contains("What git said (git fetch origin main)"), "{}", t.at("syncBlocked"));
    assert!(t.s("msg").starts_with("Behind main"), "{}", t.s("msg"));
    assert!(finish_line::approve(&id, None, false).unwrap_err().msg.contains("behind main and cannot be approved yet"));
    // nothing half done: the branch is the agent's own commit, no merge under way, nothing taken as synced, and your
    // checkout not fallen back to
    assert_eq!(parents(tree(&t)), 1); assert!(!holds(tree(&t), &theirs));
    assert!(common::worktrees::git(tree(&t), &["rev-parse", "-q", "--verify", "MERGE_HEAD"]).is_err()); assert_eq!(git(tree(&t), &["status", "--porcelain"]), "");
    assert!(!t.b("syncedTo") && !t.b("syncSeen"), "{} {}", t.at("syncedTo"), t.at("syncSeen"));
    assert_eq!(git(&w.root, &["rev-parse", "HEAD"]), yours);
    // origin answers again: the board's own retry merges it in
    git(&w.root, &["remote", "set-url", "origin", &bare]);
    update(&id, |t| { t.put("syncRetryAt", sw::clock::now() - 1); Ok(Change::Write) }).unwrap();
    clear_jobs();
    housekeep(&w);
    assert_eq!(sync_jobs(), 1);
    let t = settle(&id);
    assert_eq!(t.s("stage"), "approval", "{}", t.at("failure")); assert!(t.at("syncBlocked").is_null(), "{}", t.at("syncBlocked"));
    assert!(holds(tree(&t), &theirs)); assert_eq!(t.s("syncedTo"), theirs);
}

#[test]
fn on_a_checkout_with_commits_origin_lacks_is_not_landed_in_and_turned_off_for_the_task_it_lands_there_as_before() {
    let w = local(json!({ "merge.syncRemote": true }));
    w.script(json!({ "worker": [{ "files": { "f.txt": "v1\n" }, "commit": "v1", "note": "Did v1." }] }));
    let id = w.add("Ahead");
    delegate(&id);
    assert_eq!(settle(&id).s("stage"), "approval");
    let mine = you_commit(&w, "mine.txt"); // not pushed
    finish_line::approve(&id, None, false).unwrap();
    let t = settle(&id);
    assert_eq!(t.s("stage"), "approval"); assert!(t.b("mergeFailed"));
    assert_eq!(t.at("failure").s("title"), "Your checkout has commits that origin/main does not have", "{}", t.at("failure"));
    assert!(t.at("failure").s("next").contains("set merge.syncRemote false --task"), "{}", t.at("failure"));
    assert_eq!((git(&w.root, &["rev-parse", "HEAD"]), git(&w.root, &["status", "--porcelain"])), (mine.clone(), String::new()), "your checkout is as it was");
    assert!(!w.root.join("f.txt").exists());
    assert_eq!(sw::cli::main(vec!["set".into(), "merge.syncRemote".into(), "false".into(), "--task".into(), id.clone()]), 0);
    finish_line::approve(&id, None, false).unwrap();
    let t = settle(&id);
    assert_eq!(t.s("stage"), "done", "{}", t.at("failure"));
    assert_eq!(git(&w.root, &["rev-parse", "HEAD^"]), mine);
    assert_eq!(read(&w.root.join("f.txt")), "v1\n");
}
