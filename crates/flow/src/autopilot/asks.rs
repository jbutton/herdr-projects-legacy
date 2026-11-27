//! The `has a question` wake (`autopilot.when stuck`) for every task the coordinator added from its own pane, in any
//! turn: those it may look after (care.rs), and also one it added in a turn of yours (a research task always ends on a
//! question), in its area or outside it. That one wakes it all the same, within its hours and also with no room left,
//! and the wake does not make it its own: it may read it, never tell, retry or approve it. A question wakes once, and
//! once more each time its agent ends a turn while the task still asks, when herdr saw it go to work again after the
//! question was reported (it carried on by itself, as after a usage limit's reset).
use super::care::{added_by, may_look_after};
use super::record::task_rec;
use crate::coordinator::co::Co;
use crate::json::*;

const MARK: &str = "stuck:asks";
/// Its agent waits with a question.
pub fn asks(t: &Obj) -> bool { !t.b("archivedAt") && t.s("stage") == "asks" && t.s("kind") == "question" }
/// A task it added in a turn of yours whose agent asks: not one it may look after, nor one left to you.
pub fn of_your_turn(co: &Co, ap: &Obj, t: &Obj) -> bool {
    asks(t) && added_by(co, t) && !may_look_after(ap, t) && !task_rec(ap, t.s("id")).is_some_and(|r| r.b("left"))
}
/// When its agent ended a turn after herdr saw it go to work again (from rest, or past a dialog such as a usage limit's)
/// after the question it reported, as long as the task still asks that question; 0 if it did not. Only a new stretch of
/// work counts: the rest of the turn it asked in, however long, is not going on.
fn went_on(t: &Obj) -> i64 {
    let b = t.at("blocked");
    let (asked, from, ended) = (b.i("at"), t.i("workFrom"), t.at("handled").i("at"));
    if asked > 0 && b.s("msg") == t.s("msg") && from > asked && ended >= from { ended } else { 0 }
}
fn end_of(mark: &str) -> Option<i64> { if mark == MARK { Some(0) } else { mark.strip_prefix(MARK)?.strip_prefix(':')?.parse().ok() } }
/// The question's mark, read against the marks seen before (`had`): `stuck:asks`, then `stuck:asks:<end>` for each turn
/// its agent ended after working on. It never goes back to an older one, so the same question wakes no more than that.
pub fn mark(t: &Obj, had: &[String]) -> String {
    let before = had.iter().filter_map(|h| end_of(h)).max();
    match (before, went_on(t)) {
        (Some(b), end) if b >= end => if b > 0 { format!("{MARK}:{b}") } else { MARK.into() },
        (_, 0) => MARK.into(),
        (_, end) => format!("{MARK}:{end}"),
    }
}
/// Was this question of the task seen before (the mark is the same question, its agent went on)?
pub fn seen(had: &[String]) -> bool { had.iter().any(|h| end_of(h).is_some()) }
/// A reason it holds that is the question of a task added in a turn of yours, still asked: it goes out with no room.
pub fn held(co: &Co, ap: &Obj, board: &[Obj], pending: &[Value]) -> bool {
    pending.iter().any(|r| r.s("why") == "stuck" && board.iter().any(|t| t.s("id") == r.s("task") && of_your_turn(co, ap, t)))
}
