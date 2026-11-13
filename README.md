# herdr-projects 1.x, maintained copy

This is JButton's standalone, maintained copy of the legacy herdr-projects 1.x work board. It starts from
[eliasstravik/herdr-projects v1.2.27](https://github.com/eliasstravik/shepherdr/releases/tag/v1.2.27), the last 1.x
release, under the same MIT license with the original copyright kept ([LICENSE](LICENSE)). Upstream has moved on to
[Shepherdr](https://github.com/eliasstravik/shepherdr). This copy carries the changes listed below, publishes no
releases and is built from source. Problems go to
[this repo's issues](https://github.com/jbutton/herdr-projects-legacy/issues).

After the next two sections, the page is upstream's README; only its install and update steps, and the links to them,
were changed for this copy.

## What this copy adds

Merged here since v1.2.27, oldest first. The first eight came without a pull request, the rest through one each.

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

## Install and update this copy

This repo publishes no releases: you build the program from source. Besides what upstream's install needs (macOS or
Linux, Herdr 0.9.3 or newer, git, an agent CLI), that takes Rust ([rustup.rs](https://rustup.rs)).

```sh
git clone https://github.com/jbutton/herdr-projects-legacy && cd herdr-projects-legacy
cargo build --release             # the program, in target/release/herdr-projects
herdr plugin link "$PWD"          # hooks, the board's tab and the actions
sh bin/run.sh install             # writes its launcher, says how to put it on your PATH
```

Every command Herdr runs for the plugin goes through `bin/run.sh`, which runs `target/release/herdr-projects` while no
source file is newer than it. Then open the board ([get started](docs/get-started.md)).

**Updating** is `git pull`, then `cargo build --release` again; an open board and the keeper load the new program by
themselves. Build right after each pull: while the build is older than its source, `bin/run.sh` falls back to
upstream's v1.2.27 program, downloaded, which has none of the changes above. `herdr-projects update` is no use here:
it downloads upstream's releases, not this copy (this repo publishes none, and upstream's newest are Shepherdr's), and
a program built from source refuses it anyway.

`herdr plugin install eliasstravik/herdr-projects` installs upstream, now Shepherdr. A `herdr plugin install` of this
repo would get upstream's prebuilt 1.2.27 program in its build step instead of building this copy: use a checkout as
above. [Installing](docs/install.md) has the details.

---

<p align="center"><img src="https://img.shields.io/badge/Herdr%20Projects-Mission%20control%20for%20Herdr-2ea44f?style=flat-square&labelColor=24292f" alt="Herdr Projects | Mission control for Herdr" /></p>

<h3 align="center">Run dozens of coding agents through one Coordinator without micromanaging each one or losing sight of what needs you</h3>

<p align="center">Herdr Projects is mission control for <a href="https://herdr.dev">Herdr</a>: one list of tasks, yours and the ones you delegate. You type what needs doing and the board's own agent, the Coordinator, turns it into tasks. Every delegated task runs as an ordinary Herdr agent in its own space and worktree, gets checked and reviewed by a second agent when it finishes, and waits for your yes before it merges. The board shows which tasks need you, which are ready to approve and which are still working.</p>

<p align="center"><a href="https://github.com/jbutton/herdr-projects-legacy/blob/main/assets/herdr-projects-launch.mp4"><img src="assets/herdr-projects-launch.webp" width="88%" alt="Animation: you tell the Coordinator what you want, it adds tasks that each get an agent on their own branch, and the board groups them as needs you, ready to approve, and working" /></a></p>

<p align="center"><a href="https://github.com/jbutton/herdr-projects-legacy/blob/main/docs/get-started.md"><img src="assets/buttons/install-herdr-projects.svg" alt="Install herdr-projects" /></a></p>

<p align="center"><sub>✓&nbsp;Free,&nbsp;MIT&nbsp;licensed &nbsp; ✓&nbsp;Runs&nbsp;on&nbsp;your&nbsp;machines,&nbsp;no&nbsp;hosted&nbsp;service &nbsp; ✓&nbsp;macOS&nbsp;and&nbsp;Linux,&nbsp;Herdr&nbsp;0.9.3+</sub></p>

<br />

## Keep one list going while the work happens in parallel

The board is an overview and a remote control; Herdr is the engine. Each delegated task gets its own agent in its own Herdr space, on its own branch in a git worktree, or in a plain folder when there's no repository. Your checkout is never touched. You approve finished work and answer the tasks that need you instead of briefing every agent yourself.

## Choose between briefing each agent by hand, one long agent session, a cloud projects product, or a work board in Herdr

| | **Herdr Projects** | Briefing agents by hand | One long agent session | Cloud projects products |
|---|:---:|:---:|:---:|:---:|
| **No extra software fee** | ✅ | ✅ | ✅ | ❌ |
| **Parallel tasks on separate branches** | ✅ | ✅ | ❌ | ✅ |
| **Same rules for every agent** | ✅ | ❌ | ✅ | ✅ |
| **A second agent reviews every finished task** | ✅ | ❌ | ❌ | ❌ |
| **Tasks grouped by what needs you** | ✅ | ❌ | ❌ | ✅ |
| **Runs on your own machines** | ✅ | ✅ | ✅ | ❌ |
| **Keeps every branch up to date with main** | ✅ | ❌ | ❌ | ❌ |
| **Works with the agent CLI you already use** | ✅ | ✅ | ✅ | ❌ |
| **Runs with no machine of yours switched on** | ❌ | ❌ | ❌ | ✅ |

Keep your attention on decisions. Herdr Projects starts, checks and tracks the tasks, your agents do the work, and you choose what to answer, approve, or merge.

## Tell the Coordinator what you want. See which task needs you on the board.

### 📈 See every task at a glance

The board's list holds your tasks in five groups, in the order they need you: `needs you`, `ready to approve`, `working`, `to do` and `done`. Its header counts them (`3 need you · 1 ready · 2 working`), and each row says who is on it, where it stands, its review round and pull request, and a percent and time to go. Herdr's own sidebar carries a small copy of each live agent's row, so you see the same thing from any space. With the [Herdr web UI](docs/web-ui.md) the board is on your phone too, and `Alt+M` shows the tasks of your [other machines](docs/other-machines.md).

### ⚡ Stop briefing every agent yourself

Type what needs doing and press `Enter`: the Coordinator, one long-lived agent chat, makes it a task or several. Send it with `Tab` instead and every task it adds gets an agent at once. Each agent starts on Claude Code, Codex, pi or [another agent herdr supports](docs/agents.md), with the task and the board's rules, in a worktree of its own. Want more control? Ask for [a plan first](docs/plan.md), [several attempts and one kept](docs/attempts.md), [limits](docs/limits.md) on time and tokens, or [automations](docs/automations.md) that add tasks on a schedule.

### 💬 Know when a task needs an answer

An agent that asks you something, wants a permission or a sign-in says so to the board, so its task shows under `needs you` and stays there until you answer. An agent that finishes goes down the finish line: its work is committed, main is merged in, the repo's check runs, a draft pull request opens, and a second agent reviews it. A failed check or review goes straight back to the agent; work that passes waits for your `Ctrl+L`. With the board closed, a small keeper does the same.

## Open the board in three steps

<table>
<tr>
<td align="center" valign="top" width="33%"><h3>1️⃣</h3><b>Build and link the plugin</b><br /><sub>In a clone of this repo, run <code>cargo build --release</code> and <code>herdr plugin link "$PWD"</code> (<a href="#install-and-update-this-copy">install and update</a>). It is one program: no Node, no npm.</sub></td>
<td align="center" valign="top" width="33%"><h3>2️⃣</h3><b>Open the board</b><br /><sub>Run the plugin's action "Open the work board", or <code>herdr plugin action invoke open --plugin herdr-projects</code> in a Herdr pane. Its first screen offers a key that opens it from anywhere in Herdr.</sub></td>
<td align="center" valign="top" width="33%"><h3>3️⃣</h3><b>Type what needs doing</b><br /><sub>Press <code>Enter</code> and the Coordinator makes it a task, or <code>Tab</code> and each task gets an agent at once. <code>?</code> shows everything else you can do.</sub></td>
</tr>
</table>

## Get everything included, free

<table align="center">
<tr>
<td align="center" valign="top"><sub>For developers who run coding agents in Herdr on macOS or Linux</sub><br /><h2>Free</h2><div align="left">&nbsp;&nbsp;&nbsp;✓&nbsp; A Coordinator that turns what you type into tasks<br />&nbsp;&nbsp;&nbsp;✓&nbsp; Every task on its own worktree and branch, in its own Herdr space<br />&nbsp;&nbsp;&nbsp;✓&nbsp; A check and a second agent's review before anything merges<br />&nbsp;&nbsp;&nbsp;✓&nbsp; What needs you, on the board, in Herdr's sidebar and on your phone<br />&nbsp;&nbsp;&nbsp;✓&nbsp; Branches kept up to date with main, cleanup after a merge<br />&nbsp;&nbsp;&nbsp;✓&nbsp; Plans, attempts, limits, fallback agents and automations</div></td>
</tr>
<tr>
<td align="center"><a href="https://github.com/jbutton/herdr-projects-legacy/blob/main/docs/get-started.md"><img src="assets/buttons/install-herdr-projects.svg" alt="Install herdr-projects" /></a></td>
</tr>
</table>

## Updating

```bash
git pull && cargo build --release   # in your checkout of this repo
```

An open board and the keeper see the new program and load it by themselves, once nothing is half typed and no agent is held up. This copy publishes no releases, so `herdr-projects update` is no use here ([install and update this copy](#install-and-update-this-copy)). [Installing](docs/install.md) has the details.

## Get your questions answered

### Do I need to know how to code?

You need to be comfortable in a terminal. This copy is built once from source with one command, and you never have to edit its files: everything changes by asking the Coordinator or from the board's settings page (`Ctrl+E`). You'll need macOS or Linux, Herdr 0.9.3 or newer, Git, Rust/Cargo, and an agent CLI the board can start: Claude Code, Codex, pi or another agent herdr supports. `gh` (pull requests) and `tailscale` (private review links) are optional. [Installing](docs/install.md) covers the prerequisites.

### How do I check that Herdr Projects is running?

Run:

```bash
herdr-projects doctor
```

It checks the program and its launcher, the plugin in Herdr, your agent programs (installed, signed in) and their model and effort settings, your other machines, free disk space, and leftover worktrees it could clean. Each problem comes with what to do, and it exits non-zero when there is one. [Troubleshooting](docs/troubleshooting.md#the-health-check) lists every check. [Installing](docs/install.md) walks through the first run, the blocks the board can add to Herdr's config, and taking the plugin out again.

### What permissions do the agents get?

**Full access by default**, so a task never waits on a prompt nobody sees: `claude --dangerously-skip-permissions`, `codex --dangerously-bypass-approvals-and-sandbox`. They can run anything as you without asking; the rules and the review are guidance, not a sandbox. Set `worker.access ask` (and `reviewer.access`, `coordinator.access`) to start a program without those flags, so it asks before it acts. Nothing is written to your own agent settings, and your own hooks keep running. [How work gets done](docs/how-work-gets-done.md) has the details.

### Does the plugin send my work to a hosted service?

No. The board keeps its files in a folder on your machine, talks to your local Herdr session, and reaches your other machines over your own SSH. Reports and review evidence get a private link on your tailnet, or on `127.0.0.1`. Your agent CLI still uses its own service as usual.

### Will it touch my branches or worktrees on its own?

Only the ones it made. Every task works in its own worktree, on its own branch `work/<n>-<title>`, never in your checkout. The board merges main into those branches so the reviewer sees the work as it will really land, and rolls a real conflict back at once and hands it to the task's agent. It merges only on your yes, at the commit that was reviewed, unless you set `merge.by itself`. When a task is done its worktree and branch are removed; a checkout with unsaved or unmerged work stays on disk with its branch. `workspace.cleanup false` keeps everything.

### Where do my tasks live?

In `~/.herdr-projects/` by default: `settings/config.json` for your settings, `data/tasks/<repo>/<n>/` for each task with its closing note and review evidence, `data/coordinator/` for the Coordinator's chat, `data/automations/` for your automations, and `reports/` for reports an agent made for you that belong in no repo. A repo can hold its own `.herdr-projects.json` with its setup, its check command and a few other facts, but never a setting that loosens a guard. [The home folder](docs/home-folder.md) lists every file.

### Can I have several boards at once?

You don't need to: one board per machine holds every repo, each task tagged with its repo, and `Alt+S` groups them. Each machine has its own board, and `Alt+M` shows the others' tasks from yours. Each herdr session gets its own Coordinator, but they share the one list. The board key never opens a second board: it goes to the one that runs. [Several boards](docs/several-boards.md) has the details.

### Do I have to start every task through the Coordinator?

No. Select a task on the board and press `Tab` to put an agent on it, or run `herdr-projects add "<title>" --delegate` and `herdr-projects delegate <id>` yourself. An [automation](docs/automations.md) adds tasks on a schedule or when something happens on the board, such as a merge.

### What if the safety settings aren't enough?

They are soft. By default nothing merges without your yes, work whose review failed is never merged by a plain yes, and a repo's own file can't switch on merging by itself, drop the review or change the rules its own branch is checked by. But agents run with full access and have a shell: one can do anything you can, a review is a second agent's judgement and not a proof, and an automation runs with nobody watching. Turn on `worker.access ask` for prompts, a [plan first](docs/plan.md) for a yes before any change, and [limits](docs/limits.md) for a cap on time and tokens.

### What does it cost?

Herdr Projects is free and [MIT licensed](LICENSE). Your agent CLI's usual usage charges still apply: every task is a full agent session plus a review, the Coordinator spends tokens on each message, and the Watcher makes a small model call about once a minute per working agent.

## Open the board in three steps

<p align="center">Your board starts with an install, one key, and one sentence about what needs doing. Herdr Projects starts the agents, checks their work and keeps it in view. You choose what to answer and what to merge.</p>

<p align="center"><a href="https://github.com/jbutton/herdr-projects-legacy/blob/main/docs/get-started.md"><img src="assets/buttons/install-herdr-projects.svg" alt="Install herdr-projects" /></a></p>

<p align="center"><sub>✓&nbsp;Free,&nbsp;MIT&nbsp;licensed &nbsp; ✓&nbsp;Runs&nbsp;on&nbsp;your&nbsp;machines,&nbsp;no&nbsp;hosted&nbsp;service &nbsp; ✓&nbsp;macOS&nbsp;and&nbsp;Linux,&nbsp;Herdr&nbsp;0.9.3+</sub></p>
