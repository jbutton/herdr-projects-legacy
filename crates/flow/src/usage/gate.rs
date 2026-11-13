//! Before a start (actions/start.rs: start_queued): a queued task whose agent's program is known to be on extra usage
//! does not start under `ask` or `defer`. Under `defer` the first agent on `worker.fallback` that is not on extra usage
//! starts instead (a hand-over before it began); else it stays in the queue, holding no slot, as "waiting for the usage
//! window: resets 17:00", and starts by itself once that has passed. Under `ask`, or with its reset not known, it waits
//! for you (`usageWait.you`), and a retry starts it anyway, with leave for this episode (`extraOk`).
use super::stop::{fallback_for, why};
use super::words::{setting_of, start_line};
use crate::actions::shared::{say, Said};
use crate::clock::now;
use crate::err::Res;
use crate::json::*;
use crate::store::handovers::{handover_line, log_handover, on_now};
use crate::store::settings::lookup::setting_text;
use crate::store::settings::values::agent_of;
use crate::store::tasks::{find, update, Change};
use crate::store::usage::{now_of, Now, READ};

/// The program a task's agent runs (or will run): the one that took it over, else its own.
pub fn kind_of(t: &Obj) -> String { if t.b("on") { agent_of(t.s("on")).kind } else { t.t("agent") } }
/// Has the task leave to run on extra usage in this episode (you said so)?
pub fn extra_ok(t: &Obj, kind: &str, n: &Now, now_ms: i64) -> bool {
    let ok = t.at("extraOk");
    ok.s("agent") == kind && ((n.since > 0 && ok.i("window") == n.since) || ok.i("resets") > now_ms)
}
/// A queued task held back for extra usage (its line says why).
pub fn start_waits(t: &Obj) -> bool { t.s("stage") == "queued" && t.b("usageWait") && !t.b("archivedAt") }
/// Is this queued task held back now? Called by the queue for each task in its turn, under the slots' lock: it brings
/// the task's line up to date, or hands it to a fallback agent before it starts, or lets it go.
pub fn held(w: &Obj) -> bool {
    let (id, at) = (w.t("id"), now());
    let kind = kind_of(w);
    let s = setting_of(w);
    let n = now_of(&kind, at);
    let hold = s != "allow" && READ.contains(&kind.as_str()) && n.extra && !extra_ok(w, &kind, &n, at) && !crate::chat_only::is_chat(w);
    if !hold {
        if w.b("usageWait") { let _ = update(&id, |t| { t.del("usageWait"); t.put("msg", ""); Ok(Change::Write) }); }
        return false;
    }
    if s == "defer" {
        if let Some(next) = fallback_for(w, at) {
            let because = why(&kind, &n);
            let k = agent_of(&next).kind;
            let _ = update(&id, |t| {
                let from = on_now(t, &setting_text("worker.agent", Some(t)));
                let mut tried = strings(t.at("tried")); if !tried.contains(&from) { tried.push(from.clone()); } tried.push(next.clone());
                let h = log_handover(t, "worker", &from, &next, &because);
                t.put("on", next.clone()); t.put("agent", k.clone()); t.put("app", k.clone()); t.put("tried", json!(tried));
                t.del("usageWait"); t.put("msg", handover_line(&h));
                Ok(Change::Write)
            });
            return false;
        }
    }
    let you = s == "ask" || n.resets == 0;
    let line = start_line(&kind, n.resets, you, &s, at);
    let since = if w.at("usageWait").s("agent") == kind { w.at("usageWait").i("at") } else { at };
    let wait = json!({ "agent": kind, "resets": n.resets, "you": you, "window": n.since, "at": since });
    if tidy(wait.clone()) != *w.at("usageWait") || w.s("msg") != line {
        let _ = update(&id, |t| { t.put("usageWait", wait.clone()); t.put("msg", line.clone()); Ok(Change::Write) });
    }
    true
}
/// Start a task held back for extra usage now, on extra usage, as you said: it has leave for this episode.
pub fn start_anyway(id: &str) -> Res<Said> {
    let cur = find(id)?;
    let cid = cur.t("id");
    if !start_waits(&cur) { crate::fail!("{cid} is not held back for extra usage."); }
    let kind = kind_of(&cur);
    let n = now_of(&kind, now());
    let t = update(&cid, |t| {
        t.put("extraOk", json!({ "agent": kind, "window": n.since, "resets": n.resets, "at": now() }));
        t.del("usageWait"); t.put("msg", "");
        Ok(Change::Write)
    })?;
    let started = crate::actions::start::start_queued(t.at("link").s("socket"));
    let msg = if started.contains(&cid) { format!("{cid}: started on extra usage, as you said") } else { format!("{cid}: queued, to start on extra usage, as you said") };
    Ok(say(crate::store::tasks::get(&cid).unwrap_or(t), msg))
}
