//! The range a task's change is read over (what the reviewer is shown): from the newer of main as last merged in and
//! the task's own start, so main's history from before the task began is never counted as its change.
use crate::common;
use common::*;
use serde_json::json;
use std::path::Path;
use sw::flow::git::change_of;
use sw::json::*;

fn commit(dir: &Path, file: &str) -> String { std::fs::write(dir.join(file), format!("{file}\n")).unwrap(); git(dir, &["add", "-A"]); git(dir, &["commit", "-qm", file]); git(dir, &["rev-parse", "HEAD"]) }
/// A worktree on a branch of its own, started at the checkout's HEAD.
fn tree_at_head(w: &World) -> std::path::PathBuf { let tree = w.base.join("tree"); git(&w.root, &["worktree", "add", "-q", "-b", "work/x", &tree.display().to_string()]); tree }
fn a_task(tree: &Path, base: &str, synced: &str) -> Obj { obj(json!({ "worktree": tree.display().to_string(), "baseSha": base, "syncedTo": synced })) }

/// A sync against a checkout that was behind saves an old commit as `syncedTo`: the task started after it.
#[test]
fn main_merged_in_from_before_the_task_started_does_not_widen_its_change() {
    let w = world();
    let old = git(&w.root, &["rev-parse", "HEAD"]);
    let base = commit(&w.root, "main-before.txt");
    let tree = tree_at_head(&w);
    commit(&tree, "mine.txt");
    let ch = change_of(&a_task(&tree, &base, &old)).unwrap();
    assert_eq!(ch.s("range"), format!("{base}...HEAD"));
    assert!(ch.s("stat").starts_with("1 file changed"), "{}", ch.s("diff"));
    assert!(ch.s("diff").contains("mine.txt") && !ch.s("diff").contains("main-before.txt"), "{}", ch.s("diff"));
}

#[test]
fn main_really_merged_in_after_the_task_started_is_where_its_change_starts() {
    let w = world();
    let base = git(&w.root, &["rev-parse", "HEAD"]);
    let tree = tree_at_head(&w);
    commit(&tree, "mine.txt");
    let newer = commit(&w.root, "main-after.txt");
    git(&tree, &["merge", "-q", "--no-edit", &newer]);
    let ch = change_of(&a_task(&tree, &base, &newer)).unwrap();
    assert_eq!(ch.s("range"), format!("{newer}...HEAD"));
    assert!(ch.s("stat").starts_with("1 file changed"), "{}", ch.s("diff"));
    assert!(ch.s("diff").contains("mine.txt") && !ch.s("diff").contains("main-after.txt"), "{}", ch.s("diff"));
}
