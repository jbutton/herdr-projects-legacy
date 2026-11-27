//! The `has a question` wake for every task a coordinator on autopilot added from its own pane: one added in a turn of
//! the human's wakes it once when its agent ends on a question (and stays not its own to tell), and an agent that went
//! on after its question (its program carried on by itself after a usage limit's reset) and ended a turn wakes it again.
use crate::common;
use common::pilot::*;
use common::*;
use serde_json::json;
use sw::coordinator::co::Co;
use sw::json::*;

fn on_and_quiet(w: &World, co: &Co, extra: &[&str]) {
    let mut a = vec!["autopilot", "on", "--project", "proj", "--goal", "Docs", "--autopilot.every", "none", "--autopilot.gap", "1m"];
    a.extend_from_slice(extra);
    ok(&a);
    tick(w); tick(w);
    assert_eq!(prompts_of(w, co).len(), 1, "{:?}", prompts_of(w, co));
}
fn last_prompt(w: &World, co: &Co) -> String { prompts_of(w, co).last().cloned().unwrap_or_default() }
/// The human asks the coordinator for a research task, and it adds it from its own pane in that turn: at work.
fn added_in_your_turn(w: &World, co: &Co) -> String {
    ok(&["coordinator", "tell", "--project", "proj", "research the docs index for me"]);
    tick(w);
    assert_eq!(last_prompt(w, co), "research the docs index for me");
    in_pane(w, &pane_of(co));
    assert!(!sw::autopilot::turn::autopilot_turn(co), "the human's turn");
    let said = ok(&["add", "Research the docs index", "--research", "--repo", "proj"]);
    in_pane(w, "");
    let id = said.split(':').next().unwrap().to_string();
    assert!(id.starts_with("proj/") && task(&id).s("coordinator") == "project:proj", "{said}");
    let mut sc = w.fake.script.borrow().clone(); sc["worker"] = json!([{ "status": "working" }]); w.script(sc); // the coordinator's steps kept
    delegate(&id);
    let t = settle(&id); assert_eq!(t.s("stage"), "working", "{}", t.at("failure"));
    tick(w);
    id
}
/// Its agent reports its question itself, in the turn it is in.
fn ask(id: &str, msg: &str) { sw::actions::filing::report(id, sw::actions::filing::Report { state: "blocked".into(), kind: "question".into(), msg: msg.into(), own: "agent", ..Default::default() }).unwrap(); }
/// herdr's reading of the task's agent: `done` ends a turn.
fn pane(w: &World, id: &str, status: &str) {
    let p = task(id).at("link").t("pane");
    w.patch(|s| { let seq = s.i("seq") + 1; s["seq"] = json!(seq); for x in s["panes"].as_array_mut().unwrap() { if x.s("pane_id") == p { x["agent"]["status"] = json!(status); x["agent"]["seq"] = json!(seq); if status == "done" { x["agent"]["completion"] = json!(seq); } } } });
    on_pane(w, &p);
}
fn wakes_for(w: &World, co: &Co, id: &str) -> usize { prompts_of(w, co).iter().filter(|p| p.contains(&format!("{} has a question", short(id)))).count() }

#[test]
fn a_task_added_in_the_humans_turn_that_ends_on_a_question_wakes_it_once_and_stays_not_its_own() {
    let w = pilot_world();
    let co = proj(&w);
    w.script(json!({ "coordinator": [{}, { "said": "Added it." }, { "status": "working" }] }));
    on_and_quiet(&w, &co, &[]);
    // at today's limit: a question of a task it added in your turn goes out all the same
    ok(&["set", "autopilot.tasksPerDay", "0", "--project-coordinator", "proj"]);
    let id = added_in_your_turn(&w, &co);
    minutes(2); tick(&w);
    assert_eq!(prompts_of(&w, &co).len(), 2, "being added in its own turn wakes nothing");
    // its agent asks, and the coordinator hears it before the agent's turn has ended
    ask(&id, "Here is what I found: the index is stale. Rebuild it?");
    minutes(2); tick(&w);
    assert_eq!(last_prompt(&w, &co), format!("Autopilot: {} has a question", short(&id)));
    // the wake makes it no task of its own: it reads it, and may not answer it
    let ap = record(&co);
    assert!(sw::autopilot::record::task_rec(&ap, &id).is_none());
    in_pane(&w, &pane_of(&co));
    assert!(sw::autopilot::turn::autopilot_turn(&co), "the wake's turn");
    assert!(ok(&["autopilot", "show", "--project", "proj"]).contains(&format!("added in the human's turn (read it and tell the human; not yours to answer): {}", short(&id))));
    assert!(refused(&["tell", &id, "Yes, rebuild it."]).contains("It may only talk to the agents of tasks it may look after."));
    in_pane(&w, "");
    end_turn(&w, &co);
    // the agent's turn ends on that question: nothing new
    pane(&w, &id, "done");
    settle(&id);
    assert_eq!(task(&id).s("stage"), "asks");
    for _ in 0..3 { minutes(2); tick(&w); }
    assert_eq!(wakes_for(&w, &co, &id), 1, "once: {:?}", prompts_of(&w, &co));
}

