//! A stop ends what the agent started and keeps the agent for a reply: a command in its pane (in its group or a group
//! of its own), what runs from its worktree or temporary folder; never the agent's helper, your own checkout or a run
//! of the board's own program. What was started for it but runs elsewhere (another folder, another parent) is named
//! for you to end, on Linux and macOS alike.
use crate::common;
use common::*;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use sw::json::*;

/// A task whose agent is at work, with its pane's flags set (`child`, `helper`, `tool`).
fn working(w: &World, flags: Value) -> Obj {
    w.script(json!({ "worker": [{ "status": "working" }] }));
    let id = w.add("Long check");
    delegate(&id);
    let t = settle(&id);
    assert_eq!(t.s("stage"), "working", "{}", t.at("failure"));
    w.patch(|s| { let p = s["panes"].as_array_mut().unwrap().iter_mut().find(|p| p.s("pane_id") == t.at("link").s("pane")).unwrap(); for (k, v) in flags.as_object().unwrap() { p[k] = v.clone(); } });
    t
}
fn killed(w: &World) -> Vec<i64> { w.state().arr("killed").iter().filter_map(|v| v.as_i64()).collect() }
fn sleep_in(dir: &std::path::Path, env: &[(&str, &std::path::Path)]) -> Child {
    let mut c = Command::new("sleep");
    c.arg("60").current_dir(dir).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    for (k, v) in env { c.env(k, v); }
    c.spawn().unwrap()
}
fn ended(c: &mut Child) -> bool { (0..50).any(|_| { if c.try_wait().unwrap().is_some() { return true; } std::thread::sleep(std::time::Duration::from_millis(20)); false }) }
/// Set: this test program only waits a minute, as `sleep 60` does. What was started for the task and works elsewhere
/// is a copy of it, not `sleep`: macOS keeps the environment Apple's own programs were started with to themselves.
const HOLD: &str = "HERDR_PROJECTS_TEST_HOLD";
/// The test that waits when HOLD is set.
const HELD: &str = "a_run_of_the_boards_own_program_is_never_ended_nor_named_in_its_folders_or_started_for_it";
/// `exe` (this test program, or a copy of it) to wait a minute, started for the task: `tmp` is its temporary folder.
fn held(exe: &Path, tmp: &Path) -> Command {
    let mut c = Command::new(exe);
    c.args([format!("{}::{HELD}", module_path!().split_once("::").unwrap().1).as_str(), "--exact", "-q"]).env(HOLD, "1").env("TMPDIR", tmp);
    c.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    c
}
/// A program of the task's, outside its folders: a copy of this test program.
fn task_program(w: &World) -> PathBuf {
    let p = w.base.join("bin/dev-server");
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::copy(std::env::current_exe().unwrap(), &p).unwrap();
    p
}

#[test]
fn a_command_the_agent_runs_is_ended_and_the_agent_and_its_helper_are_kept() {
    let w = world_with(Opts { repo: true, settings: json!({ "review.when": "never" }), ..Default::default() });
    let t = working(&w, json!({ "child": true, "helper": true }));
    let said = sw::actions::talk::stop(t.s("id")).unwrap();
    assert_eq!(said.task.s("stage"), "stopped");
    assert_eq!(killed(&w), vec![3], "only the command: not the agent (1), not its MCP server (2)");
    assert_eq!(said.msg, "Ended what it started: sleep 600 (pid 3).");
    assert!(w.state().arr("panes").iter().any(|p| p.s("pane_id") == t.at("link").s("pane") && p.at("agent").is_object()), "the agent stays for a reply");
    assert_eq!(sw::actions::talk::stopped(&format!("{}: stopped", t.s("id")), &said.msg), format!("{}: stopped. Ended what it started: sleep 600 (pid 3).", t.s("id")));
}

