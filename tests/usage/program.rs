//! One program's own setting (`usage.extra.claude`, `usage.extra.codex`): each program goes by its own where the nearest
//! place says one, else by `usage.extra`. A Claude agent waits for its window while a Codex review runs on extra usage;
//! a working Codex agent works on; a fallback, the worker's or the reviewer's, goes by its own program's; `usage.extra`
//! alone counts for every program; and the lookup's order, `show`, `details` and how your file keeps both.
use super::review::{reviewers, window};
use super::*;
use sw::store::handovers::{handovers, last_handover};
use sw::store::settings::change::{set_value, unset_value};
use sw::store::settings::lookup::{lookup, Scope};
use sw::store::settings::places::yours;
use sw::store::settings::show::show_text;
use sw::usage::words::hm;

fn pass() -> Value { json!({ "verdict": { "meets": true, "findings": [], "text": "Looks right." } }) }
fn housekeep(w: &World) { sw::housekeeping::housekeeping(&w.socket, sw::housekeeping::Force::Yes); }
fn usage_of(id: &str) -> Value { sw::actions::finish_line::details(id).unwrap().at("usage").clone() }
/// A setting's value for the scope, and where it came from.
fn on(name: &str, scope: &Scope) -> (String, String) { let f = lookup(name, scope, &yours()).unwrap(); (text(&f.value), f.from) }
fn p(v: &str, from: &str) -> (String, String) { (v.to_string(), from.to_string()) }
fn saved() -> Value { serde_json::from_str(&read(&sw::store::files::config_dir().join("config.json"))).unwrap() }
fn has(text: &str, re: &str) -> bool { regex::Regex::new(re).unwrap().is_match(text) }

#[test]
fn usage_extra_per_program_holds_a_claude_start_for_its_window_and_reviews_on_codex_extra_usage() {
    let w = world_with(Opts { repo: true, settings: json!({ "usage.extra.claude": "defer", "usage.extra.codex": "allow" }), ..Default::default() });
    w.script(json!({ "worker": [step(1)], "reviewer": [pass()] }));
    let t0 = sw::clock::now();
    at(t0);
    let resets = t0 + HOUR;
    window("claude", t0 - 60_000, 100.0, resets);
    window("codex", t0 - 60_000, 100.0, t0 + 5 * HOUR);
    let id = w.add("Both on extra usage");
    delegate(&id);
    // Claude Code, the worker's program, goes by `defer`: the start waits for its window, holding no slot.
    let t = settle(&id);
    assert_eq!((t.s("stage"), t.at("usageWait").s("agent"), t.at("usageWait").b("you")), ("queued", "claude", false), "{}", t.s("msg"));
    assert_eq!(t.s("msg"), format!("waiting for the usage window: resets {}", hm(resets, t0)));
    assert!(starts(&w).is_empty());
    // details: what the agent's program goes by, what each program goes by, and where each was said.
    let u = usage_of(&id);
    assert_eq!((u.s("agent"), u.s("setting"), u.s("settingName"), u.s("settingFrom")), ("claude", "defer", "usage.extra.claude", "you"));
    assert_eq!(u.at("settings").clone(), json!({ "claude": { "value": "defer", "setting": "usage.extra.claude", "from": "you" }, "codex": { "value": "allow", "setting": "usage.extra.codex", "from": "you" } }));
    // At Claude's reset it starts by itself; its work done, Codex reviews it on extra usage, as allowed for Codex.
    at(resets + 1000);
    housekeep(&w);
    let mut t = settle(&id);
    if t.s("stage") != "approval" { t = settle(&id); }
    assert_eq!(t.s("stage"), "approval", "{} {}", t.s("msg"), t.at("failure"));
    assert!(sw::store::usage::on_extra("codex", resets + 1000), "Codex was still on extra usage");
    assert_eq!((reviewers(&w), t.at("limitStop").is_null(), handovers(&t).is_empty()), (vec!["codex".to_string()], true, true));
}

