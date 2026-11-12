//! Each GitHub Actions check is judged by its workflow's newest run on the commit, whatever an older run said (#781): a
//! workflow with `concurrency` and `cancel-in-progress` started again on the same commit (a label added, the pull
//! request edited) cancels the run still going, and GitHub keeps both; a run that failed or still goes is outrun the
//! same way. In GitHub's list, read from Actions (a fine-grained token) and in the runs of the branch's workflows. The
//! newest is the job GitHub made last (its number): a job cancelled before it began is older than its replacement
//! however late its cancel ended, and an older run's job run again after a newer run's is newer. A newest run that
//! failed or was cancelled still fails, one not begun yet is waited for, and a cancelled check with no newer run of its
//! own still fails.
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
/// Quality's job in run `run`, still going since `from`, or not begun yet (gh's `0001-…` for no time).
fn going(run: u64, from: &str) -> Value {
    let mut c = job("Quality", "quality_check", run, "", "00:00", "00:00");
    c["status"] = json!(if from.is_empty() { "QUEUED" } else { "IN_PROGRESS" });
    c["startedAt"] = json!(if from.is_empty() { "0001-01-01T00:00:00Z".to_string() } else { format!("2026-11-10T10:{from}Z") });
    c["completedAt"] = json!("0001-01-01T00:00:00Z");
    c
}
fn rollup_of(head: &str, checks: Value) -> Value { json!({ "state": "OPEN", "headRefOid": head, "statusCheckRollup": checks, "comments": [], "reviews": [], "author": { "login": "me" } }) }
/// Quality's run for the label cancelled its run for the opening while it was still going (the cancelled job ended after
/// the new one began); Safety net's run for the edit cancelled its earlier one before its job began.
fn cancelled() -> Value {
    let mut never_began = job("Safety net", "pull_request_safety_net", 102, "CANCELLED", "02:00", "02:01");
    never_began["startedAt"] = json!("0001-01-01T00:00:00Z");
    json!([job("Quality", "quality_check", 100, "CANCELLED", "00:00", "01:10"), job("Quality", "quality_check", 101, "SUCCESS", "01:05", "06:00"),
        never_began, job("Safety net", "pull_request_safety_net", 103, "SUCCESS", "02:02", "03:00")])
}
/// The same job, numbered `number` by GitHub (run again after a newer run: a number of its own, higher).
fn numbered(mut c: Value, run: u64, number: u64) -> Value { c["detailsUrl"] = json!(format!("https://github.com/o/r/actions/runs/{run}/job/{number}")); c }
/// Quality's run 100 cancelled before its job began, the cancel ending after the job of run 101 that replaced it began.
fn cancelled_before_it_began() -> Value {
    let mut never_began = job("Quality", "quality_check", 100, "CANCELLED", "00:00", "01:10");
    never_began["startedAt"] = json!("0001-01-01T00:00:00Z");
    json!([never_began, job("Quality", "quality_check", 101, "SUCCESS", "01:05", "06:00")])
}
/// Quality's run 100 run again (a job numbered 1020) after run 101's job failed: its pass is the newest.
fn run_again_after_it() -> Value { json!([job("Quality", "quality_check", 101, "FAILURE", "01:05", "02:00"), numbered(job("Quality", "quality_check", 100, "SUCCESS", "08:00", "09:00"), 100, 1020)]) }
/// An older run of Quality that failed, or still goes, then a newer one that passed.
fn failed_then_passed() -> Value { json!([job("Quality", "quality_check", 100, "FAILURE", "00:00", "01:00"), job("Quality", "quality_check", 101, "SUCCESS", "01:05", "06:00")]) }
fn going_then_passed() -> Value { json!([going(100, "00:00"), job("Quality", "quality_check", 101, "SUCCESS", "01:05", "06:00")]) }
const QUALITY: &str = ".github/workflows/quality.yml";
const FLOW: &str = "name: Quality\non:\n  pull_request:\njobs:\n  quality_check:\n    runs-on: ubuntu-24.04\n";
/// A run of Quality on the commit, as GitHub Actions gives it: `(status, conclusion)`.
fn run(id: u64, (status, conclusion): (&str, &str)) -> Value { json!([QUALITY, status, conclusion, format!("https://github.com/o/r/actions/runs/{id}"), "Quality"]) }
const CANCELLED: (&str, &str) = ("completed", "cancelled");
const FAILED: (&str, &str) = ("completed", "failure");
const GOING: (&str, &str) = ("in_progress", "");
const PASSED: (&str, &str) = ("completed", "success");
/// A task whose branch has the workflow Quality, at its approval.
fn with_quality(w: &World, title: &str) -> (String, String) {
    w.script(json!({ "worker": [{ "files": { "f.txt": "v1\n", QUALITY: FLOW }, "commit": "v1", "note": "Did v1." }] }));
    let id = to_approval(w, title);
    let head = task(&id).t("prHead");
    (id, head)
}

