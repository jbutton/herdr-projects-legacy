//! A fork's checkout, with an `upstream` remote, which gh takes for its default repo: every gh call names the task's own
//! repo, origin's (a pull request by its link, the rest with `--repo` or in the API path), so the board never reads,
//! merges or edits upstream's pull request of the same number.
use crate::common;
use common::gh::names_ours;
use common::*;
use serde_json::json;
use sw::actions::finish_line;
use sw::housekeeping::poll_prs;
use sw::json::*;

/// Origin is o/r on GitHub (its pushes go to the world's bare repo, through `insteadOf`), and `upstream` is up/r.
fn fork_world() -> World {
    let w = world_with(Opts { repo: true, origin: true, settings: json!({ "review.when": "never" }), ..Default::default() });
    let bare = w.base.join("origin.git").display().to_string();
    git(&w.root, &["config", &format!("url.{bare}.insteadOf"), "https://github.com/o/r.git"]);
    git(&w.root, &["remote", "set-url", "origin", "https://github.com/o/r.git"]);
    git(&w.root, &["remote", "add", "upstream", "https://github.com/up/r.git"]);
    w.gh_says(json!({ "fork": true }));
    w
}
/// Every gh call from the `from`th on that names no repo of its own: gh would have asked up/r.
fn unnamed(w: &World, from: usize) -> Vec<String> {
    w.gh.log.borrow().iter().skip(from).filter(|l| !names_ours(&l.split_whitespace().map(String::from).collect::<Vec<_>>())).cloned().collect()
}

#[test]
fn a_forks_pull_request_opens_checks_and_merges_on_origins_repo_never_upstreams() {
    let w = fork_world();
    let id = to_approval(&w, "In a fork");
    let t = task(&id);
    assert_eq!(t.s("pr"), "https://github.com/o/r/pull/7");
    assert!(w.gh.log.borrow().iter().any(|l| l.starts_with("pr create ") && l.ends_with(" --repo o/r")), "{}", gh_log(&w));
    finish_line::approve(&id, None, false).unwrap();
    let t = settle(&id);
    assert_eq!(t.s("stage"), "done", "{}", t.at("failure"));
    assert_eq!(t.s("msg"), "Merged: https://github.com/o/r/pull/7");
    assert!(gh_log(&w).contains("pr merge https://github.com/o/r/pull/7 --squash"), "{}", gh_log(&w));
    // GitHub's checks were asked of o/r too: the branch's protection, an outside CI.
    assert!(gh_log(&w).contains("api repos/o/r/branches/main") && gh_log(&w).contains("api repos/o/r/commits/main/check-runs"), "{}", gh_log(&w));
    assert_eq!(unnamed(&w, 0), Vec::<String>::new());
}

#[test]
fn upstreams_merged_pull_request_of_the_same_number_does_not_mark_the_forks_task_done() {
    let w = fork_world();
    // The pull request was opened while gh still named o/r by itself (`gh repo set-default`, since undone).
    w.gh_says(json!({}));
    let id = to_approval(&w, "Same number");
    assert_eq!(task(&id).s("pr"), "https://github.com/o/r/pull/7");
    w.gh_says(json!({ "fork": true }));
    let before = w.gh.log.borrow().len();
    poll_prs();
    let t = task(&id);
    assert_eq!(t.s("stage"), "approval", "up/r's pull request 7 is not this task's: {}", t.s("msg"));
    assert!(!t.b("prMerged"), "{}", t.s("msg"));
    assert_eq!(unnamed(&w, before), Vec::<String>::new());
    // Once merged on o/r, it is.
    w.gh_says(json!({ "fork": true, "view": { "state": "MERGED" } }));
    poll_prs();
    let t = task(&id);
    assert_eq!(t.s("stage"), "done"); assert_eq!(t.s("msg"), "Merged: https://github.com/o/r/pull/7");
}

#[test]
fn origins_repo_is_read_from_the_url_git_uses_else_from_the_one_written() {
    use sw::store::github::of_origin;
    let w = fork_world();
    let named = || of_origin(&w.root).map(|r| r.arg());
    // Rewritten to a folder (`insteadOf`): the URL written names it.
    assert_eq!(named(), Some("o/r".into()));
    // Written short and rewritten to GitHub: the URL git uses names it.
    git(&w.root, &["config", "url.https://github.com/.insteadOf", "gh:"]);
    git(&w.root, &["remote", "set-url", "origin", "gh:o2/r2"]);
    assert_eq!(named(), Some("o2/r2".into()));
    // Upstream is never taken for it: no origin, no repo.
    git(&w.root, &["remote", "remove", "origin"]);
    assert_eq!(named(), None);
}

#[test]
fn a_title_that_reads_like_the_repo_option_still_opens_the_pull_request_on_origins_repo() {
    let w = fork_world();
    let id = to_approval(&w, "--repo");
    assert_eq!(task(&id).s("pr"), "https://github.com/o/r/pull/7");
    assert_eq!(unnamed(&w, 0), Vec::<String>::new());
}

#[test]
fn an_origin_that_names_no_repo_on_github_beside_another_remote_asks_gh_nothing_about_a_repo() {
    use sw::store::github::for_gh;
    // Origin is a folder, and `upstream` is on GitHub: gh would take upstream.
    let w = world_with(Opts { repo: true, origin: true, settings: json!({ "review.when": "never" }), ..Default::default() });
    git(&w.root, &["remote", "add", "upstream", "https://github.com/up/r.git"]);
    w.gh_says(json!({ "fork": true }));
    w.script(json!({ "worker": [step(1)] }));
    let id = w.add("Folder origin");
    delegate(&id);
    let f = settle(&id).at("failure").clone();
    assert_eq!(f.s("title"), "Could not open the pull request", "{f}");
    assert!(f.s("detail").contains("gh was not asked: origin names no repo on GitHub") && f.s("detail").contains("(upstream)"), "{f}");
    assert_eq!(unnamed(&w, 0), Vec::<String>::new());
    let args = |s: &str| s.split(' ').map(String::from).collect::<Vec<_>>();
    assert!(for_gh(&args("repo view --json nameWithOwner"), &w.root).is_err());
    assert_eq!(for_gh(&args("api user"), &w.root), Ok(args("api user")), "about no repo");
    // Origin the only remote: gh has none to take but origin.
    git(&w.root, &["remote", "remove", "upstream"]);
    assert_eq!(for_gh(&args("pr list --head b"), &w.root), Ok(args("pr list --head b")));
}
