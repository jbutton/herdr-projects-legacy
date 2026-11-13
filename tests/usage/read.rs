//! Detection: Codex's token counts (a window at 100% with credits while answers come is extra usage, without credits
//! and refused it is the hard stop), Claude Code's status line input (a window at 100%) and its refused request
//! (`quotaLimits`), a window whose reset passed, each program's window kept with its episode, the board's own status
//! line (it keeps the reading beside the task and shows yours), and the newest Codex conversation of the machine.
use super::*;
use sw::agents::adapter::{hook_args, Hook};
use sw::agents::read_window::{claude_reading, codex_reading, status_reading, Reading};
use sw::store::usage::{fold, now_of};

fn start(at: i64) -> Value { json!({ "timestamp": sw::clock::iso(at), "type": "event_msg", "payload": { "type": "task_started" } }) }

#[test]
fn usage_codex_reads_extra_usage_from_a_window_at_100_with_credits_and_the_hard_stop_from_its_refusal() {
    let now = sw::clock::now();
    let resets = now + 2 * HOUR;
    // 99%: on its plan still, the windows as Codex says them.
    let r = codex_reading(&[start(now - 2000), codex_count(now - 1000, 99.0, resets, true)]).unwrap();
    assert_eq!((r.extra, r.hard, r.resets, r.windows.len()), (false, false, 0, 2));
    assert_eq!((r.windows[0].name.as_str(), r.windows[0].pct, r.windows[0].resets / 1000), ("5h", 99.0, resets / 1000));
    assert_eq!(r.windows[1].name, "week");
    // 100% with credits, as this machine's team plan ran on 2026-11-13: extra usage, until the 5-hour window resets.
    let r = codex_reading(&[codex_count(now - 3000, 99.0, resets, true), codex_count(now - 1000, 100.0, resets, true)]).unwrap();
    assert_eq!((r.extra, r.hard, r.resets / 1000, r.at), (true, false, resets / 1000, now - 1000));
    // Both windows full: it is off extra usage when the later one resets.
    let mut both = codex_count(now, 100.0, resets, true);
    both["payload"]["rate_limits"]["secondary"]["used_percent"] = json!(100.0);
    assert_eq!(codex_reading(&[both]).unwrap().resets / 1000, (resets + 100 * HOUR) / 1000);
    // 100% with no credits: not extra usage, and the refusal after it is the hard stop.
    let refused = json!({ "timestamp": sw::clock::iso(now), "type": "event_msg", "payload": { "type": "error", "message": "You've hit your usage limit. Upgrade to Pro, or try again at 5:00 PM." } });
    let r = codex_reading(&[codex_count(now - 1000, 100.0, resets, false), refused]).unwrap();
    assert_eq!((r.extra, r.hard, r.resets / 1000), (false, true, resets / 1000));
    // A limit Codex names as reached is the hard stop too; another error is nothing of the window's.
    let mut reached = codex_count(now, 100.0, resets, true);
    reached["payload"]["rate_limits"]["rate_limit_reached_type"] = json!("workspace_member_credits_depleted");
    assert!(codex_reading(&[reached]).unwrap().hard);
    let other = json!({ "timestamp": sw::clock::iso(now), "type": "event_msg", "payload": { "type": "error", "message": "stream disconnected" } });
    assert!(codex_reading(&[codex_count(now - 1000, 100.0, resets, true), other]).unwrap().extra);
    assert!(codex_reading(&[start(now)]).is_none(), "no token count: nothing known");
    // As it stands later: a window whose reset passed is a fresh one, and so is what it said.
    let r = codex_reading(&[codex_count(now - 1000, 100.0, resets, true)]).unwrap();
    assert!(r.at_now(r.resets - 1).extra);
    let fresh = r.at_now(r.resets);
    assert_eq!((fresh.extra, fresh.resets, fresh.windows.len()), (false, 0, 1));
    // The board's own reading of a saved conversation carries it (agents/read_turn.rs).
    let t = sw::agents::read_turn::turn_in("codex", &[start(now - 2000), codex_count(now - 1000, 100.0, resets, true)]).unwrap();
    assert!(t.window.unwrap().extra);
}

