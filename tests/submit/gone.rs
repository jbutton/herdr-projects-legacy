//! Words handed to an agent whose program has ended: its pane back at the shell while herdr still names the agent. The
//! board types nothing into that shell (a shell runs it), nor presses enter there. A start that its usage window ends
//! under `usage.extra defer` waits for that window, as a stop mid-run does, and carries on by itself at the reset; a
//! window known to be used up holds the start back before it.
use crate::common;
use common::*;
use serde_json::{json, Value};
use sw::json::*;
use sw::submit::gone::ENDED;
use sw::submit::NOT_STARTED;

const HOUR: i64 = 3_600_000;
fn drain() { flush(); sw::runner::drain_jobs(&|job, id, extra| { sw::jobs::run_job(job, id, extra); flush(); }); }
fn quiet(settings: Value) -> World {
    let mut s = json!({ "review.when": "never" });
    for (k, v) in settings.as_object().cloned().unwrap_or_default() { s[k] = v; }
    world_with(Opts { repo: true, settings: s, ..Default::default() })
}
fn count(w: &World, re: &str) -> usize { let r = regex::Regex::new(re).unwrap(); calls(w).iter().filter(|c| r.is_match(c)).count() }
/// What went into the task's pane while its program had ended.
fn into_shell(w: &World, id: &str) -> Vec<Value> {
    let p = task(id).at("link").t("pane");
    w.state().arr("panes").iter().find(|x| x.s("pane_id") == p).map(|x| x.arr("shellTyped").to_vec()).unwrap_or_default()
}
/// Codex's window as it writes it once its week is used up and it has no credits to go on.
fn codex_used_up(at: i64, resets: i64) -> Value {
    json!({ "timestamp": sw::clock::iso(at), "type": "event_msg", "payload": { "type": "token_count", "info": null, "rate_limits": { "limit_id": "codex",
        "primary": { "used_percent": 25.0, "window_minutes": 300, "resets_at": (at + HOUR) / 1000 },
        "secondary": { "used_percent": 100.0, "window_minutes": 10080, "resets_at": resets / 1000 },
        "credits": { "has_credits": false, "unlimited": false, "balance": null }, "rate_limit_reached_type": "secondary" } } })
}

#[test]
fn a_program_that_ended_as_it_started_gets_nothing_typed_into_the_shell_left_behind() {
    let w = quiet(json!({}));
    w.patch(|s| s["exitsAtStart"] = json!(["claude"]));
    let id = w.add("Never into a shell");
    delegate_with(&id, json!({ "worker.agent": "claude" }));
    drain();
    assert!(into_shell(&w, &id).is_empty(), "typed into its shell: {:?}", into_shell(&w, &id));
    assert_eq!(count(&w, r"^agent prompt"), 0);
    let t = task(&id);
    assert_eq!((t.s("stage"), t.s("msg"), t.at("failure").s("stage")), ("failed", ENDED, "send"));
    assert!(t.at("unsent").s("text").contains("Never into a shell") && !t.at("unsent").b("typed"), "kept for the retry: {}", t.at("unsent"));
    assert!(t.at("failure").s("detail").contains("nothing was typed into its pane"));
}

#[test]
fn a_program_that_ends_with_its_text_in_the_box_gets_no_enter_pressed_into_the_shell() {
    let w = quiet(json!({}));
    unsafe { std::env::set_var("HERDR_PROJECTS_READY_MS", "0"); std::env::set_var("HERDR_PROJECTS_SUBMIT_MS", "3000"); }
    w.patch(|s| s["swallow"] = json!(1)); // typed, its enter lost
    let id = w.add("Ended with it in the box");
    delegate_with(&id, json!({ "worker.agent": "claude" }));
    drain();
    assert!(task(&id).at("sent").is_object());
    let p = task(&id).at("link").t("pane");
    w.patch(|s| { for x in s["panes"].as_array_mut().unwrap() { if x.s("pane_id") == p { x["ended"] = json!(true); } } }); // herdr still names it
    sw::store::tasks::patch(&id, |t| { let at = t.at("sent").i("at") - 60_000; t.obj_mut("sent").put("at", at); }).unwrap();
    sw::housekeeping::housekeeping(&w.socket, sw::housekeeping::Force::Yes);
    assert!(into_shell(&w, &id).is_empty(), "keys into its shell: {:?}", into_shell(&w, &id));
    assert_eq!(count(&w, r"^agent send-keys"), 0);
    let t = task(&id);
    assert_eq!((t.s("stage"), t.s("msg")), ("failed", NOT_STARTED));
}

#[test]
fn a_start_its_used_up_window_ends_under_defer_waits_for_the_window_and_carries_on_at_the_reset() {
    let w = quiet(json!({ "usage.extra": "defer" }));
    let t0 = sw::clock::now();
    let resets = (t0 + 2 * HOUR) / 1000 * 1000;
    // Codex comes up, writes that its week is used up, and ends: the board learns it only now.
    w.patch(|s| { s["exitsAtStart"] = json!(["codex"]); s["codexAtStart"] = codex_used_up(t0, resets); });
    let id = w.add("Waits for Wednesday");
    delegate_with(&id, json!({ "worker.agent": "codex" }));
    drain();
    assert!(into_shell(&w, &id).is_empty(), "typed into its shell: {:?}", into_shell(&w, &id));
    let t = task(&id);
    let ls = t.at("limitStop");
    assert_eq!((t.s("stage"), ls.s("what"), ls.s("how"), ls.b("hard"), ls.i("resets")), ("stopped", "usage", "defer", true, resets), "{}", t.s("msg"));
    assert_eq!(t.s("msg"), format!("waiting for the usage window: resets {}", sw::usage::words::hm(resets, t0)));
    assert!(t.at("unsent").s("text").contains("Waits for Wednesday"));
    // herdr has caught up (no agent in the pane), and the window resets: it starts again with its own text.
    let p = t.at("link").t("pane");
    w.patch(|s| { s["exitsAtStart"] = json!([]); for x in s["panes"].as_array_mut().unwrap() { if x.s("pane_id") == p { x["agent"] = Value::Null; x["ended"] = Value::Null; } } });
    crate::usage::limits(&w, resets + 1000);
    drain();
    let t = task(&id);
    assert!(["working", "done"].contains(&t.s("stage")) && !t.has("limitStop"), "carried on: {} {}", t.s("stage"), t.s("msg"));
    assert_eq!(count(&w, r"(?s)^agent prompt .*Waits for Wednesday"), 1, "its own text, once, to the program started again: {:?}", calls(&w));
    assert!(into_shell(&w, &id).is_empty());
}

#[test]
fn a_window_known_used_up_holds_the_start_back_under_defer() {
    let w = quiet(json!({ "usage.extra": "defer" }));
    let t0 = sw::clock::now();
    let resets = (t0 + 2 * HOUR) / 1000 * 1000;
    common::endings::codex_today(&codex_used_up(t0 - 1000, resets));
    let id = w.add("Held back");
    delegate_with(&id, json!({ "worker.agent": "codex" }));
    drain();
    let t = task(&id);
    assert_eq!((t.s("stage"), t.s("msg")), ("queued", format!("waiting for the usage window: resets {}", sw::usage::words::hm(resets, t0)).as_str()));
    assert_eq!(count(&w, r"^agent start"), 0, "{:?}", calls(&w));
}
