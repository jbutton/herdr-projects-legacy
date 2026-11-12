//! The board's own restart of a working agent that went (hook/relaunch.rs) when herdr does not answer or the program
//! will not start: a look herdr did not answer changes nothing, a restart that runs late asks herdr again and leaves a
//! running agent alone, and a try that could not start it fails nothing at once: it was that try, and giving up says
//! what the last one ran into.
use super::relaunch::{exits, keep, later, starts, working};
use crate::common;
use common::*;
use serde_json::json;
use sw::json::*;

#[test]
fn herdr_not_answering_about_the_agent_changes_nothing() {
    let w = world_with(Opts { repo: true, settings: json!({ "review.when": "never" }), ..Default::default() });
    w.script(json!({ "worker": [{ "status": "working" }] }));
    let id = working(&w, "herdr times out");
    let last = task(&id).at("last").clone();
    w.patch(|s| s["agentGetFails"] = json!(true));
    keep(&w);
    let t = later(&w, &id, 21_000);
    later(&w, &id, 400_000);
    assert_eq!((t.t("stage"), starts(&w).len(), t.b("relaunched"), t.b("exitedAt")), ("working".into(), 1, false, false), "{}", t.at("failure"));
    assert_eq!(task(&id).at("last"), &last);
    w.patch(|s| s["agentGetFails"] = json!(false));
    keep(&w);
    assert_eq!((row(&id).t("state"), task(&id).t("stage")), ("working".into(), "working".into()));
}

#[test]
fn a_restart_that_runs_late_while_herdr_times_out_leaves_an_agent_that_came_back_alone() {
    let w = world_with(Opts { repo: true, settings: json!({ "review.when": "never" }), ..Default::default() });
    w.script(json!({ "worker": [{ "status": "working" }] }));
    let id = working(&w, "Back before its restart ran");
    let pane = task(&id).at("link").t("pane");
    let agent = agent_of(&w, &pane);
    exits(&w, &id);
    keep(&w);
    sw::clock::advance(21_000);
    // a hook in a plan's sandbox asks the keeper for the restart, which runs it a while later
    unsafe { std::env::set_var("HERDR_PROJECTS_BOXED", "1"); }
    keep(&w);
    unsafe { std::env::remove_var("HERDR_PROJECTS_BOXED"); }
    assert!(sw::runner::outside::asked().iter().any(|j| j.0 == "restart" && j.1 == id));
    // meanwhile its agent is back by itself, and herdr times out on looking at it
    w.patch(|s| { for p in s["panes"].as_array_mut().unwrap() { if p.s("pane_id") == pane { p["agent"] = agent.clone(); } } s["agentGetFails"] = json!(true); });
    sw::runner::outside::start_asked();
    let t = settle(&id);
    assert_eq!((t.t("stage"), starts(&w).len()), ("working".into(), 1), "{}", t.at("failure"));
    assert!(agent_of(&w, &pane).is_object());
    // herdr answers again: it runs, as it did
    w.patch(|s| s["agentGetFails"] = json!(false));
    keep(&w);
    assert_eq!((task(&id).t("stage"), row(&id).t("state")), ("working".into(), "working".into()));
}

#[test]
fn a_try_that_cannot_start_the_program_is_that_try_and_giving_up_says_what_it_ran_into() {
    let w = world_with(Opts { repo: true, settings: json!({ "review.when": "never" }), ..Default::default() });
    w.script(json!({ "worker": [{ "status": "working" }] }));
    let id = working(&w, "Will not start");
    exits(&w, &id);
    keep(&w);
    // its program is gone from the machine now: each try times out on herdr
    w.patch(|s| s["missing"] = json!(["claude"]));
    let t = later(&w, &id, 21_000);
    assert_eq!((t.t("stage"), t.at("relaunched").i("tries")), ("working".into(), 1), "{}", t.at("failure"));
    assert!(t.at("relaunched").s("error").contains("timed out"), "{}", t.at("relaunched"));
    assert_eq!(row(&id).t("note"), "The agent has exited");
    later(&w, &id, 61_000);
    let t = later(&w, &id, 301_000);
    assert_eq!((t.t("stage"), t.at("relaunched").i("tries")), ("working".into(), 3));
    // down after its last try: the task waits for you, with what the last try ran into
    let t = later(&w, &id, 21_000);
    assert_eq!((t.s("stage"), t.at("failure").s("title")), ("failed", "The agent keeps stopping"));
    assert!(t.at("failure").s("detail").contains("The last try could not start it:") && t.at("failure").s("detail").contains("timed out"), "{}", t.at("failure").s("detail"));
}
