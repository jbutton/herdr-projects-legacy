//! A task started from origin's main (`workspace.from main`, the default): that branch is fetched on every start, so a
//! commit merged at origin since the last start is where the next task starts (upstream #574); origin out of reach
//! starts it from the copy fetched last, and the task's log says so.
use crate::common;
use super::{started, they_push, tree};
use common::*;
use serde_json::json;
use sw::json::*;
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
    assert!(log.contains("could not fetch main from origin") && log.contains(&format!("origin/main as last fetched here ({})", &last[..7])), "{log}");
    assert!(log.contains("git remote -v") && log.contains("What git said (git fetch origin main)"), "{log}");
}
