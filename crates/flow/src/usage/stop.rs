//! A task stopped for extra usage, by the limits' own stop (limits/reached.rs, `limitStop` with `what: usage`): under
//! `ask` and `defer` its agent is stopped the moment it is seen on extra usage, its work kept, nothing committed or
//! merged. `ask` waits for you; `defer` hands it to the next agent on `worker.fallback` that is not on extra usage
//! (`handover_job`), else waits for its window and carries on by itself at the reset (`lifted`, seen by the limits'
//! tick). Under `defer` a turn that ended out of allowance waits for its window too (`out_of_allowance`).
use super::words::setting_of;
use crate::agents::read_turn::Turn;
use crate::clock::now;
use crate::finish::hand_over::{hand_over, usable};
use crate::json::*;
use crate::limits::reached::{stop_at, StopHow};
use crate::runner::{claim, release, Guard};
use crate::store::handovers::{next_fallback, on_now};
use crate::store::settings::lookup::work;
use crate::store::settings::values::{agent_of, list_of};
use crate::store::tasks::{update_wait, Change, Task};
use crate::store::usage::{fold, now_of, Now};

/// Stop the task's agent for extra usage, `how` the setting said (`ask`, `defer`). `since`: when its agent was seen on
/// it. Under `ask` the stop is what tells you: `board.notify` hears "needs you", not "extra usage" as well.
pub fn stop_now(id: &str, kind: &str, n: &Now, since: i64, how: &str) -> Option<Task> {
    let extra = obj(json!({ "how": how, "resets": n.resets, "window": n.since }));
    let t = stop_at(id, "usage", kind, since as f64, StopHow { role: "agent".into(), extra, ..Default::default() })?;
    if how != "ask" { return Some(t); }
    let w = n.since;
    update_wait(id, 10000, |t| { t.put("extraTold", w); Ok(Change::Write) }).ok()
}
/// Is a task stopped for extra usage free to carry on by itself, as the settings and the window stand now? `allow`
/// now: yes. `defer`: once its window has reset (not known when: once a newer reading says it is off extra usage).
/// `ask`: only you say so.
pub fn lifted(t: &Obj) -> bool {
    let ls = t.at("limitStop");
    match setting_of(t).as_str() {
        "allow" => true,
        "defer" if ls.i("resets") > 0 => now() >= ls.i("resets"),
        "defer" => { let n = now_of(ls.s("limit"), now()); n.known && !n.extra && !n.hard && n.at > ls.i("at") }
        _ => false,
    }
}
/// A task stopped for extra usage whose setting changed while it waits (`ask` to `defer`, or back): its stop and its line
/// follow, so it waits for the window, or for you, as the setting says now. Seen by the limits' tick.
pub fn follow(t: &Obj) {
    let ls = t.at("limitStop");
    if t.s("stage") != "stopped" || ls.s("what") != "usage" { return; }
    let how = super::words::stop_how(t);
    if how == ls.s("how") { return; }
    let _ = update_wait(t.s("id"), 2000, |t| {
        if t.s("stage") != "stopped" || t.at("limitStop").s("what") != "usage" { return Ok(Change::Keep); }
        let mut ls = t.o("limitStop").cloned().unwrap_or_default();
        ls.put("how", how.clone());
        t.put("msg", super::words::stop_line(&ls, now()));
        t.put("limitStop", ls);
        Ok(Change::Write)
    });
}
/// The first agent on `worker.fallback` that could take the task over now: not the one on it, not tried since you last
/// stepped in, installed, able to work under its access, and its program not on extra usage.
pub fn fallback_for(t: &Obj, now_ms: i64) -> Option<String> {
    let w = work(Some(t));
    let list = list_of(w.at("worker.fallback"));
    if list.is_empty() { return None; }
    let (from, access) = (on_now(t, &w.t("worker.agent")), w.t("worker.access"));
    let next = next_fallback(&list, &from, &strings(t.at("tried")), |a| { let u = usable(a, &access); if u.is_empty() { passed_over(a, now_ms) } else { u } }).0;
    (!next.is_empty()).then_some(next)
}
/// Why a fallback agent is passed over for extra usage: its program is on it too ("" when it is not).
pub fn passed_over(agent: &str, now_ms: i64) -> String {
    let k = agent_of(agent).kind;
    let n = now_of(&k, now_ms);
    if !n.extra { return String::new(); }
    format!("{agent} is on extra usage too{}", if n.resets > 0 { format!(", until {}", super::words::hm(n.resets, now_ms)) } else { String::new() })
}
/// Why the agent on it cannot go on, as a hand-over says it: "claude is on extra usage until 17:00".
pub fn why(kind: &str, n: &Now) -> String { format!("{kind} is on extra usage{}", if n.resets > 0 { format!(" until {}", super::words::hm(n.resets, now())) } else { String::new() }) }
/// `defer` with a fallback: the next agent on the list takes the task over in its worktree (finish/hand_over.rs); with
/// nobody to take it, it is stopped to wait for its window.
pub fn handover_job(id: &str) {
    let Ok(Some(t)) = claim(id, "usage", |_| {}) else { return };
    let mut g = Guard::new(id);
    let kind = t.t("agent");
    let n = now_of(&kind, now());
    if t.s("stage") == "working" && n.extra && !t.b("archivedAt") {
        g.expect(&t, &[]);
        let since = t.at("extra").i("since").max(n.since);
        match hand_over(id, &mut g, &why(&kind, &n), "") {
            // Its mark stays until board.notify has been told of it (tick.rs takes it off then).
            Ok(true) => {}
            Ok(false) => { stop_now(id, &kind, &n, since, "defer"); }
            Err(_) => {}
        }
    }
    release(id, false);
}
/// The next occurrence of the clock time in "out of allowance until 17:00" (store/handovers.rs words), in ms; 0: none.
fn words_reset(why: &str, now_ms: i64) -> i64 {
    use chrono::{Local, TimeZone};
    let Some((h, m)) = why.rsplit("until ").next().and_then(|s| s.trim().split_once(':')).and_then(|(h, m)| Some((h.parse::<u32>().ok()?, m.get(..2)?.parse::<u32>().ok()?))) else { return 0 };
    let Some(here) = Local.timestamp_millis_opt(now_ms).single() else { return 0 };
    let Some(at) = here.date_naive().and_hms_opt(h, m, 0).and_then(|d| Local.from_local_datetime(&d).earliest()) else { return 0 };
    let ms = at.timestamp_millis();
    if ms > now_ms { ms } else { ms + 86_400_000 }
}
/// A turn that ended out of allowance (and that nobody on `worker.fallback` took over): its program's window is kept for
/// the header, and under `defer`, with its reset known, the task waits for it and carries on by itself then. True when
/// it waits.
pub fn out_of_allowance(id: &str, t: &Task, x: &Turn, why: &str) -> bool {
    if !why.starts_with("out of allowance") { return false; }
    let (kind, at) = (t.t("agent"), now());
    if let Some(r) = &x.window { fold(&kind, r, at); }
    if setting_of(t) != "defer" || crate::chat_only::is_chat(t) { return false; }
    let resets = x.window.as_ref().map(|r| r.resets).filter(|r| *r > at).unwrap_or_else(|| words_reset(why, at));
    if resets <= at { return false; }
    let extra = obj(json!({ "how": "defer", "resets": resets, "hard": true }));
    stop_at(id, "usage", &kind, at as f64, StopHow { role: "agent".into(), extra, ..Default::default() }).is_some()
}