#[test]
fn usage_extra_per_program_leaves_a_codex_agent_at_work_on_extra_usage_under_a_tasks_usage_extra_defer() {
    let w = world();
    let id = at_work(&w, "codex", json!({ "usage.extra": "defer", "usage.extra.codex": "allow" }));
    let t0 = sw::clock::now();
    let (file, resets) = (w.base.join("rollout.jsonl"), t0 + 2 * HOUR);
    put(&id, "transcript", json!(file.display().to_string()));
    codex_turn(&file, t0, 100.0, resets, true);
    tick(&w, t0 + 10_000);
    // In one place its own setting comes before `usage.extra`: it works on, and is only marked.
    let t = task(&id);
    assert_eq!((t.s("stage"), t.at("extra").s("agent"), t.b("limitStop")), ("working", "codex", false), "{}", t.s("msg"));
    assert_eq!((usage_of(&id).s("setting"), usage_of(&id).s("settingFrom")), ("allow", "this task"));
    assert_eq!(usage_of(&id).at("settings").at("claude").clone(), json!({ "value": "defer", "setting": "usage.extra", "from": "this task (usage.extra)" }));
}

#[test]
fn usage_extra_per_program_a_worker_fallback_goes_by_its_own_programs_setting() {
    let w = world();
    w.patch(|s| s["pids"] = json!(true)); // the agent's own process in its pane, which the hand-over ends
    let id = at_work(&w, "claude", json!({ "usage.extra.claude": "defer", "usage.extra.codex": "allow", "worker.fallback": "codex" }));
    let t0 = sw::clock::now();
    let resets = (t0 + HOUR) / 1000 * 1000;
    window("codex", t0, 100.0, t0 + 2 * HOUR);
    claude_window(&id, t0, 100.0, resets);
    tick(&w, t0 + 10_000);
    // Codex is on extra usage too, but allowed it: not passed over, it takes the task over.
    let t = settle(&id);
    assert_eq!((t.s("stage"), t.s("agent"), t.b("limitStop")), ("working", "codex", false), "{} {}", t.at("fallbacks"), t.s("msg"));
    let h = last_handover(&t, "worker").unwrap();
    assert_eq!((h.s("from"), h.s("to"), h.t("why")), ("claude", "codex", format!("claude is on extra usage until {}", hm(resets, t0))));
}

#[test]
fn usage_extra_per_program_a_reviewer_fallback_goes_by_its_own_programs_setting() {
    let w = world_with(Opts { repo: true, settings: json!({ "usage.extra.codex": "defer", "usage.extra.claude": "allow", "reviewer.fallback": "claude" }), ..Default::default() });
    let t0 = sw::clock::now();
    window("codex", t0 - 60_000, 100.0, t0 + 3 * HOUR);
    window("claude", t0 - 60_000, 100.0, t0 + HOUR);
    let t = super::review::finished(&w, t0);
    // Its agent, Claude, started on extra usage, as allowed; Codex's review was handed to Claude, on extra usage too.
    assert_eq!(t.s("stage"), "approval", "{} {}", t.s("msg"), t.at("failure"));
    assert_eq!((reviewers(&w), t.s("reviewerOn")), (vec!["claude".to_string()], "claude"));
    let h = last_handover(&t, "reviewer").unwrap();
    assert_eq!((h.s("from"), h.s("to"), h.t("why")), ("codex", "claude", format!("codex is on extra usage until {}", hm(t0 + 3 * HOUR, t0))));
}

#[test]
fn usage_extra_alone_counts_for_every_program_and_a_programs_own_ask_names_itself() {
    let w = world_with(Opts { repo: true, settings: json!({ "usage.extra": "ask" }), ..Default::default() });
    w.script(json!({ "worker": [{ "status": "working" }] }));
    let t0 = sw::clock::now();
    at(t0);
    window("claude", t0 - 60_000, 100.0, t0 + HOUR);
    let id = w.add("Held");
    delegate(&id);
    let key = sw::board::keys::key_name("sendBack");
    assert_eq!(settle(&id).s("msg"), format!("waits for you: claude is on extra usage until {} (usage.extra ask); {key} starts it anyway", hm(t0 + HOUR, t0)));
    assert_eq!(usage_of(&id).at("settings").at("codex").clone(), json!({ "value": "ask", "setting": "usage.extra", "from": "you (usage.extra)" }));
    // Said for Claude Code alone, its line names that setting.
    set_value("usage.extra.claude", &json!("ask"), &Scope::you()).unwrap();
    set_value("usage.extra", &json!("allow"), &Scope::you()).unwrap();
    housekeep(&w);
    assert_eq!(settle(&id).s("msg"), format!("waits for you: claude is on extra usage until {} (usage.extra.claude ask); {key} starts it anyway", hm(t0 + HOUR, t0)));
}

