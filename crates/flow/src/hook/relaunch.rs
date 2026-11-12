//! A working task's agent whose terminal went (herdr or the computer restarted) or whose program exited is started
//! again by the board itself: the restart a reply starts (delegate/jobs.rs), in its own conversation where that can be
//! resumed, and told why. Three tries at most, kept on the task (`relaunched: { tries, at, why }`): the first once it has
//! been down 20 seconds (herdr coming back settles first), the second a minute after it, the third five minutes after
//! that. Down once more, the task fails with the reason and waits for you. A reply, or ten minutes of it running after a
//! try, counts from none again. Never a task stopped (by you or at a limit), done, archived, or an attempt (one that
//! lost its agent is out: attempts/words.rs). From upstream SHE-254 (63a8be4b, #441) and SHEP-30 (d5f0bc62, #622).
use crate::board::keys::key_name;
use crate::clock::now;
use crate::flow::task::is_agent_task;
use crate::herdr::Herdr;
use crate::json::*;
use crate::runner::{runner_alive, spawn_claimed};
use crate::store::{failures::failure_patch, inbox::queue_text, retries::waiting_of, tasks::{get, update_wait, Change, Task}};

/// How long its agent is down before a try, and after a try before it is judged again.
pub const GRACE_MS: i64 = 20_000;
/// The wait before each try after the one before it: as many tries as waits.
pub const GAPS: [i64; 3] = [0, 60_000, 300_000];
/// How long it runs after its last try before its tries count from none again.
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
/// A task whose agent the board keeps running: its agent's own turn, and nobody else starting it now.
fn kept(t: &Task) -> bool {
    t.s("stage") == "working" && !t.b("archivedAt") && !t.b("attemptOf") && is_agent_task(t) && !runner_alive(t) && waiting_of(t).is_none()
}
#[derive(PartialEq)]
enum Next { Wait, Try, GiveUp }
fn next(t: &Task, now: i64) -> Next {
    let Some((since, _)) = down(t).filter(|_| kept(t)) else { return Next::Wait };
    let r = t.at("relaunched");
    let (tries, last) = (r.i("tries").max(0) as usize, r.i("at"));
    if now - since.max(last) < GRACE_MS { return Next::Wait; }
    if tries >= GAPS.len() { return Next::GiveUp; }
    if tries > 0 && now - last < GAPS[tries] { return Next::Wait; }
    Next::Try
}
/// Its agent was just seen down (hook/reconcile.rs): started again once its time has come, or given up on.
pub fn seen_down(id: &str, api: &Herdr) {
    let Some(t) = get(id) else { return };
    let due = next(&t, now());
    // A pane herdr did not find because it did not answer is no reason to start a second copy, nor to give up.
    if due == Next::Wait || api.workspaces().is_err() { return; }
    if due == Next::GiveUp { give_up(id) } else { try_again(id) }
}
/// One more try, counted on the task: the words it is told wait in its inbox for the restart to hand over.
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
    if !go { return; }
    let _ = queue_text(id, WORDS, Obj::new()); // the restart hands it over (delegate/launch.rs: again_prompt)
    spawn_claimed("restart", id, &[]);
}
/// Its agent was just seen running: ten minutes after its last try, its tries count from none again.
pub fn seen_up(t: &Task) {
    let steady = |t: &Task| t.at("relaunched").is_object() && now() - t.at("relaunched").i("at") >= STEADY_MS;
    if !steady(t) { return; }
    let _ = update_wait(t.s("id"), 10000, |t| { if !steady(t) { return Ok(Change::Keep); } t.del("relaunched"); Ok(Change::Write) });
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
