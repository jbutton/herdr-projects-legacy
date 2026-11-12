//! A task started from origin's main (`workspace.from main`, the default): that branch is fetched on every start, so a
//! commit merged at origin since the last start is where the next task starts (upstream #574); origin out of reach
//! starts it from the copy fetched last, and the task's log says so, as it does for a named branch. A start waits for
//! origin only so long, all told: finding its main branch, another fetch of the repo and the fetch itself.
use crate::common;
use super::{from, started, they_push, tree};
use common::*;
use serde_json::json;
use sw::json::*;
use std::os::unix::fs::PermissionsExt;
use std::sync::mpsc::channel;
use sw::store::files::dir_of;

fn main_world() -> World { world_with(Opts { repo: true, origin: true, settings: json!({ "review.when": "never" }), ..Default::default() }) }
fn log_of(t: &Obj) -> String { read(&dir_of(t.s("id")).join("runner.log")) }

#[test]
fn origins_main_is_fetched_on_every_start_not_once_a_day() {
    let w = main_world();
    let t = started(&w, "First");
    assert_eq!(t.s("baseSha"), git(&w.root, &["rev-parse", "main"]), "{}", t.at("failure"));
    // merged at origin since (a pull request merged on GitHub): the next start takes it, though the repo was just fetched
    let m2 = they_push(&w, "main", "m.txt", "merged\n");
    let u = started(&w, "Second");
    assert_eq!((u.s("stage"), u.s("baseSha")), ("working", m2.as_str()), "{}", u.at("failure"));
    assert_eq!(git(tree(&u), &["rev-parse", "HEAD"]), m2);
    assert_ne!(git(&w.root, &["rev-parse", "main"]), m2, "your checkout stays where it was");
    assert!(!log_of(&u).contains("could not fetch"), "{}", log_of(&u));
}

#[test]
fn origin_out_of_reach_starts_from_the_copy_fetched_last_and_the_log_says_so() {
    let w = main_world();
    let last = git(&w.root, &["rev-parse", "origin/main"]);
    git(&w.root, &["remote", "set-url", "origin", &w.base.join("gone.git").display().to_string()]);
    let t = started(&w, "Offline");
    assert_eq!((t.s("stage"), t.s("baseSha")), ("working", last.as_str()), "{}", t.at("failure"));
    let log = log_of(&t);
    assert!(log.contains("could not fetch main from origin") && log.contains(&format!("origin/main as last fetched here ({})", &last[..12])), "{log}");
    assert!(log.contains("git remote -v") && log.contains("What git said (git fetch origin main)"), "{log}");
    // a named branch fetched before: its last copy, said the same way
    git(&w.root, &["remote", "set-url", "origin", &w.base.join("origin.git").display().to_string()]);
    let d1 = they_push(&w, "develop", "d.txt", "one\n");
    from("develop");
    assert_eq!(started(&w, "On develop").s("baseSha"), d1);
    git(&w.root, &["remote", "set-url", "origin", &w.base.join("gone.git").display().to_string()]);
    let u = started(&w, "Offline on develop");
    assert_eq!((u.s("stage"), u.s("from"), u.s("baseSha")), ("working", "origin/develop", d1.as_str()), "{}", u.at("failure"));
    assert!(log_of(&u).contains(&format!("could not fetch develop from origin, so the task starts from origin/develop as last fetched here ({})", &d1[..12])), "{}", log_of(&u));
}

/// The time a start waits for origin, shorter for one test: put back when the test ends, passed or not.
struct Budget;
impl Budget { fn ms(ms: &str) -> Budget { unsafe { std::env::set_var("HERDR_PROJECTS_START_FETCH_MS", ms) }; Budget } }
impl Drop for Budget { fn drop(&mut self) { unsafe { std::env::remove_var("HERDR_PROJECTS_START_FETCH_MS") } } }

#[test]
fn a_start_waits_for_origin_only_so_long_all_told() {
    let w = main_world();
    let _short = Budget::ms("300");
    let last = git(&w.root, &["rev-parse", "origin/main"]);
    they_push(&w, "main", "m.txt", "merged\n");
    // origin answers after 5 seconds, and its main branch was never named here (no origin/HEAD): asking counts too
    let slow = w.base.join("slow-upload-pack");
    std::fs::write(&slow, "#!/bin/sh\nsleep 5\nexec git-upload-pack \"$@\"\n").unwrap();
    std::fs::set_permissions(&slow, std::fs::Permissions::from_mode(0o755)).unwrap();
    git(&w.root, &["config", "remote.origin.uploadpack", &slow.display().to_string()]);
    let t = started(&w, "Slow origin");
    assert_eq!((t.s("stage"), t.s("baseSha")), ("working", last.as_str()), "{}", t.at("failure"));
    assert!(log_of(&t).contains("could not fetch main from origin") && log_of(&t).contains("origin did not answer within 300 ms"), "{}", log_of(&t));
    // another fetch of the repo is running: waited for only as long
    git(&w.root, &["config", "--unset", "remote.origin.uploadpack"]);
    let ((held, is_held), (go, wait)) = (channel(), channel::<()>());
    let root = w.root.clone();
    let other = std::thread::spawn(move || sw::store::fetch::with_repo_lock(&root, || { held.send(()).unwrap(); let _ = wait.recv(); Ok(()) }));
    is_held.recv().unwrap();
    let u = started(&w, "Busy repo");
    go.send(()).unwrap();
    other.join().unwrap().unwrap();
    assert_eq!((u.s("stage"), u.s("baseSha")), ("working", last.as_str()), "{}", u.at("failure"));
    assert!(log_of(&u).contains("another fetch of this repo was still running after the 300 ms a start waits for origin"), "{}", log_of(&u));
}
