//! The push of a task's branch runs in what the repo's own tool manager gives its folder: a repo with a `.tool-versions`
//! or a `mise.toml` has its pre-push hook run by the tools it names (`mise env`), as its agents push (`mise exec -- git
//! push`), whatever PATH the board's process has. A repo without one, or a mise that cannot say, pushes as before.
use crate::common;
use super::{first_on_path, script, PathBack};
use common::*;
use serde_json::json;
use std::path::{Path, PathBuf};
use sw::json::*;

/// A mise on PATH that notes where and how it was asked. `mise env --json` in a folder with a tool file gives the
/// tools' folder first on PATH, as the real one does; `works: false` is a mise that fails (an untrusted mise.toml).
fn fake_mise(w: &World, works: bool) -> PathBack {
    let (bin, tools, log) = (w.base.join("bin"), w.base.join("tools"), w.base.join("mise-calls"));
    let answer = if works { format!("if [ -f .tool-versions ] || [ -f mise.toml ]; then printf '{{\"PATH\":\"%s:%s\",\"TOOLS_ON\":\"1\"}}\\n' \"{}\" \"$PATH\"; else echo '{{}}'; fi", tools.display()) } else { "echo 'mise ERROR Config files are not trusted.' >&2; exit 1".into() };
    script(&bin.join("mise"), &format!("#!/bin/sh\necho \"$PWD $*\" >> \"{}\"\n{answer}\n", log.display()));
    // The repo's interpreter, which only the tool manager's PATH has (Ruby for an overcommit hook).
    script(&tools.join("herdr-test-ruby"), &format!("#!/bin/sh\necho \"$TOOLS_ON\" >> \"{}\"\n", w.base.join("hook-runs").display()));
    first_on_path(&bin)
}
/// The repo names its tools in `file` and its pre-push hook needs the repo's interpreter, as chargify's overcommit does.
fn repo_with_tools(w: &World, file: Option<&str>) {
    if let Some(f) = file {
        std::fs::write(w.root.join(f), "ruby 3.3.8\n").unwrap();
        git(&w.root, &["add", "-A"]); git(&w.root, &["commit", "-qm", "tools"]); git(&w.root, &["push", "-q", "origin", "main"]);
    }
    script(&w.root.join(".git/hooks/pre-push"), "#!/usr/bin/env herdr-test-ruby\n");
}
fn mise_calls(w: &World) -> Vec<String> { read(&w.base.join("mise-calls")).lines().map(String::from).collect() }
fn hook_runs(w: &World) -> Vec<String> { read(&w.base.join("hook-runs")).lines().map(String::from).collect() }
fn pr_world() -> World { world_with(Opts { repo: true, origin: true, settings: json!({ "review.when": "never" }), ..Default::default() }) }
fn pushed(w: &World, t: &Obj) -> bool { git(&w.root, &["ls-remote", "--heads", "origin", t.s("branch")]).split('\t').next() == Some(&git(Path::new(t.s("worktree")), &["rev-parse", "HEAD"])) }

#[test]
fn a_repo_that_names_its_tools_has_its_pre_push_hook_run_by_them() {
    for file in [".tool-versions", "mise.toml"] {
        let w = pr_world();
        let _path = fake_mise(&w, true);
        repo_with_tools(&w, Some(file));
        let id = to_approval(&w, "With tools");
        let t = task(&id);
        assert!(t.at("failure").is_null(), "{file}: {}", t.at("failure"));
        assert!(pushed(&w, &t), "{file}");
        assert_eq!(hook_runs(&w), vec!["1"], "{file}: the hook ran once, with the tools' environment");
        let tree = PathBuf::from(t.s("worktree"));
        assert!(mise_calls(&w).contains(&format!("{} env --json", tree.display())), "{file}: {:?}", mise_calls(&w));
    }
}

#[test]
fn a_repo_without_a_tool_file_or_a_mise_that_cannot_say_pushes_as_before() {
    // No tool file: mise is never asked, and the board's own PATH runs the hook (here it has no such interpreter).
    let w = pr_world();
    let _path = fake_mise(&w, true);
    repo_with_tools(&w, None);
    w.script(json!({ "worker": [step(1)] }));
    let id = w.add("No tools");
    delegate(&id);
    let t = settle(&id);
    assert!(mise_calls(&w).is_empty(), "{:?}", mise_calls(&w));
    assert_eq!(t.at("failure").s("title"), "Could not push the branch", "{}", t.at("failure"));
    assert!(t.at("failure").s("detail").contains("herdr-test-ruby"), "{}", t.at("failure").s("detail"));
    drop(_path);
    drop(w);
    // A mise that fails (an untrusted mise.toml): the push goes on as it would have without it.
    let w = pr_world();
    let _path = fake_mise(&w, false);
    repo_with_tools(&w, Some(".tool-versions"));
    std::fs::write(w.root.join(".git/hooks/pre-push"), "#!/bin/sh\necho plain >> \"$HOME/../hook-runs\"\n").unwrap();
    let id = to_approval(&w, "Mise fails");
    let t = task(&id);
    assert!(pushed(&w, &t));
    assert_eq!(mise_calls(&w).len(), 1);
    assert_eq!(hook_runs(&w), vec!["plain"]);
}
