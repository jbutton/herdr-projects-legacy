//! Where Codex saved a conversation, found by its id without listing every day of a long history. Codex saves it as
//! `sessions/<yyyy>/<mm>/<dd>/rollout-<time>-<id>.jsonl`, in the folder of the local day it began, and its id is a UUIDv7,
//! whose first 48 bits are that moment: only that day's folder is listed, then the day after and the day before (a
//! time zone that is not the board's). An id that carries no time is looked for newest day first, up to the first match.
use super::homes::codex_home;
use super::read_turn::list_dir;
use chrono::TimeZone;
use std::path::{Path, PathBuf};

/// The moment a UUIDv7 was made, in milliseconds; None for any other id.
pub fn began(id: &str) -> Option<i64> {
    let b = id.as_bytes();
    let v7 = b.len() == 36 && [8, 13, 18, 23].iter().all(|&i| b[i] == b'-') && b[14] == b'7' && b.iter().all(|c| *c == b'-' || c.is_ascii_hexdigit());
    if !v7 { return None; }
    i64::from_str_radix(&format!("{}{}", &id[..8], &id[9..13]), 16).ok()
}
/// A conversation's file, by its id.
pub fn transcript(session: &str) -> Option<PathBuf> {
    let want = format!("{session}.jsonl");
    let test = |n: &str| n.ends_with(&want);
    let root = codex_home().join("sessions");
    let Some(ms) = began(session) else { return first(&root, &test, 4) };
    let day = chrono::Local.timestamp_millis_opt(ms).single()?.date_naive();
    [Some(day), day.succ_opt(), day.pred_opt()].into_iter().flatten().find_map(|d| first(&root.join(d.format("%Y/%m/%d").to_string()), &test, 0))
}
/// The first file whose name passes `test` under a folder (a few levels deep), each folder read newest name first.
fn first(dir: &Path, test: &dyn Fn(&str) -> bool, depth: usize) -> Option<PathBuf> {
    let mut all: Vec<_> = list_dir(dir).ok()?.flatten().collect();
    all.sort_by_key(|e| std::cmp::Reverse(e.file_name()));
    all.into_iter().find_map(|e| match e.file_type().map(|t| t.is_dir()).unwrap_or(false) {
        true => if depth > 0 { first(&e.path(), test, depth - 1) } else { None },
        false => test(&e.file_name().to_string_lossy()).then(|| e.path()),
    })
}
