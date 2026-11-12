//! Landing in your checkout (no pull request, no branch named at its start): the commit is built beside it from the
//! reviewed one, as `merge.method` says (as direct.rs builds it, signed when the repo signs its commits), and your
//! checkout is fast-forwarded to it. Git alone judges what is at risk: your own files the merge does not touch stay as
//! they are and out of its commit, and one it would overwrite (changed, untracked or ignored there) stops it by name,
//! with nothing changed. After cb8cab47 (Shepherdr #537).
use super::direct::build;
use super::merge::{fail, landed, no_fix, reverify};
use crate::err::Res;
use crate::finish::auto_merge::called_off;
use crate::flow;
use crate::json::*;
use crate::runner::Guard;
use crate::store::keys::{repo_keys, task_ref};
use crate::store::repos::{folder_of, git_out};
use crate::store::tasks::Task;
use std::path::Path;

/// Your checkout could not be moved without overwriting your own copy of `files`: nothing was merged and nothing there
/// changed. `why`: where it was to be moved, and for what.
fn in_the_way(g: &mut Guard, root: &Path, files: &[String], why: &str) -> Res<()> {
    let mut files = files.to_vec();
    files.sort(); files.dedup();
    let named = match files.as_slice() {
        [one] => one.clone(),
        [a, b] => format!("{a} and {b}"),
        [a, b, c] => format!("{a}, {b} and {c}"),
        [a, b, c, more @ ..] => format!("{a}, {b}, {c} and {} more", more.len()),
        [] => "your files".into(),
    };
    let list = files.iter().map(|f| format!("\t{f}")).collect::<Vec<_>>().join("\n");
    fail(g, &format!("Merge would overwrite your copy of {named}"),
        &format!("Checkout: {}\n{why}\n\nYour own copy of these files is in the way (changed, untracked or ignored there):\n{list}\n\nNothing was merged, and nothing in your checkout was changed.", root.display()),
        "Commit, stash, move or remove your own copy of these files, then approve again. The agent must not alter your checkout.", no_fix())
}

/// Runs once `sync::sync_main` found the task's worktree up to date with your checkout. `at_origin`, under
/// `merge.syncRemote`: your checkout's branch, and the commit of it at origin the work lands on (catch_up.rs). Your
/// checkout is brought up to it and to the work in one fast-forward, so a refusal leaves it where it was.
pub(super) fn land(id: &str, g: &mut Guard, t: &Task, how: &str, at_origin: Option<(String, String)>) -> Res<()> {
    let root = folder_of(t.s("id"));
    if let Some((b, at)) = &at_origin { if !super::catch_up::may_land(g, t, &root, b, at)? { return Ok(()); } }
    let onto = git_out(&root, &["symbolic-ref", "--short", "HEAD"])?;
    let here = { let h = flow::git::head_of(&root); if h.is_empty() { git_out(&root, &["rev-parse", "HEAD"])? } else { h } };
    let target = at_origin.as_ref().map(|(_, at)| at.clone()).unwrap_or_else(|| here.clone());
    let up = at_origin.as_ref().map(|(b, at)| format!(", brought up to origin/{b} at {at} with it (merge.syncRemote)")).unwrap_or_default();
    // The commit that was checked and reviewed is the one merged, by its id.
    let head = { let v = t.at("mergeIntent").t("verifiedHead"); if v.is_empty() { git_out(&root, &["rev-parse", "--verify", &format!("refs/heads/{}", t.s("branch"))])? } else { v } };
    // Your checkout moved on since the work was brought up to date with it: caught up with and checked again first.
    if git_out(&root, &["merge-base", "--is-ancestor", &target, &head]).is_err() { return reverify(id, g, t, &format!("Checking the work caught up with {onto}"), None); }
    let message = format!("{}\n\nTask: {}", if t.b("title") { t.t("title") } else { id.to_string() }, task_ref(id, &repo_keys()));
    let new = match build(&root, how, &target, &head, &message) {
        Ok(c) => c,
        Err(e) => return fail(g, "Could not make the merge commit", &format!("Checkout: {}\nOnto: {target} ({onto})\nReviewed: {head}\n\nWhat git said:\n{}\n\nNothing was merged.", root.display(), e.msg), "Fix what git said (git config user.name and user.email, or the signer for commit.gpgSign), then approve again.", no_fix()),
    };
    if called_off(g) { return Ok(()); }
    if new != here {
        if let Err((files, said)) = flow::git::fast_forward(&root, &new) {
            if !files.is_empty() { return in_the_way(g, &root, &files, &format!("Merging into: {onto}{up}")); }
            return fail(g, "Merge failed", &format!("Checkout: {}\nMerging into: {onto} at {here}{up}\nMerge commit: {new}\n\nWhat git said:\n{said}", root.display()),
                "Look at your checkout (git status) and settle what git said, then approve again. The agent must not alter your checkout.", no_fix());
        }
    }
    landed(id, g, t, &format!("Merged into {onto}."), &head, &new)
}