#[test]
fn what_a_helper_runs_in_the_agents_group_is_kept_with_it_and_a_command_beside_them_is_ended() {
    let w = world_with(Opts { repo: true, settings: json!({ "review.when": "never" }), ..Default::default() });
    // `uv tool run` (2), wired to the agent, runs its MCP server (6) in the agent's group with its input on /dev/null
    let t = working(&w, json!({ "child": true, "helper": "uv" }));
    let said = sw::actions::talk::stop(t.s("id")).unwrap();
    assert_eq!(killed(&w), vec![3], "only the command: not uv (2), nor the MCP server it runs (6)");
    assert_eq!(said.msg, "Ended what it started: sleep 600 (pid 3).");
}

#[test]
fn a_dev_server_in_a_group_of_its_own_is_ended_and_a_stop_with_nothing_started_says_only_stopped() {
    let w = world_with(Opts { repo: true, settings: json!({ "review.when": "never" }), ..Default::default() });
    let t = working(&w, json!({ "tool": true }));
    let said = sw::actions::talk::stop(t.s("id")).unwrap();
    assert_eq!((killed(&w), said.msg.as_str()), (vec![2], "Ended what it started: node …/dev-server.js (pid 2)."));
    drop(w);
    let w = world_with(Opts { repo: true, settings: json!({ "review.when": "never" }), ..Default::default() });
    let t = working(&w, json!({ "helper": true }));
    let said = sw::actions::talk::stop(t.s("id")).unwrap();
    assert!(killed(&w).is_empty() && said.msg.is_empty(), "{:?} {}", killed(&w), said.msg);
    assert_eq!(sw::actions::talk::stopped(&format!("{}: stopped", t.s("id")), &said.msg), format!("{}: stopped", t.s("id")));
}

#[test]
fn what_runs_from_its_worktree_or_temporary_folder_is_ended_your_checkout_is_not_and_one_that_left_is_named() {
    let w = world_with(Opts { repo: true, settings: json!({ "review.when": "never" }), ..Default::default() });
    let t = working(&w, json!({}));
    let tmp = sw::temp::made(&t);
    let tree = std::path::PathBuf::from(t.s("worktree"));
    let mut check = sleep_in(&tree, &[]); // a check that left the agent's terminal, in its worktree
    let mut preview = sleep_in(&tmp, &[]); // a preview in its temporary folder
    let mut yours = sleep_in(&w.root, &[]); // yours, in your own checkout
    let mut away = held(&task_program(&w), &tmp).current_dir(&w.base).spawn().unwrap(); // started for the task, working elsewhere now
    let said = sw::actions::talk::stop(t.s("id")).unwrap();
    assert!(ended(&mut check) && ended(&mut preview), "{}", said.msg);
    assert!(said.msg.starts_with("Ended what it started: ") && said.msg.contains(&format!("(pid {})", check.id())) && said.msg.contains(&format!("sleep 60 (pid {})", preview.id())), "{}", said.msg);
    assert!(yours.try_wait().unwrap().is_none() && !said.msg.contains(&format!("pid {})", yours.id())), "{}", said.msg);
    assert!(away.try_wait().unwrap().is_none(), "named, not ended");
    assert!(said.msg.contains(&format!("Still running, end it yourself: {}, in {}.", sw::herdr::busy::found(away.id() as i64), w.base.display())), "{}", said.msg);
    for c in [&mut yours, &mut away] { let _ = c.kill(); let _ = c.wait(); }
}

