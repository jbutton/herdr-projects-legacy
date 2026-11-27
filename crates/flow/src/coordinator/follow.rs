//! A coordinator's conversation followed past Claude Code's continued-in records (agents/continued.rs): it is resumed
//! in the newest session of its line and that one is kept in its state, and a program herdr brought back in an older
//! one is started again on the newest. Never a program that is itself in the newest session: Claude Code carries a
//! running chat on in a new session while herdr still names the one it began in, and closing its terminal mid-turn
//! leaves that chat running in Claude Code's background (background.rs).
use super::co::Co;
use super::state::{change, home, pumping};
use crate::agents::adapter::transcript;
use crate::agents::claude_running::{in_pane, still_for};
use crate::agents::continued::newest;
use crate::herdr::Herdr;
use crate::json::*;
use crate::store::marks::due;

/// How long the newest session's transcript has been still before a program in another session is started again on
/// it: a few minutes, so that nothing that still writes it is cut off.
pub const QUIET_MS: i64 = 3 * 60_000;

/// Its conversation, followed to the newest session of its line and written down. Returns its state as it is now.
pub fn to_newest(co: &Co, c: Obj) -> Obj {
    let (from, to) = (c.t("session"), newest(c.at("run").s("kind"), c.s("session"), &home(co)));
    if to == from { return c; }
    change(co, |x| { if x.s("session") != from { return false; } x.put("session", to.clone()); true }).unwrap_or(c)
}
/// The newest session of its line, when its pane runs an older one: a program herdr brought back after a restart (in
/// another terminal than the one the board started it in, docs/checked-on-herdr.md), in the session herdr names for
/// it, at rest, that may be closed (`may_close`). A program the board started in that terminal went on into the new
/// session itself.
pub fn behind(co: &Co, c: &Obj, a: &Value) -> Option<String> {
    if c.b("terminal") && a.s("terminal_id") == c.s("terminal") { return None; }
    if !["idle", "done"].contains(&a.s("agent_status")) { return None; }
    let runs = a.at("agent_session").t("value");
    let kind = if c.at("run").b("kind") { c.at("run").t("kind") } else { a.t("agent") };
    let to = newest(&kind, &runs, &home(co));
    (to != runs && may_close(co, a, &kind, &to)).then_some(to)
}
/// Its pane may be closed for the newest session `to`: its program has exited, or `to`'s transcript has not grown for
/// `QUIET_MS` and the program is not in `to` by its own account (claude_running.rs). One that is in it, at rest between
/// two turns or not, is never closed, whatever session herdr names for it.
fn may_close(co: &Co, a: &Value, kind: &str, to: &str) -> bool {
    let api = Herdr::new(Some(co.socket.clone()));
    let pane = a.s("pane_id");
    if api.shell_in_front(pane) { return true; }
    if in_pane(&api, pane).as_deref() == Some(to) { return false; }
    transcript(kind, to, &home(co)).and_then(|f| still_for(&f)).is_none_or(|ms| ms >= QUIET_MS)
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
