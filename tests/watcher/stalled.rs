//! A stalled task: its agent works, but its screen shows nothing new for `stall.after` (20 minutes by default), or it
//! runs past the Watcher's estimate by as much again. An automation that waits for it (`--when stalled`) and a
//! coordinator on autopilot (`autopilot.when stalled`) hear of each stall once; new output or a moved guess re-arms it,
//! and a gap in looking (the machine slept) is never counted as quiet.
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
fn working(id: &str) {
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
    let id = w.add("Slow one"); delegate(&id); working(&id);
    keeper(&w);
    // nineteen minutes with nothing new on its screen: not yet
    minutes_on(&w, 19);
    assert!(stalls_of(&a, &id).is_empty(), "{:?}", firings_of(a.s("id")));
    // twenty: it stalled, and the automation fires once, however long it stays so
    minutes_on(&w, 2);
    assert_eq!(stalls_of(&a, &id).len(), 1, "{:?}", firings_of(a.s("id")));
    minutes_on(&w, 15);
    assert_eq!(stalls_of(&a, &id).len(), 1, "once per stall");
    // the task it made is done (another firing would make one); its agent shows a dialog for two minutes and works on
    // with the same screen: the same stall, never heard of again
    let made = firings_of(a.s("id")).iter().find(|f| f.b("task")).map(|f| f.t("task")).unwrap();
    sw::store::tasks::patch(&made, |t| t.put("stage", "done")).unwrap();
    let pane = task(&id).at("link").t("pane");
    set_status(&w, &pane, "blocked"); minutes_on(&w, 2);
    assert_eq!((task(&id).s("stage"), task(&id).at("last").s("status")), ("working", "blocked"));
    set_status(&w, &pane, "working"); minutes_on(&w, 2);
    assert_eq!(stalls_of(&a, &id).len(), 1, "a status that flapped: {:?}", firings_of(a.s("id")));
    // the agent's screen moves on, then shows nothing new again: a new stall, after twenty minutes more
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
    let id = w.add("Never stalls"); delegate(&id); working(&id);
    ok(&["set", "stall.after", "none", "--task", &id]);
    minutes_on(&w, 30);
    assert!(stalls_of(&a, &id).is_empty(), "stall.after none: {:?}", firings_of(a.s("id")));
    ok(&["set", "stall.after", "45m", "--task", &id]);
    minutes_on(&w, 10);
    assert!(stalls_of(&a, &id).is_empty(), "its own time, counted from its last new output");
    minutes_on(&w, 8);
    let t = task(&id);
    assert_eq!(stalls_of(&a, &id).len(), 1, "{:?} output {} since {} workAt {} now {}", firings_of(a.s("id")), t.at("output"), t.i("since"), t.i("workAt"), sw::clock::now());
    // waiting for you is not a stall: its agent asks
    w.script(json!({ "worker": [{ "status": "working", "screen": "● Reading the code\n" }] }));
    let other = w.add("Asks"); delegate(&other); working(&other);
    sw::store::tasks::patch(&other, |t| { t.put("stage", "asks"); t.put("kind", "question"); }).unwrap();
    minutes_on(&w, 30);
    assert!(stalls_of(&a, &other).is_empty(), "{:?}", firings_of(a.s("id")));
}

