//! A run cancelled for a newer run of its workflow fails nothing (#781): a workflow with `concurrency` and
//! `cancel-in-progress` started again on the same commit (a label added, the pull request edited) cancels the run still
//! going, and GitHub keeps both. Each check is judged by its newest run, in GitHub's list, read from Actions (a
//! fine-grained token) and in the runs of the branch's workflows. A cancelled newest run still fails, and so does a
//! cancelled check with no newer run of its own.
use crate::common;
use crate::pull_requests_checks::{merged, none, pane_prompts, pr_world};
use common::*;
use serde_json::{json, Value};
use sw::actions::finish_line;
use sw::housekeeping::poll_prs;
use sw::json::*;

/// The job `job` of the workflow `flow` in its run `run`, from `from` to `to` (minutes and seconds past ten).
fn job(flow: &str, job: &str, run: u64, conclusion: &str, from: &str, to: &str) -> Value {
    json!({ "__typename": "CheckRun", "name": job, "workflowName": flow, "status": "COMPLETED", "conclusion": conclusion,
        "detailsUrl": format!("https://github.com/o/r/actions/runs/{run}/job/{}", run * 10),
        "startedAt": format!("2026-11-10T10:{from}Z"), "completedAt": format!("2026-11-10T10:{to}Z") })
}
fn rollup_of(head: &str, checks: Value) -> Value { json!({ "state": "OPEN", "headRefOid": head, "statusCheckRollup": checks, "comments": [], "reviews": [], "author": { "login": "me" } }) }
/// Quality's run for the label cancelled its run for the opening while it was still going (the cancelled job ended after
/// the new one began); Safety net's run for the edit cancelled its earlier one before its job began.
fn superseded() -> Value {
    let mut never_began = job("Safety net", "pull_request_safety_net", 102, "CANCELLED", "02:00", "02:01");
    never_began["startedAt"] = json!("0001-01-01T00:00:00Z");
    json!([job("Quality", "quality_check", 100, "CANCELLED", "00:00", "01:10"), job("Quality", "quality_check", 101, "SUCCESS", "01:05", "06:00"),
        never_began, job("Safety net", "pull_request_safety_net", 103, "SUCCESS", "02:02", "03:00")])
}
const QUALITY: &str = ".github/workflows/quality.yml";
const FLOW: &str = "name: Quality\non:\n  pull_request:\njobs:\n  quality_check:\n    runs-on: ubuntu-24.04\n";
fn run(id: u64, conclusion: &str) -> Value { json!([QUALITY, "completed", conclusion, format!("https://github.com/o/r/actions/runs/{id}"), "Quality"]) }
/// A task whose branch has the workflow Quality, at its approval.
fn with_quality(w: &World, title: &str) -> (String, String) {
    w.script(json!({ "worker": [{ "files": { "f.txt": "v1\n", QUALITY: FLOW }, "commit": "v1", "note": "Did v1." }] }));
    let id = to_approval(w, title);
    let head = task(&id).t("prHead");
    (id, head)
}

#[test]
fn a_run_cancelled_by_a_newer_run_of_its_workflow_does_not_wake_the_agent_with_checks_failed() {
    let w = pr_world(json!({}));
    let id = to_approval(&w, "Labeled");
    let head = task(&id).t("prHead");
    let before = pane_prompts(&w, &id).len();
    w.gh_says(json!({ "view": rollup_of(&head, superseded()) }));
    poll_prs();
    let t = task(&id);
    assert_eq!(pane_prompts(&w, &id).len(), before, "the agent was woken: {:?}", pane_prompts(&w, &id).last());
    assert!(t.at("prFailure").is_null(), "{}", t.at("prFailure"));
    assert_eq!((t.s("stage"), t.s("prState")), ("approval", "passing"));
}

