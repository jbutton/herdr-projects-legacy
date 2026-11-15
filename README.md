# herdr-projects

**A work board for [Herdr](https://herdr.dev): one list of tasks, each delegated one run by a coding agent in its own
worktree, checked and reviewed before it merges.**

[![Check](https://github.com/jbutton/herdr-projects-legacy/actions/workflows/ci.yml/badge.svg)](https://github.com/jbutton/herdr-projects-legacy/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
![Platforms: macOS and Linux](https://img.shields.io/badge/platform-macOS%20%7C%20Linux-lightgrey.svg)

<p align="center"><a href="assets/herdr-projects-launch.mp4"><img src="assets/herdr-projects-launch.webp" width="88%" alt="Animation: you tell the Coordinator what you want, it adds tasks that each get an agent on their own branch, and the board groups them as needs you, ready to approve, and working" /></a></p>

> [!NOTE]
> This is JButton's maintained copy of the herdr-projects 1.x board. Upstream moved on to
> [Shepherdr](https://github.com/eliasstravik/shepherdr), a different program. This copy is built from source and
> publishes no releases. See [this copy and upstream](#this-copy-and-upstream).

## What it does

You type what needs doing. The board's own agent, the Coordinator, turns it into tasks. Each delegated task then goes
the same way:

1. **An agent gets it.** It runs as an ordinary Herdr agent in its own space, on its own branch in a git worktree (or
   in a plain folder when there is no repository). Your checkout is never touched.
2. **The work is checked.** When the agent finishes, its work is committed, main is merged in, the repo's check runs
   and a draft pull request opens. A second agent reviews it. A failed check or review goes straight back to the
   agent.
3. **You decide.** Work that passes waits for your yes before it merges. An agent that needs an answer, a permission
   or a sign-in shows under `needs you` until you give it.

The board is an overview and a remote control; Herdr is the engine. It runs on your own machines, with the agent CLI
you already use: Claude Code, Codex, pi or [another agent Herdr supports](docs/agents.md).

## Features

- **One list, in the order it needs you.** Five groups: `needs you`, `ready to approve`, `working`, `to do`, `done`.
  Each row says who is on it, where it stands, its review round and pull request, and a percent and time to go.
- **Visible everywhere.** Herdr's sidebar carries a small copy of each live agent's row. With the
  [Herdr web UI](docs/web-ui.md) the board is on your phone, and `Alt+M` shows the tasks of your
  [other machines](docs/other-machines.md).
- **A review before every merge.** A second agent reviews each finished task, and only the reviewed commit merges.
- **Branches kept current.** The board merges main into task branches, hands a real conflict to the task's agent, and
  removes a task's worktree and branch when it is done.
- **More control when you want it.** [A plan first](docs/plan.md), [several attempts and one kept](docs/attempts.md),
  [limits](docs/limits.md) on time and tokens, [fallback agents](docs/fallback.md), and
  [automations](docs/automations.md) that add tasks on a schedule or on a board event.
- **Works with the board closed.** A small keeper process does what the open board does and alerts you.

## Requirements

| Needed | Notes |
|---|---|
| macOS or Linux | |
| [Herdr](https://herdr.dev) 0.9.3 or newer | |
| Git | |
| Rust and Cargo | to build the program: [rustup.rs](https://rustup.rs) |
| An agent CLI | Claude Code, Codex, pi or another agent Herdr supports |
| `gh` (optional) | for pull requests |
| `tailscale` (optional) | for private review links |

There is no Node and nothing to `npm install`: the board is one program.

## Install

```sh
git clone https://github.com/jbutton/herdr-projects-legacy && cd herdr-projects-legacy
cargo build --release             # the program, in target/release/herdr-projects
herdr plugin link "$PWD"          # hooks, the board's tab and the actions
sh bin/run.sh install             # writes its launcher, says how to put it on your PATH
```

Then check it:

```sh
herdr-projects doctor
```

`doctor` checks the program and its launcher, the plugin in Herdr, your agent programs and their settings, your other
machines, free disk space and leftover worktrees. Each problem comes with what to do.
[Installing](docs/install.md) has the details, the first run, and how to take the plugin out again.

## First steps

1. **Open the board.** Run the plugin's action "Open the work board", or in a Herdr pane:
   ```sh
   herdr plugin action invoke open --plugin herdr-projects
   ```
   Its first screen offers a key that opens it from anywhere in Herdr.
2. **Type what needs doing.** `Enter` sends it to the Coordinator, which makes it a task or several. `Tab` sends it
   and gives every task it adds an agent at once.
3. **Answer and approve.** Answer the tasks under `needs you`, and press `Ctrl+L` on work that is ready to approve.

| Key | Does |
|---|---|
| `Enter` | sends what you typed to the Coordinator |
| `Tab` | the same, and starts an agent on each task; on a selected task, puts an agent on it |
| `Ctrl+L` | approves finished work |
| `Ctrl+E` | opens the settings page |
| `Alt+M` | goes round your machines |
| `Alt+S` | groups tasks by repo |
| `?` | shows every key |

You can also skip the Coordinator: `herdr-projects add "<title>" --delegate` adds a task and starts it, and
`herdr-projects delegate <id>` starts one that exists. [Get started](docs/get-started.md) walks through a first task.

## Update

```sh
git pull && cargo build --release   # in your checkout
```

An open board and the keeper see the new program and load it by themselves, once nothing is half typed and no agent
is held up.

Build right after each pull. While the build is older than its source, `bin/run.sh` runs the older build and says so
on every command. `herdr-projects update` finds nothing here, because this copy has no releases.

## Safety

> [!WARNING]
> **Agents get full access by default**, so a task never waits on a prompt nobody sees:
> `claude --dangerously-skip-permissions`, `codex --dangerously-bypass-approvals-and-sandbox`. They can run anything
> as you without asking. The rules and the review are guidance, not a sandbox.

- Set `worker.access ask` (and `reviewer.access`, `coordinator.access`) to start a program without those flags, so it
  asks before it acts.
- By default nothing merges without your yes, and work whose review failed is never merged by a plain yes.
- A repo's own `.herdr-projects.json` cannot switch on merging by itself, drop the review, or change the rules its own
  branch is checked by.
- A review is a second agent's judgement, not a proof, and an automation runs with nobody watching. A
  [plan first](docs/plan.md) gives you a yes before any change, and [limits](docs/limits.md) cap time and tokens.

Nothing is written to your own agent settings, and your own hooks keep running.
[How work gets done](docs/how-work-gets-done.md) has the details.

## FAQ

<details>
<summary><b>Do I need to know how to code?</b></summary>

You need to be comfortable in a terminal. This copy is built once from source with one command, and you never have to
edit its files: everything changes by asking the Coordinator or from the board's settings page (`Ctrl+E`).
</details>

<details>
<summary><b>Does it send my work to a hosted service?</b></summary>

No. The board keeps its files in a folder on your machine, talks to your local Herdr session, and reaches your other
machines over your own SSH. Reports and review evidence get a private link on your tailnet, or on `127.0.0.1`. Your
agent CLI still uses its own service as usual.
</details>

<details>
<summary><b>Will it touch my branches or worktrees on its own?</b></summary>

Only the ones it made. Every task works in its own worktree, on its own branch `work/<n>-<title>`, never in your
checkout. The board merges main into those branches so the reviewer sees the work as it will really land, and rolls a
real conflict back at once and hands it to the task's agent. It merges only on your yes, at the commit that was
reviewed, unless you set `merge.by itself`. When a task is done its worktree and branch are removed; a checkout with
unsaved or unmerged work stays on disk with its branch. `workspace.cleanup false` keeps everything.
</details>

<details>
<summary><b>Where do my tasks live?</b></summary>

In `~/.herdr-projects/` by default: `settings/config.json` for your settings, `data/tasks/<repo>/<n>/` for each task
with its closing note and review evidence, `data/coordinator/` for the Coordinator's chat, `data/automations/` for
your automations, and `reports/` for reports an agent made for you that belong in no repo. A repo can hold its own
`.herdr-projects.json` with its setup, its check command and a few other facts.
[The home folder](docs/home-folder.md) lists every file.
</details>

<details>
<summary><b>Can I have several boards at once?</b></summary>

You don't need to: one board per machine holds every repo, each task tagged with its repo, and `Alt+S` groups them.
Each machine has its own board, and `Alt+M` shows the others' tasks from yours. Each Herdr session gets its own
Coordinator, but they share the one list. [Several boards](docs/several-boards.md) has the details.
</details>

<details>
<summary><b>What does it cost?</b></summary>

The board is free and [MIT licensed](LICENSE). Your agent CLI's usual usage charges still apply: every task is a full
agent session plus a review, the Coordinator spends tokens on each message, and the Watcher makes a small model call
about once a minute per working agent.
</details>

<details>
<summary><b>Something is not working. Where do I look?</b></summary>

Run `herdr-projects doctor` first: it exits non-zero when there is a problem and says what to do about each.
[Troubleshooting](docs/troubleshooting.md) lists every check and the common faults.
</details>

## This copy and upstream

This copy starts from
[eliasstravik/herdr-projects v1.2.27](https://github.com/eliasstravik/shepherdr/releases/tag/v1.2.27), the last 1.x
release, under the same MIT license with the original copyright kept. Upstream's repo is now Shepherdr, so:

- `herdr plugin install eliasstravik/herdr-projects` installs Shepherdr, not this board. Use a checkout as in
  [Install](#install).
- `bin/run.sh` and `herdr-projects update` never download upstream's program: the only releases they look at are this
  repo's, and there are none yet.

What this copy changed since v1.2.27 is in the [changelog](CHANGELOG.md). Problems go to
[this repo's issues](https://github.com/jbutton/herdr-projects-legacy/issues). Every other page is in
[`docs/`](docs/).

## License

[MIT](LICENSE)
