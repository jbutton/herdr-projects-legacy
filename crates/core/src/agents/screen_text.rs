//! The words on a program's screen, for a program whose saved conversation the board cannot read. What a terminal
//! program draws is told from what it says by its shape alone: a line that is mostly drawing (a rule, a box's edge, a
//! logo made of dots or blocks, a bar) holds few letters among its marks, and is left out; a box's edges are taken off.
//! And what the agent said last, without the program's input box (with what is typed in it), the lines it draws under
//! it, or Claude Code's line after each turn and its notice over the box.
const EDGE: &[char] = &['│', '┃', '|', '╭', '╮', '╰', '╯', '┌', '┐', '└', '┘', '─', '━', '═'];
fn edge_trim(s: &str) -> &str { s.trim_matches(|c: char| c.is_whitespace() || EDGE.contains(&c)) }
fn drawn(line: &str) -> bool {
    let marks: Vec<char> = line.chars().filter(|c| !c.is_whitespace()).collect();
    let words = marks.iter().filter(|c| c.is_alphanumeric()).count();
    !marks.is_empty() && words * 2 < marks.len()
}
pub fn screen_text(screen: &str) -> String {
    let mut out: Vec<String> = vec![];
    for raw in screen.split('\n') {
        let line = edge_trim(raw);
        if !raw.trim().is_empty() && (line.is_empty() || drawn(line)) { continue; }
        if line.is_empty() && out.last().map(|l| l.is_empty()).unwrap_or(true) { continue; } // one empty line between blocks
        if line.is_empty() { out.push(String::new()); continue; }
        let mut s = raw.trim_start();
        if let Some(r) = s.strip_prefix(['│', '┃', '|']) { s = r.strip_prefix(' ').unwrap_or(r); } else { s = raw; }
        let s = s.trim_end();
        let s = s.strip_suffix(['│', '┃', '|']).map(|x| x.trim_end()).unwrap_or(s);
        out.push(s.to_string());
    }
    while out.last().map(|l| l.is_empty()).unwrap_or(false) { out.pop(); }
    out.join("\n")
}
/// A rule across the screen: a line between two parts, or a box's top or bottom edge.
fn rule(line: &str) -> bool {
    let s = line.trim().trim_matches(['╭', '╮', '╰', '╯', '┌', '┐', '└', '┘']);
    s.chars().count() >= 8 && s.chars().all(|c| ['─', '━', '═'].contains(&c))
}
/// Claude Code's line after each turn, `✻ Worked for 8m 46s · done 8:30 PM`: the program's, never the agent's words.
fn turn_line(line: &str) -> bool {
    let mut w = line.split_whitespace();
    w.next() == Some("✻") && w.next().is_some_and(|v| v.starts_with(char::is_uppercase)) && w.next() == Some("for")
        && w.next().is_some_and(|d| d.starts_with(|c: char| c.is_ascii_digit()))
}
/// The words of Claude Code's notices seen in the row right over its input box (`new task? /clear to save 599.8k
/// tokens`, `✔ Update installed · Restart to update`).
const NOTICES: &[&str] = &["/clear to save", "Update installed", "Restart to update"];
/// Claude Code's notice in the row right over its input box: pushed to the right, it ends where the box's top rule
/// ends, give or take a margin, and never past it. It is told by its words, not by where it stands alone: a line of the
/// agent's own there, indented in a pane of any width, is kept, and so is a notice with other words.
fn notice(row: &str, top: &str) -> bool {
    let (width, end) = (top.trim_end().chars().count(), row.trim_end().chars().count());
    row.starts_with(' ') && end <= width && end + 4 >= width && NOTICES.iter().any(|n| row.contains(n))
}
/// The words an agent left on its screen, for a turn that ended with nothing else: what stands above its program's
/// input box. The box is its prompt mark near the end of the screen (Claude Code's `❯`, Codex's `›`, or `>` just under
/// a rule), with the rule over it; what is drawn under it (a mode line, a status line: Claude Code's "⏵⏵ bypass
/// permissions on") is the program's, never the agent's words. A box with more than eight lines typed into it has its
/// prompt further up: it is then found from its bottom rule near the end, up to the prompt line under its top rule.
/// Claude Code's line after each turn, and its notice right over the box, are left out too.
pub fn agent_words(screen: &str) -> String {
    let lines: Vec<&str> = screen.split('\n').collect();
    let below = |i: usize| lines[i + 1..].iter().filter(|l| !l.trim().is_empty()).count();
    let above = |i: usize| (0..i).rev().find(|j| !lines[*j].trim().is_empty());
    let ruled = |i: usize| above(i).filter(|j| rule(lines[*j]));
    let near_end = || (0..lines.len()).rev().take_while(move |i| below(*i) <= 8);
    let marked = |i: usize| edge_trim(lines[i]).starts_with(['❯', '›']);
    let quoted = |i: usize| edge_trim(lines[i]).starts_with('>');
    let input = near_end().find(|i| marked(*i) || (quoted(*i) && ruled(*i).is_some())).or_else(|| {
        // a tall box: a bare `>` counts only between a box's edges, so that the agent's own quote under its own rule is
        // not taken for one
        let edged = |i: usize| lines[i].trim_start().starts_with(['│', '┃', '|']);
        let bottom = near_end().find(|i| rule(lines[*i]))?;
        (0..bottom).rev().take_while(|i| !rule(lines[*i])).find(|i| ruled(*i).is_some() && (marked(*i) || (quoted(*i) && edged(*i))))
    });
    let cut = |top: usize| if top > 0 && notice(lines[top - 1], lines[top]) { top - 1 } else { top };
    let end = input.map(|i| ruled(i).map(cut).unwrap_or(i)).unwrap_or(lines.len());
    screen_text(&lines[..end].iter().filter(|l| !turn_line(l)).copied().collect::<Vec<_>>().join("\n"))
}
