//! Your checkout caught up with origin before a task lands in it, under `merge.syncRemote`: the task was kept up to
//! date with your checkout's branch as origin has it, so it lands on that same commit. A checkout that is only behind
//! is fast-forwarded to it, your own files it does not touch left as they are (one it would overwrite stops it by
//! name: local.rs); one with commits origin does not have is refused, since those were never checked with the work.
//! With the setting off nothing here runs, and the merge goes into your checkout as it is.
use super::merge::{fail, no_fix};
use crate::err::Res;
use crate::json::*;
use crate::runner::Guard;
use crate::store::keys::{repo_keys, task_ref};
use crate::store::repos::git_out;
use crate::store::tasks::Task;
use std::path::Path;

/// `b` is the branch your checkout is on and `at` the commit of it at origin that the task was just found to hold.
/// False: it may not land, and that was said on the task; your checkout is as it was.
pub fn checkout_to(g: &mut Guard, t: &Task, root: &Path, b: &str, at: &str) -> Res<bool> {
    let head = git_out(root, &["rev-parse", "HEAD"])?;
    if head == at { return Ok(true); }
    let on = git_out(root, &["symbolic-ref", "--short", "HEAD"]).unwrap_or_default();
    let behind = on == b && git_out(root, &["merge-base", "--is-ancestor", &head, at]).is_ok();
    let said = if behind {
        match crate::flow::git::fast_forward(root, at) {
            Ok(_) => return Ok(true),
            Err((files, _)) if !files.is_empty() => { super::local::in_the_way(g, root, &files, &format!("It was first to be brought up to origin/{b} at {at} (merge.syncRemote), which the task was kept up to date with."))?; return Ok(false); }
            Err((_, e)) => format!("\n\ngit merge --ff-only:\n{e}"),
        }
    } else { String::new() };
    let title = if behind { format!("Could not bring your checkout up to origin/{b}") } else if on != b { "Your checkout is on another branch".into() } else { format!("Your checkout has commits that origin/{b} does not have") };
    fail(g, &title, &format!("Checkout: {}\nIt is on: {} at {head}\nThis task was kept up to date with origin/{b} (merge.syncRemote) at {at}.{said}\n\nNothing was merged.", root.display(), if on.is_empty() { "no branch" } else { &on }),
        &format!("Bring your checkout onto {b} with nothing that is not at origin (push your own commits, or move them to another branch), then approve again. Or turn merge.syncRemote off for this task (herdr-projects set merge.syncRemote false --task {}) to land in your checkout as it is.", task_ref(t.s("id"), &repo_keys())), no_fix())?;
    Ok(false)
}
