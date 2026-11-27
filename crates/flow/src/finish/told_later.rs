//! What people told a task after its description (`tell`, a reply: store/said_by.rs), as the work's and the plan's
//! reviewers read it: newest last, at most `MOST` of them and `CHARS` in all, the newest kept; where it differs from
//! the details, it wins. What was told before the description last changed (`describedAt`, actions/filing.rs) is
//! behind it and left out. A task nobody told anything since gets no such section.
use crate::clock::iso;
use crate::json::*;
use crate::status_words::who::YOU;
use crate::store::said_by::told;

/// The most of them read, and how much of their words in all.
pub const MOST: usize = 10;
pub const CHARS: usize = 3000;
/// The section's first line.
pub const HEAD: &str = "Later instructions (newest last; where they differ from the details, they win):";

/// The section, or nothing when nobody told the task anything since its description last changed.
pub fn later(t: &Obj) -> String {
    let all = told(t.s("id"));
    // Told before the description last changed: the new description already says what holds.
    let since = all.iter().position(|m| m.at >= t.i("describedAt")).unwrap_or(all.len());
    let (mut kept, mut room) = (vec![], CHARS);
    for m in all[since..].iter().rev().take(MOST) {
        let n = m.text.chars().count();
        // The newest is kept, cut short if it alone is longer than the cap; an older one goes in whole or not at all.
        let words: String = if kept.is_empty() && n > room { format!("{} …", m.text.chars().take(room).collect::<String>()) } else if n > room { break } else { m.text.clone() };
        room = room.saturating_sub(n);
        kept.push(format!("- {} ({}): {}", if m.by == YOU { "the human" } else { "the coordinator" }, iso(m.at), words.replace('\n', "\n  ")));
    }
    if kept.is_empty() { return String::new(); }
    let left = all.len() - kept.len();
    kept.reverse();
    let note = match left { 0 => String::new(), 1 => "(One older instruction is left out.)\n".into(), n => format!("({n} older instructions are left out.)\n") };
    format!("{HEAD}\n{note}{}", kept.join("\n"))
}
