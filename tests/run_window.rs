//! Run windows (`run.window`, `run.windowEnd`, docs/run-window.md): a task delegated outside its window waits in the
//! queue and starts by itself when it opens, `--now` starts it anyway (before.rs); a start after the machine slept says
//! it is late, or waits for the next window (late.rs); a task queued after others meets its window once they are done
//! (after.rs); under `pause` a task stops when its window has closed and carries on by itself in the next (pause.rs);
//! a task done several times at once meets its window whole (attempts.rs).
//! What they share is here: a time of this machine's day, the passes that start and carry on, a command as typed.
use crate::common;
use common::*;
use serde_json::{json, Value};
use sw::json::*;

mod after;
mod attempts;
mod before;
mod late;
mod pause;

/// `days` after today (the real one: the test's clock moves), at h:m on this machine's clock, in ms.
pub fn local(days: i64, h: u32, m: u32) -> i64 {
    use chrono::{Local, TimeZone};
    let d = Local::now().date_naive() + chrono::Duration::days(days);
    Local.from_local_datetime(&d.and_hms_opt(h, m, 0).unwrap()).earliest().unwrap().timestamp_millis()
}
/// One housekeeping pass at `at`: it starts what may start.
pub fn housekeep(w: &World, at: i64) { sw::clock::set(Some(at)); sw::housekeeping::housekeeping(&w.socket, sw::housekeeping::Force::Yes); }
/// The limits' own pass at `at`, which carries on a task whose stop no longer holds: on the board's rows and their
/// attempts, as the keeper gives it them (keeper.rs).
pub fn limits(w: &World, at: i64) {
    sw::clock::set(Some(at));
    let c = |id: &str| { let _ = sw::actions::limits::carry_on(id, true); };
    let rows: Vec<Obj> = sw::flow::live::board_for(&w.socket).iter().flat_map(|t| std::iter::once(t.clone()).chain(t.arr("tries").iter().filter_map(|x| x.as_object().cloned()))).collect();
    sw::limits::tick::limits_tick(&rows, at, Some(&c));
}
/// A command as typed: what it answers, or why it was refused.
pub fn run(a: &[&str]) -> Result<(Value, String), String> { sw::cli::tasks::command(a[0], &sw::cli::flags::parse(&a.iter().map(|s| s.to_string()).collect::<Vec<_>>())).map_err(|e| e.msg) }
/// What a task's row says under its title.
pub fn line(id: &str) -> String { sw::shown::row_says::sentence(&sw::shown::states::as_row(&row(id))).text() }
/// The `window` entry of `details`.
pub fn window(id: &str) -> Obj { sw::actions::finish_line::details(id).unwrap().at("window").as_object().cloned().unwrap_or_default() }
/// A world whose clock stands at 14:00 today, its agents working on and on.
pub fn afternoon(settings: Value) -> World {
    let w = world_with(Opts { repo: true, settings, ..Default::default() });
    w.script(json!({ "worker": vec![json!({ "status": "working" }); 6] }));
    sw::clock::set(Some(local(0, 14, 0)));
    w
}
