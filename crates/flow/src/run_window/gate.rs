//! Before the window (actions/start.rs: start_queued): a queued task outside its `run.window` does not start. The queue
//! asks `held` for each queued task before it counts the slots, as it asks usage/gate.rs: the task stays in the queue,
//! holding no slot and no place, as "waiting for its window: starts 22:00", and the first pass inside its window lets
//! it go (housekeeping runs the queue on every pass, with the board open or not). Let go well after its window opened
//! (the machine slept, the board was not running), it is marked as started late (`windowLate`). `--now` and carrying on
//! start it anyway, with leave (`windowOk`) until its window next opens.
use super::words::{start_line, SETTING};
use crate::actions::shared::{say, Said};
use crate::clock::now;
use crate::err::Res;
use crate::json::*;
use crate::store::settings::lookup::setting_text;
use crate::store::tasks::{find, get, update, Change, Task};
use crate::window_times::{next_open, open, stretch};

/// How long after its window opened a task let go counts as late: while the board runs it looks every few seconds.
pub const LATE: i64 = 5 * 60_000;

/// The task's window, as it reads it now.
pub fn window_of(t: &Obj) -> String { setting_text(SETTING, Some(t)) }
/// Leave to run outside its window, until the window next opens: `--now`, carrying on, a turn you start on it paused.
/// An attempt has its task's leave too (`--now` on a task done several times at once).
pub fn leave(t: &Obj, at: i64) -> bool {
    t.at("windowOk").i("until") > at || (t.b("attemptOf") && get(t.s("attemptOf")).is_some_and(|p| p.at("windowOk").i("until") > at))
}
/// Leave until the window next opens, as kept on the task.
pub fn leave_now(t: &Obj, at: i64) -> Value { json!({ "until": next_open(&window_of(t), at), "at": at }) }
/// Is the task outside its window now, with no leave? A chat-only task never is: it is a conversation with you.
pub fn outside(t: &Obj, at: i64) -> bool { !open(&window_of(t), at) && !crate::chat_only::is_chat(t) && !leave(t, at) }
/// A queued task held for its window (its line says until when).
pub fn start_waits(t: &Obj) -> bool { t.s("stage") == "queued" && t.b("windowWait") && !t.b("archivedAt") }
/// Is this queued task held for its window now? Brings its line up to date, or lets it go: then marked late when its
/// window had opened well before this pass, while it waited.
pub fn held(w: &Obj) -> bool {
    let (id, at) = (w.t("id"), now());
    let win = window_of(w);
    if !outside(w, at) {
        if w.b("windowWait") {
            let since = stretch(&win, at).map(|s| s.0).unwrap_or(0);
            let late = since > 0 && w.at("windowWait").i("at") < since && at - since > LATE;
            let _ = update(&id, |t| {
                if !t.b("windowWait") { return Ok(Change::Keep); }
                t.del("windowWait"); t.put("msg", "");
                if late { t.put("windowLate", json!({ "opened": since, "at": at, "how": "started" })); }
                Ok(Change::Write)
            });
        }
        return false;
    }
    let opens = next_open(&win, at);
    let line = start_line(opens, at);
    let since = if w.at("windowWait").is_object() { w.at("windowWait").i("at") } else { at };
    let wait = json!({ "window": win, "opens": opens, "at": since });
    if tidy(wait.clone()) != *w.at("windowWait") || w.s("msg") != line {
        let _ = update(&id, |t| { if t.s("stage") != "queued" { return Ok(Change::Keep); } t.put("windowWait", wait.clone()); t.put("msg", line.clone()); Ok(Change::Write) });
    }
    true
}
/// As a task is delegated (actions/start.rs, with its settings for this start already on it): what an earlier round
/// left of its window goes, and `--now` outside its window is leave until the window next opens.
pub fn on_delegate(t: &mut Task, now_said: bool) {
    for k in ["windowWait", "windowLate", "windowOk", "windowDone"] { t.del(k); }
    let at = now();
    if now_said && outside(t, at) { let ok = leave_now(t, at); t.put("windowOk", ok); }
}
/// Start a task held for its window now, as you said (`delegate --now`, carrying on): leave until its window opens.
pub fn start_anyway(id: &str) -> Res<Said> {
    let cur = find(id)?;
    let cid = cur.t("id");
    if !start_waits(&cur) { crate::fail!("{cid} does not wait for its window."); }
    let at = now();
    let ok = leave_now(&cur, at);
    let t = update(&cid, |t| { t.put("windowOk", ok.clone()); t.del("windowWait"); t.put("msg", ""); Ok(Change::Write) })?;
    let started = crate::actions::start::start_queued(t.at("link").s("socket"));
    let named = super::words::named(&t);
    // Done several times at once: it started when its attempts did.
    let started = started.iter().any(|s| *s == cid || get(s).is_some_and(|c| c.s("attemptOf") == cid));
    let msg = if started { format!("{cid}: started, outside its window ({named}), as you said") } else { format!("{cid}: queued, to start outside its window ({named}), as you said") };
    Ok(say(get(&cid).unwrap_or(t), msg))
}
