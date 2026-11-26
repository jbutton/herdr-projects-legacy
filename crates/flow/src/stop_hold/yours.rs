//! A task you stopped (`stop`, its key): it stays stopped until you carry it on (`retry`, its key, or a reply), never by
//! itself (woke.rs). Carrying on tells its agent that you stopped it and now say to go on; and where its program was
//! ended to hold it (stop_hold.rs), that the helpers it had out went with it.
use crate::actions::shared::{say, Said};
use crate::actions::talk::tell;
use crate::err::Res;
use crate::json::*;
use crate::store::tasks::Task;

/// Stopped by you: no limit's stop on it, an agent was on it, and it is not a task whose attempts wait for a pick.
pub fn stopped_by_you(t: &Obj) -> bool {
    t.s("stage") == "stopped" && !t.at("limitStop").is_object() && !t.b("archivedAt") && crate::flow::task::is_agent_task(t) && (t.arr("attempts").is_empty() || t.b("picked"))
}
/// What the agent is told of its program ended to hold it ("" when it was not), after why it was stopped.
pub fn ended_words(t: &Obj) -> &'static str {
    if t.at("hold").b("ended") { " Your program was ended after the stop, as helpers you had started in the background worked on: they are gone, so start again any you still need." } else { "" }
}
/// Carry on a task you stopped: its agent is told so, started again in its own conversation where its program was ended.
pub fn carry_on(cur: &Task) -> Res<Said> {
    let words = crate::flow::send::with_tail(cur, &format!("The human stopped you, and now says to carry on.{} Continue the task where you left off.", ended_words(cur)));
    let out = tell(cur.s("id"), &words, false)?;
    Ok(say(out.task, format!("{}: told the agent to continue", cur.s("id"))))
}
