//! Is a program signed in here: its own status command asked (kinds.rs `login`), for `doctor` and before a reviewer
//! starts. A signed-out Codex comes up at its sign-in page, which herdr reads as idle (codex 0.167.2, herdr 0.9.3), so
//! the brief typed in would only press its first choice. The answer is kept in the board's data folder (signins.json):
//! a signed-in one for a minute, a signed-out one not at all, so signing in counts at the very next review.
use super::kinds::{kind, program};
use crate::clock::now;
use crate::json::*;
use crate::store::files::{data_dir, read_obj, write_json};
use std::path::PathBuf;

/// What its sign-in command said: no such command, signed in, signed out, or no answer (why).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Sign { #[default] Unknown, In, Out, NoAnswer(String) }
/// Not signed in, by what its status command said: it failed, or it says so.
pub fn signed_out(code: i32, said: &str) -> bool {
    static RE: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| regex::Regex::new(r"(?i)not (logged|signed) in|logged out").unwrap());
    code != 0 || RE.is_match(said)
}
/// How long a sign-in command is given.
pub const WAIT_MS: u64 = 5000;
/// How long a signed-in answer is kept: a review handed to another program, or the next one soon after, does not ask again.
const KEEP_MS: i64 = 60000;
fn file() -> PathBuf { data_dir().join("signins.json") }
/// The command that asks, as typed (`codex login status`); "" when the program has none.
pub fn status_line(k: &str) -> String { kind(k).and_then(|a| a.login.as_ref()).map(|l| std::iter::once(program(k)).chain(l.status.iter().copied()).collect::<Vec<_>>().join(" ")).unwrap_or_default() }
/// The command that signs it in (`codex login`).
pub fn how(k: &str) -> &str { kind(k).and_then(|a| a.login.as_ref()).map(|l| l.how).unwrap_or(k) }
/// Asked of the program now, and kept.
pub fn ask(k: &str) -> Sign {
    let Some(l) = kind(k).and_then(|a| a.login.as_ref()) else { return Sign::Unknown };
    use std::process::{Command, Stdio};
    let child = match Command::new(program(k)).args(l.status).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn() { Ok(c) => c, Err(e) => return Sign::NoAnswer(e.to_string()) };
    let s = match crate::herdr::cli::wait_with_timeout(child, WAIT_MS) {
        Ok(r) => if signed_out(r.status, &format!("{}\n{}", r.stdout, r.stderr)) { Sign::Out } else { Sign::In },
        Err(_) => Sign::NoAnswer(format!("it did not answer in {}s", WAIT_MS / 1000)),
    };
    if matches!(s, Sign::In | Sign::Out) {
        let mut all = read_obj(&file()).unwrap_or_default();
        all.put(k, json!({ "in": s == Sign::In, "until": if s == Sign::In { now() + KEEP_MS } else { now() } }));
        let _ = write_json(&file(), &Value::Object(all));
    }
    s
}
/// The answer kept from a moment ago, else asked now.
pub fn kept_or_ask(k: &str) -> Sign {
    let all = read_obj(&file()).unwrap_or_default();
    let e = all.at(k);
    if e.is_object() && e.i("until") > now() { return if e.b("in") { Sign::In } else { Sign::Out }; }
    ask(k)
}
/// Why it cannot be started here as a reviewer: it says it is signed out (`codex is not signed in here (…)`). None when
/// it is signed in, or cannot say: then it is started, and a login error in its turn is handled as any other.
pub fn out_here(k: &str) -> Option<String> {
    (kept_or_ask(k) == Sign::Out).then(|| format!("{k} is not signed in here (`{}` says so)", status_line(k)))
}