#[test]
fn an_agent_that_goes_on_after_its_question_and_ends_a_turn_still_asking_wakes_it_again() {
    let w = pilot_world();
    let co = proj(&w);
    on_and_quiet(&w, &co, &[]);
    let id = added_in_your_turn(&w, &co);
    // it asks, as its program hits its usage limit: the coordinator hears it once
    ask(&id, "Out of usage until the reset: carry on then?");
    minutes(2); tick(&w); tick(&w);
    assert_eq!(wakes_for(&w, &co, &id), 1, "{:?}", prompts_of(&w, &co));
    // its program waits at the limit's prompt; after the reset it carries on by itself and ends its turn: the task
    // still asks the same
    pane(&w, &id, "blocked");
    minutes(120);
    pane(&w, &id, "working");
    minutes(5); tick(&w);
    assert_eq!(wakes_for(&w, &co, &id), 1, "not while it works");
    pane(&w, &id, "done");
    settle(&id);
    let t = task(&id);
    assert_eq!((t.s("stage"), t.s("msg")), ("asks", "Out of usage until the reset: carry on then?"));
    minutes(2); tick(&w);
    assert_eq!(wakes_for(&w, &co, &id), 2, "{:?}", prompts_of(&w, &co));
    for _ in 0..3 { minutes(2); tick(&w); }
    assert_eq!(wakes_for(&w, &co, &id), 2, "and no more");
}

#[test]
fn a_task_it_may_look_after_whose_agent_went_on_after_its_question_wakes_it_again_and_is_not_left_to_you() {
    let w = pilot_world();
    let co = proj(&w);
    on_and_quiet(&w, &co, &["--autopilot.startAtOnce", "no"]);
    let id = sw::autopilot::add::add(&co, &w.root, "Its own", Default::default(), &[], false).unwrap().task.t("id");
    w.script(json!({ "worker": [{ "status": "working" }] }));
    delegate(&id);
    settle(&id);
    tick(&w);
    ask(&id, "Which index?");
    minutes(2); tick(&w); tick(&w);
    assert_eq!(wakes_for(&w, &co, &id), 1);
    // its one retry used since: a new question would leave it to you, the same one asked on does not
    sw::autopilot::record::change(&co, |ap, _| { sw::autopilot::record::with_task(ap, &id, |r| { r.put("retries", 1); }); true }).unwrap();
    pane(&w, &id, "blocked"); minutes(30); pane(&w, &id, "working"); minutes(5); pane(&w, &id, "done"); settle(&id);
    minutes(2); tick(&w);
    assert_eq!(wakes_for(&w, &co, &id), 2);
    assert!(!sw::autopilot::record::task_rec(&record(&co), &id).unwrap().b("left"), "the same question, not a new one");
}

#[test]
fn a_task_it_added_in_another_repo_in_the_humans_turn_wakes_it_for_its_question_and_for_nothing_else() {
    let w = pilot_world();
    let co = proj(&w);
    on_and_quiet(&w, &co, &[]);
    let other = w.base.join("other"); std::fs::create_dir_all(&other).unwrap(); git(&other, &["init", "-q", "-b", "main"]); sw::store::repos::repo_of(&other).unwrap();
    in_pane(&w, &pane_of(&co));
    let id = ok(&["add", "Research the other docs", "--repo", "other"]).split(':').next().unwrap().to_string();
    in_pane(&w, "");
    assert!(id.starts_with("other/") && task(&id).s("coordinator") == "project:proj");
    minutes(2); tick(&w);
    sw::store::tasks::set(&id, json!({ "stage": "asks", "kind": "question", "msg": "Found it. Which part next?" })).unwrap();
    minutes(2); tick(&w);
    assert_eq!(last_prompt(&w, &co), format!("Autopilot: {} has a question", short(&id)));
    tick(&w);
    sw::store::tasks::set(&id, json!({ "stage": "done" })).unwrap();
    for _ in 0..3 { minutes(2); tick(&w); }
    assert_eq!(prompts_of(&w, &co).len(), 2, "outside its area, its finish wakes nothing: {:?}", prompts_of(&w, &co));
}
