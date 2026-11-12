//! The end of a saved conversation as this process last read it, for a reader that comes back to it every few seconds
//! while the agent works (the keeper's pass, keeper.rs): read again only when its size or time changed, and then only
//! the bytes written since. What it gives is what a read of the file's last bytes gives (read_turn.rs: `tail_lines`,
//! `read_turn`): the same lines, each parsed once, those that fell out of the last bytes let go.
//!
//! A line still being written is kept as read so far and goes on with the next bytes, as upstream's LogTail keeps it
//! (06c613c2); once it starts before the last bytes it is no line of theirs, and only its end is looked for. More added
//! at once than the last bytes is read as its last bytes alone. An agent program only adds to its file: one replaced,
//! cut shorter or changed in place (its time changed, not its size), or whose last byte read is no longer there, is
//! read from its end again, as the first time. A file is kept while it is asked for, and let go a minute after.
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
    /// Where the file's last line starts, the one with no end yet (the file's size when it ends with a line's end).
    next: u64,
    /// That line's bytes so far; none kept while it is `cut`: it starts before the last bytes, so it is no row of theirs.
    carry: Vec<u8>, cut: bool,
    /// That line parses as it is, and is the last row: taken off when more of it comes.
    open: bool,
    /// The file's last byte, as read: still there when the file only grew.
    last: Option<u8>,
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
/// Read as the first time: its last `bytes`, the line cut at their start left out.
fn fresh(kind: &str, file: &Path, bytes: u64, st: Stamp) -> Option<Kept> {
    let from = st.size.saturating_sub(bytes);
    let buf = part(file, from, st.size)?;
    let mut k = Kept { kind: kind.into(), bytes, stamp: st, starts: vec![], rows: vec![], next: from, carry: vec![], cut: false, open: false, last: None, turn: None, used: 0 };
    feed(&mut k, &buf, from);
    k.turn = turn_in(kind, &k.rows);
    Some(k)
}
/// Only the bytes written since, and the last one read before them, which says the file only grew. None: more was added
/// than the last bytes (they alone are read), or that byte changed: it is read again.
fn grown(mut k: Kept, file: &Path, st: Stamp) -> Option<Kept> {
    let end = k.stamp.size;
    if st.size - end > k.bytes { return None; }
    let buf = part(file, end.saturating_sub(1), st.size)?;
    let new = match k.last { None if end == 0 => &buf[..], Some(b) if buf.first() == Some(&b) => &buf[1..], _ => return None };
    if k.open { k.starts.pop(); k.rows.pop(); k.open = false; }
    k.stamp = st;
    feed(&mut k, new, end);
    k.turn = turn_in(&k.kind, &k.rows);
    Some(k)
}
/// The file's bytes from `at` on (where the last read ended) into the kept lines, as `tail_lines` reads them: those that
/// parse, empty ones left out, and the last one with no end yet too when it parses; those that start before the last
/// bytes of the file (`k.stamp.size`) let go.
fn feed(k: &mut Kept, data: &[u8], at: u64) {
    let start = k.stamp.size.saturating_sub(k.bytes);
    let gone = |s: u64| start > 0 && s <= start;
    if gone(k.next) { k.cut = true; k.carry = vec![]; }
    let mut from = 0;
    while let Some(n) = data[from..].iter().position(|b| *b == b'\n') {
        if !k.cut {
            let v = if k.carry.is_empty() { parse(&data[from..from + n]) } else { k.carry.extend_from_slice(&data[from..from + n]); parse(&k.carry) };
            if let Some(v) = v { k.starts.push(k.next); k.rows.push(v); }
        }
        k.carry.clear();
        from += n + 1;
        k.next = at + from as u64;
        k.cut = gone(k.next);
    }
    if !k.cut { k.carry.extend_from_slice(&data[from..]); }
    if let Some(b) = data.last() { k.last = Some(*b); }
    let out = k.starts.partition_point(|s| gone(*s));
    k.starts.drain(..out);
    k.rows.drain(..out);
    k.open = false;
    if let Some(v) = if k.cut { None } else { parse(&k.carry) } { k.starts.push(k.next); k.rows.push(v); k.open = true; }
}
/// A line as JSON; None for an empty one or one that is no JSON. A line that ends otherwise than a JSON value can (a line
/// still being written, mostly) is not parsed.
fn parse(line: &[u8]) -> Option<Value> {
    let end = line.iter().rposition(|b| !matches!(b, b' ' | b'\t' | b'\r' | b'\n'))?;
    if !matches!(line[end], b'}' | b']' | b'"' | b'e' | b'l' | b'0'..=b'9') { return None; }
    serde_json::from_str(&String::from_utf8_lossy(line)).ok()
}