#[test]
fn usage_claude_reads_extra_usage_from_its_status_line_and_the_hard_stop_from_a_refused_request() {
    let now = sw::clock::now();
    let resets = now + 3 * HOUR;
    let kept = |five: f64| json!({ "at": now, "rate_limits": { "five_hour": { "used_percentage": five, "resets_at": resets / 1000 }, "seven_day": { "used_percentage": 63, "resets_at": (now + 90 * HOUR) / 1000 } } });
    let r = status_reading(&kept(9.0)).unwrap();
    assert_eq!((r.extra, r.at, r.windows.iter().map(|w| w.name.clone()).collect::<Vec<_>>()), (false, now, vec!["5h".to_string(), "week".to_string()]));
    let r = status_reading(&kept(100.0)).unwrap();
    assert_eq!((r.extra, r.hard, r.resets / 1000), (true, false, resets / 1000));
    assert!(status_reading(&json!({ "at": now, "rate_limits": null })).is_none(), "an API key: no windows");
    // A refused request, as 2.1.306 saved it: the hard stop, with its exact reset.
    let refused = json!({ "type": "assistant", "timestamp": sw::clock::iso(now), "isApiErrorMessage": true, "error": "rate_limit", "apiError": "usage_limit_reached",
        "message": { "model": "<synthetic>", "role": "assistant", "content": [{ "type": "text", "text": "You've hit your individual spend limit · your session limit resets 3:50am (Europe/Warsaw)" }] },
        "quotaLimits": { "status": "rejected", "resetsAt": resets / 1000, "rateLimitType": "five_hour", "overageStatus": "rejected", "overageDisabledReason": "org_level_disabled_until", "isUsingOverage": false } });
    let r = claude_reading(&[refused.clone()]).unwrap();
    assert_eq!((r.hard, r.extra, r.resets / 1000, r.windows[0].name.as_str()), (true, false, resets / 1000, "5h"));
    // An answer served after it: that stop is over.
    let served = json!({ "type": "assistant", "timestamp": sw::clock::iso(now + 1000), "message": { "id": "m1", "model": "claude-opus-5-5", "role": "assistant", "stop_reason": "end_turn", "content": [{ "type": "text", "text": "Done." }] } });
    assert!(claude_reading(&[refused, served]).is_none());
}

#[test]
fn usage_a_program_window_keeps_its_episode_of_extra_usage_until_its_reset() {
    let _w = world();
    let t0 = sw::clock::now();
    sw::clock::set(Some(t0));
    let resets = t0 + HOUR;
    let at = |dt: i64, extra: bool| Reading { at: t0 + dt, extra, resets: if extra { resets } else { 0 }, windows: vec![sw::agents::read_window::Window { name: "5h".into(), pct: if extra { 100.0 } else { 50.0 }, resets }], ..Default::default() };
    assert!(!now_of("claude", t0).known);
    let n = fold("claude", &at(0, true), t0);
    assert_eq!((n.known, n.extra, n.since, n.resets), (true, true, t0, resets));
    // Later readings on it are the same episode; an older one changes nothing.
    assert_eq!(fold("claude", &at(60_000, true), t0 + 60_000).since, t0);
    assert_eq!(fold("claude", &at(-60_000, false), t0 + 60_000).since, t0);
    // At the reset the window is a fresh one, with nothing more read.
    assert!(!now_of("claude", resets).extra);
    // On it again in the next window: a new episode.
    let next = Reading { at: resets + 1000, extra: true, resets: resets + 5 * HOUR, windows: vec![sw::agents::read_window::Window { name: "5h".into(), pct: 100.0, resets: resets + 5 * HOUR }], ..Default::default() };
    assert_eq!(fold("claude", &next, resets + 1000).since, resets + 1000);
    assert!(!now_of("codex", resets).known, "each program its own");
}

