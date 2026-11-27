//! Claude Code's running sessions: the one a program in a pane is in, one it keeps running in its background, how long a
//! transcript has been still, and `claude stop`. A running Claude Code keeps `<config>/sessions/<pid>.json` up to date
//! with the session it is in, also after it carried the chat on in a new one while herdr still names the one it began
//! in. A chat whose terminal was closed while it worked goes on in Claude Code's background, and a resume of it is
//! refused with "That session is running in the background (<id>)" until `claude stop <id>` ends it there. A test
//! answers for `claude stop` (`set_fake`); under a fake herdr no real program is run.
use super::homes::claude_home;
use crate::clock::now;
use crate::herdr::Herdr;
use crate::json::*;
use crate::store::files::read_json;
use std::cell::RefCell;
use std::path::Path;
use std::process::{Command, Stdio};
use std::rc::Rc;
use std::sync::LazyLock;

type Fake = Rc<dyn Fn(&[String]) -> Result<String, String>>;
thread_local! { static FAKE: RefCell<Option<Fake>> = const { RefCell::new(None) }; }
/// A stand-in for `claude`: it gets the arguments, and answers what it printed or why it failed.
pub fn set_fake(f: Option<Fake>) { FAKE.with(|x| *x.borrow_mut() = f); }
/// How long `claude stop` is given to answer.
const STOP_MS: u64 = 30000;

/// The session the Claude Code program with this pid is in, as it wrote it down.
pub fn session_of(pid: i64) -> Option<String> {
    if pid <= 1 { return None; }
    let v = read_json(&claude_home().join("sessions").join(format!("{pid}.json")))?;
    (v.i("pid") == pid).then(|| v.t("sessionId")).filter(|s| !s.is_empty())
}
/// The session the Claude Code program in a pane's foreground is in. None: no such program, or it did not say.
pub fn in_pane(api: &Herdr, pane: &str) -> Option<String> {
    let i = api.process_info(pane)?;
    std::iter::once(i.i("foreground_process_group_id")).chain(i.arr("foreground_processes").iter().map(|p| p.i("pid"))).find_map(session_of)
}
static HELD: LazyLock<regex::Regex> = LazyLock::new(|| regex::Regex::new(r"(?i)session is running in the background \(([^)\s]+)\)").unwrap());
/// The session Claude Code said runs in its background, as it named it, when that is why it did not start.
pub fn held(text: &str) -> Option<String> { HELD.captures(text).map(|c| c[1].to_string()) }
/// How long since the file was last written, in ms. None: there is no such file.
pub fn still_for(file: &Path) -> Option<i64> {
    let at = std::fs::metadata(file).ok()?.modified().ok()?.duration_since(std::time::UNIX_EPOCH).ok()?.as_millis() as i64;
    Some((now() - at).max(0))
}
/// `claude stop <id>`: the background session ends, its conversation is kept. What it printed, or why it failed.
pub fn stop(id: &str) -> Result<String, String> {
    let args = vec!["stop".to_string(), id.to_string()];
    if let Some(f) = FAKE.with(|x| x.borrow().clone()) { return f(&args); }
    if crate::herdr::cli::fake().is_some() { return Err("no claude to run in a test".into()); }
    let child = Command::new(super::kinds::program("claude")).args(&args).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().map_err(|e| format!("it did not start ({e})"))?;
    let o = crate::herdr::cli::wait_with_timeout(child, STOP_MS).map_err(|_| format!("it did not answer within {} s", STOP_MS / 1000))?;
    let said = if o.stderr.trim().is_empty() { o.stdout.trim() } else { o.stderr.trim() }.to_string();
    if o.status == 0 { Ok(o.stdout.trim().to_string()) } else { Err(if said.is_empty() { format!("it ended with status {}", o.status) } else { said }) }
}
