//! Each agent program's usage window on this machine, as the board last read it (docs/extra-usage.md): kept in
//! `usage.json` in the data folder, one record per program (`claude`, `codex`), from the newest reading of any of its
//! agents (agents/read_window.rs). Whether it is on extra usage now, since when (the episode: from its first reading on
//! it until its window resets) and until when. A window whose reset has passed is a fresh one.
use super::files::{data_dir, read_obj, with_lock, write_json};
use crate::agents::read_window::{Reading, Window};
use crate::json::*;
use std::path::PathBuf;

/// The programs whose window the board can read.
pub const READ: [&str; 2] = ["claude", "codex"];
/// A reading older than this says nothing about now.
const STALE_MS: i64 = 8 * 24 * 3_600_000;
fn file() -> PathBuf { data_dir().join("usage.json") }
/// Every program's record as kept.
pub fn all() -> Obj { read_obj(&file()).unwrap_or_default() }

/// A program's window as it stands now. `since`: when this episode of extra usage began (0: not on it); it names the
/// episode too, so a leave given for it (`extraOk`) ends with it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Now { pub known: bool, pub at: i64, pub windows: Vec<Window>, pub extra: bool, pub hard: bool, pub since: i64, pub resets: i64 }
fn reading_of(rec: &Value) -> Reading { serde_json::from_value(rec.clone()).unwrap_or_default() }
/// A program's window now, from what was kept.
pub fn now_of(kind: &str, now: i64) -> Now {
    let rec = all().at(kind).clone();
    if !rec.is_object() || now - rec.i("at") > STALE_MS { return Now::default(); }
    let r = reading_of(&rec).at_now(now);
    Now { known: true, at: r.at, extra: r.extra, hard: r.hard, since: if r.extra { rec.i("since") } else { 0 }, resets: r.resets, windows: r.windows }
}
/// Is this program on extra usage now?
pub fn on_extra(kind: &str, now: i64) -> bool { now_of(kind, now).extra }
/// A newer reading, kept as the program's record; an older one changes nothing. The record is written only when it
/// changed. Returns the program's window now.
pub fn fold(kind: &str, r: &Reading, now: i64) -> Now {
    if !READ.contains(&kind) { return Now::default(); }
    let _ = with_lock("usage", || {
        let mut all = all();
        let old = all.at(kind).clone();
        if old.is_object() && r.at < old.i("at") { return Ok(()); }
        let was = if old.is_object() { reading_of(&old).at_now(r.at) } else { Reading::default() };
        // The same episode goes on while the window it began in has not reset.
        let since = if !r.extra { 0 } else if was.extra && old.i("since") > 0 { old.i("since") } else { r.at };
        let mut rec = obj(serde_json::to_value(r).unwrap_or(Value::Null));
        rec.put("since", since);
        if old == tidy(Value::Object(rec.clone())) { return Ok(()); }
        all.put(kind, Value::Object(rec));
        write_json(&file(), &Value::Object(all))
    });
    now_of(kind, now)
}