#[test]
fn a_task_of_its_own_that_stalls_wakes_its_coordinator_on_autopilot_once_also_with_its_slots_full() {
    let w = pilot_world();
    let co = proj(&w);
    // one task at once: the one that stalls fills it, and a stall needs looking after all the same
    ok(&["autopilot", "on", "--project", "proj", "--goal", "Docs", "--autopilot.every", "none", "--autopilot.gap", "1m", "--autopilot.when", "stalled", "--autopilot.tasksAtOnce", "1"]);
    keeper(&w); keeper(&w);
    assert_eq!(prompts_of(&w, &co).len(), 1, "the wake of turning on");
    assert_eq!(record(&co).arr("wakes")[0].at("answered"), &json!(true));
    w.script(json!({ "worker": [{ "status": "working", "screen": "● Running the tests\n" }] }));
    let id = sw::autopilot::add::add(&co, &w.root, "Its own", Default::default(), &[], false).unwrap().task.t("id");
    delegate(&id); working(&id);
    keeper(&w);
    minutes_on(&w, 19);
    assert_eq!(prompts_of(&w, &co).len(), 1, "{:?}", prompts_of(&w, &co));
    minutes_on(&w, 2);
    assert_eq!(prompts_of(&w, &co).len(), 2, "{:?}", prompts_of(&w, &co));
    let said = prompts_of(&w, &co)[1].clone();
    assert!(said.starts_with(&format!("Autopilot: {} has shown nothing new for 2", short(&id))), "{said}");
    let shown = ok(&["autopilot", "show", "--project", "proj"]);
    assert!(shown.contains("Today: 1 of 5 added · 1 of 1 under way") && shown.contains(&format!("Needs looking after: {} (has stalled)", short(&id))), "{shown}");
    minutes_on(&w, 20);
    assert_eq!(prompts_of(&w, &co).len(), 2, "once per stall");
    // its status flaps: still the same stall
    let pane = task(&id).at("link").t("pane");
    set_status(&w, &pane, "blocked"); minutes_on(&w, 2); set_status(&w, &pane, "working"); minutes_on(&w, 12);
    assert_eq!(prompts_of(&w, &co).len(), 2, "{:?}", prompts_of(&w, &co));
}

#[test]
fn a_task_well_past_the_watchers_estimate_stalls_once_though_its_screen_moves_and_a_moved_guess_re_arms_it() {
    let w = world_with(Opts { repo: true, settings: json!({ "review.when": "never", "watcher.agent": "claude" }), ..Default::default() });
    sw::clock::set(Some(sw::clock::now()));
    let a = l::add(Add { name: "Look at a stall", prompt: "See why it stopped.", trigger: Some(parse_event("stalled", false).unwrap()), repo: "", folder: &w.root, settings: Obj::new(), research: None }).unwrap();
    w.script(json!({ "worker": [{ "status": "working", "screen": "● Running the tests\n" }] }));
    let id = w.add("Loops"); delegate(&id); working(&id);
    sw::store::tasks::patch(&id, |t| { t.put("since", sw::clock::now() - 9 * MIN); t.put("workAt", 0); }).unwrap(); // ten minutes in at the Watcher's first guess, a minute on
    sw::watcher::watcher_reset();
    let guess = std::cell::Cell::new(50);
    // a minute: its screen moves on, the board looks at it, the Watcher guesses (the same percent until told), the automations look
    let minute = |n: i64| {
        sw::clock::advance(MIN);
        screen(&w, &id, if n % 2 == 0 { "● Running the tests again\n" } else { "● Reading the same failure\n" }); // digits are no change
        let rows = sw::flow::live::board(true);
        sw::watcher::output::look(&rows, sw::clock::now());
        sw::watcher::watch_tick_with(&rows, sw::clock::now(), Some(0.0), &|_, _, done| done(0, json!([{ "n": 1, "msg": "running the tests again", "progress": guess.get() }]).to_string(), String::new()));
        sw::automations::tick::automations_tick(sw::clock::now());
    };
    // at 50% after ten minutes: ten minutes left. Past them by as much again, at least stall.after (20m): thirty minutes on
    for n in 0..29 { minute(n); }
    assert!(stalls_of(&a, &id).is_empty(), "never quiet, and not yet past its estimate: {:?}", firings_of(a.s("id")));
    minute(29); minute(30);
    assert_eq!(stalls_of(&a, &id).len(), 1, "{:?}", task(&id).at("watch"));
    for n in 31..40 { minute(n); }
    assert_eq!(stalls_of(&a, &id).len(), 1, "once per stall");
    // its guess moves: a new estimate, so no stall until that one runs out too
    let made = firings_of(a.s("id")).iter().find(|f| f.b("task")).map(|f| f.t("task")).unwrap();
    sw::store::tasks::patch(&made, |t| t.put("stage", "done")).unwrap();
    guess.set(80);
    for n in 40..60 { minute(n); }
    assert_eq!(stalls_of(&a, &id).len(), 1, "{:?}", task(&id).at("watch"));
}

