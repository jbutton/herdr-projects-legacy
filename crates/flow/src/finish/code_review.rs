//! `merge.by none`: work the board never lands, for a repo whose work lands through code review on GitHub. A task whose
//! review passed waits as "Review passed: ready for code review" (its message, so `list` and `details`; its row on the
//! board and the web board), and `approve` is refused, `--anyway` too. Its review rounds, its check and its pushes to
//! its pull request are any task's. Read each time, as `merge.by` is: a change counts for a task that already waits,
//! and a merge approved before it was set and not made yet is called off (finish/auto_merge.rs: `called_off`, `hold`),
//! a pull request already in GitHub's merge queue taken out of it.
use super::shared::{approval_message, ASK};
use crate::flow::git::{gh, pr_number, GhOut};
use crate::gate;
use crate::json::*;
use crate::runner::runner_alive;
use crate::status_words::{CODE_REVIEW, CODE_REVIEW_READY};
use crate::store::failures::failure_patch;
use crate::store::repos::folder_of;
use crate::store::settings::lookup::setting;
use crate::store::tasks::{update, Change, Task};

/// Why `approve` is refused.
pub const WHY: &str = "this repo is landed through code review on GitHub, not by the board";
/// Does the board never land this task's work?
pub fn by_code_review(t: &Obj) -> bool { setting("merge.by", Some(t)) == json!("none") }
/// The same for a row: another machine's says so itself (`codeReview`, flow/live.rs); a task here is read now.
pub fn is(t: &Obj) -> bool { t.b("codeReview") || (!t.b("host") && by_code_review(t)) }
/// What `approve` and the yes key answer.
pub fn refusal(id: &str) -> String { format!("{id} was not merged: {WHY} (merge.by none).") }
/// What its message starts with, in place of "Approve to merge?": the label with what the review found
/// (`Review passed: ready for code review · 1 minor.`); a review that failed, as any task says it; none, only ready.
pub fn opening(rv: Option<&Value>) -> String {
    match rv {
        Some(rv) if gate::passes(rv) && gate::is_tagged(rv) => { let t = gate::tally(rv); format!("{CODE_REVIEW}{}.", if t.is_empty() { t } else { format!(" · {t}") }) }
        Some(rv) if gate::passes(rv) => format!("{CODE_REVIEW}. {}", gate::sentence(rv, "merge")),
        Some(rv) => gate::sentence(rv, "merge"),
        None => format!("{CODE_REVIEW_READY}."),
    }
}
/// What its row says first, and whether that is a review that passed (green, as a passed review is).
pub fn row_label(rv: Option<&Value>) -> (&'static str, bool) { let passed = rv.map(gate::passes).unwrap_or(false); (if passed { CODE_REVIEW } else { CODE_REVIEW_READY }, passed) }
/// Its message is of the other kind than `merge.by` says now. Only a message the approval itself wrote: one that says
/// what went wrong (a failure, a merge, GitHub's checks, main moving) stays.
fn other_kind(t: &Obj) -> bool {
    if t.s("stage") != "approval" || ["failure", "prFailure", "syncBlocked", "mergeFailed"].iter().any(|k| t.b(k)) { return false; }
    let m = t.s("msg");
    if by_code_review(t) { m.starts_with(ASK) } else { [CODE_REVIEW, CODE_REVIEW_READY, "Review: ", "Reviewed "].iter().any(|p| m.starts_with(p)) }
}
/// `merge.by` changed to or from `none` under a task that waits: its message says again what it waits for (the
/// housekeeping pass, finish/auto_merge.rs).
pub fn relabel(seen: &Task) {
    if !other_kind(seen) || runner_alive(seen) { return; }
    let _ = update(seen.s("id"), |t| {
        if !other_kind(t) || runner_alive(t) { return Ok(Change::Keep); }
        let m = approval_message(t); t.put("msg", m);
        Ok(Change::Write)
    });
}

/// The failure a task shows while its pull request is still in GitHub's merge queue.
const QUEUED: &str = "Still in GitHub's merge queue";
// `gh pr view --json` has no field for the merge queue: GitHub's GraphQL API is asked, and does the dequeuing.
const LOOK: &str = "query=query($owner: String!, $name: String!, $number: Int!) { repository(owner: $owner, name: $name) { pullRequest(number: $number) { id isInMergeQueue } } }";
const DEQUEUE: &str = "query=mutation($id: ID!) { dequeuePullRequest(input: { id: $id }) { mergeQueueEntry { id } } }";
fn said(r: &GhOut) -> String { let s = if r.stderr.trim().is_empty() { &r.stdout } else { &r.stderr }; s.trim().lines().last().unwrap_or("gh failed").to_string() }
/// The pull request's repo for that question, from its address (`https://github.com/o/r/pull/7`) as plain strings; with
/// no such address, gh's own placeholders for the repo of its folder, which only a typed field (`-F`) fills in.
fn repo_fields(pr: &str) -> Vec<String> {
    let parts: Vec<&str> = pr.split("/pull/").next().unwrap_or("").trim_end_matches('/').rsplitn(3, '/').collect();
    match parts.as_slice() {
        [name, owner, _] if pr.contains("/pull/") && !name.is_empty() && !owner.is_empty() => vec!["-f".into(), format!("owner={owner}"), "-f".into(), format!("name={name}")],
        _ => vec!["-F".into(), "owner={owner}".into(), "-F".into(), "name={repo}".into()],
    }
}
/// A pull request GitHub already lined up in its merge queue, by a yes from before `merge.by none`, is taken out of it:
/// switching GitHub's auto-merge off leaves a queued one where it is. Only once GitHub no longer lists it there is it
/// out. Err: why it is still there.
pub fn dequeue(t: &Obj) -> Result<(), String> {
    let (root, pr) = (folder_of(t.s("id")), pr_number(t.s("pr")));
    // Its node id, and whether it is in the queue.
    let look = || -> Result<(String, bool), String> {
        let mut a: Vec<String> = vec!["api".into(), "graphql".into()];
        a.extend(repo_fields(t.s("pr")));
        a.extend(["-F".into(), format!("number={pr}"), "-f".into(), LOOK.into()]);
        let r = gh(&a.iter().map(|x| x.as_str()).collect::<Vec<_>>(), &root, 60000);
        if r.status != 0 { return Err(said(&r)); }
        let v: Value = serde_json::from_str(&r.stdout).map_err(|_| format!("GitHub's answer about {} could not be read", t.s("pr")))?;
        let p = v.at("data").at("repository").at("pullRequest");
        if !p.is_object() { return Err(format!("GitHub did not find {}", t.s("pr"))); }
        Ok((p.t("id"), p.b("isInMergeQueue")))
    };
    let (id, queued) = look()?;
    if !queued { return Ok(()); }
    let r = gh(&["api", "graphql", "-f", DEQUEUE, "-f", &format!("id={id}")], &root, 60000);
    if r.status != 0 { return Err(said(&r)); }
    if look()?.1 { return Err(format!("GitHub still lists {} in its merge queue", t.s("pr"))); }
    Ok(())
}
/// It could not be taken out: the task says so where it waits, still merging as far as GitHub goes, and the
/// housekeeping pass tries again.
pub fn still_queued(t: &Task, why: &str) {
    let _ = update(t.s("id"), |t| {
        if t.at("failure").s("title") == QUEUED { return Ok(Change::Keep); }
        let detail = format!("{why}\n\n{} was lined up to merge by a yes given before merge.by none, and GitHub may still merge it.", t.s("pr"));
        let f = obj(json!({ "stage": "merge", "title": QUEUED, "detail": detail, "next": "Remove it from the merge queue on its page on GitHub. The board tries again by itself.", "agentCanFix": false, "auth": false }));
        for (k, v) in failure_patch(&f) { t.insert(k, v); }
        Ok(Change::Write)
    });
}
/// Out of the queue: that failure of its own goes.
pub fn out_of_queue(t: &mut Task) { if t.at("failure").s("title") == QUEUED { t.put("failure", Value::Null); } }
