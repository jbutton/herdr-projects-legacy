//! What `run.window` may be (`Ty::Window`, docs/run-window.md): `any`, or windows in this machine's time zone, each
//! days and hours as `autopilot.hours` writes them (pilot_values.rs: `mon-fri 22:00-06:00`, `sat,sun 10:00-16:00`), with
//! days alone (all day), the words `nights` (22:00-06:00), `weekends` and `weekdays`, and several windows with commas
//! between them. A value is read into windows, kept in one plain spelling, or refused with the nearest right one.
use super::pilot_values::{clock, day, DAYS};
use crate::err::{Error, Res};
use crate::json::*;

pub const ANY: &str = "any";
/// What a value may be, as a refusal says it.
pub const HOW: &str = "any, or windows in this machine's time zone such as nights, weekends, mon-fri 22:00-06:00 or sat,sun 10:00-16:00, with commas between them";
const WORDS: [&str; 4] = ["nights", "weekends", "weekdays", ANY];
const NIGHTS: (u32, u32) = (22 * 60, 6 * 60);

/// One window: the days it begins on (0 Monday … 6 Sunday) and the minutes of the day from and to. `to` at or before
/// `from`: across midnight, the night belongs to the day it began on; 0 to 1440: all day.
#[derive(Clone, Debug, PartialEq)]
pub struct Window { pub days: Vec<u32>, pub from: u32, pub to: u32 }
/// A value as read: its windows (none: `any`) and its kept spelling.
#[derive(Clone, Debug, PartialEq)]
pub struct Read { pub windows: Vec<Window>, pub text: String }