#[test]
fn a_stall_seen_is_remembered_while_its_status_flaps_and_fires_no_second_time() {
    use sw::automations::events::new_events;
    let _w = world();
    let now = 100 * MIN;
    sw::clock::set(Some(now));
    let mut t = obj(json!({ "id": "proj/1", "stage": "working", "link": { "pane": "p1" }, "last": { "status": "working" }, "since": now - 30 * MIN, "output": { "hash": "aa", "at": now - 25 * MIN, "seen": now - MIN, "from": now - 25 * MIN } }));
    let mut before = Obj::new(); before.put("proj/1", vec!["added"]);
    let stalled = |e: &[sw::automations::events::Happened]| e.iter().filter(|x| x.event == "stalled").count();
    let (first, seen, _) = new_events(&[t.clone()], Some(&before));
    assert_eq!(stalled(&first), 1);
    // its agent shows a dialog: no stall now, but the one seen is kept
    t.obj_mut("last").put("status", "blocked");
    let (none, seen, _) = new_events(&[t.clone()], Some(&seen));
    assert_eq!(stalled(&none), 0);
    assert!(strings(seen.at("proj/1")).iter().any(|m| m.starts_with("stalled:quiet:")), "{seen:?}");
    t.obj_mut("last").put("status", "working");
    let (again, seen, _) = new_events(&[t.clone()], Some(&seen));
    assert_eq!(stalled(&again), 0, "the same stall: no new output, no new run");
    // new output re-arms it: forgotten, and a stall twenty minutes on is a new one
    t.put("output", json!({ "hash": "bb", "at": now + MIN, "seen": now + MIN, "from": now - 25 * MIN }));
    let (_, seen, _) = new_events(&[t.clone()], Some(&seen));
    assert!(!strings(seen.at("proj/1")).iter().any(|m| m.starts_with("stalled:")), "{seen:?}");
    sw::clock::set(Some(now + 21 * MIN)); t.put("output", json!({ "hash": "bb", "at": now + MIN, "seen": now + 20 * MIN, "from": now - 25 * MIN }));
    assert_eq!(stalled(&new_events(&[t], Some(&seen)).0), 1);
}

#[test]
fn a_gap_in_looking_is_never_counted_as_quiet() {
    use sw::watcher::output::{noted, FRESH_MS, NOTE_MS};
    let first = noted(&json!(null), "aa", 1000).unwrap();
    assert_eq!(first, json!({ "hash": "aa", "at": 1000, "seen": 1000, "from": 1000 }));
    assert_eq!(noted(&first, "aa", 1000 + MIN), None, "the same words a minute on: nothing to write");
    let later = noted(&first, "aa", 1000 + NOTE_MS).unwrap();
    assert_eq!((later.i("at"), later.i("seen"), later.i("from")), (1000, 1000 + NOTE_MS, 1000), "the look itself, now and then");
    assert_eq!(noted(&later, "bb", 2 * NOTE_MS).unwrap().i("at"), 2 * NOTE_MS, "new words");
    let woke = noted(&later, "aa", 1000 + NOTE_MS + FRESH_MS + 1).unwrap();
    assert_eq!((woke.i("at"), woke.i("from")), (1000 + NOTE_MS + FRESH_MS + 1, 1000 + NOTE_MS + FRESH_MS + 1), "after a gap the count starts again");
    // a task judged from what it keeps: quiet only while it is looked at
    let _w = world();
    let t = |o: serde_json::Value| obj(json!({ "id": "proj/1", "stage": "working", "link": { "pane": "p1" }, "last": { "status": "working" }, "since": 0, "output": o }));
    let now = 100 * MIN;
    assert_eq!(sw::stalled::marks(&t(json!({ "hash": "aa", "at": 0, "seen": now - MIN, "from": 0 })), now), [format!("stalled:quiet:0:{}", 20 * MIN)]);
    assert!(sw::stalled::marks(&t(json!({ "hash": "aa", "at": 0, "seen": now - FRESH_MS - 1, "from": 0 })), now).is_empty(), "a last look too old says nothing");
    assert!(sw::stalled::marks(&t(json!(null)), now).is_empty(), "never looked at");
    assert_eq!(sw::stalled::say("stalled:quiet:0:1200000", 25 * MIN), "has shown nothing new for 25m");
}
