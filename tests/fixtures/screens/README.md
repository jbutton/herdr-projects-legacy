Screens of a real agent program, as Herdr gives them to the board when a turn ends (`recent-unwrapped`: one line per
row, no spaces at a line's end, nothing after the last drawn row), as test input for `agent_words`
(tests/turns/ended_late.rs).

Recorded on 2026-11-12 from Claude Code 2.1.322 (`--dangerously-skip-permissions`, a one-line custom status line, model
Haiku 5.5) in a 150 by 50 tmux window, after two short turns, with `tmux capture-pane -J` (wrapped rows joined, as
Herdr's read does) and the spaces at each line's end taken off. Only the folder in its start screen was changed, to
`~/work/cap-repo`:

- `claude-at-rest`: the turn has ended, the input box is empty. Above it, Claude Code's own line for each turn
  (`✻ Cooked for 0s · done 9:03 PM`); under it, the status line and the mode line.
- `claude-draft-in-box`: the same screen with ten lines typed into the input box and not sent, so that eleven lines
  stand under its prompt: the input box grows with what is typed in it, between its two rules.

Recorded on 2026-11-12 from a Claude Code 2.1.322 at rest on the board (Opus 5.5, a 152-column pane), its last 14
lines, with the task's words, title and branch replaced by plain ones of their own kind:

- `claude-notice-over-box`: Claude Code's notice in the row right over its input box, pushed to the right edge
  (`new task? /clear to save 599.8k tokens`; on other screens `✔ Update installed · Restart to update`).
