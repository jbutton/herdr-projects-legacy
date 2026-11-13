# Extra usage: an agent past its usage window

**An agent on a subscription works within a 5-hour window and a weekly one. Past them, it either stops ("out of
allowance") or, where extra usage is on, goes on and is paid for by it**: Claude Code's usage credits, Codex's credits on
a ChatGPT plan. Nothing fails then, so nothing used to show it. The board now reads where each agent stands in its
windows, says when one works on extra usage, and does what `usage.extra` says.

```
herdr-projects set usage.extra defer                 for every task of yours: wait for the window instead
herdr-projects set usage.extra ask --repo my-app     for one repo: stop and ask me
herdr-projects set usage.extra allow --task 12       this one may run on it
```

## What the board can see

It reads what each program saves anyway. It costs no model call.

| Program | What it says | Where the board reads it |
|---|---|---|
| Claude Code | its 5-hour and weekly windows, how much of each is used and when each resets | the input Claude Code gives its status line, after every answer |
| Claude Code | a request refused because the window is used up, with the exact reset | its saved conversation (`quotaLimits`) |
| Codex | both windows, when each resets, and whether credits are there | its saved conversation, after every answer (`token_count`, `rate_limits`) |
| Codex | a request refused | its saved conversation |
| pi and the others | nothing about windows | — |

**On extra usage** means: a window is at 100% or more, and answers are still being served. For Codex that is with credits
there; for Claude Code, the status line's input only comes after an answer was served.

- **Claude Code's status line.** The board already gives each Claude agent it starts a settings file of the task's own
  (for its turn-end hook). That file now names the board's own status line: it keeps the windows beside the task
  (`window-agent.json` in the task's folder) and then runs **your own status line command** with the same input, so the
  agent's pane shows yours as before. Yours is found where Claude Code looks: the project's local settings, the
  project's, then yours, with your `padding`. With no status line of yours, the agent's is empty.
- **Codex's own sessions.** The newest Codex conversation of the machine, yours or a task's, is read once a minute, so
  Codex's window is known before a task runs it.
- **What is not seen:**
  - Claude Code's saved conversation says nothing of extra usage while answers are served.
  - Claude Code's notice "You're now using usage credits" is a short-lived note on its screen, not something the board
    can rely on.
  - Claude's per-model weekly bucket (the "Fable limit") is not in the status line's input.
  - With an API key, or where your organisation's managed settings set the status line, there are no windows to read.
  - For pi and Herdr's other programs, `usage.extra` does nothing, and `show` says so (`does nothing for pi: its usage
    window cannot be seen`).

Each program's window is kept for the machine in `usage.json` in the board's data folder: the newest reading of any of
its agents. A window whose reset has passed is a fresh one.

## What you see

For every task, whatever `usage.extra` says:

- **The row**, while its agent works on extra usage, before what the agent is doing: `On extra usage since 14:10 ·
  Running the tests`. Herdr's sidebar follows the row.
- **`herdr-projects details <id>`**: a `usage` entry: the program, the setting, its windows, whether it is on extra
  usage, since when (`extraSince`, `extraSinceAt`) and when the window resets (`resets`, `resetsAt`), and a stop or a
  wait for it.
- **`board.notify`**: once per task and episode of extra usage, not on every turn: the bell rings, or your command runs
  with `HERDR_PROJECTS_EVENT=extra usage`. Once the window resets, a new episode is told again. A stop under `ask` tells
  you as "needs you" instead.
- **The header**, for each program whose window is known: `Claude: extra usage since 14:10, resets 17:00`,
  `Claude: out of allowance until 17:00`, or `Codex: 5h 72%, week 78%`.

## The setting

`usage.extra` is a work setting: the task → your entry for the repo → you → the repo's own file → built in. It is one
of the facts the repo's own file may hold: your own places come before it, so it can never loosen what you set, and
`allow` is the built-in value anyway.

### `allow` (built in)

Today's behaviour: the agent works on, and the board only says so (above).

### `ask`

- **The moment the board sees the agent on extra usage, the agent is stopped**: interrupted as the stop key does, its
  work kept as it is, nothing committed, pushed or merged. The task waits for you under "needs you":
  `stopped: claude moved onto extra usage at 14:10 (resets 17:00)`.
- **Carrying on** (`Ctrl+R`, `herdr-projects retry <id>`, or a reply) continues on extra usage, and is leave for the
  rest of that episode: it is not stopped again until the window resets. The next time it moves onto extra usage, it
  asks again.
- **Take it as it is** (`Ctrl+L`, `herdr-projects finish <id>`) sends its work through the check and the review.
- Setting it to `allow` carries it on by itself; to `defer`, it waits for the window instead.
- **A task does not start** while its program is known to be on extra usage. It waits for you in the queue:
  `waits for you: claude is on extra usage until 17:00 (usage.extra ask); Ctrl+R starts it anyway`.

### `defer`

- **`worker.fallback` first**: the next agent on the list that is not on extra usage takes the task over, in the same
  worktree, as any hand-over does ([the fallback agent](fallback.md)): `codex took over from claude: claude is on extra
  usage until 17:00`. One whose program is on extra usage too is passed over (`codex is on extra usage too, until
  12:31`).
- **Nobody to take it: the agent is stopped at once**, as under `ask`, its work kept. The task waits as `waiting for
  the usage window: resets 17:00`, with the queued tasks, not under "needs you". **At the reset it carries on by
  itself**: the agent is told it was stopped for extra usage and that the window has reset.
- **A task does not start** while its program is on extra usage: started with an agent on the list that is not, else it
  stays in the queue as `waiting for the usage window: resets 17:00`, holding no slot, and starts by itself once the
  window has reset.
- **A turn that ends out of allowance** (extra usage off or used up), with nobody on the list to take it: it waits for
  that window too, instead of "Agent stopped: out of allowance until 17:00", and carries on by itself at the reset.
- **When the reset is not known**, it waits for you instead, under "needs you", with the reason.

`Ctrl+R` (or `retry`) carries on or starts it now, on extra usage, as under `ask`.

A chat-only task gets the warnings only: you are in that conversation.

## Beside the other settings

- **`worker.fallback`**: under `defer`, the list is used before any wait, at the start and at work. Under `ask`, nothing
  is handed over for extra usage: you decide. Under `ask` and `defer`, a hand-over for any reason passes over an agent
  whose program is on extra usage.
- **`limit.tokens`**: a cap per task, in tokens, whoever pays; `usage.extra` is about who pays, per window. Tokens spent
  on extra usage count as any others. A task stopped at its token limit stays stopped at it, and is not stopped for
  extra usage as well. Carrying on from either stop meets the other again where it applies.
- **`limit.time`**: a run that waits for the window is not running: its time does not count.
- **`--after`**: a task waiting for others is not started anyway. Once they are done it is queued, and the start's rule
  above applies to it. A task that waits for the window is not done, so the tasks after it go on waiting.
- **Attempts and the plan step**: each attempt's agent and a plan's writer are agents at work like any other.
- **The reviewer**: its readings count for its program's window (the header, the next start), but a review is not
  stopped or held back for extra usage.
- **A turn the board starts by itself** (findings sent back, a failed check, pull-request feedback, a retry after a
  provider error) is not held back before it starts: the same reading stops it once it runs.

## For contributors

- `crates/core/src/agents/read_window.rs`: a reading, from Codex's conversation, Claude Code's refused requests and its
  kept status-line input;
- `crates/core/src/agents/statusline.rs`: the board's status line for Claude Code, and your own run after it;
- `crates/core/src/store/usage.rs`: each program's window on this machine;
- `crates/flow/src/usage/`: the keeper's pass (`tick.rs`), the start held back (`gate.rs`), the stops and their carrying
  on (`stop.rs`), the words (`words.rs`); the stops are the limits' own (`limitStop` with `what: usage`);
- tests: `tests/usage.rs` and `tests/usage/`.
