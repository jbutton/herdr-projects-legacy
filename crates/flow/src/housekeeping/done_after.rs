//! Done tasks that archive themselves once they have been done for `archive.doneAfter` (`none` by default), each as one
//! archive does: its space closes, its worktree stays. Only what `archive --done` takes (actions/archive_done.rs): a
//! pinned task, one that works, waits or is to do stays. So does a done one whose clean-up is still to come or whose
//! space is in use (a command of yours, its agent at work), until it is free. Ported from Shepherdr 7745118a.
use crate::actions::{archive::archive, archive_done::left_why};
use crate::clock::now;
use crate::herdr::{cli::{same_socket, socket_exists}, Herdr};
use crate::json::*;
use crate::runner::runner_alive;
use crate::store::settings::{lookup::setting_text, values::ms};

/// Since when a task has stood done: the last change of its stage, or its restore, whichever came later.
pub fn done_since(t: &Obj) -> i64 { t.i("at").max(t.i("restoredAt")) }

/// The housekeeping pass's part, for the herdr session at `socket`: the tasks it archived.
pub fn archive_old(socket: &str) -> Vec<String> {
    let api = Herdr::new(Some(socket.into()));
    let mut out = vec![];
    for r in crate::flow::live::board_for(socket) {
        if r.b("archivedAt") || r.s("stage") != "done" || !left_why(&r).is_empty() || r.b("tidy") || runner_alive(&r) { continue; }
        let after = ms(&setting_text("archive.doneAfter", Some(&r)));
        if after == 0 || now() - done_since(&r) < after { continue; }
        // A space in another herdr session that runs is closed by that session's own pass.
        let sock = r.at("link").s("socket");
        if !sock.is_empty() && !same_socket(sock, socket) && socket_exists(sock) { continue; }
        if same_socket(sock, socket) && !quiet(&r, &api) { continue; }
        if archive(r.s("id")).is_ok() { out.push(r.t("id")); }
    }
    out
}
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
