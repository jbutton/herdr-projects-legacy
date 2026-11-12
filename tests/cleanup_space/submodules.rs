//! A done task's worktree with a submodule checked out in it is removed like any other (git removes one only when
//! forced), also once its nested submodules were taken out again; one whose submodule holds changes (also nested ones,
//! and one saved by what clean-up ended), or commits found nowhere else, is kept. Upstream issue #857.
use crate::common;
use common::*;
use serde_json::{json, Value};
use std::path::Path;
use std::process::{Command, Stdio};
use sw::actions::finish_line;
use sw::json::*;

/// A repo beside the world's, with one file and, `with`, a submodule of its own.
fn repo(w: &World, name: &str, file: &str, with: Option<&str>) -> String {
    let r = w.base.join(name);
    std::fs::create_dir_all(&r).unwrap();
    git(&r, &["init", "-q", "-b", "main"]);
    std::fs::write(r.join(file), "s\n").unwrap();
    git(&r, &["add", "-A"]);
    if let Some(m) = with { git(&r, &["-c", "protocol.file.allow=always", "submodule", "add", "-q", m, m.rsplit('/').next().unwrap()]); }
    git(&r, &["commit", "-qm", name]);
    r.display().to_string()
}
/// The world's repo with a submodule `sub`, which has a submodule `leaf` of its own, committed on main.
fn with_submodule() -> World {
    let w = world_with(Opts { repo: true, settings: json!({ "review.when": "never" }), ..Default::default() });
    let sub = repo(&w, "sub", "s.txt", Some(&repo(&w, "leaf", "l.txt", None)));
    git(&w.root, &["-c", "protocol.file.allow=always", "submodule", "add", "-q", &sub, "sub"]);
    git(&w.root, &["commit", "-qm", "add sub"]);
    w
}
/// What a set-up step does in the worktree: its submodule checked out there (`--recursive`: the one in it too).
fn init() -> Value { json!(["-c", "protocol.file.allow=always", "submodule", "update", "--init", "-q"]) }
fn init_all() -> Value { json!(["-c", "protocol.file.allow=always", "submodule", "update", "--init", "--recursive", "-q"]) }
/// A task whose agent committed `<n>.txt`, then ran `then` (git calls) in its worktree: approved, merged and settled.
fn landed(w: &World, n: &str, then: Value) -> Obj { landed_after(w, n, then, |_| {}) }
/// The same, with `before` done in its worktree right before the approval.
fn landed_after(w: &World, n: &str, then: Value, before: impl FnOnce(&Path)) -> Obj {
    w.script(json!({ "worker": [{ "files": { format!("{n}.txt"): "x\n" }, "commit": n, "git": then, "note": "Did it." }] }));
    let id = to_approval(w, n);
    let wt = task(&id).t("worktree");
    before(Path::new(&wt));
    finish_line::approve(&id, None, false).unwrap();
    let mut t = settle(&id);
    assert_eq!(t.s("stage"), "done", "{}", t.at("failure"));
    t.put("was", wt);
    t
}
fn kept(t: &Obj) -> String { t.arr("kept").iter().map(text).collect::<Vec<_>>().join(", ") }
fn removes(w: &World) -> Vec<String> { calls(w).into_iter().filter(|c| c.starts_with("worktree remove")).collect() }

#[test]
fn a_done_tasks_worktree_with_its_submodule_checked_out_is_removed_one_without_is_not_forced() {
    let w = with_submodule();
    // as in most repos with submodules today: the worktree never checks it out, and git removes it unforced
    let a = landed(&w, "a", json!([]));
    assert_eq!(kept(&a), ""); assert!(a.b("cleaned")); assert!(!Path::new(a.s("was")).exists());
    assert_eq!(removes(&w).len(), 1); assert!(!removes(&w)[0].contains("--force"), "{:?}", removes(&w));
    // a set-up step checked it out: git refuses that unforced ("working trees containing submodules cannot be moved or
    // removed"), so it is forced once nothing in it would be lost
    let b = landed(&w, "b", json!([init()]));
    assert_eq!(kept(&b), ""); assert!(b.b("cleaned"));
    assert!(!Path::new(b.s("was")).exists(), "{} is still there", b.s("was"));
    assert!(!git(&w.root, &["worktree", "list"]).contains(b.s("was")));
    assert!(removes(&w).last().unwrap().ends_with("--force"), "{:?}", removes(&w));
}

