# Run windows: nights, weekends

**A task can wait for a quiet time window, such as nights or weekends, when your machine and your usage allowance are
free.** Delegated outside its window, it does not start: it waits in the queue and starts by itself when the window
opens. When the window closes while it still works, it runs on, or it pauses and goes on in the next window: you choose.

```
herdr-projects delegate 12 --run.window nights                  run this tonight
herdr-projects set run.window weekends --repo my-app            only on weekends in this repo
herdr-projects set run.window "mon-fri 22:00-06:00, weekends"    nights and weekends, for every task of yours
herdr-projects set run.windowEnd pause                          stop when the window closes, go on in the next
herdr-projects delegate 12 --now                                start it now anyway
herdr-projects unset run.window                                 any time again
```

## The settings

| Setting | Built in | Means |
|---|---|---|
| `run.window` | `any` | when a task's agent may run, in this machine's time zone: `any`, or one or more windows |
| `run.windowEnd` | `finish` | what a task still working does when its window closes: `finish` runs to its end, `pause` waits for the next window |

They are work settings like every other ([settings](settings.md)): looked up nearest answer first (the task, your entry
for the repo, you, the repo's own file, built in), listed by `show` and `show <id>`, and on the settings page of a task.
Both are facts the repo's own file may hold; your own places come before it. Built in, nothing changes: `any` never
holds a task.

## What a window may be

The days and hours are those of `autopilot.hours` ([autopilot](autopilot.md)), with a few words, days on their own, and
several windows in one value.

| Value | Means |
|---|---|
| `any` | no window: it runs at any time (built in) |
| `nights` | every night, 22:00 to 06:00 |
| `nights 23:00-07:00` | every night, those hours |
| `weekends` | Saturday and Sunday, all day: Saturday 00:00 to Monday 00:00 |
| `weekdays` | Monday to Friday, all day |
| `22:00-06:00` | every day, those hours |
| `mon-fri 22:00-06:00` | those days, those hours |
| `sat,sun 10:00-16:00` | as in `autopilot.hours` |
| `sat`, `mon,wed`, `fri-sun` | those days, all day |
| `mon-fri nights` | Monday to Friday, 22:00 to 06:00 |
| `mon-fri 22:00-06:00, weekends` | several windows, with commas between them |

- **Hours across midnight** belong to the day they begin on: `mon-fri 22:00-06:00` has Friday 22:00 to Saturday 06:00,
  and not Sunday night. `24:00` may end a window.
- **Windows that touch are one**: `mon-fri 22:00-06:00, weekends` is open from Friday 22:00 to Monday 00:00 in one go.
- **A comma** separates windows, and the days of one list, as in `autopilot.hours`: days on their own before a window
  that starts with days are that window's days too, so `sat,sun 10:00-16:00` is one window. Saturday all day beside a
  weekday window is written after it: `mon-fri 22:00-06:00, sat`.
- **Kept in one spelling**: lower case, day lists without spaces, windows joined by `, `; spaces around a range's dash
  are dropped (`mon - fri 22:00 - 06:00` is `mon-fri 22:00-06:00`). `show` says how it was read.
- **The night summer time starts**, a time the clock skips is taken as the first moment after it; where the clock goes
  back, a time it repeats is taken the first time.

A value the board cannot read changes nothing and names the nearest right one: `22-06` and `10pm-6am` ("Did you mean
22:00-06:00?"), `mon-fri 9-17` ("Did you mean mon-fri 09:00-17:00?"), `saturday` ("Did you mean sat?"), `wekends`,
`nigths`, and `none` ("Did you mean any?": `any` is no window). `22:00-22:00` begins where it ends, and `any` stands
alone.

## Before the window

- **A task delegated outside its window does not start.** It stays in the queue as `waiting for its window: starts
  22:00` (with its day when that is more than a day away: `starts Sat 00:00`): on its row, in `list`, in
  `herdr-projects details <id>` and in herdr's sidebar (`waiting for its window`).
- **It holds no slot and has no place in the queue** (no `Q#`): the tasks after it start as slots are free. It waits as
  a queued task does: not under "needs you", and `board.notify` and autopilot hear nothing of it.
- **It starts by itself** at the first look inside its window: the keeper, an open board or any command looks every few
  seconds. From there it queues for a slot like any other task.
- **A task done several times at once** ([attempts](attempts.md)) waits whole, and is split into its attempts when its
  window opens. Under `pause` each attempt waits for the window as a task does, and the task waits with them (its row
  says `A claude waiting for its window: continues 22:00`); none of them counts as out.
- **It has no agent yet**: delegated again, with new settings or `--now`, it is queued afresh. Stopped, it is no longer
  held.

## At the window's end

- **`finish`** (built in): nothing happens. The task runs to its end, its check, review and merge included.
- **`pause`**: once the window has closed, the board starts no new step of the task's run by itself. The step under way
  ends as usual (its agent's turn, the check, a review); the next one waits. The task then stops, its work kept as it is
  (nothing committed, pushed or merged), and waits as `waiting for its window: continues 22:00`:
  - at the end of its agent's turn: its check and review wait;
  - at a send-back the board starts itself: a failed check's output, a review's findings, a research task's change, a
    plan sent back by the reviewer; the words are kept and sent then;
  - before a review starts.
- **When the window opens, it carries on by itself**: a turn that ended goes through the check and the review, what was
  kept goes to the agent, a review that waited is done. It waits by itself as a queued task does: not "needs you", no
  word to `board.notify`.
- **What `pause` does not hold**: anything after the task's run is over, that is once its work was reviewed and stands
  up for your approval (or merges). A repair the board sends for it (a conflict with main, GitHub's checks failing,
  pull-request feedback), a sync with main, the repair's check and review and what that review sends back all go on,
  until the task is delegated again. A failed check or a review's findings within the run itself are held. Nor a turn
  a provider error cut off and the board tries again; a hand-over to `worker.fallback`; an agent started again after its terminal went; an approved merge on
  its way. A chat-only task is a conversation with you: it is never held or paused.

## The machine asleep, or the board closed

- **The window opened while nothing looked** (the machine slept, the board and the keeper were not running): the task
  starts the next time the board runs inside the window, and its row says so before what its agent does: `Started late:
  its window opened at 22:00 · Running the tests`. A paused task carried on late says `Continued late: …`. `details`
  keeps it (`late`) until the task is next delegated. Late is more than five minutes after the opening.
- **The window has closed again by then**: it waits for the next one, and its line names that opening.

## Starting it anyway

| You want | Do |
|---|---|
| to start it now | `herdr-projects delegate <id> --now` (also on a task that waits already), or `Ctrl+R` on it, or `herdr-projects retry <id>` |
| a paused one to go on now | `Ctrl+R`, `herdr-projects retry <id>`, or a reply to its agent; `Ctrl+L` (`finish <id>`) takes its work as it is |
| it to run at any time again | `herdr-projects unset run.window --task <id>` |
| it to run to its end tonight | `herdr-projects set run.windowEnd finish --task <id>`: a paused one goes on at once |

Starting or carrying on outside the window is leave until the window next opens: under `pause` the task is not paused
again before that. A task done several times at once gives its attempts the same leave. `--now` also skips what a task
waits for with `--after`.

## Beside the other settings

- **`--after`** ([after](after.md)): a task whose tasks are done is queued, and then meets its window. If the window is
  closed, it waits for it (`waiting for its window: starts 22:00`, not "queued after"), holding no slot, and starts at
  the opening, its agent told what its tasks did. A task that waits for its window is not done, so the tasks after it
  go on waiting.
- **`usage.extra defer`** ([extra usage](extra-usage.md)): two waits on one start, the window first. Inside its window, a
  task whose program is on extra usage waits for the usage window (`waiting for the usage window: resets 13:00`); if
  that reset comes after the run window has closed again, it waits for the next run window. Under `pause`, a task
  stopped for extra usage whose usage window resets while its run window is closed waits for its run window (its line
  says so) and carries on then. A task carried on at its window's opening while its program is on extra usage is stopped
  again by the same reading, as every turn the board starts is.
- **`limit.time`** ([limits](limits.md)): a task that waits for its window, held or paused, is not running: its time
  does not count. Under `finish`, a run past the window's end counts as any run.
- **`board.agents`**: the held tasks go when their window opens, in their queue order (the first delegated first), and
  those beyond `board.agents` wait for a free slot as usual (`waiting for a free slot, Q#1`). A paused task carries on
  at once when its window opens, as a stop for extra usage does at its reset: it was under way already. While it works
  it counts against `board.agents`, so new starts wait for the slots it frees.
- **Automations and autopilot** ([automations](automations.md), [autopilot](autopilot.md)): the tasks they start meet
  their window like any other, and an automation's or a coordinator's own settings may say `run.window`.
- **When the settings are read**: `run.window` at every look at the queue, and both at every point where `pause` can
  stop a task. A change counts at once: set on a waiting or paused task, it goes, or waits, as the setting now says.

## In your words

The Coordinator knows these ([the Coordinator](coordinator.md)):

| You say | It is |
|---|---|
| "run this tonight" | `herdr-projects delegate <id> --run.window nights` (or `add … --delegate --run.window nights`) |
| "only on weekends in this repo" | `herdr-projects set run.window weekends --repo <repo>` |
| "nights and weekends" | `--run.window "mon-fri 22:00-06:00, weekends"` |
| "stop when the night is over and go on the next night" | `herdr-projects set run.windowEnd pause` |
| "start it now anyway" | `herdr-projects delegate <id> --now` |
| "any time again" | `herdr-projects unset run.window` |

## For contributors

- `crates/core/src/store/settings/run_window.rs`: what a value may be, its kept spelling, the refusals;
- `crates/core/src/window_times.rs`: whether a window is open at a moment, when it next opens, the open stretch;
- `crates/flow/src/run_window/`: the start held in the queue (`gate.rs`, asked by `start_queued` beside
  `usage/gate.rs`), the pause (`pause.rs`: the limits' own stop, `limitStop.what: window`, and its carry-on by the
  limits' tick), the words and the `window` entry of `details` (`words.rs`);
- tests: `tests/run_window.rs` and `tests/run_window/`, and the unit tests of the two core files.
