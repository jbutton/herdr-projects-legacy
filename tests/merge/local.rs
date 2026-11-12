//! Landing in your checkout with your own work in it: git alone says what is at risk. A stray, changed or staged file
//! the merge does not touch stays as it was and out of its commit, under every `merge.method`; a file it would
//! overwrite (changed, untracked or ignored there) stops it by name, with your checkout as it was and nothing stashed,
//! also when it is first brought up to origin (`merge.syncRemote`).
use crate::common;
use common::*;
use serde_json::json;
use sw::actions::finish_line;
use sw::json::*;

fn plain(method: &str) -> World { world_with(Opts { repo: true, settings: json!({ "review.when": "never", "merge.method": method }), ..Default::default() }) }
fn head(w: &World) -> String { git(&w.root, &["rev-parse", "HEAD"]) }
fn ignore(w: &World, file: &str) { std::fs::write(w.root.join(".git/info/exclude"), format!("{file}\n")).unwrap(); }

/// Your own work beside a task that adds f.txt: an untracked file, an edit, a staged file and an ignored one.
fn lands_beside_your_work(method: &str) {
    let w = plain(method);
    let id = to_approval(&w, "Work");
    let reviewed = git(std::path::Path::new(task(&id).s("worktree")), &["rev-parse", "HEAD"]);
    std::fs::write(w.root.join("stray.txt"), "stray\n").unwrap();
    std::fs::write(w.root.join("a.txt"), "one\nmine\n").unwrap();
    std::fs::write(w.root.join("staged.txt"), "staged\n").unwrap(); git(&w.root, &["add", "staged.txt"]);
    ignore(&w, "notes.local"); std::fs::write(w.root.join("notes.local"), "notes\n").unwrap();
    let before = head(&w);
    finish_line::approve(&id, None, false).unwrap();
    let t = settle(&id);
    assert_eq!(t.s("stage"), "done", "{method}: {}", t.at("failure"));
    assert_eq!(t.s("msg"), "Merged into main.", "{method}");
    assert_eq!(read(&w.root.join("f.txt")), "v1\n", "{method}");
    for (f, body) in [("stray.txt", "stray\n"), ("a.txt", "one\nmine\n"), ("staged.txt", "staged\n"), ("notes.local", "notes\n")] { assert_eq!(read(&w.root.join(f)), body, "{method}: {f}"); }
    assert_eq!(git(&w.root, &["diff", "--name-only", &before, "HEAD"]), "f.txt", "{method}: only the work landed, none of yours");
    assert_eq!(git(&w.root, &["diff", "--cached", "--name-only"]), "staged.txt", "{method}: still staged, not committed");
    assert_eq!(git(&w.root, &["diff", "--name-only"]), "a.txt", "{method}: still your edit");
    let parents = git(&w.root, &["rev-list", "--parents", "-n1", "HEAD"]).split(' ').count() - 1;
    match method {
        "rebase" => assert_eq!(head(&w), reviewed, "the reviewed commit itself"),
        "merge" => assert_eq!((parents, git(&w.root, &["rev-parse", "HEAD^2"])), (2, reviewed)),
        _ => assert_eq!((parents, git(&w.root, &["rev-parse", "HEAD^"])), (1, before)),
    }
    assert_eq!(t.s("mergeCommit"), head(&w), "{method}");
}

#[test]
fn a_stray_changed_staged_or_ignored_file_of_yours_leaves_a_squash_merge_alone_and_stays_out_of_its_commit() { lands_beside_your_work("squash"); }

#[test]
fn so_it_does_for_a_merge_commit() { lands_beside_your_work("merge"); }

#[test]
fn and_for_the_reviewed_commit_itself() { lands_beside_your_work("rebase"); }

