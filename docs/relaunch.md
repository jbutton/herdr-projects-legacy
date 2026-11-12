# An agent that stopped comes back by itself

**When a working task's agent goes away (Herdr or the computer restarted, Herdr crashed, its program crashed), the
board starts it again by itself**, in its own conversation where that can be resumed, and tells it why. It tries three
times at most. Only then does the task wait for you, with the reason.

Before this, such a task sat under "The agent's terminal is gone" or "The agent has exited" until you replied.

## What counts as gone

The board looks at every task's agent on each of its rounds (the keeper's, with the board closed too).

- **Its terminal is gone.** The task's pane is not in Herdr any more, under its id or any other the board can find it by
  ([linking](linking.md)). The row says `The agent's terminal is gone`.
- **Its program exited.** The pane is there, at its shell, with no agent in it. The row says `The agent has exited`.

**Nothing is done on a look Herdr did not answer.** An agent lookup that timed out leaves the task as it was. Before a
try, the board asks Herdr again for every pane and every agent, and starts nothing unless all of that answers and none
of it is the task's agent. So a slow Herdr never makes a second copy, nor fails a task whose agent still runs.

**A pane id Herdr handed to another is not the task's.** After a restart Herdr may give the task's old pane id to
another terminal. The board neither types into it nor ends what runs there: the task comes back in a new pane of its
space.

## The tries

| Try | When |
|---|---|
| 1st | once the agent has been down for 20 seconds (Herdr coming back settles first) |
| 2nd | a minute after the first, if it is down again |
| 3rd | five minutes after the second |

Each try is the same restart a reply starts: in the task's terminal if it is still there, else in a new one in its
space, else in its space opened again from its worktree. The agent resumes its own conversation when its program still
has it, and is told:

> Your program had stopped while you worked (its terminal closed, or herdr or the computer restarted), and the board
> started it again. Continue the task where you left off.

With no conversation to resume, it starts afresh with the task, told that it worked on it before.

**The count is kept on the task** (`relaunched`). It starts again from none after a reply, a new delegation, or ten
minutes of the agent running without a break: seen down once in between, those ten minutes start again.

**A try that runs late is still checked.** A restart asked for from inside a plan's sandbox runs a while later, from
the keeper. When it runs, it goes ahead only if the task is still at work and still counted for a try: a task you
stopped, replied to or archived meanwhile is left alone.

## When it keeps stopping

Down once more after the third try, the task fails: `The agent keeps stopping: started again 3 times`. The task panel
says how it stopped and what the board did. `board.notify` tells you, as for any failed task.

- **A reply** starts it again, and the count starts from none.
- **Retry** (`Ctrl+L`) tells it to continue, and does the same.
- A start that fails outright (Herdr refused, the program is not installed) fails at once with
  [what went wrong](could-not-start.md), with no more tries.

## Never started again

- **A task you stopped**, or one stopped at a [limit](limits.md): a stop is on purpose.
- **A task that is done**, or archived.
- **A task not at work** in its agent's turn: one under review, waiting for your approval, asking you, or failed. A
  reply starts its agent, as before.
- **An attempt** of a task done several times at once: one that lost its agent is out, as before
  ([attempts](attempts.md)).
- **The reviewer.** A review whose reviewer went ends without a verdict ([the gate](gate.md)).

## Details

From Shepherdr's SHE-254 (63a8be4b, #441) and SHEP-30 (d5f0bc62, #622), rewritten for the board.

- `crates/flow/src/hook/relaunch.rs`: when to try, the count, giving up;
- `crates/flow/src/hook/reconcile.rs`: where a gone or exited agent is seen;
- `crates/flow/src/delegate/jobs.rs` (`restart_job`): the restart itself;
- `tests/lifecycle/relaunch.rs`.
