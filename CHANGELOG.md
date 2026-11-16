# Changelog

What this copy changed since [upstream's v1.2.27](https://github.com/eliasstravik/shepherdr/releases/tag/v1.2.27), the
last 1.x release of herdr-projects. Oldest first. The first eight came without a pull request, the rest through one
each. This copy makes no version tags yet, so the list is one section.

## Since v1.2.27

- A task's review covers only its own work: the range starts at the newer of main as merged in and the task's start, so
  a sync against a checkout that was behind no longer pulls old main history into it.
  ([1101d71a](https://github.com/jbutton/herdr-projects-legacy/commit/1101d71a))
- Two fixes to 1.2.27's own changes: an outside CI's passing check no longer hides another CI's failing check of the
  same name, and autopilot no longer takes a turn typed straight into the terminal for its own.
  ([e33c42c3](https://github.com/jbutton/herdr-projects-legacy/commit/e33c42c3),
  [4e02936a](https://github.com/jbutton/herdr-projects-legacy/commit/4e02936a))
- On macOS a process the board ended counts as ended (zombies were told only from `/proc`), and the clean-up names the
  folder a leftover process runs in. ([8381e6f2](https://github.com/jbutton/herdr-projects-legacy/commit/8381e6f2))
- The check passes on macOS from a task's long temporary folder: each test's socket is in a short `/tmp/swt-…` folder,
  under macOS's 104-byte limit, and a flaky clock test stands still.
  ([7d34070c](https://github.com/jbutton/herdr-projects-legacy/commit/7d34070c))
- `stop` and the clean-up find a task's processes on macOS (from the system, where Linux reads `/proc`), and never end
  a run of the board's own program. ([2e599ea0](https://github.com/jbutton/herdr-projects-legacy/commit/2e599ea0))
- `merge.syncRemote` (off by default): a task that lands in your checkout keeps up with that branch as origin has it,
  not with a checkout that may be behind, and your checkout is fast-forwarded to it before the work lands.
  ([8313210d](https://github.com/jbutton/herdr-projects-legacy/commit/8313210d))
- `merge.by none`, for a repo whose work lands through code review on GitHub: a task whose review passed waits as
  `Review passed: ready for code review`, and the board never merges it.
  ([4b561cb5](https://github.com/jbutton/herdr-projects-legacy/commit/4b561cb5))
- Every `gh` call names origin's repo, so in a fork's checkout with an `upstream` remote the board never reads
  upstream's pull request of the same number.
  ([6e32aa4a](https://github.com/jbutton/herdr-projects-legacy/commit/6e32aa4a))
- A task starts from origin's base branch fetched at that moment (the whole start held to 10 seconds), not from a copy
  up to a day old. ([#2](https://github.com/jbutton/herdr-projects-legacy/pull/2))
- The push of a task's branch leases the exact commit origin has and never pushes over commits the task lacks, skips a
  head origin already has (so its pre-push hook does not run for nothing), and runs with the worktree's `mise` tools.
  ([#4](https://github.com/jbutton/herdr-projects-legacy/pull/4))
- Each GitHub check is judged by its newest run, so an older run that was cancelled or failed no longer fails or holds
  up a pull request. ([#7](https://github.com/jbutton/herdr-projects-legacy/pull/7))
- A done task's clean worktree is removed also when it has submodules, nested ones included, and is looked at again
  right before it goes. ([#8](https://github.com/jbutton/herdr-projects-legacy/pull/8))
- `archive.doneAfter` (off by default): a task done for that long archives itself; pinned tasks stay.
  ([#9](https://github.com/jbutton/herdr-projects-legacy/pull/9))
- A Codex reviewer starts with its hooks off and is never started signed out; a Codex on a provider of your own that
  needs no sign-in still reviews. ([#3](https://github.com/jbutton/herdr-projects-legacy/pull/3))
- A Codex transcript is found in the day folder its id names, not by listing all of `~/.codex/sessions`.
  ([#10](https://github.com/jbutton/herdr-projects-legacy/pull/10))
- An agent that went away or exited mid-task is started again in its own conversation, three tries at most, before the
  task waits for you. ([#1](https://github.com/jbutton/herdr-projects-legacy/pull/1))
- An MCP server that a helper such as `uv tool run` starts with its input on `/dev/null` counts as the agent's: it no
  longer keeps a done task's space in use, and `stop` leaves it running.
  ([#12](https://github.com/jbutton/herdr-projects-legacy/pull/12))
- A task that stops without a summary shows the agent's last words, not Claude Code's footer or its notices over the
  input box. ([#6](https://github.com/jbutton/herdr-projects-legacy/pull/6))
- `review.prComment` (on by default): turned off, the review posts no comment on the pull request; it still runs and is
  judged at the gate. ([#13](https://github.com/jbutton/herdr-projects-legacy/pull/13))
- A merge into your checkout (no pull request) is a fast-forward that git guards: a stray file no longer blocks it, an
  ignored file it would overwrite stops it, and a repo that signs its commits gets a signed merge commit.
  ([#14](https://github.com/jbutton/herdr-projects-legacy/pull/14))
- The keeper rereads a working task's transcript only when it changed, and then only the added bytes.
  ([#11](https://github.com/jbutton/herdr-projects-legacy/pull/11))
- A `stalled` task event for automations and autopilot: a task whose agent works but shows nothing new for
  `stall.after`, or runs well past its estimate, fires it once per stall.
  ([#5](https://github.com/jbutton/herdr-projects-legacy/pull/5))
- A command Claude Code runs in the background counts as running work until it ends, so the task is not finished
  under it. ([#16](https://github.com/jbutton/herdr-projects-legacy/pull/16))
- `usage.extra` (`allow`, `ask` or `defer`): the board reads each agent's usage windows with no model call, says when
  one works on paid extra usage, and can stop to ask or wait for the window to reset.
  ([#18](https://github.com/jbutton/herdr-projects-legacy/pull/18))
- The TLS crates behind the voice call no longer accept certificates through distrusted emSign intermediates
  (rustls-webpki 0.103.17, webpki-roots 1.0.12), and the other crates are at their newest compatible versions.
  ([#19](https://github.com/jbutton/herdr-projects-legacy/pull/19),
  [#24](https://github.com/jbutton/herdr-projects-legacy/pull/24))
- Claude takes `claude-fable-5-2`: a setting that names it is no longer refused.
  ([#20](https://github.com/jbutton/herdr-projects-legacy/pull/20))
- `bin/run.sh` and `herdr-projects update` look for releases in this repo only: upstream's program is never downloaded,
  and a build older than its source runs with a line saying so.
  ([#21](https://github.com/jbutton/herdr-projects-legacy/pull/21))
- The board never types into a pane whose agent's program has ended: a task's text no longer goes into the shell left
  behind (where it ran as commands). Under `usage.extra defer`, a start whose usage window ends it waits for that window
  and carries on by itself at the reset, instead of "Not started", and a window known to be used up holds the start back.
  ([#30](https://github.com/jbutton/herdr-projects-legacy/pull/30))
