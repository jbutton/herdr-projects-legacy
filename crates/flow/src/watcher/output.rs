//! When each working agent's screen last showed something new: the Watcher's eye without its model. About once a
//! minute (LOOK_MS) the screen of every agent at work is read, and its words compared as the Watcher compares them
//! (pace.rs: a spinner, a timer or a token count that moved is no change). It costs no model call and runs with the
//! Watcher off too. The task keeps `output`: `hash` (the words' fingerprint), `at` (when they last changed), `seen` (the
//! last look, written down at most every NOTE_MS while nothing changes) and `from` (when this unbroken stretch of looks
//! began). A gap longer than FRESH_MS (the machine slept, herdr or the keeper was down) starts a new stretch: nobody
//! looked, so nothing counts as quiet. A stall is told by it (stalled.rs).
use super::pace::{last_lines, words_of};
use crate::flow::task::is_agent_task;
use crate::herdr::Herdr;
use crate::json::*;
use crate::store::tasks::{update, Change};
use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

/// Each working agent's screen is read this often.
pub const LOOK_MS: i64 = 60_000;
/// A look that found the same words is written down at most this often.
pub const NOTE_MS: i64 = 5 * 60_000;
/// A last look older than this says nothing about now; a gap longer than this starts a new stretch.
pub const FRESH_MS: i64 = 10 * 60_000;

/// When this process last read each agent's screen, by herdr session and task.
static LOOKED: LazyLock<Mutex<HashMap<String, i64>>> = LazyLock::new(Default::default);
/// An agent at work, as the board's row shows it: the only screens looked at. Not an attempt's: no stall is told of one.
fn at_work(t: &Obj) -> bool { !t.b("archivedAt") && !t.b("attemptOf") && is_agent_task(t) && t.s("stage") == "working" && t.at("live").s("status") == "working" }

/// One look, once per tick of whoever ticks (the keeper, an open board). `tasks`: rows as the board shows them.
pub fn look(tasks: &[Obj], now: i64) {
    let key = |t: &Obj| format!("{}|{}", t.at("link").s("socket"), t.s("id"));
    let mut looked = LOOKED.lock().unwrap_or_else(|e| e.into_inner());
    looked.retain(|_, at| (now - *at).abs() < FRESH_MS); // a task long gone from work is forgotten
    for t in tasks.iter().filter(|t| at_work(t)) {
        if looked.get(&key(t)).map(|&at| now >= at && now - at < LOOK_MS).unwrap_or(false) { continue; }
        looked.insert(key(t), now);
        // never a deep read: it would scroll a working agent's view
        let Ok(screen) = Herdr::new(Some(t.at("link").t("socket"))).agent_read(t.at("link").s("pane"), "visible", None) else { continue };
        let hash = sha1_smol::Sha1::from(words_of(&last_lines(&screen))).digest().to_string()[..16].to_string();
        let _ = update(t.s("id"), |c| match noted(c.at("output"), &hash, now) {
            Some(o) if c.s("stage") == "working" => { c.put("output", o); Ok(Change::Write) }
            _ => Ok(Change::Keep),
        });
    }
}
/// What a look writes down, if anything: new words, a new stretch after a gap, or the look itself once the last one
/// written down is NOTE_MS old.
pub fn noted(o: &Value, hash: &str, now: i64) -> Option<Value> {
    let gap = !o.b("seen") || now - o.i("seen") > FRESH_MS;
    let changed = o.s("hash") != hash;
    if !gap && !changed && now - o.i("seen") < NOTE_MS { return None; }
    Some(json!({ "hash": hash, "at": if gap || changed { now } else { o.i("at") }, "seen": now, "from": if gap { now } else { o.i("from") } }))
}
