//! The history of each program's window (`usage-log.jsonl`, `herdr-projects usage`): a line for each reading that
//! changed something, none for one that only came later, each program apart, and where extra usage began and ended.
use super::*;
use sw::agents::read_window::{Reading, Window};
use sw::store::usage::fold;
use sw::store::usage_log::{say, since};

fn win(name: &str, pct: f64, resets: i64) -> Window { Window { name: name.into(), pct, resets } }

#[test]
fn usage_log_keeps_a_line_for_each_change_of_a_window_and_says_where_extra_usage_began_and_ended() {
    let _w = world();
    let t0 = sw::clock::now();
    let (five, week) = (t0 + 3 * HOUR, t0 + 100 * HOUR);
    let read = |at: i64, pct: f64, resets: i64| {
        let extra = pct >= 100.0;
        fold("claude", &Reading { at, windows: vec![win("5h", pct, resets), win("week", 40.0, week)], extra, resets: if extra { resets } else { 0 }, ..Default::default() }, at);
    };
    read(t0, 90.0, five);
    read(t0 + 60_000, 90.0, five); // the same, only later: no line
    read(t0 + 120_000, 100.0, five); // the 5-hour window used up, answers still served: extra usage
    read(t0 + 180_000, 100.0, five); // still on it: no line
    read(five + 60_000, 2.0, five + 5 * HOUR); // a fresh window
    fold("codex", &Reading { at: t0 + 30_000, windows: vec![win("5h", 10.0, five)], ..Default::default() }, t0 + 30_000);
    let lines = since("claude", t0);
    assert_eq!(lines.iter().map(|l| (l.i("at") - t0, l.b("extra"))).collect::<Vec<_>>(), vec![(0, false), (120_000, true), (five + 60_000 - t0, false)]);
    assert_eq!(lines[1].i("since"), t0 + 120_000, "the episode is kept with its line");
    assert_eq!((since("", t0).len(), since("codex", t0).len(), since("claude", t0 + 100_000).len()), (4, 1, 2));
    let text = say(&lines);
    let said: Vec<&str> = text.lines().collect();
    assert_eq!(said.len(), 3, "{text}");
    assert!(said[0].contains("claude") && said[0].contains("5h 90%") && said[0].contains("week 40%") && !said[0].contains("EXTRA"), "{text}");
    assert!(said[1].contains("5h 100%") && said[1].ends_with("<< ON EXTRA USAGE from here"), "{text}");
    assert!(said[2].contains("5h 2%") && said[2].ends_with(">> off extra usage"), "{text}");
    assert!(say(&[]).starts_with("No readings kept"));
}
