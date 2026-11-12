//! What herdr's `completion_seq` says of an agent's turns: herdr gives it at rest (the turn that ended last) and leaves
//! it out while a turn is going, so a missing one while the agent works means "in a turn", never "none finished yet".
use crate::json::*;

/// Still in the turn that began after `mark` was read at rest (its `seq`: herdr's completion_seq then): the same
/// completion, or none while it works. Where the turn was seen going right after its words went in (`work`: its
/// state_change_seq then, `working_seq`), it must still be that one: a turn that ended unseen, with another typed
/// straight into the terminal after it, has a later one. A mark without it (an older one, a turn not seen to start)
/// is judged by the completion alone.
pub fn same_turn(a: &Value, mark: &Value) -> bool {
    *a.at("completion_seq") == *mark.at("seq") || (working_seq(a).is_some() && (!mark.has("work") || a.at("state_change_seq") == mark.at("work")))
}
/// herdr's number for the state an agent is in, while it works on a turn that has not ended: None at rest.
pub fn working_seq(a: &Value) -> Option<Value> {
    (a.s("agent_status") == "working" && a.at("completion_seq").is_null() && !a.at("state_change_seq").is_null()).then(|| a.at("state_change_seq").clone())
}
/// Its completion_seq, and "working" in its place while herdr leaves it out during a turn: any turn that ends after
/// this was read is then a change.
pub fn completion(a: &Value) -> Value {
    if a.s("agent_status") == "working" && a.at("completion_seq").is_null() { json!("working") } else { a.at("completion_seq").clone() }
}
