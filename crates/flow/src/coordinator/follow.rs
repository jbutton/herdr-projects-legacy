//! A coordinator's conversation followed past Claude Code's continued-in records (agents/continued.rs): it is resumed
//! in the newest session of its line and that one is kept in its state, and a program herdr brought back in an older
//! one is started again on the newest.
use super::co::Co;
use super::state::{change, home, pumping};
use crate::agents::continued::newest;
use crate::herdr::Herdr;
use crate::json::*;
use crate::store::marks::due;

/// Its conversation, followed to the newest session of its line and written down. Returns its state as it is now.
pub fn to_newest(co: &Co, c: Obj) -> Obj {
    let (from, to) = (c.t("session"), newest(c.at("run").s("kind"), c.s("session"), &home(co)));
    if to == from { return c; }
    change(co, |x| { if x.s("session") != from { return false; } x.put("session", to.clone()); true }).unwrap_or(c)
}
/// The newest session of its line, when its pane runs an older one: a program herdr brought back after a restart (in
/// another terminal than the one the board started it in, docs/checked-on-herdr.md), in the session herdr names for
/// it, at rest. A program the board started in that terminal went on into the new session itself.
pub fn behind(co: &Co, c: &Obj, a: &Value) -> Option<String> {
    if c.b("terminal") && a.s("terminal_id") == c.s("terminal") { return None; }
    if !["idle", "done"].contains(&a.s("agent_status")) { return None; }
    let runs = a.at("agent_session").t("value");
    let kind = if c.at("run").b("kind") { c.at("run").t("kind") } else { a.t("agent") };
    let to = newest(&kind, &runs, &home(co));
    (to != runs).then_some(to)
}
/// On the tick, with nothing else for it to do: one it finds behind is restarted, on the newest session (the restart
/// resumes the conversation it is in, followed: pump.rs `active`).
pub fn check(co: &Co, c: &Obj) {
    if !c.b("run") || !c.b("pane") || c.b("want") || c.b("typed") || c.b("stopped") || !c.arr("queue").is_empty() || pumping(c) { return; }
    if !due(&co.mark("coordinator-follow"), 10000) { return; }
    let Some(a) = Herdr::new(Some(co.socket.clone())).agent_get(c.s("pane")).ok().flatten() else { return };
    let Some(to) = behind(co, c, &a) else { return };
    let _ = change(co, |x| { x.put("session", to.clone()); true });
    let _ = super::requests::restart(co.clone());
}
