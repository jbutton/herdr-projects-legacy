//! A working task's agent whose terminal went (herdr or the computer restarted) or whose program exited is started
//! again by the board itself: the restart a reply starts (delegate/jobs.rs, `restart --relaunch`), in its own
//! conversation where that can be resumed, and told why. Three tries at most, kept on the task (`relaunched: { tries,
//! at, why, upSince }`): the first once it has been down 20 seconds (herdr coming back settles first), the second a
//! minute after it, the third five minutes after that. Down once more, the task fails with the reason and waits for you.
//! A reply, or ten minutes of it running without a break, counts from none again. Nothing is done on a look herdr did
//! not answer. Never a task stopped (by you or at a limit), done, archived, or an attempt (one that lost its agent is
//! out: attempts/words.rs). From upstream SHE-254 (63a8be4b, #441) and SHEP-30 (d5f0bc62, #622).
use crate::board::keys::key_name;
use crate::clock::now;
use crate::flow::{live::{owns_pane, terminals}, pane_owner::coordinators_pane, task::is_agent_task};
use crate::herdr::Herdr;
use crate::json::*;
use crate::runner::{runner_alive, spawn_claimed};
use crate::store::{failures::failure_patch, retries::waiting_of, tasks::{get, update_wait, Change, Task}};

/// How long its agent is down before a try, and after a try before it is judged again.
pub const GRACE_MS: i64 = 20_000;
/// The wait before each try after the one before it: as many tries as waits.
pub const GAPS: [i64; 3] = [0, 60_000, 300_000];
/// How long it runs without a break before its tries count from none again.
pub const STEADY_MS: i64 = 600_000;
/// What the agent is told once it is back.
pub const WORDS: &str = "Your program had stopped while you worked (its terminal closed, or herdr or the computer restarted), and the board started it again. Continue the task where you left off.";

/// Since when its agent is down, and how ("gone": its terminal; "exited": its program), as the last look recorded it.
fn down(t: &Task) -> Option<(i64, &'static str)> {
    let l = t.at("last");
    if l.b("gone") { return Some((l.i("at"), "gone")); }
    let exited = l.as_object().is_some_and(|o| o.get("status") == Some(&Value::Null)) && t.b("exitedAt");
    exited.then(|| (l.i("at"), "exited"))
}
/// A task whose agent the board keeps running: its agent's own turn, nothing it waits for, not an attempt.
fn wanted(t: &Task) -> bool {
    t.s("stage") == "working" && !t.b("archivedAt") && !t.b("attemptOf") && is_agent_task(t) && waiting_of(t).is_none()
}
/// The restart asked for is still one to make when it runs (delegate/jobs.rs): a stop, a reply (which starts its own),
/// the end of the task or its archive since it was asked for leaves the task alone.
pub fn still_due(t: &Task) -> bool { wanted(t) && t.at("relaunched").is_object() }
#[derive(PartialEq)]
enum Next { Wait, Try, GiveUp }
fn next(t: &Task, now: i64) -> Next {
    let Some((since, _)) = down(t).filter(|_| wanted(t) && !runner_alive(t)) else { return Next::Wait };
    let r = t.at("relaunched");
    let (tries, last) = (r.i("tries").max(0) as usize, r.i("at"));
    if now - since.max(last) < GRACE_MS { return Next::Wait; }
    if tries >= GAPS.len() { return Next::GiveUp; }
    if tries > 0 && now - last < GAPS[tries] { return Next::Wait; }
    Next::Try
}
/// herdr asked again, and every answer in: no pane of the task's own holds a running agent of its, and no agent runs
/// under its name. A lookup that failed says nothing, and a pane whose id was handed to another is not the task's.
fn really_down(t: &Task, api: &Herdr) -> bool {
    let (Ok(panes), Ok(agents)) = (api.panes(None), api.agents()) else { return false };
    let link = t.at("link");
    if link.b("agent") && agents.iter().any(|a| a.at("name") == link.at("agent")) { return false; }
    let terms = terminals(t);
    let theirs = |p: &Value| coordinators_pane(link.s("socket"), p.s("pane_id"), p.s("terminal_id"));
    let own = |p: &&Value| !theirs(p) && ((p.s("pane_id") == link.s("pane") && owns_pane(t, p)) || terms.contains(&p.t("terminal_id")));
    match panes.iter().find(own) { None => true, Some(p) => matches!(api.agent_get(p.s("pane_id")), Ok(None)) }
}
/// Its agent was just seen down (hook/reconcile.rs): started again once its time has come, or given up on. Any time
/// it is seen down, the time it has run without a break starts again.
pub fn seen_down(id: &str, api: &Herdr) {
    let Some(t) = get(id) else { return };
    if t.at("relaunched").b("upSince") { let _ = update_wait(id, 10000, |t| { if !t.at("relaunched").b("upSince") { return Ok(Change::Keep); } t.obj_mut("relaunched").del("upSince"); Ok(Change::Write) }); }
    let due = next(&t, now());
    if due == Next::Wait || !really_down(&t, api) { return; }
    if due == Next::GiveUp { give_up(id) } else { try_again(id) }
}
/// One more try, counted on the task; the restart tells it why (delegate/jobs.rs).
fn try_again(id: &str) {
    let mut go = false;
    let _ = update_wait(id, 10000, |t| {
        let n = now();
        if next(t, n) != Next::Try { return Ok(Change::Keep); }
        let why = down(t).map(|d| d.1).unwrap_or("gone");
        t.put("relaunched", json!({ "tries": t.at("relaunched").i("tries") + 1, "at": n, "why": why }));
        go = true;
        Ok(Change::Write)
    });
    if go { spawn_claimed("restart", id, &["--relaunch"]); }
}
/// Its agent was just seen running: from when it is first seen so after a try, and once that has lasted ten minutes
/// without its being seen down, its tries count from none again.
pub fn seen_up(t: &Task) {
    let r = t.at("relaunched");
    if !r.is_object() || (r.b("upSince") && now() - r.i("upSince") < STEADY_MS) { return; }
    let _ = update_wait(t.s("id"), 10000, |t| {
        let r = t.at("relaunched");
        if !r.is_object() { return Ok(Change::Keep); }
        if !r.b("upSince") { let n = now(); t.obj_mut("relaunched").put("upSince", n); }
        else if now() - r.i("upSince") >= STEADY_MS { t.del("relaunched"); }
        else { return Ok(Change::Keep); }
        Ok(Change::Write)
    });
}
/// Started again as often as it may be, and down once more: the task waits for you, with why.
fn give_up(id: &str) {
    let _ = update_wait(id, 10000, |t| {
        if next(t, now()) != Next::GiveUp { return Ok(Change::Keep); }
        let how = if down(t).map(|d| d.1) == Some("exited") { "its program exited" } else { "its terminal closed" };
        let n = GAPS.len();
        let detail = format!("While the agent worked, {how} (herdr or the computer restarted, or the program crashed). The board started it again {n} times, in the same conversation where it could, the last {} minutes after the one before, and each time it stopped again.", GAPS[n - 1] / 60_000);
        let next = format!("Go into the task ({}) to see its terminal. A reply starts it again, and so does retry ({}).", key_name("open"), key_name("yes"));
        t.put("stage", "failed"); t.put("msg", format!("The agent keeps stopping: started again {n} times"));
        for (k, v) in failure_patch(&obj(json!({ "stage": "agent", "title": "The agent keeps stopping", "detail": detail, "next": next, "agentCanFix": false, "auth": false }))) { t.insert(k, v); }
        Ok(Change::Write)
    });
}
