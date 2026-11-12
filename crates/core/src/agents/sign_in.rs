//! Is a program signed in here: its own status command asked (kinds.rs `login`), for `doctor` and before a reviewer
//! starts. A signed-out Codex comes up at its sign-in page, which herdr reads as idle (codex 0.167.2, herdr 0.9.3), so
//! the brief typed in would only press its first choice. "Not logged in" counts only where the provider needs that
//! sign-in: Codex says "Not logged in" on a `model_provider` of yours that needs no OpenAI sign-in, and works there.
//! The answer is kept in the board's data folder (signins.json): one it can start on for a minute, a signed-out one not
//! at all, so signing in counts at the very next review.
use super::kinds::{kind, program};
use crate::clock::now;
use crate::json::*;
use crate::store::files::{data_dir, read_obj, write_json};
use std::path::PathBuf;

/// What its sign-in command said: no such command, signed in, signed out, signed out on a provider that needs no
/// sign-in of its own, or no answer (why).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Sign { #[default] Unknown, In, Out, NotNeeded, NoAnswer(String) }
/// Not signed in, by what its status command said: it failed, or it says so.
pub fn signed_out(code: i32, said: &str) -> bool {
    static RE: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| regex::Regex::new(r"(?i)not (logged|signed) in|logged out").unwrap());
    code != 0 || RE.is_match(said)
}
/// How long a sign-in command is given.
pub const WAIT_MS: u64 = 5000;
/// How long an answer it can start on is kept: a review handed to another program, or the next one soon after, does
/// not ask again.
const KEEP_MS: i64 = 60000;
fn file() -> PathBuf { data_dir().join("signins.json") }
/// The command that asks, as typed (`codex login status`); "" when the program has none.
pub fn status_line(k: &str) -> String { kind(k).and_then(|a| a.login.as_ref()).map(|l| std::iter::once(program(k)).chain(l.status.iter().copied()).collect::<Vec<_>>().join(" ")).unwrap_or_default() }
/// The command that signs it in (`codex login`).
pub fn how(k: &str) -> &str { kind(k).and_then(|a| a.login.as_ref()).map(|l| l.how).unwrap_or(k) }
/// Does the provider it is set to need its sign-in? Its app-server says, where it has one (Codex's `account/read`:
/// `requiresOpenaiAuth`, and no account); else, or when that gives no answer, a provider of your own needs none
/// (models/listed.rs `own_provider`: Codex's `model_provider`, Claude Code's `ANTHROPIC_BASE_URL` or a cloud).
fn needs_sign_in(k: &str) -> bool {
    let server = kind(k).map(|a| a.models.server).unwrap_or(&[]);
    if !server.is_empty() {
        if let Ok(r) = super::models::server::account(program(k), server, WAIT_MS) {
            if let Some(needs) = r.at("requiresOpenaiAuth").as_bool() { return needs && r.at("account").is_null(); }
        }
    }
    !super::models::listed::own_provider(k)
}
/// Asked of the program now, and kept.
pub fn ask(k: &str) -> Sign {
    let Some(l) = kind(k).and_then(|a| a.login.as_ref()) else { return Sign::Unknown };
    use std::process::{Command, Stdio};
    let child = match Command::new(program(k)).args(l.status).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn() { Ok(c) => c, Err(e) => return Sign::NoAnswer(e.to_string()) };
    let s = match crate::herdr::cli::wait_with_timeout(child, WAIT_MS) {
        Ok(r) if !signed_out(r.status, &format!("{}\n{}", r.stdout, r.stderr)) => Sign::In,
        Ok(_) => if needs_sign_in(k) { Sign::Out } else { Sign::NotNeeded },
        Err(_) => Sign::NoAnswer(format!("it did not answer in {}s", WAIT_MS / 1000)),
    };
    let word = match s { Sign::In => "in", Sign::Out => "out", Sign::NotNeeded => "not needed", _ => "" };
    if !word.is_empty() {
        let mut all = read_obj(&file()).unwrap_or_default();
        all.put(k, json!({ "sign": word, "until": if s == Sign::Out { now() } else { now() + KEEP_MS } }));
        let _ = write_json(&file(), &Value::Object(all));
    }
    s
}
/// The answer kept from a moment ago, else asked now.
pub fn kept_or_ask(k: &str) -> Sign {
    let all = read_obj(&file()).unwrap_or_default();
    let e = all.at(k);
    if e.i("until") > now() {
        match e.s("sign") { "in" => return Sign::In, "out" => return Sign::Out, "not needed" => return Sign::NotNeeded, _ => {} }
    }
    ask(k)
}
/// Why it cannot be started here as a reviewer: it says it is signed out, and its provider needs that sign-in
/// (`codex is not signed in here (…)`). None otherwise, or when it cannot say: then it is started, and a login error
/// in its turn is handled as any other.
pub fn out_here(k: &str) -> Option<String> {
    (kept_or_ask(k) == Sign::Out).then(|| format!("{k} is not signed in here (`{}` says so)", status_line(k)))
}
