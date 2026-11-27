//! A coordinator's conversation that Claude Code kept running in its background (its terminal was closed while it
//! worked): a start or resume of it is refused with "That session is running in the background (<id>)". The board waits
//! until that session's transcript stops growing, stops it there (`claude stop <id>`) and resumes it in the board's
//! pane, each step written to the start log. When that fails, the error names the session and the two commands that
//! do the same by hand.
use super::co::{Co, Level};
use super::state::{dir_of, home};
use crate::agents::adapter::transcript;
use crate::agents::claude_running::{self as running, still_for};
use crate::agents::read_turn::{read_turn, TAIL};
use crate::agents::start_failed::screen_of;
use crate::clock::{iso, now, pause};
use crate::herdr::Herdr;
use std::path::Path;

/// Still this long with its last turn ended, it has stopped growing; still for `LONG_MS` it has, whatever its end says
/// (a tool that runs a long while writes nothing meanwhile: a short wait alone could stop it in the middle of a turn).
const QUIET_MS: i64 = 30_000;
const LONG_MS: i64 = 5 * 60_000;
/// The longest the board waits: a session still at work after that is left to the human.
const WAIT_MS: i64 = 20 * 60_000;
const POLL_MS: u64 = 2000;

/// The start that was refused: in which pane, under which name, with which arguments, of which session.
pub struct Refused<'a> { pub api: &'a Herdr, pub co: &'a Co, pub kind: &'a str, pub pane: &'a str, pub name: &'a str, pub args: &'a [String], pub session: &'a str }

/// The session Claude Code says runs in its background, when that is why it did not start: in herdr's answer or on
/// the terminal it quit in.
pub fn held(api: &Herdr, kind: &str, pane: &str, said: &str) -> Option<String> {
    if kind != "claude" { return None; }
    running::held(said).or_else(|| running::held(&screen_of(api, pane)))
}
/// Takes it back into the board's pane: Ok once it runs there again (true: a dialog of its own is in the way, as for
/// any start), else the words for the error box.
pub fn take_back(r: &Refused, id: &str) -> Result<bool, String> {
    let log = dir_of(r.co).join("start.log");
    let session = if r.session.is_empty() { id } else { r.session };
    let fail = |why: String| { note(&log, &format!("Not taken back: {why}.")); Err(words(r.co, session, id, &why, &log)) };
    note(&log, &format!("{} was not started: Claude Code says session {session} is running in its background ({id}).", r.kind));
    if let Some(f) = transcript(r.kind, session, &home(r.co)) {
        note(&log, &format!("Waiting until its transcript stops growing ({}).", f.display()));
        if !wait_still(r.kind, &f) { return fail(format!("its transcript still grew after {} minutes", WAIT_MS / 60_000)); }
        note(&log, "Its transcript stopped growing.");
    }
    note(&log, &format!("Running `claude stop {id}`."));
    match running::stop(id) {
        Ok(out) => note(&log, &format!("`claude stop {id}` answered: {}", if out.is_empty() { "nothing" } else { out.as_str() })),
        Err(e) => return fail(format!("`claude stop {id}` failed: {e}")),
    }
    note(&log, &format!("Resuming session {session} in the board's pane {}.", r.pane));
    match r.api.agent_start(r.name, r.kind, r.pane, r.args, 60000) {
        Ok(_) => { note(&log, "Resumed."); Ok(false) }
        Err(e) if e.code == "agent_not_ready" => { note(&log, "Resumed; a dialog of its own is in the way."); Ok(true) }
        Err(e) => fail(format!("the resume failed: {}", e.message.trim())),
    }
}
/// Waits until the transcript stops growing. False: it still grew after `WAIT_MS`.
fn wait_still(kind: &str, f: &Path) -> bool {
    let size = || std::fs::metadata(f).map(|m| m.len()).unwrap_or(0);
    let (until, mut was, mut since) = (now() + WAIT_MS, size(), now() - still_for(f).unwrap_or(0));
    loop {
        let still = now() - since;
        if still >= LONG_MS || (still >= QUIET_MS && read_turn(kind, f, TAIL).is_none_or(|t| t.finished)) { return true; }
        if now() >= until { return false; }
        pause(POLL_MS);
        let s = size();
        if s != was { (was, since) = (s, now()); }
    }
}
/// The command that starts this coordinator again, in the same conversation.
fn restart_command(co: &Co) -> String {
    match &co.level { Level::Top => "herdr-projects coordinator restart".into(), Level::Machine => "herdr-projects coordinator restart --machine".into(), Level::Project(r) => format!("herdr-projects coordinator restart --project {r}") }
}
/// The error box's words: the session and the two commands first, so that a box cut short still holds them.
fn words(co: &Co, session: &str, id: &str, why: &str, log: &Path) -> String {
    format!("The {}'s conversation {session} is running in Claude Code's background. Run `claude stop {id}`, then `{}`. The board could not: {why} (each step is in {}).", co.name(), restart_command(co), log.display())
}
fn note(log: &Path, line: &str) {
    use std::io::Write;
    if let Some(d) = log.parent() { let _ = std::fs::create_dir_all(d); }
    let _ = std::fs::OpenOptions::new().create(true).append(true).open(log).and_then(|mut f| writeln!(f, "{} {line}", iso(now())));
}
