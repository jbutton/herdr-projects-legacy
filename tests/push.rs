//! The push of a task's branch to origin, one file each in tests/push/: what it may replace there and when it is not
//! needed (lease.rs), how long it may run (timeout.rs), and the tools the repo's hooks run with (tools.rs). What they
//! share is here: a program of the test's own put first on PATH.
mod lease;
mod timeout;
mod tools;

use std::path::Path;

/// PATH as it was before the test put a folder of its own first; put back when the test ends, failed or not.
pub struct PathBack(Option<std::ffi::OsString>);
impl Drop for PathBack { fn drop(&mut self) { unsafe { match &self.0 { Some(p) => std::env::set_var("PATH", p), None => std::env::remove_var("PATH") } } } }
/// `dir` first on PATH, until what this returns goes.
pub fn first_on_path(dir: &Path) -> PathBack {
    let was = std::env::var_os("PATH");
    let mut all = vec![dir.to_path_buf()];
    all.extend(std::env::split_paths(&was.clone().unwrap_or_default()));
    unsafe { std::env::set_var("PATH", std::env::join_paths(all).unwrap()); }
    PathBack(was)
}
/// A program at `path` that runs `body`.
pub fn script(path: &Path, body: &str) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, body).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
}
