//! The push of a task's branch to origin, for its pull request. It pushes the exact commit it read, keeps that commit as
//! the one it pushed, and replaces what origin holds only where that is exactly the commit it pushed there last (the
//! agent may have rebased since), or a commit the pushed one holds; never a commit someone else pushed, whatever a fetch
//! did to `origin/<branch>` meanwhile (upstream d63af260). A head origin has already is not pushed again, so the repo's
//! pre-push hook does not run for nothing (upstream #795). It runs with the repo's own tools (tool_env.rs), for as long
//! as `merge.pushTimeout` allows.
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
/// How the push may replace `there`, the branch's head at origin, with `head`: leased on no branch at all, leased on
/// exactly `pushed` (the commit the board pushed last, which its agent may have rebased or amended since), or not forced
/// where `head` holds it. None: it is someone else's, and nothing may be pushed over it.
fn lease(tp: &Path, full: &str, there: &str, pushed: &str, head: &str) -> Option<Option<String>> {
    if there.is_empty() { return Some(Some(format!("--force-with-lease={full}:"))); }
    if there == pushed { return Some(Some(format!("--force-with-lease={full}:{pushed}"))); }
    git_out(tp, &["merge-base", "--is-ancestor", there, head]).is_ok().then_some(None)
}
/// Push `branch` from the worktree `tp` to origin, unless origin has its head already. `pushed`: the commit the board
/// pushed there last. The commit origin has from it now; else what git said, or `NOT_OURS` and origin's head.
pub fn push(tp: &Path, branch: &str, pushed: &str, most_ms: u64, repo: &str) -> Res<String> {
    let full = format!("refs/heads/{branch}");
    // This very commit is pushed and kept as pushed, whatever the branch or `origin/<branch>` do meanwhile.
    let head = git_out(tp, &["rev-parse", "--verify", &format!("{full}^{{commit}}")])?;
    let there = at_origin(tp, branch)?;
    if there == head { return Ok(head); }
    let Some(how) = lease(tp, &full, &there, pushed, &head) else { return Err(Error::new(format!("{NOT_OURS}{there}"))) };
    let what = format!("{head}:{full}");
    let args: Vec<&str> = ["push"].into_iter().chain(how.as_deref()).chain(["origin", what.as_str()]).collect();
    git_slow_env(tp, &args, most_ms, &tool_env(tp)).map_err(|e| if e.msg.contains(TIMED_OUT) { Error::new(format!("{} (merge.pushTimeout): a slow pre-push hook? Raise it with herdr-projects set merge.pushTimeout 10m --repo {repo}", e.msg)) } else { e })?;
    // The branch's upstream, as `git push -u` would write it (it writes none for a commit), for the agent's own pulls.
    if git_out(tp, &["config", "--get", &format!("branch.{branch}.merge")]).is_err() {
        let _ = git_out(tp, &["config", &format!("branch.{branch}.remote"), "origin"]).and_then(|_| git_out(tp, &["config", &format!("branch.{branch}.merge"), &full]));
    }
    Ok(head)
}
/// The failure of a push that would have lost what origin holds: `there`, which the task's branch does not have.
pub fn not_ours(branch: &str, tree: &str, there: &str, pushed: &str) -> Obj {
    let here = git_out(Path::new(tree), &["rev-parse", "HEAD"]).unwrap_or_default();
    let detail = format!("Branch: {branch}\nFolder: {tree}\nAt origin: {there}\nPushed by the board last: {}\nHere: {here}\n\nNothing was pushed: it would have lost the commits at origin. Someone else pushed them (a suggestion accepted on GitHub, another machine), or they were pushed from this worktree by hand.", if pushed.is_empty() { "nothing" } else { pushed });
    let mut f = failure("push", "The branch at origin has commits this task does not have", &detail, &format!("Bring origin's commits into your worktree (git pull origin {branch}) and keep what they add, then finish again. Do not push over them. Only if they are your own commits, pushed by you and rewritten since (a rebase), push your branch yourself, then finish again."));
    f.put("agentCanFix", true); f.put("auth", false);
    f
}
