//! Done tasks that archive themselves once they have been done for `archive.doneAfter` (`none` by default), each as one
//! archive does: its space closes, its worktree stays. Only what `archive --done` takes (actions/archive_done.rs): a
//! pinned task, one that works, waits or is to do stays. So does a done one whose clean-up is still to come or whose
//! space is in use (a command of yours, its agent at work), until it is free. What it found is asked again under the
//! task's lock (a pin meanwhile keeps it). Its space closes first, by the pass of the herdr session it is in, and only
//! then is it archived, asked once more: a pass cut off in between, or a space herdr did not close, leaves a done task
//! that the next pass takes up again. Ported from Shepherdr 7745118a.
use crate::actions::{archive::{archive_on, close_space}, archive_done::left_why};
use crate::clock::now;
use crate::herdr::{cli::{same_socket, socket_exists}, Herdr};
use crate::json::*;
use crate::runner::runner_alive;
use crate::store::retries::waiting_of;
use crate::store::settings::{lookup::setting_text, values::ms};
use crate::store::tasks::{update, Change};

/// Since when a task has stood done: the last change of its stage, or its restore, whichever came later.
pub fn done_since(t: &Obj) -> i64 { t.i("at").max(t.i("restoredAt")) }
/// Done for `archive.doneAfter` by `now`, and nothing keeps it: pinned, waiting to try again, its clean-up still to
/// come, a runner on it. Asked of the board's row, and again of the task under its lock.
fn due(t: &Obj, now: i64) -> bool {
    if t.b("archivedAt") || t.s("stage") != "done" || t.b("pinned") || t.b("tidy") || waiting_of(t).is_some() || runner_alive(t) { return false; }
    let after = ms(&setting_text("archive.doneAfter", Some(t)));
    after > 0 && now - done_since(t) >= after
}

/// The housekeeping pass's part, for the herdr session at `socket`: the tasks it archived.
pub fn archive_old(socket: &str) -> Vec<String> {
    let api = Herdr::new(Some(socket.into()));
    let mut out = vec![];
    for r in crate::flow::live::board_for(socket) {
        if !left_why(&r).is_empty() || !due(&r, now()) { continue; }
        // A space in another herdr session that runs is closed by that session's own pass.
        let sock = r.at("link").s("socket");
        if !sock.is_empty() && !same_socket(sock, socket) && socket_exists(sock) { continue; }
        if same_socket(sock, socket) && !quiet(&r, &api) { continue; }
        let still = |t: &Obj| if due(t, now()) { Ok(()) } else { crate::fail!("{} is no longer due to archive by itself.", t.s("id")) };
        // Asked again under its lock; its space closes, and then it is archived, asked once more. A space herdr did
        // not close keeps it for the next pass, and so does a pass cut off before it was archived.
        let Ok(t) = update(r.s("id"), |t| { still(t)?; Ok(Change::Keep) }) else { continue };
        if close_space(&t, socket) == Some(false) && !gone(&t, &api) { continue; }
        if archive_on(r.s("id"), None, still).is_ok() { out.push(r.t("id")); }
    }
    out
}
/// Its own space is no longer open in herdr, as herdr says in so many words.
fn gone(t: &Obj, api: &Herdr) -> bool { api.workspaces().is_ok_and(|ws| !ws.iter().any(|w| w.at("workspace_id") == t.at("link").at("workspace"))) }
/// Nothing runs in the task's own space: an agent in it at rest with nothing beside it, and no command in its other
/// panes. What herdr cannot tell counts as in use.
fn quiet(t: &Obj, api: &Herdr) -> bool {
    let link = t.at("link");
    if !link.b("workspace") || !link.b("ownsWorkspace") { return true; }
    let Ok(spaces) = api.workspaces() else { return false };
    if !spaces.iter().any(|w| w.at("workspace_id") == link.at("workspace")) { return true; } // its space is gone already
    let Ok(panes) = api.panes(Some(link.s("workspace"))) else { return false };
    panes.iter().all(|p| match api.agent_get(p.s("pane_id")) {
        Ok(Some(a)) => ["idle", "done"].contains(&a.s("agent_status")) && api.pane_childless(p.s("pane_id"), a.s("agent")),
        Ok(None) => api.pane_idle(p.s("pane_id")),
        Err(_) => false,
    })
}
