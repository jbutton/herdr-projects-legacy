//! What herdr's `completion_seq` says of an agent's turns: herdr gives it at rest (the turn that ended last) and leaves
//! it out while a turn is going, so a missing one while the agent works means "in a turn", never "none finished yet".
use crate::json::*;

/// Still in the turn that began after `mark` was read at rest (its `seq`: herdr's completion_seq then). At rest: the
/// same completion. While it works herdr leaves the completion out, and the turn is told by `work`: herdr's
/// state_change_seq when it was seen going right after its words went in (`working_seq`). A turn that ended unseen,
/// with another typed straight into the terminal after it, has a later one; a turn never seen going (`work` null) is
/// not known to be this one. A mark from before `work` was kept is judged as it was then: any turn going.
pub fn same_turn(a: &Value, mark: &Value) -> bool {
    if working_seq(a).is_some() { return mark.as_object().is_some_and(|m| !m.contains_key("work")) || a.at("state_change_seq") == mark.at("work"); }
    *a.at("completion_seq") == *mark.at("seq")
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
