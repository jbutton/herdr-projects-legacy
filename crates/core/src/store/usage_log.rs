//! The history of each program's usage window on this machine, so that you can tell afterwards when its agents worked
//! on extra usage: `usage-log.jsonl` in the data folder, one line for each reading that changed something (a window's
//! percent or reset, extra usage starting or ending, a refusal), written beside `usage.json` (usage.rs). Past 2 MB the
//! oldest half goes. `herdr-projects usage` shows it (docs/extra-usage.md).
use super::files::{append_line, data_dir, read_lines, write_atomic};
use crate::agents::read_window::Reading;
use crate::json::*;
use chrono::TimeZone;
use std::path::PathBuf;

const MAX_BYTES: u64 = 2_000_000;
pub fn file() -> PathBuf { data_dir().join("usage-log.jsonl") }
/// A reading that says something the last one kept did not (its time aside) is one more line. Called under the usage
/// lock (usage.rs: `fold`), so lines never interleave.
pub fn note(kind: &str, r: &Reading, was: &Reading, since: i64) {
    let gist = |x: &Reading| Reading { at: 0, ..x.clone() };
    if gist(r) == gist(was) { return; }
    trim();
    let _ = append_line(&file(), &json!({ "at": r.at, "program": kind, "windows": r.windows, "extra": r.extra, "hard": r.hard, "resets": r.resets, "since": since }));
}
fn trim() {
    if std::fs::metadata(file()).map(|m| m.len() <= MAX_BYTES).unwrap_or(true) { return; }
    let lines = read_lines(&file());
    let keep: Vec<String> = lines[lines.len() / 2..].iter().map(|v| v.to_string()).collect();
    let _ = write_atomic(&file(), format!("{}\n", keep.join("\n")).as_bytes());
}
/// The lines kept since `from` (ms), oldest first; of one program, or of every one for "".
pub fn since(kind: &str, from: i64) -> Vec<Value> {
    read_lines(&file()).into_iter().filter(|l| l.i("at") >= from && (kind.is_empty() || l.s("program") == kind)).collect()
}
fn when(ms: i64) -> String { chrono::Local.timestamp_millis_opt(ms).single().map(|d| d.format("%a %m-%d %H:%M").to_string()).unwrap_or_default() }
fn hm(ms: i64) -> String { chrono::Local.timestamp_millis_opt(ms).single().map(|d| d.format("%a %H:%M").to_string()).unwrap_or_default() }
/// One line a reading, in your clock's time: each window's percent and reset, and where extra usage starts and ends.
pub fn say(lines: &[Value]) -> String {
    if lines.is_empty() { return "No readings kept for that time. They are kept from this version on.".into(); }
    let mut on: std::collections::HashMap<String, bool> = Default::default();
    let mut out = vec![];
    for l in lines {
        let windows: Vec<String> = l.arr("windows").iter().map(|w| format!("{} {}%{}", w.s("name"), w.f("pct").round() as i64, if w.i("resets") > 0 { format!(" (resets {})", hm(w.i("resets"))) } else { String::new() })).collect();
        let was = on.insert(l.t("program"), l.b("extra")).unwrap_or(false);
        let mark = match (was, l.b("extra")) { (false, true) => "  << ON EXTRA USAGE from here", (true, false) => "  >> off extra usage", (true, true) => "  (on extra usage)", _ => "" };
        let hard = if l.b("hard") { "  refused: out of allowance" } else { "" };
        out.push(format!("{}  {:6} {}{mark}{hard}", when(l.i("at")), l.s("program"), windows.join(" · ")));
    }
    out.join("\n")
}