#[test]
fn a_cancelled_newest_run_still_fails_and_so_does_one_with_no_newer_run_of_its_own() {
    for (checks, failed) in [
        // the newest run was cancelled: the older run's pass says nothing of it
        (json!([job("Quality", "quality_check", 100, "SUCCESS", "00:00", "01:00"), job("Quality", "quality_check", 101, "CANCELLED", "01:05", "01:10")]), "quality_check"),
        // cancelled, and nothing of it ran again (another job did)
        (json!([job("Quality", "quality_check", 100, "CANCELLED", "00:00", "01:00"), job("Quality", "lint", 101, "SUCCESS", "01:05", "02:00")]), "quality_check"),
        // another workflow's job of the same name is another check
        (json!([job("Quality", "check", 100, "CANCELLED", "00:00", "01:00"), job("Safety net", "check", 101, "SUCCESS", "01:05", "02:00")]), "check"),
    ] {
        let w = pr_world(json!({}));
        let id = to_approval(&w, "Cancelled");
        let head = task(&id).t("prHead");
        w.gh_says(json!({ "view": rollup_of(&head, checks.clone()) }));
        poll_prs();
        let t = task(&id);
        assert_eq!(strings(t.at("prFailure").at("checks")), vec![failed.to_string()], "{checks}");
        let last = pane_prompts(&w, &id).last().cloned().unwrap_or_default();
        assert!(last.contains(&format!("Checks failed on the pull request https://github.com/o/r/pull/7: {failed}.")), "{last}");
    }
}

#[test]
fn a_merge_waits_for_no_run_its_workflow_cancelled_for_a_newer_one_and_a_cancelled_newest_run_stops_it() {
    // GitHub's list, and the runs of the branch's workflow Quality on the commit: the newer run passed, it merges
    let w = pr_world(json!({}));
    let (id, head) = with_quality(&w, "Superseded");
    w.gh_says(json!({ "view": rollup_of(&head, superseded()), "runs": [run(100, "cancelled"), run(101, "success")] }));
    finish_line::approve(&id, None, false).unwrap();
    let t = settle(&id);
    assert_eq!(t.s("stage"), "done", "{} {}", t.t("msg"), t.at("prFailure")); assert!(merged(&w), "{}", gh_log(&w));
    drop(w); // one world at a time: each holds the tests' one lock
    // the newer run was cancelled: nothing merges
    let w = pr_world(json!({ "review.rounds": 0 }));
    let (id, head) = with_quality(&w, "Cancelled last");
    let checks = json!([job("Quality", "quality_check", 100, "SUCCESS", "00:00", "01:00"), job("Quality", "quality_check", 101, "CANCELLED", "01:05", "01:10")]);
    w.gh_says(json!({ "view": rollup_of(&head, checks), "runs": [run(100, "success"), run(101, "cancelled")] }));
    finish_line::approve(&id, None, false).unwrap();
    let t = settle(&id);
    assert_eq!(t.at("prFailure").s("remoteKind"), "checks", "{}", t.t("msg")); assert!(!merged(&w));
}

#[test]
fn read_from_actions_a_run_cancelled_for_a_newer_one_fails_nothing_and_a_cancelled_newest_run_still_does() {
    // a fine-grained token: the commit's runs and their jobs, with no times, the later run the higher number
    let actions = |head: &str, first: &str, then: &str| {
        let jobs = |id: u64, c: &str| json!([["quality_check", "completed", c, format!("https://github.com/o/r/actions/runs/{id}/job/{}", id * 10)]]);
        json!({ "noChecks": true, "view": none(head), "statuses": [], "runs": [run(5, first), run(6, then)], "jobs": { "5": jobs(5, first), "6": jobs(6, then) } })
    };
    let w = pr_world(json!({}));
    let (id, head) = with_quality(&w, "Superseded");
    w.gh_says(actions(&head, "cancelled", "success"));
    finish_line::approve(&id, None, false).unwrap();
    let t = settle(&id);
    assert_eq!(t.s("stage"), "done", "{} {}", t.t("msg"), t.at("prFailure")); assert!(merged(&w), "{}", gh_log(&w));
    drop(w);
    let w = pr_world(json!({ "review.rounds": 0 }));
    let (id, head) = with_quality(&w, "Cancelled last");
    w.gh_says(actions(&head, "success", "cancelled"));
    finish_line::approve(&id, None, false).unwrap();
    let t = settle(&id);
    assert_eq!(t.at("prFailure").s("remoteKind"), "checks", "{}", t.t("msg")); assert!(!merged(&w));
}
