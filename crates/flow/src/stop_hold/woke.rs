//! Whether a stopped agent at work again was told to, or woke by itself: herdr sees it work either way, and so does its
//! own turn-end report. Only a prompt somebody wrote after the stop (a reply typed in its terminal) is a new turn of
//! yours; a helper's notification coming back is not (agents/read_turn.rs: `began`). Woken by itself, the task stays
//! stopped: no check, no review, no leave for extra usage, nothing carried on at a window's reset (hook.rs,
//! finish/turn_ended.rs), and the agent is watched to be held (stop_hold.rs).
use crate::agents::installed::read_turn_of;
use crate::json::*;
use crate::store::repos::cwd_of;
use std::path::Path;

/// Did somebody write the agent a prompt since `since`, as its saved conversation says? None: the board cannot tell (no
/// conversation it reads), and a turn is then taken as somebody's, as before.
pub fn typed_since(t: &Obj, since: i64) -> Option<bool> {
    let k = t.s("agent");
    let turn = read_turn_of(k, t.so("transcript").map(Path::new), t.s("session"), &cwd_of(t), false)?;
    // Where the end of the file holds no prompt, the whole file is read this once.
    let began = if turn.began > 0 { turn.began } else { read_turn_of(k, turn.file.as_deref(), "", Path::new("."), true).map(|x| x.began).unwrap_or(0) };
    Some(began >= since)
}
/// A stopped task's agent works, or ended a turn, with nobody's prompt since the stop: a helper woke it.
pub fn woke_itself(t: &Obj) -> bool { t.s("stage") == "stopped" && typed_since(t, t.i("stoppedAt")) == Some(false) }
