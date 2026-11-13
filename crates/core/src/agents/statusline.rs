//! Claude Code's status line, as the board starts it (docs/extra-usage.md): the settings file the board passes a Claude
//! agent (adapter.rs) names `herdr-projects statusline` as its status line. Claude Code runs it after each answer with
//! its status input on stdin, whose `rate_limits` are its 5-hour and weekly windows: the one live reading of where the
//! agent stands in them. The entry keeps that part beside the task (`window-<role>.json`, written only when it changed),
//! then runs your own status line command on the same input and prints what it prints, so the agent's pane shows yours.
use super::adapter::Hook;
use super::homes::claude_home;
use super::read_window::status_file;
use crate::json::*;
use crate::store::files::{dir_of, read_json, write_json};
use crate::util::shq;
use std::io::{Read, Write};
use std::path::Path;

/// The command Claude Code runs for its status line, with the task's own token, as its turn-end hook has.
fn command(h: &Hook) -> String {
    let mut c = vec![h.shim.clone(), "statusline".into(), "--id".into(), h.id.clone(), "--role".into(), h.role.clone(), "--token".into(), h.token.clone()];
    if h.role == "reviewer" { c.extend(["--round".to_string(), h.round.to_string()]); }
    c.iter().map(|s| shq(s)).collect::<Vec<_>>().join(" ")
}
/// The `statusLine` the board's settings file gives Claude Code: its own command, with your status line's padding.
pub fn setting(h: &Hook) -> Value {
    let mut v = json!({ "type": "command", "command": command(h) });
    let yours = read_json(&claude_home().join("settings.json")).unwrap_or(Value::Null);
    if yours.at("statusLine").at("padding").is_number() { v["padding"] = yours.at("statusLine").at("padding").clone(); }
    v
}
/// Your own status line command, where Claude Code would find it for this folder: the project's local settings, the
/// project's, then yours. Never the board's own (it would run itself).
pub fn yours(project: &Path) -> Option<String> {
    let files = [project.join(".claude").join("settings.local.json"), project.join(".claude").join("settings.json"), claude_home().join("settings.json")];
    files.iter().filter_map(|f| read_json(f)).map(|s| s.at("statusLine").clone()).find(|l| l.is_object())
        .filter(|l| l.s("type") == "command" && l.b("command") && !l.s("command").contains(" statusline --id "))
        .map(|l| l.t("command"))
}
/// Keeps the status input's `rate_limits` beside the task, with when they were first seen so; true when written.
pub fn keep(id: &str, role: &str, input: &Value, now: i64) -> bool {
    let r = input.at("rate_limits");
    if id.is_empty() || !dir_of(id).is_dir() { return false; }
    let file = status_file(&dir_of(id), role);
    let was = read_json(&file).unwrap_or(Value::Null);
    if was.at("rate_limits") == &tidy(r.clone()) { return false; }
    write_json(&file, &json!({ "at": now, "rate_limits": r })).is_ok()
}
/// Your command run on the same input, in the folder Claude Code works in: what it printed.
pub fn pass_on(cmd: &str, input: &str, cwd: &Path) -> String {
    let mut c = std::process::Command::new("sh");
    c.args(["-c", cmd]).stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::null());
    if cwd.is_dir() { c.current_dir(cwd); }
    let Ok(mut child) = c.spawn() else { return String::new() };
    if let Some(mut i) = child.stdin.take() { let _ = i.write_all(input.as_bytes()); }
    child.wait_with_output().map(|o| String::from_utf8_lossy(&o.stdout).into_owned()).unwrap_or_default()
}
/// `herdr-projects statusline --id <id> --role <role> --token <token>`: run by Claude Code, never by hand. It keeps
/// the reading only for the task whose token it carries, and prints your status line whatever happens.
pub fn entry(id: &str, role: &str, token: &str, round: i64) {
    let mut raw = String::new();
    let _ = std::io::stdin().read_to_string(&mut raw);
    let input: Value = serde_json::from_str(&raw).unwrap_or(Value::Null);
    if ["agent", "reviewer"].contains(&role) && crate::store::marks::token_ok(id, role, token, round) { keep(id, role, &input, crate::clock::now()); }
    let ws = input.at("workspace");
    let project = [ws.s("project_dir"), ws.s("current_dir"), input.s("cwd")].into_iter().find(|p| !p.is_empty()).map(std::path::PathBuf::from).unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
    let Some(cmd) = yours(&project) else { return };
    let cwd = [input.s("cwd"), ws.s("current_dir")].into_iter().find(|p| !p.is_empty()).map(std::path::PathBuf::from).unwrap_or_else(|| project.clone());
    print!("{}", pass_on(&cmd, &raw, &cwd));
    let _ = std::io::stdout().flush();
}
