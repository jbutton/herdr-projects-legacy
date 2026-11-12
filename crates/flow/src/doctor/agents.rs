//! The agent programs your settings name (yours, a repo's, a task's), for `doctor`: each one installed here, its
//! version said in time and not older than the board was checked on, signed in where its own command can say
//! (kinds.rs `login`), and with herdr's integration where it needs one.
use super::Found;
use crate::agents::installed::{at_least, installed, late, version_of};
use crate::agents::kinds::{kind, program};
use crate::agents::sign_in::{self, status_line};
/// How a sign-in is asked and read: the same before a reviewer starts (sign_in.rs).
pub use crate::agents::sign_in::{signed_out, Sign};
/// One program. `integration`: None where it does not apply.
#[derive(Clone, Debug, Default)]
pub struct Prog { pub kind: String, pub named_by: Vec<String>, pub found: bool, pub version: String, pub late: bool, pub sign: Sign, pub integration: Option<bool> }
/// Each program the settings name, with where (`worker.agent`, `worker.agent for repo`, `worker.agent of HP-3`).
pub fn named() -> Vec<(String, Vec<String>)> {
    let mut out: Vec<(String, Vec<String>)> = vec![];
    for at in super::settings::places() {
        for (name, n, from) in super::settings::agents_at(&at) {
            // A repo or a task is named only where it says so itself, not for what it inherits.
            let own = at.label.is_empty() || from.ends_with(&at.label) || !at.repo.is_empty() && from.starts_with("your entry for");
            if !own { continue; }
            let by = format!("{name}{}", at.label);
            match out.iter_mut().find(|(x, _)| *x == n.kind) { Some((_, v)) => if !v.contains(&by) { v.push(by) }, None => out.push((n.kind, vec![by])) }
        }
    }
    out
}
/// Asked of the program on this machine (and of herdr, for its integration).
pub fn ask(k: &str, named_by: Vec<String>) -> Prog {
    let found = installed(k);
    let (version, late) = if found { (version_of(k), late(k)) } else { (String::new(), false) };
    let needs = kind(k).map(|a| a.needs_integration).unwrap_or(false);
    let integration = (found && needs).then(|| crate::herdr::Herdr::new(None).integration_ready(k).is_ok());
    Prog { kind: k.into(), named_by, found, sign: if found && !late { sign_in::ask(k) } else { Sign::Unknown }, version, late, integration }
}
/// The settings that name it, the first few.
fn by(p: &Prog) -> String {
    let more = p.named_by.len().saturating_sub(3);
    format!("{}{}", p.named_by.iter().take(3).cloned().collect::<Vec<_>>().join(", "), if more > 0 { format!(" and {more} more") } else { String::new() })
}

pub fn check(progs: &[Prog]) -> Vec<Found> {
    let mut out = vec![];
    for p in progs {
        let (k, by) = (&p.kind, by(p));
        let Some(a) = kind(k) else { continue };
        if !p.found {
            let first = p.named_by.first().map(String::as_str).unwrap_or("worker.agent");
            let set = match first.split_once(' ') { Some((n, r)) => format!("{n} <program> {}", r.replacen("for ", "--repo ", 1).replacen("of ", "--task ", 1)), None => format!("{first} <program>") };
            out.push(Found::bad(format!("{k} is not installed here ({by})"), format!("Install {k}, or choose another program: herdr-projects set {set}")));
            continue;
        }
        if p.late {
            out.push(Found::bad(format!("{k} did not say its version in time: `{} --version` hangs ({by})", program(k)), format!("Run `{} --version` in a terminal here to see why, and fix or reinstall {k}.", program(k))));
            continue;
        }
        let v = if p.version.is_empty() { String::new() } else { format!(" {}", p.version) };
        let before = out.len();
        if !p.version.is_empty() && a.min_version != "0" && !at_least(&p.version, a.min_version) {
            out.push(Found::bad(format!("{k}{v} is older than {}, the oldest the board was checked on: it runs without its turn-end hook", a.min_version), format!("Update {k}.")));
        }
        match &p.sign {
            Sign::Out => out.push(Found::bad(format!("{k} is not signed in ({by})"), format!("Run `{}` in a terminal here.", a.login.as_ref().map(|l| l.how).unwrap_or(k)))),
            Sign::NoAnswer(why) => out.push(Found::bad(format!("Could not tell whether {k} is signed in: `{}`: {why}", status_line(k)), format!("Run `{}` in a terminal here to see why; if it says you are signed out, run `{}`.", status_line(k), a.login.as_ref().map(|l| l.how).unwrap_or(k)))),
            _ => {}
        }
        if p.integration == Some(false) {
            out.push(Found::bad(format!("{k} needs herdr's own integration, or herdr cannot see when it works"), format!("Run: herdr integration install {}", a.integration)));
        }
        if out.len() == before { out.push(Found::ok(format!("{k}{v}{} ({by})", if p.sign == Sign::In { ", signed in" } else { "" }))); }
    }
    out
}