#[test]
fn a_file_the_merge_would_overwrite_stops_it_by_name_and_your_copy_is_kept_nothing_stashed() {
    let w = plain("squash");
    w.script(json!({ "worker": [{ "files": { "a.txt": "agent\n", "f.txt": "v1\n" }, "commit": "v1", "note": "Did v1." }] }));
    let id = to_approval(&w, "Work");
    // git would stash your work for the merge and put it back on top, if it were let
    git(&w.root, &["config", "merge.autoStash", "true"]);
    std::fs::write(w.root.join("a.txt"), "mine\n").unwrap();
    std::fs::write(w.root.join("f.txt"), "yours\n").unwrap();
    std::fs::write(w.root.join("stray.txt"), "stray\n").unwrap();
    let before = head(&w);
    finish_line::approve(&id, None, false).unwrap();
    let t = settle(&id);
    assert_eq!(t.s("stage"), "approval");
    let f = t.at("failure");
    assert_eq!(f.s("title"), "Merge would overwrite your copy of a.txt and f.txt", "{f}");
    assert!(f.s("detail").contains("\n\ta.txt\n\tf.txt\n"), "{}", f.s("detail"));
    assert!(f.s("detail").contains("Nothing was merged, and nothing in your checkout was changed."), "{}", f.s("detail"));
    assert_eq!(f.at("agentCanFix"), &json!(false));
    let e = finish_line::send_failure(&id).unwrap_err().msg;
    assert!(e.contains("Commit, stash, move or remove your own copy"), "{e}");
    assert_eq!(head(&w), before);
    for (f, body) in [("a.txt", "mine\n"), ("f.txt", "yours\n"), ("stray.txt", "stray\n")] { assert_eq!(read(&w.root.join(f)), body, "{f}"); }
    assert_eq!(git(&w.root, &["stash", "list"]), "", "nothing stashed");
    // you put yours away: approved again, it lands, the stray file still beside it
    git(&w.root, &["checkout", "--", "a.txt"]); std::fs::remove_file(w.root.join("f.txt")).unwrap();
    finish_line::approve(&id, None, false).unwrap();
    let t = settle(&id);
    assert_eq!(t.s("stage"), "done", "{}", t.at("failure"));
    for (f, body) in [("a.txt", "agent\n"), ("f.txt", "v1\n"), ("stray.txt", "stray\n")] { assert_eq!(read(&w.root.join(f)), body, "{f}"); }
}

#[test]
fn a_file_your_checkout_ignores_is_never_overwritten_by_one_the_work_adds() {
    let w = plain("squash");
    w.script(json!({ "worker": [{ "files": { "local.env": "TASK\n" }, "commit": "env", "note": "Added local.env." }] }));
    let id = to_approval(&w, "Work");
    ignore(&w, "local.env");
    std::fs::write(w.root.join("local.env"), "HUMAN\n").unwrap();
    let before = head(&w);
    finish_line::approve(&id, None, false).unwrap();
    let t = settle(&id);
    assert_eq!(t.at("failure").s("title"), "Merge would overwrite your copy of local.env", "{}", t.at("failure"));
    assert_eq!((read(&w.root.join("local.env")), head(&w)), ("HUMAN\n".to_string(), before));
}

/// Someone else pushes to main at origin, and your checkout does not pull it.
fn they_push(w: &World, file: &str, body: &str) -> String {
    let other = w.base.join("other");
    git(&w.base, &["clone", "-q", &w.base.join("origin.git").display().to_string(), &other.display().to_string()]);
    std::fs::write(other.join(file), body).unwrap(); git(&other, &["add", "-A"]); git(&other, &["commit", "-qm", "theirs"]); git(&other, &["push", "-q", "origin", "main"]);
    git(&other, &["rev-parse", "HEAD"])
}

#[test]
fn brought_up_to_origin_first_a_stray_file_stays_and_one_origin_adds_over_yours_stops_it_by_name() {
    let w = world_with(Opts { repo: true, origin: true, settings: json!({ "review.when": "never", "merge.pr": false, "merge.syncRemote": true }), ..Default::default() });
    let id = to_approval(&w, "Work");
    let theirs = they_push(&w, "g.txt", "theirs\n");
    std::fs::write(w.root.join("stray.txt"), "stray\n").unwrap();
    ignore(&w, "g.txt"); std::fs::write(w.root.join("g.txt"), "mine\n").unwrap();
    let before = head(&w);
    finish_line::approve(&id, None, false).unwrap();
    let t = settle(&id);
    assert_eq!(t.at("failure").s("title"), "Merge would overwrite your copy of g.txt", "{}", t.at("failure"));
    assert!(t.at("failure").s("detail").contains("origin/main"), "{}", t.at("failure").s("detail"));
    assert_eq!((read(&w.root.join("g.txt")), head(&w)), ("mine\n".to_string(), before));
    std::fs::remove_file(w.root.join("g.txt")).unwrap();
    finish_line::approve(&id, None, false).unwrap();
    let t = settle(&id);
    assert_eq!(t.s("stage"), "done", "{}", t.at("failure"));
    assert_eq!(git(&w.root, &["rev-parse", "HEAD^"]), theirs);
    for (f, body) in [("f.txt", "v1\n"), ("g.txt", "theirs\n"), ("stray.txt", "stray\n")] { assert_eq!(read(&w.root.join(f)), body, "{f}"); }
}
