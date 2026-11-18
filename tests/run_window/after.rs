//! A task queued after others (`--after`) whose window is closed when they are done: it waits for its window, not
//! after them, holding no slot, and starts at the opening, told what they did.
use super::*;

#[test]
fn run_window_an_after_chain_whose_window_is_closed_waits_for_the_window() {
    let w = afternoon(json!({}));
    let first = w.add("First");
    let then = w.add("Then");
    let (_, said) = run(&["delegate", &then, "--after", &first, "--run.window", "nights"]).unwrap();
    assert!(said.contains("queued after"), "{said}");
    assert_eq!(task(&then).s("stage"), "after");
    // The first is done at 15:00: the window is closed, so it waits for that, holding no slot.
    sw::store::tasks::set(&first, json!({ "stage": "done", "msg": "Did it." })).unwrap();
    housekeep(&w, local(0, 15, 0));
    let t = settle(&then);
    assert_eq!((t.s("stage"), t.s("msg")), ("queued", "waiting for its window: starts 22:00"));
    assert_eq!(starts(&w).len(), 0);
    // At the opening it starts by itself, and its agent is told what came before it.
    housekeep(&w, local(0, 22, 0) + 30_000);
    let t = settle(&then);
    assert_eq!(t.s("stage"), "working");
    let told = prompts(&w, t.at("link").s("pane")).join("\n");
    assert!(told.contains("This task waited for the tasks below to be done, and they are."), "{told}");
}
