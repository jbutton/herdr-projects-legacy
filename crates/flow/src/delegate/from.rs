//! Where a task's worktree starts (`workspace.from`): the remote's main, your checkout's commit, or a branch named at
//! the remote. The remote's branch is fetched on every start; origin's main that cannot be fetched starts from the copy
//! fetched last, and a named branch from its last copy too, both said in the task's log. A start that cannot begin from
//! a named branch at all is refused with the reason.
use crate::clock::{iso, now};
use crate::err::Res;
use crate::flow;
use crate::flow::git::has_origin;
use crate::json::*;
use crate::runner::Guard;
use crate::store::failures::failure_patch;
use crate::store::fetch;
use crate::store::files::dir_of;
use crate::store::repos::git_out;
use crate::store::settings::from::named;
use crate::store::settings::lookup::setting_text;
use crate::sync::default_branch;
use std::path::Path;
use std::process::{Command, Stdio};

/// How long a start waits for origin to give its branch: never asking for a password.
const FETCH_MS: u64 = 10000;
fn quiet_git(root: &Path, args: &[&str]) -> Command {
    let mut c = Command::new("git");
    c.args(args).current_dir(root).env("GIT_TERMINAL_PROMPT", "0").env("GIT_SSH_COMMAND", "ssh -oBatchMode=yes").stdin(Stdio::null());
    c
}
/// Fetch branch `b` of origin into `origin/<b>` for a start, `FETCH_MS` at most. One fetch at a time in a repo: every
/// worktree of it writes the same remote-tracking refs. Err: what git said.
fn fetch_now(root: &Path, b: &str) -> Result<(), String> {
    let spec = format!("+refs/heads/{b}:refs/remotes/origin/{b}");
    let ran = fetch::with_repo_lock(root, || Ok(quiet_git(root, &["fetch", "--no-tags", "origin", &spec]).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().map_err(|e| e.to_string())
        .and_then(|c| crate::herdr::cli::wait_with_timeout(c, FETCH_MS).map_err(|e| if e.code == "timeout" { format!("origin did not answer within {} seconds", FETCH_MS / 1000) } else { e.message })))).unwrap_or_else(|e| Err(e.msg));
    match ran { Ok(r) if r.status == 0 => Ok(()), Ok(r) => Err(r.stderr.trim().to_string()), Err(e) => Err(e) }
}
/// A start whose fetch of `b` failed begins from `r` as this repo has it: the task's log says so, with what git said.
fn stale_start(t: &Obj, root: &Path, b: &str, r: &str, said: &str) {
    use std::io::Write;
    let at: String = git_out(root, &["rev-parse", "--verify", "-q", &format!("{r}^{{commit}}")]).unwrap_or_default().chars().take(12).collect();
    let what = if r == "HEAD" { format!("your checkout's commit ({at})") } else { format!("origin/{b} as last fetched here ({at}), which may be behind origin") };
    let log = dir_of(t.s("id")).join("runner.log");
    let _ = std::fs::create_dir_all(dir_of(t.s("id")));
    let _ = std::fs::OpenOptions::new().create(true).append(true).open(&log).and_then(|mut f| writeln!(f, "{} the start could not fetch {b} from origin, so the task starts from {what}: {}.\nWhat git said (git fetch origin {b}):\n{}\n",
        iso(now()), fetch::plain(said, b), if said.is_empty() { "nothing" } else { said }));
}
/// `main`: the remote's default branch, fetched first on every start (`FETCH_MS` at most), so the task starts from
/// what was merged there since the last start. With no remote, or with `here`: your checkout's current commit.
fn main_ref(root: &Path, t: &Obj, from: &str) -> String {
    if from == "here" || !has_origin(root) { return "HEAD".into(); }
    // Origin's main branch never named here (a remote added by hand): asked of origin, so the task keeps up with it.
    fetch::learn_origin_head(root);
    let b = default_branch(root);
    let fetched = fetch_now(root, &b);
    let r = ["origin/HEAD", "origin/main", "origin/master"].into_iter().find(|r| git_out(root, &["rev-parse", "--verify", &format!("refs/remotes/{r}")]).is_ok()).unwrap_or("HEAD");
    // Another fetch (an agent's own, yours) moved the ref at that moment: the copy here is as new as that one made it.
    if let Err(said) = fetched { if !fetch::moved(&said) { stale_start(t, root, &b, r, &said); } }
    r.into()
}
/// The commit-ish a task's worktree starts from: `origin/<branch>` for a named branch, fetched now. Refused, with the
/// reason, for a branch origin does not have or a repo with no origin. With `merge.pr` off it lands there by a push of
/// its own (land/direct.rs), never on your checkout's branch.
pub fn base_ref(root: &Path, t: &Obj) -> Res<String> {
    let from = setting_text("workspace.from", Some(t));
    let Some(b) = named(&from) else { return Ok(main_ref(root, t, &from)) };
    if !has_origin(root) { crate::refuse!("workspace.from is {b}, a branch at the remote, and this repo has no remote named origin. Add the remote, or set workspace.from to main or here."); }
    let said = match fetch_now(root, b) { Ok(()) => return Ok(format!("origin/{b}")), Err(e) => e };
    if fetch::no_such_branch(&said) {
        crate::refuse!("origin has no branch {b} (workspace.from), so the task cannot start from it. Push {b} first, or set workspace.from to a branch origin has (git ls-remote --heads origin lists them), main or here.");
    }
    // Origin cannot be reached now: the copy of the branch it last gave is where the work starts.
    if git_out(root, &["rev-parse", "--verify", "-q", &format!("refs/remotes/origin/{b}")]).is_ok() { stale_start(t, root, b, &format!("origin/{b}"), &said); return Ok(format!("origin/{b}")); }
    crate::refuse!("Could not fetch {b} (workspace.from) from origin, and it was never fetched here: {said}. Make sure origin can be reached without a password (git fetch origin {b}), then delegate again.")
}
/// Does this task's start name a branch (`workspace.from develop`)? Kept on it as `fromNamed`: it then lands on that
/// branch, also when that is origin's main (sync.rs).
pub fn named_start(t: &Obj) -> bool { named(&setting_text("workspace.from", Some(t))).is_some() }
/// A start refused before anything was made: the task waits, failed, with the reason and what to do.
pub fn refuse_start(g: &mut Guard, why: &str) -> Res<()> {
    let first = why.split(". ").next().unwrap_or(why).trim_end_matches('.').to_string();
    let t = g.set(|t| {
        t.put("stage", "failed"); t.put("msg", format!("Could not start: {first}"));
        for (k, v) in failure_patch(&obj(json!({ "stage": "start", "title": "Could not start from that branch", "detail": why, "agentCanFix": false, "auth": false,
            "next": "Change workspace.from or the remote as the reason says, then delegate again." }))) { t.insert(k, v); }
    })?;
    flow::sidebar::tell_sidebar(&t);
    Ok(())
}
