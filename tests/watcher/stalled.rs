//! A stalled task: its agent works, but its screen shows nothing new for `stall.after` (20 minutes by default). An
//! automation that waits for it (`--when stalled`) and a coordinator on autopilot (`autopilot.when stalled`) hear of
//! each stall once; new output re-arms it.
use crate::common;
use common::pilot::{ok, pilot_world, prompts_of, proj, record};
use common::*;
use serde_json::json;
use sw::automations::list::{self as l, Add};
use sw::automations::triggers::parse_event;
use sw::json::*;
use sw::store::automations::firings_of;

const MIN: i64 = 60_000;
/// What the keeper does once a second, with its look at each working agent's screen; then the jobs it started.
fn keeper(w: &World) { sw::keeper::tick_once(&w.socket, true); common::pilot::drain(); }
/// `n` minutes on, the keeper ticking each minute.
fn minutes_on(w: &World, n: i64) { for _ in 0..n { sw::clock::advance(MIN); keeper(w); } }
fn screen(w: &World, id: &str, s: &str) {
    let pane = task(id).at("link").t("pane");
    w.patch(|st| { for p in st["panes"].as_array_mut().unwrap() { if p.s("pane_id") == pane { p["agent"]["screen"] = json!(s); } } });
}
/// A task whose agent works on, its screen as the script left it.
fn working(w: &World, id: &str) {
    settle(id);
    assert_eq!((task(id).s("stage"), task(id).at("last").s("status")), ("working", "working"), "{:?}", task(id));
}
fn stalls_of(a: &Obj, id: &str) -> Vec<String> { firings_of(a.s("id")).iter().map(|f| f.t("what")).filter(|x| x.starts_with(&format!("{id} "))).collect() }

#[test]
fn an_automation_waiting_for_a_stall_fires_once_when_the_screen_shows_nothing_new_and_again_after_new_output() {
    let w = world();
    sw::clock::set(Some(sw::clock::now()));
    let a = l::add(Add { name: "Look at a stall", prompt: "See why it stopped.", trigger: Some(parse_event("stalled", false).unwrap()), repo: "", folder: &w.root, settings: Obj::new(), research: None }).unwrap();
    w.script(json!({ "worker": [{ "status": "working", "screen": "● Running the tests\n" }] }));
    let id = w.add("Slow one"); delegate(&id); working(&w, &id);
    keeper(&w);
    // nineteen minutes with nothing new on its screen: not yet
    minutes_on(&w, 19);
    assert!(stalls_of(&a, &id).is_empty(), "{:?}", firings_of(a.s("id")));
    // twenty: it stalled, and the automation fires once, however long it stays so
    minutes_on(&w, 2);
    assert_eq!(stalls_of(&a, &id).len(), 1, "{:?}", firings_of(a.s("id")));
    minutes_on(&w, 15);
    assert_eq!(stalls_of(&a, &id).len(), 1, "once per stall");
    // the task it made is done; the agent's screen moves on, then shows nothing new again: a new stall, after twenty minutes more
    let made = firings_of(a.s("id")).iter().find(|f| f.b("task")).map(|f| f.t("task")).unwrap();
    sw::store::tasks::patch(&made, |t| t.put("stage", "done")).unwrap();
    screen(&w, &id, "● Fixing the two failing tests\n");
    minutes_on(&w, 19);
    assert_eq!(stalls_of(&a, &id).len(), 1, "new output re-arms it, counting again from then");
    minutes_on(&w, 2);
    assert_eq!(stalls_of(&a, &id).len(), 2, "{:?}", firings_of(a.s("id")));
}

#[test]
fn a_task_that_waits_for_you_or_one_under_stall_after_none_never_stalls() {
    let w = world();
    sw::clock::set(Some(sw::clock::now()));
    let a = l::add(Add { name: "Look at a stall", prompt: "See why it stopped.", trigger: Some(parse_event("stalled", true).unwrap()), repo: "", folder: &w.root, settings: Obj::new(), research: None }).unwrap();
    w.script(json!({ "worker": [{ "status": "working", "screen": "● Running the tests\n" }] }));
    let id = w.add("Never stalls"); delegate(&id); working(&w, &id);
    ok(&["set", "stall.after", "none", "--task", &id]);
    minutes_on(&w, 30);
    assert!(stalls_of(&a, &id).is_empty(), "stall.after none: {:?}", firings_of(a.s("id")));
    ok(&["set", "stall.after", "45m", "--task", &id]);
    minutes_on(&w, 10);
    assert!(stalls_of(&a, &id).is_empty(), "its own time, counted from its last new output");
    minutes_on(&w, 6);
    assert_eq!(stalls_of(&a, &id).len(), 1);
    // waiting for you is not a stall: its agent asks
    let other = w.add("Asks"); delegate(&other); working(&w, &other);
    sw::store::tasks::patch(&other, |t| { t.put("stage", "asks"); t.put("kind", "question"); }).unwrap();
    minutes_on(&w, 30);
    assert!(stalls_of(&a, &other).is_empty(), "{:?}", firings_of(a.s("id")));
}

#[test]
fn a_task_of_its_own_that_stalls_wakes_its_coordinator_on_autopilot_once() {
    let w = pilot_world();
    let co = proj(&w);
    ok(&["autopilot", "on", "--project", "proj", "--goal", "Docs", "--autopilot.every", "none", "--autopilot.gap", "1m", "--autopilot.when", "stalled"]);
    keeper(&w); keeper(&w);
    assert_eq!(prompts_of(&w, &co).len(), 1, "the wake of turning on");
    assert_eq!(record(&co).arr("wakes")[0].at("answered"), &json!(true));
    w.script(json!({ "worker": [{ "status": "working", "screen": "● Running the tests\n" }] }));
    let id = sw::autopilot::add::add(&co, &w.root, "Its own", Default::default(), &[], false).unwrap().task.t("id");
    working(&w, &id);
    keeper(&w);
    minutes_on(&w, 19);
    assert_eq!(prompts_of(&w, &co).len(), 1, "{:?}", prompts_of(&w, &co));
    minutes_on(&w, 2);
    assert_eq!(prompts_of(&w, &co).len(), 2, "{:?}", prompts_of(&w, &co));
    let said = prompts_of(&w, &co)[1].clone();
    assert!(said.starts_with(&format!("Autopilot: {} has shown nothing new for 2", short(&id))), "{said}");
    minutes_on(&w, 20);
    assert_eq!(prompts_of(&w, &co).len(), 2, "once per stall");
}
