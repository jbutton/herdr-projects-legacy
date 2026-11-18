//! Before the window: a task delegated outside it waits in the queue, holding no slot and no place, as "waiting for its
//! window: starts 22:00", and starts by itself when it opens; `--now` and carrying on start it anyway; a value that
//! cannot be read is refused with the nearest right one.
use super::*;

#[test]
fn run_window_holds_a_start_without_a_slot_and_starts_it_when_the_window_opens() {
    let w = afternoon(json!({ "board.agents": 1 }));
    let held = w.add("Tonight");
    delegate_with(&held, json!({ "run.window": "nights" }));
    let t = settle(&held);
    assert_eq!((t.s("stage"), t.s("msg")), ("queued", "waiting for its window: starts 22:00"));
    assert_eq!(t.at("windowWait").i("opens"), local(0, 22, 0));
    // It waits as a queued task does, not for you; it has no place in the queue and its sidebar word says why.
    let r = row(&held);
    assert_eq!((r.s("state"), sw::notify::event_of(&t)), ("working", ""));
    assert_eq!(sw::shown::queue::words(&r), "waiting for its window: starts 22:00");
    assert_eq!((sw::shown::queue::tag(&r), sw::shown::queue::short(&r)), (String::new(), "waiting for its window"));
    assert_eq!(line(&held), "waiting for its window: starts 22:00");
    let d = window(&held);
    assert_eq!((d.s("setting"), d.at("open"), d.s("opens"), d.s("words")), ("nights", &json!(false), "22:00", "waiting for its window: starts 22:00"));
    // It takes no slot: another task starts at once.
    let other = w.add("Other");
    delegate(&other);
    assert_eq!(settle(&other).s("stage"), "working");
    assert_eq!(starts(&w).len(), 1);
    sw::store::tasks::set(&other, json!({ "stage": "done" })).unwrap();
    // Just before the window nothing starts; once it is open it starts by itself, on time, so not late.
    housekeep(&w, local(0, 21, 59));
    assert_eq!(settle(&held).s("stage"), "queued");
    housekeep(&w, local(0, 22, 0) + 30_000);
    let t = settle(&held);
    assert_eq!(t.s("stage"), "working");
    assert!(!t.b("windowWait") && !t.b("windowLate"), "{t:?}");
    assert_eq!(starts(&w).len(), 2);
}

#[test]
fn run_window_now_starts_it_anyway_with_leave_until_the_window_opens() {
    let w = afternoon(json!({}));
    // Delegated with --now: it starts at once, outside its window, as you said.
    let a = w.add("Now");
    let (_, said) = run(&["delegate", &a, "--run.window", "nights", "--now"]).unwrap();
    assert_eq!(said, format!("{a}: started, outside its window (run.window nights), as you said"));
    let t = settle(&a);
    assert_eq!(t.s("stage"), "working");
    assert_eq!(t.at("windowOk").i("until"), local(0, 22, 0));
    // Held already: delegate --now starts it.
    let b = w.add("Held");
    let (_, said) = run(&["delegate", &b, "--run.window", "nights"]).unwrap();
    assert_eq!(said, format!("{b}: delegated, waiting for its window: starts 22:00"));
    run(&["delegate", &b, "--now"]).unwrap();
    assert_eq!(settle(&b).s("stage"), "working");
    // Held already: carrying on (Ctrl+R, retry) starts it too.
    let c = w.add("Carried on");
    delegate_with(&c, json!({ "run.window": "nights" }));
    assert_eq!(settle(&c).s("stage"), "queued");
    let said = sw::actions::limits::carry_on(&c, false).unwrap().msg;
    assert_eq!(said, format!("{c}: started, outside its window (run.window nights), as you said"));
    assert_eq!(settle(&c).s("stage"), "working");
}

#[test]
fn run_window_is_kept_in_one_spelling_and_a_value_it_cannot_read_names_the_nearest_right_one() {
    let _w = afternoon(json!({}));
    run(&["set", "run.window", "Mon-Fri 22:00-06:00 ,Weekends"]).unwrap();
    assert_eq!(sw::store::settings::lookup::setting_text("run.window", None), "mon-fri 22:00-06:00, weekends");
    let refused = |v: &str| run(&["set", "run.window", v]).unwrap_err();
    assert!(refused("22-06").ends_with("Did you mean 22:00-06:00?"), "{}", refused("22-06"));
    assert!(refused("wekends").ends_with("Did you mean weekends?"));
    assert!(refused("none").ends_with("Did you mean any?"));
    assert!(run(&["set", "run.windowEnd", "stop"]).unwrap_err().contains("finish, pause"));
    run(&["set", "run.windowEnd", "pause"]).unwrap();
    assert_eq!(sw::store::settings::lookup::setting_text("run.windowEnd", None), "pause");
}
