//! The environment a repo's own tool manager gives one of its folders: mise's (`mise env`), where the folder names its
//! tools in a file mise reads (`.tool-versions`, `mise.toml`, …). A git hook then runs with the repo's own Ruby or Node,
//! as the repo's agents run it (`mise exec -- git push`), whatever PATH the board's process was started with: the keeper's
//! comes from a login shell in the home folder, where `ruby` may be the system's, without the gems the hook loads.
//! Nothing where the folder names no tools, mise is not installed, or it cannot say (an untrusted `mise.toml`): the
//! command then runs as it always did.
use std::path::Path;
use std::process::{Command, Stdio};

/// The files in a folder by which mise knows the tools it wants.
pub const FILES: [&str; 9] = [".tool-versions", "mise.toml", ".mise.toml", "mise.local.toml", ".mise.local.toml", ".config/mise.toml", ".config/mise/config.toml", "mise/config.toml", ".mise/config.toml"];
/// How long mise may take to say: it only reads its files, it installs nothing.
const WAIT_MS: u64 = 30_000;

/// Whether the folder names its tools in a file of mise's.
pub fn names_tools(dir: &Path) -> bool { FILES.iter().any(|f| dir.join(f).is_file()) }
/// The variables mise sets for a command run in `dir` (PATH with the repo's tools first, the `[env]` of its config),
/// for that command to take in; empty where `dir` names no tools or mise cannot say.
pub fn tool_env(dir: &Path) -> Vec<(String, String)> {
    if !names_tools(dir) { return vec![]; }
    let Ok(child) = Command::new("mise").args(["env", "--json"]).current_dir(dir).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn() else { return vec![] };
    match crate::herdr::cli::wait_with_timeout(child, WAIT_MS) {
        Ok(r) if r.status == 0 => vars(&r.stdout),
        _ => vec![],
    }
}
/// `mise env --json`'s answer: its text variables, PATH among them.
pub fn vars(json: &str) -> Vec<(String, String)> {
    let Ok(serde_json::Value::Object(m)) = serde_json::from_str::<serde_json::Value>(json.trim()) else { return vec![] };
    m.into_iter().filter_map(|(k, v)| v.as_str().map(|s| (k, s.to_string()))).filter(|(k, _)| !k.is_empty()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn what_mise_env_says_is_read_as_text_variables_and_anything_else_is_nothing() {
        let said = vars("{\"PATH\":\"/tools/bin:/usr/bin\",\"RAILS_ENV\":\"test\",\"N\":3}\n");
        assert_eq!(said, [("PATH", "/tools/bin:/usr/bin"), ("RAILS_ENV", "test")].map(|(k, v)| (k.to_string(), v.to_string())));
        assert!(vars("").is_empty());
        assert!(vars("mise ERROR Config files are not trusted.").is_empty());
        assert!(vars("[1, 2]").is_empty());
    }
}