#[test]
fn a_worktree_whose_submodule_holds_a_commit_found_nowhere_else_is_kept() {
    let w = with_submodule();
    // a commit made in the submodule and pinned by the task: only the worktree's own copy of the submodule has it
    let a = landed(&w, "a", json!([init(), ["-C", "sub", "commit", "-q", "--allow-empty", "-m", "only here"], ["add", "sub"], ["commit", "-qm", "pin"]]));
    assert!(kept(&a).contains("(commits in its submodule sub found nowhere else)"), "{}", kept(&a));
    assert!(Path::new(a.s("was")).join("sub/s.txt").exists());
    assert!(removes(&w).iter().all(|c| !c.contains("--force")), "{:?}", removes(&w));
}

#[test]
fn a_worktree_whose_submodule_holds_a_change_git_status_is_told_to_leave_out_is_kept() {
    let w = with_submodule();
    // `ignore = all`: git status says nothing of the submodule, and a forced remove would take its change with it
    let a = landed(&w, "a", json!([init(), ["config", "submodule.sub.ignore", "all"], ["-C", "sub", "mv", "s.txt", "t.txt"]]));
    assert_eq!(git(Path::new(a.s("was")), &["status", "--porcelain"]), "");
    assert!(kept(&a).contains("(uncommitted changes in its submodules)"), "{}", kept(&a));
    assert!(Path::new(a.s("was")).join("sub/t.txt").exists());
    assert!(removes(&w).iter().all(|c| !c.contains("--force")), "{:?}", removes(&w));
}

#[test]
fn a_change_in_a_nested_submodule_its_parent_tells_git_status_to_leave_out_is_kept() {
    let w = with_submodule();
    // `sub` leaves `leaf` out of its status: the worktree's own status, with nothing left out, still says nothing
    let a = landed(&w, "a", json!([init_all(), ["-C", "sub", "config", "submodule.leaf.ignore", "all"], ["-C", "sub/leaf", "mv", "l.txt", "m.txt"]]));
    assert!(kept(&a).contains("(uncommitted changes in its submodules)"), "kept: {}; cleaned: {}", kept(&a), a.b("cleaned"));
    assert_eq!(git(Path::new(a.s("was")), &["status", "--porcelain", "--ignore-submodules=none"]), "");
    assert!(Path::new(a.s("was")).join("sub/leaf/m.txt").exists());
}

#[test]
fn a_clean_worktree_whose_nested_submodules_were_taken_out_again_is_removed() {
    let w = with_submodule();
    // `deinit`: nothing is checked out any more, but git keeps their repos for the worktree, the nested one pointing at
    // a folder that is gone; git still removes it only when forced
    let a = landed(&w, "a", json!([init_all(), ["submodule", "deinit", "-q", "-f", "--all"]]));
    assert_eq!(kept(&a), ""); assert!(a.b("cleaned"));
    assert!(!Path::new(a.s("was")).exists(), "{} is still there", a.s("was"));
    assert!(removes(&w).last().unwrap().ends_with("--force"), "{:?}", removes(&w));
}

#[test]
fn a_file_saved_by_what_clean_up_ends_right_before_the_worktree_goes_is_kept() {
    let w = with_submodule();
    // a dev server left running in it saves a tracked file when it is told to end, after the worktree was looked at
    let mut server = None;
    let a = landed_after(&w, "a", json!([init()]), |wt| {
        let trap = "trap 'echo saved > a.txt; exit 0' TERM; while :; do sleep 1 & wait; done";
        server = Some(Command::new("sh").args(["-c", trap]).current_dir(wt).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn().unwrap());
    });
    let mut server = server.unwrap();
    let gone = (0..50).any(|_| { if server.try_wait().unwrap().is_some() { return true; } std::thread::sleep(std::time::Duration::from_millis(20)); false });
    if !gone { let _ = server.kill(); let _ = server.wait(); }
    assert!(gone, "{}", a.s("msg"));
    assert!(kept(&a).contains("(uncommitted changes)"), "kept: {}; cleaned: {}", kept(&a), a.b("cleaned"));
    assert_eq!(read(&Path::new(a.s("was")).join("a.txt")), "saved\n");
}