/// `mon`, `fri-mon`: one day or a range, as one piece of a day list.
fn day_item(w: &str) -> Option<Vec<u32>> {
    match w.split_once('-') {
        Some((a, b)) => { let (a, b) = (day(a)?, day(b)?); let mut out = vec![]; let mut x = a; loop { out.push(x); if x == b { break; } x = (x + 1) % 7; } Some(out) }
        None => day(w).map(|d| vec![d]),
    }
}
/// The days a window's first word names: a day list (`mon-fri`, `sat,sun`), `weekends` or `weekdays`.
fn days_of(w: &str) -> Option<Vec<u32>> {
    match w {
        "weekends" => Some(vec![5, 6]),
        "weekdays" => Some((0..5).collect()),
        _ => { let mut out = vec![]; for p in w.split(',') { out.extend(day_item(p)?); } out.sort(); out.dedup(); Some(out) }
    }
}
/// `22:00-06:00` as minutes from and to.
fn hours_of(w: &str) -> Option<(u32, u32)> { let (a, b) = w.split_once('-')?; Some((clock(a)?, clock(b)?)) }
fn window(text: &str) -> Result<Window, String> {
    let words: Vec<&str> = text.split(' ').collect();
    let (days, rest) = match days_of(words[0]) { Some(d) => (Some(d), &words[1..]), None => (None, &words[..]) };
    let (from, to) = match rest {
        [] => (0, 1440),
        ["nights"] => NIGHTS,
        ["nights", h] | [h] => match hours_of(h) { Some(x) => x, None => return Err(format!("\"{h}\" is not a day, hours or window")) },
        _ => return Err(format!("\"{text}\" is not one window: days, then hours (mon-fri 22:00-06:00)")),
    };
    if from == to { return Err(format!("\"{text}\" begins where it ends")); }
    Ok(Window { days: days.unwrap_or_else(|| (0..7).collect()), from, to })
}
/// Reads a value. Days on their own before a window that starts with days are that window's days too, as
/// `autopilot.hours` reads `sat,sun 10:00-16:00`; elsewhere they are those days, all day.
pub fn read(typed: &str) -> Result<Read, String> {
    let s = typed.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase();
    if s.is_empty() { return Err("it needs a value".into()); }
    if s == ANY { return Ok(Read { windows: vec![], text: ANY.into() }); }
    let pieces: Vec<String> = s.split(',').map(|p| p.trim().to_string()).collect();
    if pieces.iter().any(|p| p.is_empty()) { return Err(format!("\"{s}\" has nothing between two commas")); }
    let starts_days = |p: &str| day_item(p.split(' ').next().unwrap_or("")).is_some();
    let (mut kept, mut held): (Vec<String>, Vec<String>) = (vec![], vec![]);
    for (i, p) in pieces.iter().enumerate() {
        if !p.contains(' ') && starts_days(p) && pieces.get(i + 1).is_some_and(|n| starts_days(n)) { held.push(p.clone()); continue; }
        let w = if held.is_empty() { p.clone() } else { format!("{},{p}", held.join(",")) };
        held.clear();
        kept.push(w);
    }
    if kept.iter().any(|w| w == ANY) { return Err("any stands alone: it is no window at all".into()); }
    let windows = kept.iter().map(|w| window(w)).collect::<Result<Vec<_>, _>>()?;
    Ok(Read { windows, text: kept.join(", ") })
}
/// The windows of a kept value; `any` and a value that cannot be read (a file edited by hand) have none: no window.
pub fn windows_of(text: &str) -> Vec<Window> { read(text).map(|r| r.windows).unwrap_or_default() }
/// The nearest right word for one that cannot be read: `22-06` → `22:00-06:00`, `saturday` → `sat`, `nigths` → `nights`.
fn meant(w: &str) -> Option<String> {
    if w == "none" || w == "always" { return Some(ANY.into()); }
    // A day's whole name, or a longer short one: `saturday`, `tues`.
    let named = |w: &str| DAYS.iter().find(|d| w.len() > 3 && w.starts_with(*d) && w.chars().all(|c| c.is_ascii_alphabetic())).map(|d| d.to_string()).or_else(|| day(w).map(|_| w.to_string()));
    if let Some((a, b)) = w.split_once('-') {
        let hm = |x: &str| -> Option<u32> {
            let (num, pm) = if let Some(n) = x.strip_suffix("pm") { (n, Some(true)) } else if let Some(n) = x.strip_suffix("am") { (n, Some(false)) } else { (x, None) };
            let num = num.replace('.', ":");
            let (h, m) = match num.split_once(':') { Some((h, m)) => (h.parse::<u32>().ok()?, m.parse::<u32>().ok()?), None => (num.parse::<u32>().ok()?, 0) };
            let h = match pm { Some(true) if h < 12 => h + 12, Some(false) if h == 12 => 0, _ => h };
            (h <= 24 && m < 60 && h * 60 + m <= 1440).then_some(h * 60 + m)
        };
        if let (Some(a), Some(b)) = (hm(a), hm(b)) { if a != b { return Some(format!("{:02}:{:02}-{:02}:{:02}", a / 60, a % 60, b / 60, b % 60)); } }
        if let (Some(a), Some(b)) = (named(a), named(b)) { return Some(format!("{a}-{b}")); }
        return None;
    }
    if let Some(d) = named(w) { return Some(d); }
    // A word mistyped: the nearest of the words a value may hold (never for numbers: `9` is no day).
    if !w.chars().all(|c| c.is_ascii_alphabetic()) { return None; }
    let all: Vec<&str> = WORDS.iter().copied().chain(DAYS.iter().copied()).collect();
    crate::store::settings::values::nearest(w, &all)
}
/// Each word that cannot be read put right, as long as that leads somewhere: the whole value that would be taken.
fn put_right(s: &str) -> Option<String> {
    let mut cur = s.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase();
    for _ in 0..6 {
        if read(&cur).is_ok() { return (cur != s.to_lowercase()).then_some(cur); }
        let fine = |w: &str| WORDS.contains(&w) || day_item(w).is_some() || hours_of(w).is_some();
        let bad = cur.split([' ', ',']).find(|w| !w.is_empty() && !fine(w))?.to_string();
        let good = meant(&bad)?;
        cur = cur.split_inclusive([' ', ',']).map(|w| { let (word, sep) = w.split_at(w.trim_end_matches([' ', ',']).len()); if word == bad { format!("{good}{sep}") } else { w.to_string() } }).collect();
    }
    None
}
/// The value as kept, or a refusal that names the nearest right one.
pub fn check(name: &str, v: &Value) -> Res<Value> {
    let typed = text(v);
    match read(&typed) {
        Ok(r) => Ok(json!(r.text)),
        Err(why) => {
            let hint = put_right(&typed).map(|x| format!(" Did you mean {x}?")).unwrap_or_default();
            Err(Error::refused(format!("{name}: {why}. It is {HOW}.{hint}")))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn kept(s: &str) -> String { read(s).unwrap().text }
    #[test]
    fn values_are_read_and_kept_in_one_spelling() {
        assert_eq!(kept("Mon-Fri  22:00-06:00 ,Weekends"), "mon-fri 22:00-06:00, weekends");
        assert_eq!(read("sat,sun 10:00-16:00").unwrap().windows, vec![Window { days: vec![5, 6], from: 600, to: 960 }]);
        assert_eq!(read("sat, sun").unwrap().windows, vec![Window { days: vec![5, 6], from: 0, to: 1440 }]);
        assert_eq!(kept("mon-fri 22:00-06:00, sat"), "mon-fri 22:00-06:00, sat");
        assert_eq!(kept("sat, mon-fri 22:00-06:00"), "sat,mon-fri 22:00-06:00");
        assert_eq!(read("nights").unwrap().windows[0], Window { days: (0..7).collect(), from: 1320, to: 360 });
        assert_eq!(read("nights 23:00-07:00").unwrap().windows[0].from, 1380);
        assert_eq!(read("mon-fri nights").unwrap().windows[0].days, vec![0, 1, 2, 3, 4]);
        assert_eq!(read("weekdays").unwrap().windows[0], Window { days: vec![0, 1, 2, 3, 4], from: 0, to: 1440 });
        assert!(read("any").unwrap().windows.is_empty());
        assert_eq!(read("fri-mon 00:00-24:00").unwrap().windows[0].days, vec![0, 4, 5, 6]);
    }
    #[test]
    fn a_value_it_cannot_read_names_the_nearest_right_one() {
        let refused = |s: &str| check("run.window", &json!(s)).unwrap_err().msg;
        assert!(refused("nigths").ends_with("Did you mean nights?"), "{}", refused("nigths"));
        assert!(refused("wekends").ends_with("Did you mean weekends?"));
        assert!(refused("saturday").ends_with("Did you mean sat?"));
        assert!(refused("22-06").ends_with("Did you mean 22:00-06:00?"));
        assert!(refused("10pm-6am").ends_with("Did you mean 22:00-06:00?"));
        assert!(refused("mon-fri 9-17").ends_with("Did you mean mon-fri 09:00-17:00?"));
        assert!(refused("saturday,sunday").ends_with("Did you mean sat,sun?"), "{}", refused("saturday,sunday"));
        assert!(refused("none").ends_with("Did you mean any?"));
        assert!(refused("22:00-22:00").contains("begins where it ends"));
        assert!(refused("any, nights").contains("any stands alone"));
        assert!(refused("nights,").contains("nothing between two commas"));
    }
}
