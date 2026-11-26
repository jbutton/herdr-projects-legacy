//! Claude Code's continued-in records, followed to the newest session of a conversation. When Claude Code carries a
//! chat on in a new session, it writes `{"type":"continued-in","continuedInSessionId":"<new>"}` into the old session's
//! transcript; `claude --resume <old>` still opens the old one, without what was said in the new one.
use super::adapter::transcript;
use super::read_turn::tail_lines;
use crate::json::*;
use std::path::Path;

/// How much of a transcript's end is read: after the record come only a few lines of the program's own bookkeeping
/// (its cost, the tools it used, the last prompt), 1 to 6 KB in the transcripts this was checked on. Only the first line
/// read can be cut, so whatever follows a record that is read is read whole.
const TAIL: u64 = 64 * 1024;

/// The session a transcript says its chat went on in: its last continued-in record, when nothing was said in it since.
/// An old session that was resumed and talked in after its record went on there, so it is the newer one.
pub fn continued_in(file: &Path) -> Option<String> {
    for row in tail_lines(file, TAIL).ok()?.iter().rev() {
        match row.s("type") {
            "continued-in" => return Some(row.t("continuedInSessionId")).filter(|s| !s.is_empty()),
            "user" | "assistant" => return None,
            _ => {}
        }
    }
    None
}
/// The newest session of a conversation: its continued-in records followed while the session each one names is saved
/// (a missing file ends the line at the session before it). Only Claude Code writes them; other programs' sessions
/// are returned as they are.
pub fn newest(kind: &str, session: &str, cwd: &Path) -> String {
    let mut at = session.to_string();
    if kind != "claude" || at.is_empty() { return at; }
    let mut seen = vec![at.clone()];
    while let Some(next) = transcript(kind, &at, cwd).and_then(|f| continued_in(&f)) {
        if seen.contains(&next) || transcript(kind, &next, cwd).is_none() { break; }
        seen.push(next.clone());
        at = next;
    }
    at
}