#[test]
fn usage_the_board_status_line_keeps_the_reading_beside_the_task_and_runs_your_own() {
    let w = world();
    let id = w.add("Some work");
    let dir = sw::store::files::dir_of(&id);
    let now = sw::clock::now();
    // Its input's rate_limits are kept, once: the same again writes nothing, so `at` says when it was first so.
    let input = json!({ "rate_limits": { "five_hour": { "used_percentage": 100, "resets_at": (now + HOUR) / 1000 } } });
    assert!(sw::agents::statusline::keep(&id, "agent", &input, now));
    assert!(!sw::agents::statusline::keep(&id, "agent", &input, now + 5000));
    let kept = sw::store::files::read_json(&dir.join("window-agent.json")).unwrap();
    assert_eq!(kept.i("at"), now);
    assert!(status_reading(&kept).unwrap().extra);
    // Your own status line: the project's local settings first, then the project's, then yours; never the board's own.
    let claude = w.base.join("claude");
    std::fs::create_dir_all(&claude).unwrap();
    std::fs::write(claude.join("settings.json"), json!({ "statusLine": { "type": "command", "command": "bash ~/.claude/statusline-command.sh", "padding": 2 } }).to_string()).unwrap();
    let project = w.base.join("wt");
    std::fs::create_dir_all(project.join(".claude")).unwrap();
    assert_eq!(sw::agents::statusline::yours(&project).as_deref(), Some("bash ~/.claude/statusline-command.sh"));
    std::fs::write(project.join(".claude/settings.json"), json!({ "statusLine": { "type": "command", "command": "echo project" } }).to_string()).unwrap();
    assert_eq!(sw::agents::statusline::yours(&project).as_deref(), Some("echo project"));
    std::fs::write(project.join(".claude/settings.local.json"), json!({ "statusLine": { "type": "command", "command": "/x/herdr-projects statusline --id a --role agent --token t" } }).to_string()).unwrap();
    assert_eq!(sw::agents::statusline::yours(&project), None);
    // The settings file the board gives a Claude agent names its status line, with your padding, and keeps its hooks.
    let h = Hook { shim: "/x/herdr-projects".into(), id: id.clone(), role: "agent".into(), token: "tok".into(), round: 0, dir: dir.clone() };
    let args = hook_args("claude", &h).unwrap().unwrap();
    let file = sw::store::files::read_json(Path::new(&args[1])).unwrap();
    assert_eq!(file.at("statusLine").s("command"), format!("'/x/herdr-projects' 'statusline' '--id' '{id}' '--role' 'agent' '--token' 'tok'"));
    assert_eq!((file.at("statusLine").s("type"), file.at("statusLine").i("padding")), ("command", 2));
    assert!(file.at("hooks").at("Stop").is_array());
}

#[test]
fn real_programs_usage_the_board_status_line_prints_what_your_command_prints() {
    let dir = std::env::temp_dir();
    assert_eq!(sw::agents::statusline::pass_on("read -r x; printf 'dir %s · %s' \"$(basename \"$PWD\")\" \"$x\"", "{\"a\":1}\n", &dir), format!("dir {} · {{\"a\":1}}", dir.file_name().unwrap().to_string_lossy()));
}

#[test]
fn usage_the_newest_codex_conversation_of_the_machine_says_its_window_before_any_task_ran_it() {
    let w = world();
    use chrono::TimeZone;
    let now = sw::clock::now();
    sw::clock::set(Some(now));
    let day = chrono::Local.timestamp_millis_opt(now).single().unwrap().format("%Y/%m/%d").to_string();
    let folder = w.base.join("codex/sessions").join(day);
    codex_turn(&folder.join("rollout-old.jsonl"), now - HOUR, 50.0, now + HOUR, true);
    std::thread::sleep(std::time::Duration::from_millis(20)); // the newer file by its time on disk
    codex_turn(&folder.join("rollout-new.jsonl"), now - 1000, 100.0, now + HOUR, true);
    assert!(sw::agents::read_window::codex_newest().unwrap().extra);
    // The pass reads it once a minute, and keeps it as Codex's window.
    tick(&w, now);
    assert!(now_of("codex", now).extra);
    assert_eq!(sw::usage::words::header(now), format!("Codex: extra usage since {}, resets {}", sw::usage::words::hm(now - 1000, now), sw::usage::words::hm(now + HOUR, now)));
}
