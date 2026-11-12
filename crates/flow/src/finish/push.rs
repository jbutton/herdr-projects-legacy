//! The push of a task's branch to origin, for its pull request. It replaces what origin holds there only where that is
//! the task's own (the head the board pushed last, a commit the branch holds, or one it was at before its own rebase),
//! never a commit someone else pushed there, whatever a fetch did to `origin/<branch>` meanwhile (upstream d63af260). A
//! head origin has already is not pushed again, so the repo's pre-push hook does not run for nothing (upstream #795). The
//! push runs with the repo's own tools (tool_env.rs), for as long as `merge.pushTimeout` allows.
use super::shared::failure;
use crate::err::{Error, Res};
use crate::json::*;
use crate::store::repos::{git_out, git_slow, git_slow_env, TIMED_OUT};
use crate::tool_env::tool_env;
use std::path::Path;

/// What a push that would lose someone else's commits fails with: origin's head of the branch follows.
pub const NOT_OURS: &str = "notours:";
/// How long asking origin where the branch stands may take.
const ASK_MS: u64 = 60_000;

/// Where the branch stands at origin now, asked where the push goes (origin's push URL, which may not be the one it is
/// fetched from), never `origin/<branch>`, which is wherever the last fetch left it: "" when there is no such branch.
pub fn at_origin(tp: &Path, branch: &str) -> Res<String> {
    let full = format!("refs/heads/{branch}");
    let url = git_out(tp, &["remote", "get-url", "--push", "origin"])?;
    let out = git_slow(tp, &["ls-remote", &url, &full], ASK_MS)?;
    Ok(out.lines().filter_map(|l| l.split_once('\t')).find(|(_, r)| r.trim() == full).map(|(h, _)| h.trim().to_string()).unwrap_or_default())
}
/// Whether `there`, the branch's head at origin, is the task's own to replace: the head the board pushed last, a commit
/// the branch holds (the push only adds to it), or one the branch itself was at (rewritten since by its own rebase).
fn own(tp: &Path, branch: &str, there: &str, pushed: &str, head: &str) -> bool {
    there == pushed
        || git_out(tp, &["merge-base", "--is-ancestor", there, head]).is_ok()
        || git_out(tp, &["log", "-g", "--format=%H", &format!("refs/heads/{branch}")]).is_ok_and(|l| l.lines().any(|h| h == there))
}
/// Push `branch` from the worktree `tp` to origin, unless origin has its head already. `pushed`: the head the board
/// pushed there last. The head origin holds after it; else what git said, or `NOT_OURS` and origin's head.
pub fn push(tp: &Path, branch: &str, pushed: &str, most_ms: u64, repo: &str) -> Res<String> {
    let full = format!("refs/heads/{branch}");
    let head = git_out(tp, &["rev-parse", "--verify", &format!("{full}^{{commit}}")])?;
    let there = at_origin(tp, branch)?;
    if there == head { return Ok(head); }
    if !there.is_empty() && !own(tp, branch, &there, pushed, &head) { return Err(Error::new(format!("{NOT_OURS}{there}"))); }
    // Leased on exactly what origin was just seen to hold (no branch at all, the first time): git refuses the push when
    // anything else landed there meanwhile.
    let lease = format!("--force-with-lease={full}:{there}");
    git_slow_env(tp, &["push", "-u", &lease, "origin", &format!("{full}:{full}")], most_ms, &tool_env(tp)).map_err(|e| if e.msg.contains(TIMED_OUT) { Error::new(format!("{} (merge.pushTimeout): a slow pre-push hook? Raise it with herdr-projects set merge.pushTimeout 10m --repo {repo}", e.msg)) } else { e })?;
    // What was pushed, as the push left it in origin's copy here; the head read above where origin's fetch does not
    // keep a copy of this branch (a clone of one branch only).
    Ok(git_out(tp, &["rev-parse", "--verify", "--quiet", &format!("refs/remotes/origin/{branch}^{{commit}}")]).ok().filter(|h| !h.is_empty()).unwrap_or(head))
}
/// The failure of a push that would have lost what origin holds: `there`, which the task's branch does not have.
pub fn not_ours(branch: &str, tree: &str, there: &str, pushed: &str) -> Obj {
    let here = git_out(Path::new(tree), &["rev-parse", "HEAD"]).unwrap_or_default();
    let detail = format!("Branch: {branch}\nFolder: {tree}\nAt origin: {there}\nPushed from here last: {}\nHere: {here}\n\nNothing was pushed: it would have lost the commits at origin. Someone else pushed them (a suggestion accepted on GitHub, another machine), or another copy of this work did.", if pushed.is_empty() { "nothing" } else { pushed });
    let mut f = failure("push", "The branch at origin has commits this task does not have", &detail, &format!("Bring origin's commits into your worktree (git pull origin {branch}) and keep what they add, then finish again. Do not push over them."));
    f.put("agentCanFix", true); f.put("auth", false);
    f
}
