//! The push of a task's branch: it replaces what origin holds only where that is exactly the commit the board pushed
//! last, or one the pushed commit holds; never a commit someone else pushed there, even after a fetch moved
//! `origin/<branch>` or put it in the branch's own history, and what it keeps as pushed is the commit it pushed
//! (upstream d63af260). A branch origin already has at its head is not pushed again, so the repo's pre-push hook does
//! not run for nothing (upstream #795, #901).
use crate::common;
use super::{first_on_path, script, PathBack};
use common::*;
use serde_json::json;
use std::path::Path;
use sw::actions::talk;
use sw::json::*;

fn pr_world() -> World { world_with(Opts { repo: true, origin: true, settings: json!({ "review.when": "never" }), ..Default::default() }) }
fn at_origin(w: &World, branch: &str) -> String { git(&w.base.join("origin.git"), &["rev-parse", &format!("refs/heads/{branch}")]) }
/// Someone else pushes a commit onto the task's branch at origin: a suggestion accepted on GitHub, another machine.
fn someone_pushes(w: &World, branch: &str) -> String {
    let other = w.base.join("other");
    git(&w.base, &["clone", "-q", "-b", branch, &w.base.join("origin.git").display().to_string(), &other.display().to_string()]);
    std::fs::write(other.join("theirs.txt"), "theirs\n").unwrap();
    git(&other, &["add", "-A"]); git(&other, &["commit", "-qm", "a suggestion"]); git(&other, &["push", "-q", "origin", branch]);
    git(&other, &["rev-parse", "HEAD"])
}
/// The repo's pre-push hook from now on: it notes each run, then does what `then` says.
fn pre_push(w: &World, then: &str) {
    use std::os::unix::fs::PermissionsExt;
    let hook = w.root.join(".git/hooks/pre-push");
    std::fs::write(&hook, format!("#!/bin/sh\necho ran >> \"{}\"\n{then}\n", w.base.join("hook-runs").display())).unwrap();
    std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o755)).unwrap();
}
fn hook_runs(w: &World) -> usize { read(&w.base.join("hook-runs")).lines().count() }
/// A git first on PATH that, once `armed` exists, has someone else push onto the task's branch and fetches that here
/// right after the board's next push returns, before the board reads anything more.
fn fetch_right_after_push(w: &World, branch: &str) -> PathBack {
    let real = std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()).map(|d| d.join("git")).find(|p| p.is_file()).unwrap();
    let other = w.base.join("other");
    git(&w.base, &["clone", "-q", "-b", branch, &w.base.join("origin.git").display().to_string(), &other.display().to_string()]);
    let (g, o, root, armed) = (real.display(), other.display(), w.root.display(), w.base.join("armed").display().to_string());
    script(&w.base.join("gitbin/git"), &format!("#!/bin/sh\n\"{g}\" \"$@\"; s=$?\nif [ \"$1\" = push ] && [ $s -eq 0 ] && [ -f \"{armed}\" ]; then\n  rm -f \"{armed}\"\n  cd \"{o}\" && \"{g}\" fetch -q origin && \"{g}\" reset -q --hard \"origin/{branch}\" && echo theirs > theirs.txt && \"{g}\" add -A && \"{g}\" commit -qm 'a suggestion' && \"{g}\" push -q origin \"{branch}\" && \"{g}\" -C \"{root}\" fetch -q origin\nfi\nexit $s\n"));
    first_on_path(&w.base.join("gitbin"))
}
/// The agent is asked for more and does `step`; its turn ends and the finish line runs again.
fn again(w: &World, id: &str, step: serde_json::Value) -> Obj {
    w.script(json!({ "worker": [step] }));
    talk::tell(id, "One more thing", false).unwrap();
    settle(id)
}

