//! `allow`, the built-in value: a task whose agent moves onto extra usage works on, and the board says so: its row
//! (`On extra usage since 14:10`), `details` (since when, until when), `board.notify` once per task and episode, not on
//! every turn, and the board's header (`Codex: extra usage since 14:10, resets 17:00`).
use super::*;
use sw::usage::words::hm;

thread_local! { static RAN: std::cell::RefCell<Vec<(String, Vec<String>)>> = const { std::cell::RefCell::new(vec![]) }; }
/// The commands `board.notify` ran so far (a detached command is a job of its own in a test, kept here, not run).
fn ran() -> Vec<(String, Vec<String>)> {
    flush();
    sw::runner::drain_jobs(&|job, id, extra| { if job == "notify" { RAN.with(|r| r.borrow_mut().push((id.to_string(), extra.to_vec()))); } else { sw::jobs::run_job(job, id, extra); } flush(); });
    RAN.with(|r| r.borrow().clone())
}
fn notify() { sw::notify::notify_pass(&sw::store::tasks::list(Default::default())); }

#[test]
fn usage_allow_works_on_and_says_so_on_its_row_in_details_once_to_board_notify_and_in_the_header() {
    let w = world_with(Opts { repo: true, settings: json!({ "board.notify": "say-it" }), ..Default::default() });
    RAN.with(|r| r.borrow_mut().clear());
    let id = at_work(&w, "codex", json!({}));
    let t0 = sw::clock::now();
    let (file, resets) = (w.base.join("rollout.jsonl"), t0 + 2 * HOUR);
    put(&id, "transcript", json!(file.display().to_string()));
    // On its plan: nothing said.
    codex_turn(&file, t0, 99.0, resets, true);
    tick(&w, t0 + 10_000);
    assert!(!task(&id).b("extra"));
    assert_eq!(sw::usage::words::header(t0 + 10_000), "Codex: 5h 99%, week 78%");
    notify();
    assert!(ran().is_empty());
    // Past its window, on credits: it works on, and is marked since then.
    codex_turn(&file, t0 + 20_000, 100.0, resets, true);
    tick(&w, t0 + 30_000);
    let t = task(&id);
    assert_eq!((t.s("stage"), t.at("extra").s("agent"), t.at("extra").i("since"), t.at("extra").i("resets")), ("working", "codex", t0 + 20_000, resets / 1000 * 1000));
    let r = row(&id);
    assert_eq!(r.s("state"), "working");
    let says = sw::shown::row_says::sentence(&sw::shown::states::as_row(&r)).text();
    assert!(says.starts_with(&format!("On extra usage since {}", hm(t0 + 20_000, t0))), "{says}");
    // details: since when, until when, and what the setting says.
    let d = sw::actions::finish_line::details(&id).unwrap();
    let u = d.at("usage");
    assert_eq!((u.s("agent"), u.s("setting"), u.b("onExtraUsage"), u.t("extraSince"), u.t("resets")), ("codex", "allow", true, hm(t0 + 20_000, t0), hm(resets, t0)));
    assert_eq!(u.s("extraSinceAt"), sw::clock::iso(t0 + 20_000));
    // The header says it for the program.
    assert_eq!(sw::usage::words::header(t0 + 30_000), format!("Codex: extra usage since {}, resets {}", hm(t0 + 20_000, t0), hm(resets, t0)));
    let mut b = common::board::open(&w.root, 30);
    let head = b.screen(200).into_iter().next().unwrap_or_default();
    assert!(head.contains(&format!("Codex: extra usage since {}", hm(t0 + 20_000, t0))), "{head}");
    // board.notify: once, with the event's name.
    notify();
    let got = ran();
    assert_eq!(got.len(), 1, "{got:?}");
    assert_eq!(got[0].0, "say-it");
    assert!(got[0].1.contains(&"HERDR_PROJECTS_EVENT=extra usage".to_string()), "{:?}", got[0].1);
    // Not again on later readings or later turns of the same episode.
    for n in 1..4 { codex_turn(&file, t0 + 30_000 + n * 10_000, 100.0, resets, true); tick(&w, t0 + 35_000 + n * 10_000); notify(); }
    assert_eq!(ran().len(), 1);
    assert_eq!(task(&id).s("stage"), "working");
    // The window resets: the mark goes; on it again in the next window, it is told once more.
    let next = resets + 5 * HOUR;
    codex_turn(&file, resets + 1000, 3.0, next, true);
    tick(&w, resets + 10_000);
    assert!(!task(&id).b("extra"));
    assert_eq!(sw::usage::words::header(resets + 10_000), "Codex: 5h 3%, week 78%");
    codex_turn(&file, next - 60_000, 100.0, next, true);
    tick(&w, next - 50_000);
    notify();
    assert_eq!(ran().len(), 2);
    assert_eq!(task(&id).s("stage"), "working");
}

#[test]
fn usage_extra_is_a_setting_like_every_other_and_says_where_it_does_nothing() {
    use sw::store::settings::{change::set_value, lookup::{setting_text, Scope}};
    let w = world_with(Opts { repo: true, repo_settings: Some(json!({ "usage.extra": "defer" })), ..Default::default() });
    let id = w.add("Some work");
    // Built in `allow`; the repo's own file may say it, as the task asks; your own places come first.
    assert_eq!(setting_text("usage.extra", None), "allow");
    assert!(sw::store::settings_list::repo_facts().contains(&"usage.extra"));
    assert_eq!(setting_text("usage.extra", Some(&task(&id))), "defer");
    let e = set_value("usage.extra", &json!("sometimes"), &Scope::you()).unwrap_err().msg;
    assert!(e.contains("allow") && e.contains("defer"), "{e}");
    set_value("usage.extra", &json!("ask"), &Scope::you()).unwrap();
    assert_eq!(setting_text("usage.extra", Some(&task(&id))), "ask");
    set_value("usage.extra", &json!("allow"), &Scope::task(&task(&id))).unwrap();
    assert_eq!(setting_text("usage.extra", Some(&task(&id))), "allow");
    // A program whose window cannot be read: it does nothing, and show says so.
    set_value("usage.extra", &json!("defer"), &Scope::task(&task(&id))).unwrap();
    sw::store::tasks::patch(&id, |t| { t.put("agent", "pi"); }).unwrap();
    let rows = sw::store::settings::show::rows(&Scope::task(&task(&id)));
    let row = rows.iter().find(|r| r.name == "usage.extra").unwrap();
    assert_eq!(row.note, "does nothing for pi: its usage window cannot be seen");
}