#[test]
fn usage_extra_per_program_is_looked_up_place_by_place_its_own_before_usage_extra_and_shown_with_where_it_came_from() {
    let w = world_with(Opts { repo: true, repo_settings: Some(json!({ "usage": { "extra.claude": "allow", "extra": "ask" } })), ..Default::default() });
    let id = w.add("One");
    crate::settings::started(&w, &id);
    let ot = |name: &str| { let t = task(&id); on(name, &Scope::task(&t)) };
    let you = Scope::you;
    assert_eq!(on("usage.extra.claude", &you()), p("allow", "built in"));
    // The repo's file says both: its own comes first there.
    assert_eq!(ot("usage.extra.claude"), p("allow", "the repo's file"));
    assert_eq!(ot("usage.extra.codex"), p("ask", "the repo's file (usage.extra)"));
    // Your `usage.extra` comes before the repo's file: the file cannot loosen it for one program.
    set_value("usage.extra", &json!("defer"), &you()).unwrap();
    assert_eq!(ot("usage.extra.claude"), p("defer", "you (usage.extra)"));
    set_value("usage.extra.codex", &json!("allow"), &you()).unwrap();
    assert_eq!((ot("usage.extra.codex"), on("usage.extra.claude", &you())), (p("allow", "you"), p("defer", "you (usage.extra)")));
    // Your file keeps both: the program's own beside `usage.extra`, never in place of it.
    assert_eq!(saved()["usage"], json!({ "extra": "defer", "extra.codex": "allow" }));
    let mine = show_text(&you());
    assert!(has(&mine, r"\n\* usage\.extra +defer +you\n"), "{mine}");
    assert!(has(&mine, r"\n {2}usage\.extra\.claude +defer +you \(usage\.extra\)\n"), "not yours as such: {mine}");
    assert!(has(&mine, r"\n\* usage\.extra\.codex +allow +you\n"), "{mine}");
    // A nearer place's `usage.extra` comes before your program's own.
    set_value("usage.extra", &json!("ask"), &Scope::task(&task(&id))).unwrap();
    assert_eq!(ot("usage.extra.codex"), p("ask", "this task (usage.extra)"));
    set_value("usage.extra.codex", &json!("allow"), &Scope::task(&task(&id))).unwrap();
    let theirs = show_text(&Scope::task(&task(&id)));
    assert!(has(&theirs, r"\n\* usage\.extra\.codex +allow +this task\n") && has(&theirs, r"\n {2}usage\.extra\.claude +ask +this task \(usage\.extra\)\n"), "{theirs}");
    let rows: Vec<Value> = sw::store::settings::show::rows(&Scope::task(&task(&id))).iter().map(|r| r.json()).collect();
    let claude = rows.iter().find(|r| r.s("name") == "usage.extra.claude").unwrap();
    assert_eq!((claude.b("own"), claude.s("via"), claude.s("place")), (false, "usage.extra", "task"));
    // Each `unset` hands it to the next place down; yours of `usage.extra` goes, its program's own stays.
    unset_value("usage.extra.codex", &Scope::task(&task(&id))).unwrap();
    unset_value("usage.extra", &Scope::task(&task(&id))).unwrap();
    unset_value("usage.extra", &you()).unwrap();
    assert_eq!(saved()["usage"], json!({ "extra.codex": "allow" }));
    assert_eq!((ot("usage.extra.codex"), ot("usage.extra.claude")), (p("allow", "you"), p("allow", "the repo's file")));
}

#[test]
fn usage_extra_per_program_written_by_hand_below_usage_extra_is_read_and_kept() {
    let _w = world_with(Opts { repo: true, settings: json!({ "usage": { "extra": { "codex": "defer" } } }), ..Default::default() });
    assert_eq!(on("usage.extra.codex", &Scope::you()), p("defer", "you"));
    assert!(sw::store::settings::show::problems(None).is_empty(), "{:?}", sw::store::settings::show::problems(None));
    set_value("usage.extra", &json!("ask"), &Scope::you()).unwrap();
    assert_eq!(saved()["usage"], json!({ "extra.codex": "defer", "extra": "ask" }));
    assert_eq!((on("usage.extra.codex", &Scope::you()), on("usage.extra.claude", &Scope::you())), (p("defer", "you"), p("ask", "you (usage.extra)")));
}
