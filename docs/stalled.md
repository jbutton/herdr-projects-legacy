# Stalled tasks

**A task stalls when its agent works but gets nowhere you can see.** The board tells the automations that wait for it
and a coordinator on autopilot that looks after the task, once per stall. It is a hint that something may hang, never
proof: nothing is stopped, and what to do about it is theirs (or yours).

A working task counts as stalled when either holds:

- **Quiet:** its agent's screen has shown nothing new for `stall.after` (20 minutes unless you set it).
- **Late:** it ran past the Watcher's estimate by as much again, and by at least `stall.after`. The estimate is the
  minutes left the board worked out when the Watcher's guess last moved ([the Watcher](watcher.md)): ten minutes left
  at 50% means stalled thirty minutes later if the guess is still 50%. A screen that keeps moving while the guess does
  not (an agent trying the same thing again and again) stalls this way.

```
herdr-projects set stall.after 45m                     # for every task of yours
herdr-projects set stall.after 1h --repo my-app        # a repo whose builds print nothing for a long while
herdr-projects set stall.after none --task my-app/12   # never for this one
```

`stall.after` is a setting like every other ([settings](settings.md)): the task, your entry for the repo, you, built
in. `none` turns stalls off.

## Who hears of it

- **Automations:** `--when stalled` fires once per stall ([automations](automations.md)). The new task is told
  `Event: stalled: its agent showed nothing new for a while, or ran well past its estimate`.
- **A coordinator on autopilot:** `stalled` in `autopilot.when` (on unless you left it out) wakes it for a task it may
  look after: `Autopilot: HSW-84 has shown nothing new for 21m`, or `HSW-84 ran well past its estimate`
  ([autopilot](autopilot.md)). Its rules say to look at the task first (`details`), then nudge it with `tell`, or stop
  it. A stalled task needs looking after, so it wakes the coordinator also when its `autopilot.tasksAtOnce` are full,
  and `autopilot show` lists it (`HSW-84 (has stalled)`).

Each stall is heard of once, however long it lasts. It is remembered until something re-arms it: new output on the
screen, a new run of the agent, or a guess of the Watcher's that moved. Only that makes the next stall a new one; a
dialog for a minute or a status that flapped does not, nor does a change of `stall.after`. A task that stalls both ways
at once is one event.

## What counts

- **Only an agent at work**: the task's stage is working and herdr's last word on its pane is working. A task waiting
  for you (a question, a permission, a yes), one under review, one in the board's own check, and one waiting for the
  board to try again after a provider error never stall. Nor does a task done as several attempts at once
  ([attempts](attempts.md)), nor another machine's task here: that one is judged on its own machine.
- **New output is new words.** About once a minute the board reads the screen of every agent at work and compares its
  words as the Watcher does: a spinner, a timer, a token count or any other number that moved is not new. This look
  costs no model call and runs with the Watcher off too. With the Watcher off there is no estimate, so only quiet
  counts.
- **Only while the board looks.** What a task keeps (`output` in its file: when its words last changed, and the last
  look) is what everyone judges by, the keeper and an open board alike. A last look more than ten minutes old says
  nothing, and a gap in looking that long (the machine slept, herdr or the keeper was down) starts the count again: a
  sleeping laptop does not make every task stalled when it wakes.

Code: `crates/flow/src/stalled.rs` (the judgement), `crates/flow/src/watcher/output.rs` (the look at the screen).
