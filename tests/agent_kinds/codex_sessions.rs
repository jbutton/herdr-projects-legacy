//! Codex's saved conversations are looked for only in the day folder where Codex put them, never by listing every day of
//! a long history: a conversation's id (a UUIDv7) carries the moment it began, which names its folder.
use crate::common;
use chrono::TimeZone;
use common::*;
use std::path::{Path, PathBuf};
use sw::agents::adapter::transcript;
use sw::agents::read_turn::folders_listed;

const DAY: i64 = 86_400_000;
/// A Codex thread id: a UUIDv7, whose first 48 bits are the moment it began.
fn v7(ms: i64, n: i64) -> String { format!("{:08x}-{:04x}-7abc-8def-{:012x}", ms >> 16, ms & 0xffff, n) }
/// Where Codex saves a conversation begun at `ms`: the folder of that local day, the file named after its time and id.
fn save(sessions: &Path, ms: i64, id: &str) -> PathBuf {
    let t = chrono::Local.timestamp_millis_opt(ms).unwrap();
    let f = sessions.join(t.format("%Y/%m/%d").to_string()).join(format!("rollout-{}-{id}.jsonl", t.format("%Y-%m-%dT%H-%M-%S")));
    std::fs::create_dir_all(f.parent().unwrap()).unwrap();
    std::fs::write(&f, "{}\n").unwrap();
    f
}
/// What a lookup costs: the folders it lists.
fn listed<T>(f: impl FnOnce() -> T) -> (T, usize) { let before = folders_listed(); let r = f(); (r, folders_listed() - before) }

#[test]
fn a_codex_conversation_is_found_in_the_day_folder_it_began_in_not_by_listing_every_day() {
    let _w = world_with(Opts::default());
    let sessions = PathBuf::from(std::env::var("CODEX_HOME").unwrap()).join("sessions");
    let cwd = Path::new("/work/app");
    // Two years of history, three conversations a day (2,190 files in 730 day folders), then the task's, begun now.
    let now = 1_794_505_473_212; // 2026-11-12; the day is the local one, as Codex's folders are
    for d in 1..=730 { for i in 0..3 { let at = now - d * DAY + i * 3_600_000; save(&sessions, at, &v7(at, d * 3 + i)); } }
    let id = v7(now, 1);
    let mine = save(&sessions, now, &id);
    let (found, n) = listed(|| transcript("codex", &id, cwd));
    assert_eq!(found, Some(mine));
    assert!(n <= 3, "its own day's folder and the days on each side at most, not {n} folders");
    // One begun a year ago is found the same way, in its own day's folder.
    let old = v7(now - 365 * DAY, 365 * 3);
    let (found, n) = listed(|| transcript("codex", &old, cwd));
    assert!(found.as_ref().is_some_and(|f| f.to_string_lossy().ends_with(&format!("{old}.jsonl"))), "{found:?}");
    assert!(n <= 3, "{n} folders");
    // One not saved (yet): saying so lists no more.
    let (found, n) = listed(|| transcript("codex", &v7(now + 60_000, 9), cwd));
    assert_eq!(found, None);
    assert!(n <= 3, "{n} folders");
    // An id that carries no time (older Codex) is still found, the newest day first, at the first match.
    let plain = save(&sessions, now, "plain-id");
    let (found, n) = listed(|| transcript("codex", "plain-id", cwd));
    assert_eq!(found, Some(plain));
    assert!(n <= 4, "the sessions, year, month and day folders, not {n}");
}