#[test]
fn a_commit_someone_else_pushed_to_the_branch_is_never_pushed_over_even_after_a_fetch_moved_origins_branch() {
    let w = pr_world();
    let id = to_approval(&w, "Leased");
    let branch = task(&id).t("branch");
    let theirs = someone_pushes(&w, &branch);
    // Any fetch moves `origin/<branch>` to their commit: a task started from origin, a merge, the agent's own.
    git(&w.root, &["fetch", "-q", "origin"]);
    // The agent changes more on its own branch, without their commit.
    let t = again(&w, &id, step(2));
    assert_eq!(at_origin(&w, &branch), theirs, "their commit is still the head of the branch at origin");
    let f = t.at("failure");
    assert_eq!((f.s("stage"), f.s("title")), ("push", "The branch at origin has commits this task does not have"), "{f}");
    assert!(f.s("detail").contains(&format!("At origin: {theirs}")), "{}", f.s("detail"));
    assert!(f.s("next").contains(&format!("git pull origin {branch}")), "{}", f.s("next"));
    assert_eq!(f.at("agentCanFix"), &json!(true));
    // The agent brings their commit in: then its work goes on top of it.
    let tree = task(&id).t("worktree");
    git(Path::new(&tree), &["pull", "-q", "--no-rebase", "--no-edit", "origin", &branch]);
    let t = again(&w, &id, json!({ "note": "Took their suggestion in." }));
    assert!(t.at("failure").is_null(), "{}", t.at("failure"));
    assert_eq!(at_origin(&w, &branch), git(Path::new(&tree), &["rev-parse", "HEAD"]));
    assert!(common::worktrees::git(&w.root, &["merge-base", "--is-ancestor", &theirs, &at_origin(&w, &branch)]).is_ok(), "their commit is kept");
}

#[test]
fn a_commit_someone_else_pushed_is_not_the_tasks_own_for_having_been_pulled_into_its_branch_and_dropped_again() {
    let w = pr_world();
    let id = to_approval(&w, "Pulled, dropped");
    let t = task(&id);
    let (branch, tree, pushed) = (t.t("branch"), t.t("worktree"), t.t("prHead"));
    let theirs = someone_pushes(&w, &branch);
    // The agent pulls their commit in, goes back to what the board pushed and rewrites that: their commit is in the
    // branch's own history (its reflog), and in nothing it would push.
    git(Path::new(&tree), &["pull", "-q", "--ff-only", "origin", &branch]);
    git(Path::new(&tree), &["reset", "-q", "--hard", &pushed]);
    git(Path::new(&tree), &["commit", "-q", "--amend", "-m", "v1, worded better"]);
    let t = again(&w, &id, json!({ "note": "Reworded it." }));
    assert_eq!(at_origin(&w, &branch), theirs, "their commit is still the head of the branch at origin");
    assert_eq!(t.at("failure").s("title"), "The branch at origin has commits this task does not have", "{}", t.at("failure"));
    assert_eq!(t.t("prHead"), pushed);
}

#[test]
fn what_the_board_keeps_as_pushed_is_the_commit_it_pushed_not_what_a_fetch_right_after_the_push_found() {
    let w = pr_world();
    let id = to_approval(&w, "Fetched after");
    let t = task(&id);
    let (branch, tree) = (t.t("branch"), t.t("worktree"));
    let _git = fetch_right_after_push(&w, &branch);
    std::fs::write(w.base.join("armed"), "").unwrap();
    let t = again(&w, &id, step(2));
    assert!(t.at("failure").is_null(), "{}", t.at("failure"));
    let (mine, theirs) = (git(Path::new(&tree), &["rev-parse", "HEAD"]), at_origin(&w, &branch));
    assert_ne!(theirs, mine, "someone pushed on top of it right after");
    assert_eq!(git(&w.root, &["rev-parse", &format!("refs/remotes/origin/{branch}")]), theirs, "and a fetch here found it");
    assert_eq!(t.t("prHead"), mine, "kept as pushed: the commit the board pushed");
    // The agent's next commit does not have theirs: nothing is pushed over it.
    let t = again(&w, &id, step(3));
    assert_eq!(at_origin(&w, &branch), theirs, "their commit is still the head of the branch at origin");
    assert_eq!(t.at("failure").s("title"), "The branch at origin has commits this task does not have", "{}", t.at("failure"));
}