#[test]
fn one_started_for_it_that_left_its_folder_and_its_parent_is_still_found_and_named() {
    let w = world_with(Opts { repo: true, settings: json!({ "review.when": "never" }), ..Default::default() });
    let t = working(&w, json!({}));
    let tmp = sw::temp::made(&t);
    let elsewhere = w.base.join("elsewhere");
    std::fs::create_dir_all(&elsewhere).unwrap();
    // started in its temporary folder by a shell that went elsewhere and ended: the system is its parent now
    let server = held(&task_program(&w), &tmp);
    let o = Command::new("sh").arg("-c").arg(r#"cd "$0" && { "$@" </dev/null >/dev/null 2>&1 & echo $!; }"#).arg(&elsewhere).arg(server.get_program()).args(server.get_args())
        .envs(server.get_envs().filter_map(|(k, v)| Some((k, v?)))).current_dir(&tmp).output().unwrap();
    let away: i32 = String::from_utf8_lossy(&o.stdout).trim().parse().unwrap();
    // a fork of the shell until it has started the program
    let runs = || sw::herdr::procs::command(away as i64).is_some_and(|c| c.split_whitespace().next().is_some_and(|p| p.ends_with("/dev-server")));
    let started = (0..500).any(|_| { if runs() { return true; } std::thread::sleep(std::time::Duration::from_millis(20)); false });
    assert!(started && sw::leftovers::procs::folder_of(away) == Some(elsewhere.clone()), "{:?}", sw::leftovers::procs::folder_of(away));
    assert!(!sw::leftovers::procs::in_folder(&tmp).contains(&away), "out of its folders");
    assert!(sw::leftovers::procs::started_in(&tmp).contains(&away), "found by the temporary folder it was started with");
    let said = sw::actions::talk::stop(t.s("id")).unwrap();
    assert!(sw::herdr::procs::running(away as i64), "named, not ended: {}", said.msg);
    assert!(said.msg.contains(&format!("Still running, end it yourself: {}, in {}.", sw::herdr::busy::found(away as i64), elsewhere.display())), "{}", said.msg);
    unsafe { libc::kill(away, libc::SIGKILL) };
}

#[test]
fn a_run_of_the_boards_own_program_is_never_ended_nor_named_in_its_folders_or_started_for_it() {
    if std::env::var_os(HOLD).is_some() { std::thread::sleep(std::time::Duration::from_secs(60)); return; }
    let w = world_with(Opts { repo: true, settings: json!({ "review.when": "never" }), ..Default::default() });
    let t = working(&w, json!({}));
    let tmp = sw::temp::made(&t);
    let tree = PathBuf::from(t.s("worktree"));
    let own = |dir: &Path| held(&std::env::current_exe().unwrap(), &tmp).current_dir(dir).spawn().unwrap();
    let (mut in_tree, mut away) = (own(&tree), own(&w.base)); // in its worktree; started for it, elsewhere now
    let mut check = sleep_in(&tree, &[]);
    let this = |c: &Child| sw::leftovers::procs::this_program(c.id() as i32);
    assert!(this(&in_tree) && this(&away) && sw::leftovers::procs::this_program(std::process::id() as i32) && !this(&check));
    assert!(sw::leftovers::procs::in_folder(&tree).contains(&(in_tree.id() as i32)), "it runs in the worktree");
    assert!(!sw::leftovers::procs::started_in(&tmp).iter().any(|p| [in_tree.id(), away.id()].contains(&(*p as u32))), "never among what was started for the task");
    let said = sw::actions::talk::stop(t.s("id")).unwrap();
    assert!(ended(&mut check), "{}", said.msg);
    for c in [&mut in_tree, &mut away] {
        assert!(c.try_wait().unwrap().is_none() && !said.msg.contains(&format!("pid {})", c.id())), "{}", said.msg);
        let _ = c.kill(); let _ = c.wait();
    }
}

#[test]
fn where_herdr_does_not_say_what_runs_in_its_panes_what_runs_from_its_worktree_is_named_not_ended() {
    let w = world_with(Opts { repo: true, settings: json!({ "review.when": "never" }), ..Default::default() });
    let t = working(&w, json!({ "child": true }));
    let mut shell = sleep_in(std::path::Path::new(t.s("worktree")), &[]); // could be your own shell in another of its panes
    w.patch(|s| s["paneListFails"] = json!(true));
    let said = sw::actions::talk::stop(t.s("id")).unwrap();
    assert!(shell.try_wait().unwrap().is_none(), "not ended: {}", said.msg);
    assert!(said.msg.starts_with("Ended what it started: sleep 600 (pid 3)."), "the agent's own command still goes: {}", said.msg);
    assert!(said.msg.contains(&format!("sleep 60 (pid {}), in its folders, not ended as herdr does not say what runs in its other panes", shell.id())), "{}", said.msg);
    let _ = shell.kill(); let _ = shell.wait();
    drop(w);
    let w = world_with(Opts { repo: true, settings: json!({ "review.when": "never" }), ..Default::default() });
    let t = working(&w, json!({ "child": true }));
    let mut check = sleep_in(std::path::Path::new(t.s("worktree")), &[]);
    w.patch(|s| s["processInfoFails"] = json!(true));
    let said = sw::actions::talk::stop(t.s("id")).unwrap();
    assert!(check.try_wait().unwrap().is_none() && killed(&w).is_empty(), "nothing ended: {}", said.msg);
    assert!(said.msg.starts_with("Still running, end it yourself: what runs in its agent's terminal, as herdr does not say what runs in its agent's terminal; "), "{}", said.msg);
    assert!(said.msg.contains(&format!("sleep 60 (pid {}), in its folders, not ended as herdr does not say what runs in its agent's terminal", check.id())), "{}", said.msg);
    let _ = check.kill(); let _ = check.wait();
}

#[test]
fn what_still_runs_is_named_every_one_and_what_was_ended_is_cut_short() {
    let names = |a: i32, b: i32| (a..=b).map(|p| format!("x (pid {p})")).collect::<Vec<_>>();
    let e = sw::actions::stop_ends::Ended { ended: names(1, 8), left: names(101, 108) };
    let s = e.said();
    assert!(s.starts_with("Ended what it started: x (pid 1), x (pid 2), x (pid 3), x (pid 4), x (pid 5), and 3 more."), "{s}");
    assert!((101..=108).all(|p| s.contains(&format!("x (pid {p})"))), "{s}");
}

#[test]
fn what_runs_in_your_own_pane_of_its_space_is_never_ended_also_when_herdr_cannot_be_asked_about_it() {
    let w = world_with(Opts { repo: true, settings: json!({ "review.when": "never" }), ..Default::default() });
    let t = working(&w, json!({ "child": true }));
    let tree = std::path::PathBuf::from(t.s("worktree"));
    // a pane you split off in its space, whose shell (this test's process) runs a command in the worktree
    let mine = sw::herdr::Herdr::new(None).pane_split(t.at("link").s("pane"), t.s("worktree"), "right", &[]).unwrap().t("pane_id");
    w.patch(|s| { let p = s["panes"].as_array_mut().unwrap().iter_mut().find(|p| p.s("pane_id") == mine).unwrap(); p["shell"] = json!(std::process::id()); });
    let mut yours = sleep_in(&tree, &[]);
    w.patch(|s| s["paneGetFails"] = json!(true)); // herdr cannot say whether that pane is still there
    let said = sw::actions::talk::stop(t.s("id")).unwrap();
    assert!(yours.try_wait().unwrap().is_none() && !said.msg.contains(&format!("pid {})", yours.id())), "{}", said.msg);
    assert_eq!(said.msg, "Ended what it started: sleep 600 (pid 3).");
    // nor what runs in it at all: then what runs from its folders is named, not ended
    w.patch(|s| { let p = s["panes"].as_array_mut().unwrap().iter_mut().find(|p| p.s("pane_id") == mine).unwrap(); p["infoFails"] = json!(true); });
    let said = sw::actions::talk::stop(t.s("id")).unwrap();
    assert!(yours.try_wait().unwrap().is_none(), "{}", said.msg);
    assert!(said.msg.contains(&format!("sleep 60 (pid {}), in its folders, not ended as herdr does not say what runs in its other panes", yours.id())), "{}", said.msg);
    let _ = yours.kill(); let _ = yours.wait();
}