#[test]
fn an_older_run_cancelled_failed_or_still_going_wakes_no_one_once_the_newest_run_passed() {
    for checks in [cancelled(), cancelled_before_it_began(), failed_then_passed(), going_then_passed(), run_again_after_it()] {
        let w = pr_world(json!({}));
        let id = to_approval(&w, "Labeled");
        let head = task(&id).t("prHead");
        let before = pane_prompts(&w, &id).len();
        w.gh_says(json!({ "view": rollup_of(&head, checks.clone()) }));
        poll_prs();
        let t = task(&id);
        assert_eq!(pane_prompts(&w, &id).len(), before, "{checks}: the agent was woken: {:?}", pane_prompts(&w, &id).last());
        assert!(t.at("prFailure").is_null(), "{checks}: {}", t.at("prFailure"));
        assert_eq!((t.s("stage"), t.s("prState")), ("approval", "passing"), "{checks}");
    }
}

#[test]
fn a_newest_run_that_failed_or_was_cancelled_still_fails_whatever_an_older_run_said_and_one_not_begun_is_waited_for() {
    for (checks, failed) in [
        (json!([job("Quality", "quality_check", 100, "SUCCESS", "00:00", "01:00"), job("Quality", "quality_check", 101, "CANCELLED", "01:05", "01:10")]), "quality_check"),
        (json!([job("Quality", "quality_check", 100, "SUCCESS", "00:00", "01:00"), job("Quality", "quality_check", 101, "FAILURE", "01:05", "02:00")]), "quality_check"),
        // an older run's job run again after the newer run's passed, and failed
        (json!([job("Quality", "quality_check", 101, "SUCCESS", "01:05", "02:00"), numbered(job("Quality", "quality_check", 100, "FAILURE", "08:00", "09:00"), 100, 1020)]), "quality_check"),
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
    // the newest run has not begun yet: after a failure or a pass of an older one, it is waited for
    for older in ["FAILURE", "SUCCESS"] {
        let w = pr_world(json!({}));
        let id = to_approval(&w, "Queued");
        let head = task(&id).t("prHead");
        let before = pane_prompts(&w, &id).len();
        w.gh_says(json!({ "view": rollup_of(&head, json!([job("Quality", "quality_check", 100, older, "00:00", "01:00"), going(101, "")])) }));
        poll_prs();
        let t = task(&id);
        assert_eq!((t.s("prState"), pane_prompts(&w, &id).len()), ("waiting", before), "{older}: {}", t.at("prFailure"));
    }
}

#[test]
fn a_merge_goes_by_the_newest_run_of_each_check_and_of_the_branchs_workflow() {
    // GitHub's list, and the runs of the branch's workflow Quality on the commit: the newest run passed, it merges
    for (checks, older) in [(cancelled(), CANCELLED), (cancelled_before_it_began(), CANCELLED), (failed_then_passed(), FAILED), (going_then_passed(), GOING)] {
        let w = pr_world(json!({}));
        let (id, head) = with_quality(&w, "Newest passed");
        w.gh_says(json!({ "view": rollup_of(&head, checks.clone()), "runs": [run(100, older), run(101, PASSED)] }));
        finish_line::approve(&id, None, false).unwrap();
        let t = settle(&id);
        assert_eq!(t.s("stage"), "done", "{checks}: {} {}", t.t("msg"), t.at("prFailure")); assert!(merged(&w), "{}", gh_log(&w));
    }
    // the newest run was cancelled, or failed: nothing merges
    for newest in ["CANCELLED", "FAILURE"] {
        let w = pr_world(json!({ "review.rounds": 0 }));
        let (id, head) = with_quality(&w, "Newest stopped");
        let checks = json!([job("Quality", "quality_check", 100, "SUCCESS", "00:00", "01:00"), job("Quality", "quality_check", 101, newest, "01:05", "01:10")]);
        w.gh_says(json!({ "view": rollup_of(&head, checks), "runs": [run(100, PASSED), run(101, ("completed", &newest.to_lowercase()))] }));
        finish_line::approve(&id, None, false).unwrap();
        let t = settle(&id);
        assert_eq!(t.at("prFailure").s("remoteKind"), "checks", "{newest}: {}", t.t("msg")); assert!(!merged(&w));
    }
}

#[test]
fn read_from_actions_the_newest_run_decides_too() {
    // a fine-grained token: the commit's runs and their jobs, with no times, the later run the higher number
    let actions = |head: &str, first: (&str, &str), then: (&str, &str)| {
        let jobs = |id: u64, (s, c): (&str, &str)| json!([["quality_check", s, c, format!("https://github.com/o/r/actions/runs/{id}/job/{}", id * 10)]]);
        json!({ "noChecks": true, "view": none(head), "statuses": [], "runs": [run(5, first), run(6, then)], "jobs": { "5": jobs(5, first), "6": jobs(6, then) } })
    };
    for older in [CANCELLED, FAILED, GOING] {
        let w = pr_world(json!({}));
        let (id, head) = with_quality(&w, "Newest passed");
        w.gh_says(actions(&head, older, PASSED));
        finish_line::approve(&id, None, false).unwrap();
        let t = settle(&id);
        assert_eq!(t.s("stage"), "done", "{older:?}: {} {}", t.t("msg"), t.at("prFailure")); assert!(merged(&w), "{}", gh_log(&w));
    }
    for newest in [CANCELLED, FAILED] {
        let w = pr_world(json!({ "review.rounds": 0 }));
        let (id, head) = with_quality(&w, "Newest stopped");
        w.gh_says(actions(&head, PASSED, newest));
        finish_line::approve(&id, None, false).unwrap();
        let t = settle(&id);
        assert_eq!(t.at("prFailure").s("remoteKind"), "checks", "{newest:?}: {}", t.t("msg")); assert!(!merged(&w));
    }
}
