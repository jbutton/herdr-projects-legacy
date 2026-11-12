//! The end of a saved conversation as this process last read it, for a reader that comes back to it every few seconds
//! while the agent works (the keeper's pass, keeper.rs): read again only when its size or time changed, and then only
//! what was written since. What it gives is what a read of the file's last bytes gives (read_turn.rs: `tail_lines`,
//! `read_turn`): the same lines, each parsed once, those that fell out of the last bytes let go.
//!
//! An agent program only adds to its file. One replaced, cut shorter or changed in place (its time changed, not its
//! size), or one whose end read before is no longer the end of a line, is read from its end again, as the first time.
//! A file is kept while it is asked for, and let go a minute after.
use super::read_turn::{turn_in, Turn};
use crate::clock::now;
use crate::json::*;
use std::cell::Cell;
use std::collections::HashMap;
use std::io::{Read, Seek, SeekFrom};
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};

/// A file not asked for this long is let go: a task that stopped working.
const FORGET_MS: i64 = 60_000;
/// What the file system says of a file: which file it is, its size and its time.
#[derive(Clone, Copy, PartialEq, Eq)]
struct Stamp { dev: u64, ino: u64, size: u64, mtime: (i64, i64) }
/// A file's end as read: the lines that parsed, each with the byte it starts at, oldest first; and the turn they hold.
struct Kept {
    kind: String, bytes: u64, stamp: Stamp, starts: Vec<u64>, rows: Vec<Value>,
    /// Where the next read goes on: the byte after the last whole line read. None: the part read held no line's start.
    next: Option<u64>,
    /// The last row is the file's last line, with no end yet: it is read again, with what follows it.
    open: bool,
    turn: Option<Turn>, used: i64,
}
static KEPT: LazyLock<Mutex<HashMap<PathBuf, Kept>>> = LazyLock::new(Default::default);
thread_local! { static READ: Cell<u64> = const { Cell::new(0) }; }
/// How many bytes this thread's reads here took from files: what tests count.
pub fn bytes_read() -> u64 { READ.with(|r| r.get()) }

fn stamp(m: &std::fs::Metadata) -> Stamp { Stamp { dev: m.dev(), ino: m.ino(), size: m.size(), mtime: (m.mtime(), m.mtime_nsec()) } }
/// The end of a conversation by this program's reader, as `read_turn(kind, file, bytes)` reads it, from what was kept of
/// the file since the last time.
pub fn read_turn_kept(kind: &str, file: &Path, bytes: u64) -> Option<Turn> {
    let at = now();
    let old = { let mut all = KEPT.lock().unwrap_or_else(|e| e.into_inner()); all.retain(|_, k| at - k.used < FORGET_MS); all.remove(file) };
    let st = stamp(&std::fs::metadata(file).ok()?);
    let mut k = match old.filter(|k| k.kind == kind && k.bytes == bytes) {
        Some(k) if k.stamp == st => k, // nothing changed: nothing read
        Some(k) if (k.stamp.dev, k.stamp.ino) == (st.dev, st.ino) && st.size > k.stamp.size => match grown(k, file, st) { Some(k) => k, None => fresh(kind, file, bytes, st)? },
        _ => fresh(kind, file, bytes, st)?,
    };
    k.used = at;
    let turn = k.turn.clone();
    KEPT.lock().unwrap_or_else(|e| e.into_inner()).insert(file.to_path_buf(), k);
    turn
}
/// The bytes of a file from `from` up to `to`.
fn part(file: &Path, from: u64, to: u64) -> Option<Vec<u8>> {
    let mut f = std::fs::File::open(file).ok()?;
    f.seek(SeekFrom::Start(from)).ok()?;
    let mut buf = vec![0u8; (to - from) as usize];
    f.read_exact(&mut buf).ok()?;
    READ.with(|r| r.set(r.get() + buf.len() as u64));
    Some(buf)
}
/// Read as the first time: its last `bytes`, a line cut at their start left out.
fn fresh(kind: &str, file: &Path, bytes: u64, st: Stamp) -> Option<Kept> {
    let from = st.size.saturating_sub(bytes);
    let buf = part(file, from, st.size)?;
    let mut k = Kept { kind: kind.into(), bytes, stamp: st, starts: vec![], rows: vec![], next: None, open: false, turn: None, used: 0 };
    let skip = if from == 0 { Some(0) } else { buf.iter().position(|b| *b == b'\n').map(|n| n + 1) };
    if let Some(skip) = skip { take(&mut k, &buf[skip..], from + skip as u64); }
    k.turn = turn_in(kind, &k.rows);
    Some(k)
}
/// Only what was written since: from the start of the line after the last whole one read. None: it cannot go on from
/// there (all of it fell out of the last bytes, or the byte before is no longer a line's end), so it is read again.
fn grown(mut k: Kept, file: &Path, st: Stamp) -> Option<Kept> {
    let next = k.next?;
    let from = st.size.saturating_sub(k.bytes);
    if from >= next { return None; }
    let buf = part(file, next.saturating_sub(1), st.size)?;
    let buf = if next == 0 { &buf[..] } else if buf.first() == Some(&b'\n') { &buf[1..] } else { return None };
    if k.open { k.starts.pop(); k.rows.pop(); k.open = false; }
    take(&mut k, buf, next);
    // The lines that start before the last bytes are out, as a read of the last bytes leaves them out.
    let out = k.starts.partition_point(|s| *s <= from);
    if from > 0 && out > 0 { k.starts.drain(..out); k.rows.drain(..out); }
    k.stamp = st;
    k.turn = turn_in(&k.kind, &k.rows);
    Some(k)
}
/// The lines of `buf`, which is the file from `at` on and starts with a line, as `tail_lines` reads them: those that
/// parse, empty ones left out; the last one with no end yet too, when it parses.
fn take(k: &mut Kept, buf: &[u8], at: u64) {
    let mut from = 0;
    while let Some(n) = buf[from..].iter().position(|b| *b == b'\n') {
        add(k, at + from as u64, &buf[from..from + n]);
        from += n + 1;
    }
    k.next = Some(at + from as u64);
    k.open = from < buf.len() && add(k, at + from as u64, &buf[from..]);
}
fn add(k: &mut Kept, start: u64, line: &[u8]) -> bool {
    if line.is_empty() { return false; }
    let Ok(v) = serde_json::from_str::<Value>(&String::from_utf8_lossy(line)) else { return false };
    k.starts.push(start);
    k.rows.push(v);
    true
}
