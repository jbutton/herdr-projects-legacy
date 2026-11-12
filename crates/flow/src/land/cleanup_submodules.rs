//! Whether removing a worktree takes `--force`: git removes one with a submodule checked out only when forced, and a
//! forced remove looks at nothing. So what it would lose is looked at here first, right before each removal: a change
//! in it or in any submodule, nested ones too and also one a repo's `ignore` leaves out of its status, and a commit in
//! a submodule that no remote branch of it holds (git keeps a worktree's submodule repos in the worktree's own git
//! folder, and they go with it). Upstream issue #857.
use crate::store::repos::git_out;
use std::path::{Path, PathBuf};

/// `Ok(false)`: no submodule is checked out, git removes it as it is. `Ok(true)`: forced, nothing of them is lost.
/// `Err`: why it is kept.
pub fn force(wt: &Path) -> Result<bool, String> {
    let unsure = |_| "its submodules could not be looked at".to_string();
    let modules = PathBuf::from(git_out(wt, &["rev-parse", "--absolute-git-dir"]).map_err(unsure)?).join("modules");
    if !modules.is_dir() && !wt.join(".gitmodules").exists() { return Ok(false); }
    // Run in every submodule checked out, nested ones too.
    let each = |cmd: &str| git_out(wt, &["submodule", "foreach", "--quiet", "--recursive", cmd]).map_err(unsure);
    let mut repos: Vec<PathBuf> = each("git rev-parse --absolute-git-dir")?.lines().map(PathBuf::from).collect();
    if repos.is_empty() && !modules.is_dir() { return Ok(false); }
    let status = |s: &str| format!("status --porcelain --untracked-files=normal --ignore-submodules={s}");
    let changed = |s: &str| git_out(wt, &status(s).split(' ').collect::<Vec<_>>()).map_err(unsure).map(|o| !o.is_empty());
    if changed("all")? { return Err("uncommitted changes".into()); }
    // Each repo apart: a submodule's own `ignore` hides what changed in the ones under it from the repo above.
    if changed("none")? || !each(&format!("git {}", status("none")))?.is_empty() { return Err("uncommitted changes in its submodules".into()); }
    kept_in(&modules, &mut repos);
    let mut seen = vec![];
    for r in repos {
        let r = std::fs::canonicalize(&r).unwrap_or(r);
        if seen.contains(&r) { continue; }
        // `--work-tree`: a repo kept for a submodule no longer checked out may name a folder that is gone.
        let only_here = git_out(wt, &["--git-dir", &r.display().to_string(), "--work-tree", &wt.display().to_string(), "rev-list", "-n1", "--all", "--not", "--remotes"]).map_err(unsure)?;
        if !only_here.is_empty() { return Err(format!("commits in its submodule {} found nowhere else", name(&r, &modules, wt))); }
        seen.push(r);
    }
    Ok(true)
}
/// The repos under a git folder's `modules` (a submodule's name may hold slashes), and those of their own submodules.
fn kept_in(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(all) = std::fs::read_dir(dir) else { return };
    for p in all.flatten().map(|e| e.path()).filter(|p| p.is_dir()) {
        if p.join("HEAD").is_file() && p.join("objects").is_dir() { kept_in(&p.join("modules"), out); out.push(p); } else { kept_in(&p, out); }
    }
}
/// A submodule's repo as you would find it: its path in the worktree.
fn name(repo: &Path, modules: &Path, wt: &Path) -> String {
    let under = |base: &Path| repo.strip_prefix(std::fs::canonicalize(base).unwrap_or(base.to_path_buf())).ok().map(|p| p.display().to_string());
    under(modules).map(|n| n.replace("/modules/", "/")).or_else(|| under(wt).map(|n| n.trim_end_matches("/.git").to_string())).unwrap_or_else(|| repo.display().to_string())
}