#[test]
fn the_tasks_own_rewritten_branch_is_still_pushed_over_what_it_pushed_before() {
    let w = pr_world();
    let id = to_approval(&w, "Rebuilt");
    let t = task(&id);
    let (branch, tree) = (t.t("branch"), t.t("worktree"));
    let first = t.t("prHead");
    assert_eq!(at_origin(&w, &branch), first);
    assert_eq!(git(&w.root, &["config", "--get", &format!("branch.{branch}.merge")]), format!("refs/heads/{branch}"), "its upstream, as git push -u writes it");
    // The agent rewrites its own commit (a rebase, an amend): origin's head is not in the branch any more.
    git(Path::new(&tree), &["commit", "-q", "--amend", "-m", "v1, worded better"]);
    let t = again(&w, &id, json!({ "note": "Reworded it." }));
    assert!(t.at("failure").is_null(), "{}", t.at("failure"));
    let now = git(Path::new(&tree), &["rev-parse", "HEAD"]);
    assert_ne!(now, first);
    assert_eq!((at_origin(&w, &branch), t.t("prHead")), (now.clone(), now));
}

#[test]
fn a_branch_origin_already_has_at_its_head_is_not_pushed_again_through_the_repos_pre_push_hook() {
    let w = pr_world();
    let id = to_approval(&w, "Pushed already");
    let branch = task(&id).t("branch");
    // From now on the repo's pre-push hook fails, as one over its time budget does.
    pre_push(&w, "echo 'over the time budget' >&2\nexit 1");
    // Woken again, the agent changes nothing: the same head finishes again, and origin has it.
    let t = again(&w, &id, json!({ "note": "Nothing to change." }));
    assert!(t.at("failure").is_null(), "{}", t.at("failure"));
    assert_eq!(t.s("stage"), "approval");
    assert_eq!(hook_runs(&w), 0, "no push, so no hook");
    assert_eq!(t.t("prHead"), at_origin(&w, &branch));
    // The agent pushed its next commit itself: origin has the head, nothing is pushed.
    let tree = task(&id).t("worktree");
    std::fs::write(Path::new(&tree).join("g.txt"), "g\n").unwrap();
    git(Path::new(&tree), &["add", "-A"]); git(Path::new(&tree), &["commit", "-qm", "g"]); git(Path::new(&tree), &["push", "-q", "--no-verify", "origin", &branch]);
    let t = again(&w, &id, json!({ "note": "Pushed it myself." }));
    assert!(t.at("failure").is_null(), "{}", t.at("failure"));
    assert_eq!((hook_runs(&w), t.t("prHead")), (0, git(Path::new(&tree), &["rev-parse", "HEAD"])));
    // A commit origin lacks is pushed, through the hook, and what the hook said is the failure.
    let t = again(&w, &id, step(2));
    assert_eq!(hook_runs(&w), 1);
    let f = t.at("failure");
    assert_eq!((f.s("stage"), f.s("title")), ("push", "Could not push the branch"), "{f}");
    assert!(f.s("detail").contains("over the time budget"), "{}", f.s("detail"));
}

#[test]
fn work_the_agent_pushed_itself_and_then_built_on_is_pushed_on_top_of_it() {
    let w = pr_world();
    let id = to_approval(&w, "Own push");
    let t = task(&id);
    let (branch, tree) = (t.t("branch"), t.t("worktree"));
    // The agent pushes a commit itself (no fetch moved anything), then makes one more.
    std::fs::write(Path::new(&tree).join("g.txt"), "g\n").unwrap();
    git(Path::new(&tree), &["add", "-A"]); git(Path::new(&tree), &["commit", "-qm", "g"]); git(Path::new(&tree), &["push", "-q", "origin", &branch]);
    let t = again(&w, &id, step(2));
    assert!(t.at("failure").is_null(), "{}", t.at("failure"));
    assert_eq!(at_origin(&w, &branch), git(Path::new(&tree), &["rev-parse", "HEAD"]));
}
