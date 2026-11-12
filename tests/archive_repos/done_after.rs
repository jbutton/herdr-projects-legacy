//! Done tasks archive themselves once they have been done for `archive.doneAfter` (`none` by default), from the
//! housekeeping pass, each as one archive does. Only what `archive --done` takes: a pinned task, one that works, waits
//! or is to do stays, and so does a done one whose space is still in use. A restored task counts again from its
//! restore. Ported from Shepherdr 7745118a ("Archive Done after").
use crate::common;
use common::*;
use serde_json::json;
use sw::json::*;
use sw::store::settings::{change::set_value, lookup::Scope, values::ms};

const DAY: i64 = 86_400_000;
const MIN: i64 = 60_000;

fn archived(id: &str) -> bool { task(id).b("archivedAt") }
fn mark(id: &str, fields: serde_json::Value) { sw::store::tasks::set(id, fields).unwrap(); }
/// Done now: the change of its stage is stamped with the board's clock.
fn done(id: &str) { mark(id, json!({ "stage": "done" })); }
fn after(v: &str, scope: &Scope) { set_value("archive.doneAfter", &json!(v), scope).unwrap_or_else(|e| panic!("archive.doneAfter {v}: {}", e.msg)); }
/// One housekeeping pass, as the keeper or any hook runs it, with nothing it waits for between passes.
fn pass(w: &World) {
    let _ = std::fs::remove_dir_all(sw::store::files::data_dir().join("stamps"));
    sw::housekeeping::housekeeping(&w.socket, sw::housekeeping::Force::Yes);
    sw::runner::JOBS.with(|j| j.borrow_mut().clear());
}

#[test]
fn a_done_task_archives_itself_once_it_has_been_done_that_long_and_nothing_else_does() {
    let w = world();
    sw::clock::set(Some(1_800_000_000_000));
    let ids = ["Done first", "Done later", "Pinned done", "Working", "Waiting", "To do"].map(|t| w.add(t));
    done(&ids[0]);
    done(&ids[2]); mark(&ids[2], json!({ "pinned": true }));
    mark(&ids[3], json!({ "stage": "progress" }));
    mark(&ids[4], json!({ "stage": "asks", "kind": "question", "msg": "Which one?" }));
    after("3d", &Scope::you());
    sw::clock::advance(DAY);
    done(&ids[1]);
    // A minute short of three days: nothing yet.
    sw::clock::advance(2 * DAY - MIN);
    pass(&w);
    assert!(ids.iter().all(|id| !archived(id)), "nothing has been done for 3d yet");
    // Three days done: the first goes, the one done a day later not yet.
    sw::clock::advance(2 * MIN);
    pass(&w);
    assert!(archived(&ids[0]), "done for 3d: archived");
    assert!(!archived(&ids[1]), "done for 2d: stays");
    sw::clock::advance(DAY);
    pass(&w);
    assert!(archived(&ids[1]), "its 3d are over too");
    // Restored, it counts again from its restore: not archived again at the next pass.
    sw::actions::archive::restore(&ids[0]).unwrap();
    pass(&w);
    assert!(!archived(&ids[0]), "restored: it stays for another 3d");
    sw::clock::advance(3 * DAY + MIN);
    pass(&w);
    assert!(archived(&ids[0]), "3d after its restore: archived again");
    // However long it is: a pinned done task, one at work, one waiting and one to do are never archived, and a done one
    // whose clean-up is still to come waits for it.
    let tidy = w.add("Clean-up to come");
    done(&tidy); mark(&tidy, json!({ "tidy": { "deleteBranch": "merged" } }));
    sw::clock::advance(100 * DAY);
    pass(&w);
    assert!(ids[2..].iter().all(|id| !archived(id)), "pinned, working, waiting and to do stay");
    assert!(!archived(&tidy), "its clean-up first");
}

#[test]
fn none_by_default_and_a_repo_or_a_task_can_say_its_own() {
    let w = world();
    sw::clock::set(Some(1_800_000_000_000));
    let (a, b) = (w.add("Kept by default"), w.add("Its own"));
    done(&a); done(&b);
    sw::clock::advance(100 * DAY);
    pass(&w);
    assert!(!archived(&a) && !archived(&b), "built in: none, nothing archives itself");
    // Yours says 1d, your entry for the repo none, one task 2d: the nearest answer wins.
    after("1d", &Scope::you());
    after("none", &Scope::repo(task(&a).s("repo")));
    after("2d", &Scope::task(&task(&b)));
    pass(&w);
    assert!(!archived(&a), "the repo says none");
    assert!(archived(&b), "the task says 2d");
}

#[test]
fn a_done_task_whose_space_is_in_use_waits_until_it_is_free() {
    let w = world_with(Opts { repo: true, settings: json!({ "review.when": "never", "workspace.cleanup": false }), ..Default::default() });
    sw::clock::set(Some(1_800_000_000_000));
    w.script(json!({ "worker": [{ "files": { "a.txt": "x\n" }, "commit": "a", "note": "Did it." }] }));
    let id = w.add("Merged, space kept");
    delegate(&id);
    assert_eq!(settle(&id).s("stage"), "approval");
    sw::actions::finish_line::approve(&id, None, false).unwrap();
    let t = settle(&id);
    assert_eq!(t.s("stage"), "done", "{}", t.at("failure"));
    let space = t.at("link").t("workspace");
    let open = || w.state().arr("workspaces").iter().any(|x| x.s("workspace_id") == space);
    assert!(open(), "workspace.cleanup off: its space stays");
    // A command of yours in a split pane of its space.
    let mine = sw::herdr::Herdr::new(None).pane_split(t.at("link").s("pane"), t.s("worktree"), "right", &[]).unwrap().t("pane_id");
    w.patch(|s| { let p = s["panes"].as_array_mut().unwrap().iter_mut().find(|p| p.s("pane_id") == mine).unwrap(); p["busy"] = json!(true); });
    after("1d", &Scope::you());
    sw::clock::advance(DAY + MIN);
    pass(&w);
    assert!(!archived(&id) && open(), "its space is in use: it stays");
    w.patch(|s| { let p = s["panes"].as_array_mut().unwrap().iter_mut().find(|p| p.s("pane_id") == mine).unwrap(); p["busy"] = json!(false); });
    pass(&w);
    assert!(archived(&id), "free: archived");
    assert!(!open(), "as one archive does: its space closes");
    assert!(std::path::Path::new(t.s("worktree")).exists(), "and its worktree stays");
}

#[test]
fn a_time_may_be_in_days() {
    let _w = world();
    assert_eq!(ms("3d"), 3 * DAY);
    after("3d", &Scope::you());
    assert_eq!(text(&sw::store::settings::lookup::setting("archive.doneAfter", None)), "3d");
    let refused = |v: &str| set_value("archive.doneAfter", &json!(v), &Scope::you()).map(|_| ()).expect_err(v).msg;
    assert_eq!(refused("3 days"), "archive.doneAfter is a time with a unit, such as 3d (s, m, h or d), or none, not \"3 days\". Did you mean 3d?");
    assert_eq!(refused("3"), "archive.doneAfter is a time with a unit, such as 3d (s, m, h or d), or none, not \"3\". Did you mean 3d?");
    assert_eq!(refused("never"), "archive.doneAfter is a time with a unit, such as 3d (s, m, h or d), or none, not \"never\". Did you mean none?");
}
