//! A review held back for extra usage (docs/extra-usage.md): under `ask` and `defer` a review does not start while its
//! reviewer's program is on extra usage (under `defer`, nor while its window is used up with nothing to pay on). Under
//! `defer` the first on `reviewer.fallback` whose program has room reviews instead (a hand-over before it began); else
//! the task is stopped as an agent is (limitStop `what: usage`, `role: reviewer`), its row "Reviewing: waiting for the
//! usage window: resets 17:00", and is reviewed again by itself once the window has room (stop.rs: `lifted`). Under
//! `ask` it waits for you, and your carrying on is leave for that episode. A review under way is stopped the same way
//! the moment its program is seen on extra usage (tick.rs).
use super::gate::extra_ok;
use super::stop::{passed_over, why};
use super::words::{hm, setting_of};
use crate::agents::installed::installed;
use crate::clock::now;
use crate::err::Res;
use crate::finish::hand_over::usable;
use crate::finish::review_fallback::reviewer_now;
use crate::json::*;
use crate::limits::reached::{stop_at, StopHow};
use crate::runner::Guard;
use crate::store::handovers::{handover_line, log_handover, next_fallback};
use crate::store::settings::lookup::work;
use crate::store::settings::values::{agent_of, list_of};
use crate::store::usage::{fold, now_of, Now, READ};

/// Who would review now and its program: the reviewer on the task (finish/review_fallback.rs), on the working agent's
/// program where its own is not installed here, as finish/review_once.rs starts it.
pub fn program_of(t: &Obj, ch: &Obj) -> (String, String) {
    let (agent, _) = reviewer_now(t, ch);
    let k = agent_of(&agent).kind;
    if !installed(&k) && installed(t.s("agent")) { return (agent, t.t("agent")); }
    (agent, k)
}
/// Is a review by this program held back now, as `setting` says and its window stands? Your leave for the episode
/// (`extraOk`) lets it run.
pub fn held_now(t: &Obj, setting: &str, kind: &str, n: &Now, at: i64) -> bool {
    setting != "allow" && READ.contains(&kind) && !crate::chat_only::is_chat(t) && !extra_ok(t, kind, n, at) && (n.extra || (setting == "defer" && n.hard && n.resets > at))
}
/// Why a fallback reviewer has no room: its program is on extra usage too, or out of allowance ("" when it has room).
pub fn no_room(agent: &str, at: i64) -> String {
    let over = passed_over(agent, at);
    if !over.is_empty() { return over; }
    let n = now_of(&agent_of(agent).kind, at);
    if n.hard && n.resets > at { format!("{agent} is out of allowance too, until {}", hm(n.resets, at)) } else { String::new() }
}
/// The first on `reviewer.fallback` that could review now: not `from`, able to review here under its access, and its
/// program with room.
pub fn fallback_for(ch: &Obj, from: &str, at: i64) -> Option<String> {
    let list = list_of(ch.at("reviewer.fallback"));
    if list.is_empty() { return None; }
    let next = next_fallback(&list, from, &[], |a| { let u = usable(a, ch.s("reviewer.access")); if u.is_empty() { no_room(a, at) } else { u } }).0;
    (!next.is_empty()).then_some(next)
}
/// A review stopped for extra usage: the next on `reviewer.fallback` that has room now, as the settings read now.
pub fn taker(t: &Obj, at: i64) -> Option<String> {
    let ch = work(Some(t));
    if list_of(ch.at("reviewer.fallback")).is_empty() { return None; }
    fallback_for(&ch, &program_of(t, &ch).0, at)
}
/// Stop the task's review for extra usage, `how` the setting says (`ask`, `defer`), its program's window `n`.
pub fn stop_review(id: &str, kind: &str, n: &Now, how: &str, at: i64) -> bool {
    let mut extra = obj(json!({ "how": how, "resets": n.resets, "window": n.since }));
    if !n.extra { extra.put("hard", true); }
    let since = if n.extra && n.since > 0 { n.since } else { at };
    stop_at(id, "usage", kind, since as f64, StopHow { role: "reviewer".into(), extra, ..Default::default() }).is_some()
}
/// Before a review starts (finish/review.rs, at each try): true when it goes on, by the reviewer on the task or by one
/// that took it over now; false when the task was stopped to wait for the window, or for you.
pub fn may_review(id: &str, g: &mut Guard, ch: &Obj) -> Res<bool> {
    let t = g.check()?;
    let s = setting_of(&t);
    let (agent, kind) = program_of(&t, ch);
    if s == "allow" || !READ.contains(&kind.as_str()) { return Ok(true); }
    let at = now();
    // Codex's window read afresh before a review, as before a start (gate.rs).
    if kind == "codex" && crate::store::marks::due("usage-codex-review", 5_000) && let Some(r) = crate::agents::read_window::codex_newest() { fold("codex", &r, at); }
    let n = now_of(&kind, at);
    if !held_now(&t, &s, &kind, &n, at) { return Ok(true); }
    if s == "defer" && let Some(next) = fallback_for(ch, &agent, at) {
        let because = why(&kind, &n);
        g.set(|t| { let h = log_handover(t, "reviewer", &agent, &next, &because); t.put("reviewerOn", next.clone()); t.put("msg", handover_line(&h)); })?;
        return Ok(true);
    }
    // A stop that could not be made leaves the review to run, never the task half way down its finish line.
    Ok(!stop_review(id, &kind, &n, &s, at))
}
